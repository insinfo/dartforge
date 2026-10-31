//! Resolver and archive rejection tests without delegating to Pub.
use super::*;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dartforge-pub-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn package(&self, path: &str, text: &str) -> PathBuf {
        let p = self.0.join(path);
        std::fs::create_dir_all(p.join("lib")).unwrap();
        std::fs::write(p.join("pubspec.yaml"), text).unwrap();
        p.canonicalize().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn resolver(cache: PathBuf) -> resolver::Resolver {
    resolver::Resolver::new(
        hosted::Hosted::new(cache, true, "https://pub.dev".into()),
        semver::Version::new(3, 6, 2),
        BTreeMap::new(),
    )
}
#[test]
fn dart_intervals_and_sdk_compatibility() {
    use constraints::*;
    let c = Constraint::parse("^0.0.3").unwrap();
    assert!(c.allows(&parse_version("0.0.9").unwrap()));
    assert!(!c.allows(&parse_version("0.1.0-dev").unwrap()));
    let c = Constraint::parse(">= 1.0.0 < 2.0.0").unwrap();
    assert!(c.allows(&parse_version("1.5.0-dev.1").unwrap()));
    assert!(!c.allows(&parse_version("2.0.0-dev.1").unwrap()));
    // pub_semver needs no space between bounds (seen in published pubspecs).
    let c = Constraint::parse(">=2.0.0-dev<3.0.0").unwrap();
    assert!(c.allows(&parse_version("2.0.0").unwrap()));
    assert!(!c.allows(&parse_version("3.0.0").unwrap()));
    assert!(Constraint::parse(">=").is_err());
    assert!(Constraint::parse("1.2.*").is_err());
    assert!(Constraint::parse("^1.0.0 || ^2.0.0").is_err());
    let c = Constraint::parse(">=2.12.0 <3.0.0").unwrap();
    assert!(
        c.for_sdk(&parse_version("3.6.2").unwrap())
            .allows(&parse_version("3.6.2").unwrap())
    );
}
#[test]
fn offline_path_transitive_cycles_and_root_dev() {
    let f = Fixture::new();
    let root=f.package("app","name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  a:\n    path: ../a\ndev_dependencies:\n  test_support:\n    path: ../support\n");
    f.package("a","name: a\nversion: 1.0.0\nenvironment:\n  sdk: '>=2.12.0 <3.0.0'\ndependencies:\n  b:\n    path: ../b\ndev_dependencies:\n  ignored:\n    git: nowhere\n");
    f.package("b","name: b\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  a:\n    path: ../a\n");
    f.package(
        "support",
        "name: test_support\nenvironment:\n  sdk: ^3.0.0\n",
    );
    let manifest = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let mut r = resolver(f.0.join("cache"));
    let selected = r.resolve(&manifest, &root).unwrap();
    assert_eq!(
        selected.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "b", "test_support"]
    );
    output::write(&root, &manifest, &selected, &r.sdk).unwrap();
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(".dart_tool/package_config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(config["packages"].as_array().unwrap().len(), 4);
    assert_eq!(config["packages"][0]["languageVersion"], "2.12");
    let lock = output::read_lock(&root, "https://pub.dev").unwrap();
    assert_eq!(lock.len(), 3);
}
#[test]
fn sdk_source_identity_conflicts_are_errors() {
    let f = Fixture::new();
    let root = f.package("app", "name: app\nenvironment:\n  sdk: ^4.0.0\n");
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    assert!(
        resolver(f.0.join("cache"))
            .resolve(&m, &root)
            .unwrap_err()
            .contains("requires Dart SDK")
    );
    // Git and workspaces are supported now; what stays outside the subset
    // (relative repository paths, workspaces below language 3.5) is refused.
    for text in [
        "name: app\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  foo:\n    git: ../foo\n",
        "name: app\nworkspace: []\n",
    ] {
        assert!(
            manifest::Manifest::parse(
                &manifest::yaml_json(text).unwrap(),
                Some(&root),
                "https://pub.dev"
            )
            .is_err()
        );
    }
}

#[test]
fn root_overrides_replace_transitive_sources_and_include_unused_packages() {
    let f = Fixture::new();
    let root = f.package("app", "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  a:\n    path: ../a\n  b: ^1.0.0\ndependency_overrides:\n  b:\n    path: ../b\n  extra:\n    path: ../extra\n");
    f.package("a", "name: a\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  b: ^1.0.0\ndependency_overrides:\n  b:\n    git: ignored\n");
    let b = f.package(
        "b",
        "name: b\nversion: 2.0.0\nenvironment:\n  sdk: ^3.6.0\n",
    );
    f.package("extra", "name: extra\nenvironment:\n  sdk: ^3.6.0\n");
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let mut r = resolver(f.0.join("cache"));
    let selected = r.resolve(&m, &root).unwrap();
    assert_eq!(selected["b"].source, manifest::Source::Path(b));
    assert_eq!(
        selected["b"].candidate.manifest.version,
        semver::Version::new(2, 0, 0)
    );
    assert!(selected.contains_key("extra"));
    output::write(&root, &m, &selected, &r.sdk).unwrap();
    let lock =
        manifest::yaml_json(&std::fs::read_to_string(root.join("pubspec.lock")).unwrap()).unwrap();
    // Pub 3.6.2 keeps a root dependency direct even when it is also overridden.
    assert_eq!(lock["packages"]["b"]["dependency"], "direct main");
    assert_eq!(lock["packages"]["extra"]["dependency"], "direct overridden");
}

#[test]
fn sidecar_replaces_override_map_and_dependencies_ignore_their_sidecars() {
    let f = Fixture::new();
    let root = f.package("app", "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  b:\n    path: ../b\ndependency_overrides:\n  missing:\n    path: ../not-there\n");
    let b = f.package(
        "b",
        "name: b\nversion: 2.0.0\nenvironment:\n  sdk: ^3.6.0\n",
    );
    std::fs::write(
        b.join("pubspec_overrides.yaml"),
        "dependency_overrides:\n  ignored:\n    git: unsupported\n",
    )
    .unwrap();
    std::fs::write(
        root.join("pubspec_overrides.yaml"),
        "dependency_overrides:\n  b:\n    path: ../b\n",
    )
    .unwrap();
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    assert!(!m.overrides.contains_key("missing"));
    let selected = resolver(f.0.join("cache")).resolve(&m, &root).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected["b"].source, manifest::Source::Path(b));
    // Exercise the CLI entry too: a parsed sidecar must not be refused before loading.
    super::run(&[
        "--offline".into(),
        "--directory".into(),
        root.into_os_string(),
    ])
    .unwrap();
}

#[test]
fn overrides_keep_package_identity_and_root_sdk_checks() {
    for (name, root_sdk) in [("wrong", "^3.6.0"), ("b", "^4.0.0")] {
        let f = Fixture::new();
        let root = f.package(
            "app",
            &format!("name: app\nenvironment:\n  sdk: {root_sdk}\ndependency_overrides:\n  b:\n    path: ../b\n"),
        );
        f.package(
            "b",
            &format!("name: {name}\nversion: 2.0.0\nenvironment:\n  sdk: ^3.6.0\n"),
        );
        let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
        assert!(resolver(f.0.join("cache")).resolve(&m, &root).is_err());
    }
}
#[test]
fn overrides_relax_the_overridden_sdk_and_root_back_reference() {
    for sdk in ["^4.0.0", ">=2.0.0 <3.0.0", "any"] {
        let f = Fixture::new();
        let root = f.package("app", "name: app\nversion: 1.0.0\nenvironment:\n  sdk: ^3.6.0\ndependency_overrides:\n  b:\n    path: ../b\n");
        f.package("b", &format!("name: b\nversion: 2.0.0\nenvironment:\n  sdk: '{sdk}'\ndependencies:\n  app: ^99.0.0\n"));
        let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
        let mut r = resolver(f.0.join("cache"));
        let selected = r.resolve(&m, &root).unwrap();
        output::write(&root, &m, &selected, &r.sdk).unwrap();
        let lock =
            manifest::yaml_json(&std::fs::read_to_string(root.join("pubspec.lock")).unwrap())
                .unwrap();
        assert_eq!(lock["sdks"]["dart"], ">=3.6.0 <4.0.0");
    }
}
fn archive(name: &str, kind: tar::EntryType) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(if kind.is_file() { 1 } else { 0 });
    header.set_mode(0o644);
    header.set_entry_type(kind);
    if kind.is_symlink() {
        header.set_link_name("/outside").unwrap();
    }
    header.set_cksum();
    tar.append_data(
        &mut header,
        name,
        if kind.is_file() { &b"x"[..] } else { &b""[..] },
    )
    .unwrap();
    let data = tar.into_inner().unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gzip, &data).unwrap();
    gzip.finish().unwrap()
}
#[test]
fn archive_files_allowed_links_refused() {
    let f = Fixture::new();
    hosted::unpack(&archive("lib/a.dart", tar::EntryType::Regular), &f.0).unwrap();
    assert_eq!(std::fs::read(f.0.join("lib/a.dart")).unwrap(), b"x");
    assert!(
        hosted::unpack(&archive("link", tar::EntryType::Symlink), &f.0)
            .unwrap_err()
            .contains("links")
    );
}
fn cache_versions(cache: &std::path::Path, name: &str, versions: Vec<serde_json::Value>) {
    let dir = cache.join("dartforge/pub.dev");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{name}-versions.json")),
        serde_json::to_vec(&serde_json::json!({"name":name,"versions":versions})).unwrap(),
    )
    .unwrap();
}
fn version_row(name: &str, version: &str, deps: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"version":version,"pubspec":{"name":name,"version":version,"environment":{"sdk":"^3.0.0"},"dependencies":deps},"archive_url":format!("https://pub.dev/api/archives/{name}-{version}.tar.gz"),"archive_sha256":"0".repeat(64)})
}
#[test]
fn hosted_backtracks_transitive_constraints_and_prefers_existing_lock() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    cache_versions(
        &cache,
        "a",
        vec![
            version_row("a", "1.0.0", serde_json::json!({"b":"^1.0.0"})),
            version_row("a", "2.0.0", serde_json::json!({"b":"^2.0.0"})),
        ],
    );
    cache_versions(
        &cache,
        "b",
        vec![
            version_row("b", "1.0.0", serde_json::json!({})),
            version_row("b", "2.0.0", serde_json::json!({})),
        ],
    );
    let root = f.package(
        "app",
        "name: app\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  a: any\n  b: ^1.0.0\n",
    );
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let selected = resolver(cache.clone()).resolve(&m, &root).unwrap();
    assert_eq!(
        selected["a"].candidate.manifest.version.to_string(),
        "1.0.0"
    );
    assert_eq!(
        selected["b"].candidate.manifest.version.to_string(),
        "1.0.0"
    );
    let mut any = m.clone();
    any.dependencies.get_mut("b").unwrap().constraint =
        constraints::Constraint::parse("any").unwrap();
    let mut r = resolver(cache.clone());
    r.locked.insert(
        "a".into(),
        (
            manifest::Source::Hosted("https://pub.dev".into()),
            semver::Version::new(1, 0, 0),
        ),
    );
    assert_eq!(
        r.resolve(&any, &root).unwrap()["a"]
            .candidate
            .manifest
            .version
            .to_string(),
        "1.0.0"
    );
    let mut incompatible = m;
    incompatible.dependencies.get_mut("a").unwrap().constraint =
        constraints::Constraint::parse("^2.0.0").unwrap();
    let error = resolver(cache).resolve(&incompatible, &root).unwrap_err();
    assert!(
        error.contains("no compatible version for b")
            && error.contains("app requires ^1.0.0")
            && error.contains("a requires ^2.0.0")
    );
}
#[test]
fn hosted_integrity_missing_metadata_and_source_conflict_fail() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    let mut r = resolver(cache.clone());
    assert!(
        r.hosted
            .candidates("missing", "https://pub.dev")
            .unwrap_err()
            .contains("offline")
    );
    let mut row = version_row("a", "1.0.0", serde_json::json!({}));
    row.as_object_mut().unwrap().remove("archive_sha256");
    cache_versions(&cache, "a", vec![row]);
    assert!(
        r.hosted
            .candidates("a", "https://pub.dev")
            .unwrap_err()
            .contains("SHA256")
    );
    let root=f.package("app","name: app\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  a:\n    path: ../a\n  b:\n    path: ../b\n");
    f.package("a", "name: a\nenvironment:\n  sdk: ^3.0.0\n");
    f.package(
        "b",
        "name: b\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  a: any\n",
    );
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    assert!(
        resolver(cache)
            .resolve(&m, &root)
            .unwrap_err()
            .contains("conflicting package sources for a")
    );
}
#[test]
fn irrelevant_unavailable_sdk_historical_versions_do_not_poison_supported_release() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    let mut supported = version_row("a", "2.0.0", serde_json::json!({}));
    supported["pubspec"]["resolution"] = serde_json::json!("workspace");
    supported["pubspec"]["dependency_overrides"] = serde_json::json!({"ignored":{"git":"nowhere"}});
    cache_versions(
        &cache,
        "a",
        vec![
            version_row("a", "1.0.0", serde_json::json!({"sdk_pkg":{"sdk":"dart"}})),
            supported,
        ],
    );
    let root = f.package(
        "app",
        "name: app\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  a: any\n",
    );
    let mut m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    assert_eq!(
        resolver(cache.clone()).resolve(&m, &root).unwrap()["a"]
            .candidate
            .manifest
            .version
            .to_string(),
        "2.0.0"
    );
    m.dependencies.get_mut("a").unwrap().constraint =
        constraints::Constraint::parse("^1.0.0").unwrap();
    assert!(
        resolver(cache)
            .resolve(&m, &root)
            .unwrap_err()
            .contains("SDK packages require a selected SDK")
    );
}
#[test]
fn native_http_checksum_failure_does_not_publish_archive() {
    use std::io::Write;
    let f = Fixture::new();
    let bytes = archive("lib/a.dart", tar::EntryType::Regular);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_http_headers(&mut stream);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .unwrap();
        stream.write_all(&bytes).unwrap();
    });
    let root = f.package(
        "app",
        "name: app\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\n",
    );
    let manifest = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let candidate = hosted::Candidate {
        manifest,
        manifest_error: None,
        directory: f.0.join("cached/app-1.0.0"),
        archive: Some(format!("http://{address}/archive")),
        digest: Some("0".repeat(64)),
    };
    let hosted = hosted::Hosted::new(f.0.join("cache"), false, "https://pub.dev".into());
    assert!(
        hosted
            .materialize(&candidate, &format!("http://{address}"))
            .unwrap_err()
            .contains("SHA256 mismatch")
    );
    server.join().unwrap();
    assert!(!candidate.directory.exists());
}

#[test]
fn cached_pubspec_must_match_the_resolved_registry_graph() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    let directory = cache.join("hosted/pub.dev/app-1.0.0");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("pubspec.yaml"),
        "name: app\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  unlisted: any\n",
    )
    .unwrap();
    let hash_dir = cache.join("hosted-hashes/pub.dev");
    std::fs::create_dir_all(&hash_dir).unwrap();
    std::fs::write(hash_dir.join("app-1.0.0.sha256"), "0".repeat(64)).unwrap();
    let manifest = manifest::Manifest::parse(
        &version_row("app", "1.0.0", serde_json::json!({}))["pubspec"],
        None,
        "https://pub.dev",
    )
    .unwrap();
    let candidate = hosted::Candidate {
        manifest,
        manifest_error: None,
        directory,
        archive: None,
        digest: Some("0".repeat(64)),
    };
    let hosted = hosted::Hosted::new(cache, true, "https://pub.dev".into());
    assert!(
        hosted
            .materialize(&candidate, "https://pub.dev")
            .unwrap_err()
            .contains("cached package identity mismatch")
    );
}

#[test]
fn pub_build_suffixes_are_ordered_and_exact_constraints_remain_exact() {
    use constraints::{Constraint, compare_versions, parse_version};
    for (left, right) in [
        ("1.0.0", "1.0.0+1"),
        ("1.0.0+2", "1.0.0+10"),
        ("1.0.0+1", "1.0.0+alpha"),
        ("1.0.0+alpha", "1.0.0+alpha.1"),
    ] {
        assert!(
            compare_versions(
                &parse_version(left).unwrap(),
                &parse_version(right).unwrap()
            )
            .is_lt()
        );
    }
    let exact = Constraint::parse("1.0.0+1").unwrap();
    assert!(exact.allows(&parse_version("1.0.0+1").unwrap()));
    assert!(!exact.allows(&parse_version("1.0.0").unwrap()));
    assert!(!exact.allows(&parse_version("1.0.0+2").unwrap()));
    assert!(
        !Constraint::parse(">=1.0.0 <1.0.0+2")
            .unwrap()
            .allows(&parse_version("1.0.0+10").unwrap())
    );
}

#[test]
fn explicit_lower_prerelease_allows_same_release_below_stable_upper() {
    use constraints::{Constraint, parse_version};
    let beta = parse_version("2.0.0-beta").unwrap();
    assert!(Constraint::parse("<2.0.0+1").unwrap().allows(&beta));
    assert!(
        Constraint::parse(">2.0.0-alpha <2.0.0")
            .unwrap()
            .allows(&beta)
    );
    assert!(
        !Constraint::parse(">2.0.0-alpha <2.0.0")
            .unwrap()
            .allows(&parse_version("2.0.0").unwrap())
    );
    assert!(
        !Constraint::parse(">1.0.0-alpha <2.0.0")
            .unwrap()
            .allows(&beta)
    );
    assert!(!Constraint::parse(">=1.0.0 <2.0.0").unwrap().allows(&beta));
    assert!(
        Constraint::parse("<2.0.0-beta")
            .unwrap()
            .allows(&parse_version("2.0.0-alpha").unwrap())
    );
}

fn declared_sdk(fixture: &Fixture) -> PathBuf {
    let sdk = fixture.0.join("selected-sdk");
    std::fs::create_dir_all(sdk.join("lib")).unwrap();
    std::fs::write(sdk.join("lib/libraries.json"), "{}").unwrap();
    std::fs::write(sdk.join("version"), "3.6.2\n").unwrap();
    std::fs::write(sdk.join("sdk_packages.yaml"), "version: 1\nsdk: dart\npackages:\n  - name: vendor_one\n    path: declared/one\n  - name: vendor_two\n    path: declared/two\n").unwrap();
    fixture.package("selected-sdk/declared/one", "name: vendor_one\nversion: 1.2.3\nenvironment:\n  sdk: ^3.4.0\ndependencies:\n  vendor_two:\n    sdk: dart\n    version: ^2.0.0\ndev_dependencies:\n  ignored:\n    git: unsupported\n");
    fixture.package(
        "selected-sdk/declared/two",
        "name: vendor_two\nversion: 2.1.0\nenvironment:\n  sdk: ^3.0.0\n",
    );
    sdk.canonicalize().unwrap()
}

#[test]
fn declared_sdk_packages_resolve_transitively_offline_and_write_sdk_lock_identity() {
    let fixture = Fixture::new();
    let sdk = declared_sdk(&fixture);
    let root = fixture.package("app", "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  vendor_one:\n    sdk: dart\n    version: ^1.0.0\n");
    let manifest = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let mut resolver = resolver(fixture.0.join("cache"));
    resolver.sdk_root = Some(sdk.clone());
    let selected = resolver.resolve(&manifest, &root).unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(
        selected["vendor_one"].source,
        manifest::Source::Sdk("dart".into())
    );
    assert_eq!(
        selected["vendor_one"].candidate.directory,
        sdk.join("declared/one")
    );
    output::write(&root, &manifest, &selected, &resolver.sdk).unwrap();
    let lock = output::read_lock(&root, "https://pub.dev").unwrap();
    assert_eq!(
        lock["vendor_one"],
        (
            manifest::Source::Sdk("dart".into()),
            constraints::parse_version("1.2.3").unwrap()
        )
    );
    let text = std::fs::read_to_string(root.join("pubspec.lock")).unwrap();
    // Pub writes identifier-like values without quotes.
    assert!(text.contains("description: dart\n    source: sdk"));
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(".dart_tool/package_config.json")).unwrap(),
    )
    .unwrap();
    let entry = config["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "vendor_one")
        .unwrap();
    assert_eq!(entry["languageVersion"], "3.4");
    assert_eq!(
        entry["rootUri"],
        url::Url::from_file_path(sdk.join("declared/one"))
            .unwrap()
            .as_str()
    );
    let hashes = [
        root.join("pubspec.lock"),
        root.join(".dart_tool/package_config.json"),
    ]
    .map(|path| std::fs::read(path).unwrap());
    resolver.locked = lock;
    let repeated = resolver.resolve(&manifest, &root).unwrap();
    output::write(&root, &manifest, &repeated, &resolver.sdk).unwrap();
    assert_eq!(
        hashes,
        [
            root.join("pubspec.lock"),
            root.join(".dart_tool/package_config.json")
        ]
        .map(|path| std::fs::read(path).unwrap())
    );
}

#[test]
fn sdk_lookup_requires_selected_version_and_declared_unique_matching_package() {
    let fixture = Fixture::new();
    let sdk_root = declared_sdk(&fixture);
    let version = constraints::parse_version("3.6.2").unwrap();
    let lookup = |root: Option<&std::path::Path>, name: &str, version: &semver::Version| {
        sdk::candidate(root, "dart", name, version, "https://pub.dev")
    };
    assert!(
        lookup(None, "vendor_one", &version)
            .unwrap_err()
            .contains("selected SDK")
    );
    assert!(
        lookup(
            Some(&sdk_root),
            "vendor_one",
            &constraints::parse_version("3.7.0").unwrap()
        )
        .unwrap_err()
        .contains("version differs")
    );
    fixture.package(
        "selected-sdk/pkg/undeclared",
        "name: undeclared\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\n",
    );
    assert!(
        lookup(Some(&sdk_root), "undeclared", &version)
            .unwrap_err()
            .contains("not declared")
    );
    let config = sdk_root.join("sdk_packages.yaml");
    let original = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        format!("{original}  - name: vendor_one\n    path: declared/two\n"),
    )
    .unwrap();
    assert!(
        lookup(Some(&sdk_root), "vendor_one", &version)
            .unwrap_err()
            .contains("duplicate")
    );
    std::fs::write(&config, &original).unwrap();
    fixture.package(
        "selected-sdk/declared/one",
        "name: different\nversion: 1.2.3\nenvironment:\n  sdk: ^3.4.0\n",
    );
    assert!(
        lookup(Some(&sdk_root), "vendor_one", &version)
            .unwrap_err()
            .contains("pubspec name")
    );
}

#[test]
fn sdk_vendor_packages_refuse_non_sdk_regular_sources_and_source_identity_conflicts() {
    let fixture = Fixture::new();
    let sdk_root = declared_sdk(&fixture);
    let version = constraints::parse_version("3.6.2").unwrap();
    fixture.package("selected-sdk/declared/one", "name: vendor_one\nversion: 1.2.3\nenvironment:\n  sdk: ^3.4.0\ndependencies:\n  hosted_dependency: any\n");
    assert!(
        sdk::candidate(
            Some(&sdk_root),
            "dart",
            "vendor_one",
            &version,
            "https://pub.dev"
        )
        .unwrap_err()
        .contains("only declare SDK")
    );
    let root = fixture.package("app", "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  vendor_one:\n    sdk: dart\n  bridge:\n    path: ../bridge\n");
    fixture.package(
        "bridge",
        "name: bridge\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  vendor_one: any\n",
    );
    let manifest = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let mut resolver = resolver(fixture.0.join("cache"));
    resolver.sdk_root = Some(sdk_root);
    assert!(
        resolver
            .resolve(&manifest, &root)
            .unwrap_err()
            .contains("conflicting package sources")
    );
    for source in ["flutter", "unknown"] {
        let value = serde_json::json!({"name":"app", "dependencies":{"vendor_one":{"sdk":source}}});
        assert!(manifest::Manifest::parse(&value, Some(&root), "https://pub.dev").is_err());
    }
}

#[test]
fn sdk_package_versions_are_constraints_not_hosted_fallbacks_and_selected_mapping_changes() {
    let fixture = Fixture::new();
    let sdk_root = declared_sdk(&fixture);
    let root = fixture.package("app", "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  vendor_one:\n    sdk: dart\n    version: '>=2.0.0 <3.0.0'\n");
    let manifest = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let mut resolver = resolver(fixture.0.join("cache"));
    resolver.sdk_root = Some(sdk_root.clone());
    assert!(
        resolver
            .resolve(&manifest, &root)
            .unwrap_err()
            .contains("no compatible version for vendor_one")
    );
    fixture.package(
        "selected-sdk/replacement",
        "name: vendor_one\nversion: 2.2.0\nenvironment:\n  sdk: ^3.6.0\n",
    );
    std::fs::write(
        sdk_root.join("sdk_packages.yaml"),
        "version: 1\nsdk: dart\npackages:\n  - name: vendor_one\n    path: replacement\n",
    )
    .unwrap();
    let selected = resolver.resolve(&manifest, &root).unwrap();
    assert_eq!(
        selected["vendor_one"].candidate.directory,
        sdk_root.join("replacement")
    );
    assert_eq!(
        selected["vendor_one"].candidate.manifest.version,
        constraints::parse_version("2.2.0").unwrap()
    );
    // Hosted registry metadata can declare a selected SDK package; its source identity remains SDK.
    let hosted = manifest::Manifest::parse(&serde_json::json!({"name":"wrapper","version":"1.0.0","environment":{"sdk":"^3.6.0"},"dependencies":{"vendor_one":{"sdk":"dart","version":"^2.0.0"}}}), None, "https://pub.dev").unwrap();
    assert_eq!(
        hosted.dependencies["vendor_one"].source,
        manifest::Source::Sdk("dart".into())
    );
}

#[cfg(unix)]
#[test]
fn sdk_vendor_package_symlink_cannot_resolve_outside_the_selected_sdk() {
    let fixture = Fixture::new();
    let root = declared_sdk(&fixture);
    let outside = fixture.package(
        "outside",
        "name: vendor_one\nversion: 1.2.3\nenvironment:\n  sdk: ^3.4.0\n",
    );
    std::fs::remove_dir_all(root.join("declared/one")).unwrap();
    std::os::unix::fs::symlink(outside, root.join("declared/one")).unwrap();
    let error = sdk::candidate(
        Some(&root),
        "dart",
        "vendor_one",
        &constraints::parse_version("3.6.2").unwrap(),
        "https://pub.dev",
    )
    .unwrap_err();
    assert!(error.contains("escapes the selected SDK"));
}

/// Read complete request headers even when TCP splits them across packets.
fn read_http_headers(stream: &mut std::net::TcpStream) {
    use std::io::{BufRead, Read};
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut reader = std::io::BufReader::new(stream.take(8192));
    let mut line = String::new();
    loop {
        line.clear();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "incomplete HTTP request"
        );
        if line == "\r\n" {
            break;
        }
    }
}

/// A bounded local registry; no external registry or environment mutation.
fn metadata_server(responses: Vec<serde_json::Value>) -> (String, std::thread::JoinHandle<()>) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        for value in responses {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "registry request timed out"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(error) => panic!("registry accept failed: {error}"),
                }
            };
            read_http_headers(&mut stream);
            stream
                .set_write_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let body = serde_json::to_vec(&value).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    (host, server)
}

#[test]
fn rejected_registry_updates_preserve_offline_metadata() {
    let good = serde_json::json!({"name":"a","versions":[version_row("a", "1.0.0", serde_json::json!({}))]});
    let mut failures = Vec::new();
    for case in 0..8 {
        let f = Fixture::new();
        let cache = f.0.join("cache");
        let mut bad = good.clone();
        match case {
            0 => bad["name"] = serde_json::json!("different_package"),
            1 => {
                bad.as_object_mut().unwrap().remove("versions");
            }
            2 => bad["versions"][0]["version"] = serde_json::json!("invalid-version"),
            3 => {
                let duplicate = bad["versions"][0].clone();
                bad["versions"].as_array_mut().unwrap().push(duplicate);
            }
            4 => bad["versions"][0]["pubspec"]["name"] = serde_json::json!("different_package"),
            5 => bad["versions"][0]["pubspec"]["version"] = serde_json::json!("2.0.0"),
            6 => bad["versions"][0]["archive_sha256"] = serde_json::json!("invalid-digest"),
            7 => bad["versions"][0]["archive_url"] = serde_json::json!("file:///archive.tar.gz"),
            _ => unreachable!(),
        }
        let (host, server) = metadata_server(vec![good.clone(), bad]);
        assert_eq!(
            hosted::Hosted::new(cache.clone(), false, host.clone())
                .candidates("a", &host)
                .unwrap()
                .len(),
            1
        );
        let rejected =
            hosted::Hosted::new(cache.clone(), false, host.clone()).candidates("a", &host);
        server.join().unwrap();
        assert!(rejected.is_err(), "case {case}: invalid response accepted");
        let cached = hosted::Hosted::new(cache, true, host.clone()).candidates("a", &host);
        match cached {
            Ok(rows)
                if rows.len() == 1 && rows[0].manifest.version == semver::Version::new(1, 0, 0) => {
            }
            other => failures.push(format!(
                "case {case}: valid cached metadata lost: {other:?}"
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn invalid_first_registry_response_is_not_cached() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    let (host, server) = metadata_server(vec![serde_json::json!({"name":"a"})]);
    let error = hosted::Hosted::new(cache.clone(), false, host.clone())
        .candidates("a", &host)
        .unwrap_err();
    server.join().unwrap();
    assert!(error.contains("missing versions"));
    let offline = hosted::Hosted::new(cache, true, host.clone())
        .candidates("a", &host)
        .unwrap_err();
    assert!(offline.contains("no cached hosted metadata"), "{offline}");
}

#[test]
fn validated_registry_updates_replace_offline_metadata() {
    let f = Fixture::new();
    let cache = f.0.join("cache");
    let responses = ["1.0.0", "2.0.0"].map(|version| serde_json::json!({"name":"a","versions":[version_row("a", version, serde_json::json!({}))]}));
    let (host, server) = metadata_server(responses.to_vec());
    for version in [1, 2] {
        let rows = hosted::Hosted::new(cache.clone(), false, host.clone())
            .candidates("a", &host)
            .unwrap();
        assert_eq!(
            rows[0].manifest.version,
            semver::Version::new(version, 0, 0)
        );
    }
    server.join().unwrap();
    let rows = hosted::Hosted::new(cache, true, host.clone())
        .candidates("a", &host)
        .unwrap();
    assert_eq!(rows[0].manifest.version, semver::Version::new(2, 0, 0));
}

fn read_json(path: PathBuf) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn relative_path_dependencies_stay_relative_like_pub() {
    let f = Fixture::new();
    let root = f.package(
        "app",
        "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  shared:\n    path: ../shared\n",
    );
    let shared = f.package(
        "shared",
        &format!(
            "name: shared\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  absolute:\n    path: {}\n",
            f.0.join("absolute").display()
        ),
    );
    let absolute = f.package("absolute", "name: absolute\nenvironment:\n  sdk: ^3.0.0\n");
    let manifest = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    let mut r = resolver(f.0.join("cache"));
    let selected = r.resolve(&manifest, &root).unwrap();
    output::write(&root, &manifest, &selected, &r.sdk).unwrap();
    let lock =
        manifest::yaml_json(&std::fs::read_to_string(root.join("pubspec.lock")).unwrap()).unwrap();
    // The values `dart pub get` 3.6.2 writes for the same layout.
    assert_eq!(
        lock["packages"]["shared"]["description"],
        serde_json::json!({"path": "../shared", "relative": true})
    );
    assert_eq!(
        lock["packages"]["absolute"]["description"],
        serde_json::json!({"path": absolute.to_str().unwrap(), "relative": false})
    );
    let config = read_json(root.join(".dart_tool/package_config.json"));
    let uris: Vec<(&str, &str)> = config["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap(), p["rootUri"].as_str().unwrap()))
        .collect();
    let absolute_uri = url::Url::from_file_path(&absolute).unwrap().to_string();
    assert_eq!(
        uris,
        [
            ("absolute", absolute_uri.as_str()),
            ("shared", "../../shared"),
            ("app", "../")
        ]
    );
    assert!(shared.join("pubspec.yaml").is_file());
    assert!(!root.join(".dart_tool/pub/workspace_ref.json").exists());
}

const WORKSPACE_ROOT: &str = "name: ws_root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - pkgs/a\n  - pkgs/b\ndev_dependencies:\n  tool:\n    path: tool\n";

#[test]
fn workspace_members_resolve_together_at_the_root() {
    let f = Fixture::new();
    let root = f.package("ws", WORKSPACE_ROOT);
    let a = f.package(
        "ws/pkgs/a",
        "name: a\nversion: 1.0.0\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  b: ^0.2.0\n  local:\n    path: ../../local\ndependency_overrides:\n  pinned:\n    path: ../../pinned\n",
    );
    let b = f.package(
        "ws/pkgs/b",
        "name: b\nversion: 0.2.0\nenvironment:\n  sdk: ^3.5.0\nresolution: workspace\ndev_dependencies:\n  pinned:\n    path: ../../elsewhere\n",
    );
    f.package(
        "ws/local",
        "name: local\nversion: 3.0.0\nenvironment:\n  sdk: ^3.0.0\n",
    );
    f.package("ws/tool", "name: tool\nenvironment:\n  sdk: ^3.0.0\n");
    f.package(
        "ws/pinned",
        "name: pinned\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\n",
    );
    f.package(
        "ws/elsewhere",
        "name: pinned\nversion: 9.0.0\nenvironment:\n  sdk: ^3.0.0\n",
    );
    // Stray files of a member are removed, as Pub reports and does.
    std::fs::write(a.join("pubspec.lock"), "packages: {}\n").unwrap();
    std::fs::create_dir_all(a.join(".dart_tool")).unwrap();
    std::fs::write(a.join(".dart_tool/package_config.json"), "{}").unwrap();

    let workspace = super::workspace(&a, "https://pub.dev").unwrap();
    let names: Vec<&str> = workspace.iter().map(|(m, _)| m.name.as_str()).collect();
    assert_eq!(names, ["ws_root", "a", "b"]);
    assert_eq!(workspace[0].1, root);
    assert_eq!(super::workspace(&root, "https://pub.dev").unwrap().len(), 3);

    let mut r = resolver(f.0.join("cache"));
    let selected = r.resolve_workspace(&workspace).unwrap();
    assert_eq!(
        selected.keys().map(String::as_str).collect::<Vec<_>>(),
        ["local", "pinned", "tool"]
    );
    // A member's override applies to the whole workspace.
    assert_eq!(
        selected["pinned"].candidate.manifest.version.to_string(),
        "1.0.0"
    );
    output::write_workspace(&workspace, &selected, &r.sdk).unwrap();

    let lock =
        manifest::yaml_json(&std::fs::read_to_string(root.join("pubspec.lock")).unwrap()).unwrap();
    assert_eq!(lock["packages"]["local"]["dependency"], "transitive");
    assert_eq!(lock["packages"]["local"]["description"]["path"], "local");
    assert_eq!(
        lock["packages"]["pinned"]["dependency"],
        "direct overridden"
    );
    assert_eq!(lock["packages"]["tool"]["dependency"], "direct dev");
    assert_eq!(lock["sdks"]["dart"], ">=3.6.0 <4.0.0");
    // Pub 3.6.2's text for this layout, apart from its two header lines.
    assert_eq!(
        std::fs::read_to_string(root.join("pubspec.lock")).unwrap(),
        "# Generated by dartforge pub get.\npackages:\n  local:\n    dependency: transitive\n    description:\n      path: local\n      relative: true\n    source: path\n    version: \"3.0.0\"\n  pinned:\n    dependency: \"direct overridden\"\n    description:\n      path: pinned\n      relative: true\n    source: path\n    version: \"1.0.0\"\n  tool:\n    dependency: \"direct dev\"\n    description:\n      path: tool\n      relative: true\n    source: path\n    version: \"0.0.0\"\nsdks:\n  dart: \">=3.6.0 <4.0.0\"\n"
    );
    let config = read_json(root.join(".dart_tool/package_config.json"));
    let entries: Vec<(&str, &str, &str)> = config["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["name"].as_str().unwrap(),
                p["rootUri"].as_str().unwrap(),
                p["languageVersion"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        entries,
        [
            ("local", "../local", "3.0"),
            ("pinned", "../pinned", "3.0"),
            ("tool", "../tool", "3.0"),
            ("ws_root", "../", "3.6"),
            ("a", "../pkgs/a", "3.6"),
            ("b", "../pkgs/b", "3.5"),
        ]
    );
    for (package, expected) in [(&root, "../.."), (&a, "../../../.."), (&b, "../../../..")] {
        let text =
            std::fs::read_to_string(package.join(".dart_tool/pub/workspace_ref.json")).unwrap();
        assert_eq!(
            text,
            format!("{{\n  \"workspaceRoot\": \"{expected}\"\n}}\n")
        );
    }
    assert!(!a.join("pubspec.lock").exists());
    assert!(!a.join(".dart_tool/package_config.json").exists());
    assert!(!b.join("pubspec.lock").exists());
}

#[test]
fn workspace_errors_follow_pub() {
    let f = Fixture::new();
    f.package("ws", WORKSPACE_ROOT);
    f.package("ws/tool", "name: tool\nenvironment:\n  sdk: ^3.0.0\n");
    f.package(
        "ws/pkgs/a",
        "name: a\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\ndependencies:\n  b: ^9.0.0\n",
    );
    f.package(
        "ws/pkgs/b",
        "name: b\nversion: 0.2.0\nenvironment:\n  sdk: ^3.5.0\nresolution: workspace\n",
    );
    let unlisted = f.package(
        "ws/pkgs/c",
        "name: c\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\n",
    );
    let error = super::workspace(&unlisted, "https://pub.dev").unwrap_err();
    assert!(
        error.contains("But it has resolution `workspace`.\nBut found no workspace root including it in parent directories."),
        "{error}"
    );
    // A sibling constraint that the workspace package does not satisfy fails.
    let workspace = super::workspace(&f.0.join("ws"), "https://pub.dev").unwrap();
    let error = resolver(f.0.join("cache"))
        .resolve_workspace(&workspace)
        .unwrap_err();
    assert!(error.contains("no compatible version for b"), "{error}");
    let old = f.package(
        "old",
        "name: old\nenvironment:\n  sdk: ^3.4.0\nworkspace:\n  - member\n",
    );
    f.package(
        "old/member",
        "name: member\nenvironment:\n  sdk: ^3.6.0\nresolution: workspace\n",
    );
    let error = manifest::Manifest::read(&old, "https://pub.dev", true).unwrap_err();
    assert!(
        error.contains("`workspace` and `resolution` requires at least language version 3.5"),
        "{error}"
    );
    let plain = f.package(
        "plain",
        "name: plain\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - member\n",
    );
    f.package(
        "plain/member",
        "name: member\nenvironment:\n  sdk: ^3.6.0\n",
    );
    let error = super::workspace(&plain, "https://pub.dev").unwrap_err();
    assert!(error.contains("lacks `resolution: workspace`"), "{error}");
    for (name, extra) in [
        ("glob", "workspace:\n  - pkgs/*\n"),
        ("local", "resolution: local\n"),
    ] {
        let path = f.package(
            name,
            &format!("name: {name}\nenvironment:\n  sdk: ^3.6.0\n{extra}"),
        );
        assert!(manifest::Manifest::read(&path, "https://pub.dev", true).is_err());
    }
}

fn get(args: &[&str]) -> Result<u8, super::Failure> {
    let args: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
    super::run(&args)
}

#[test]
fn example_is_resolved_unless_disabled_and_dry_run_writes_nothing() {
    let f = Fixture::new();
    let root = f.package("pkg", "name: pkg\nenvironment:\n  sdk: ^3.6.0\n");
    f.package(
        "pkg/example",
        "name: pkg_example\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  pkg:\n    path: ..\n",
    );
    let directory = root.to_str().unwrap();
    assert_eq!(
        get(&["--offline", "--dry-run", "-C", directory]).ok(),
        Some(0)
    );
    assert!(!root.join("pubspec.lock").exists());
    assert!(!root.join("example/pubspec.lock").exists());
    assert_eq!(
        get(&["--offline", "--no-example", "--directory", directory]).ok(),
        Some(0)
    );
    assert!(root.join("pubspec.lock").is_file());
    assert!(!root.join("example/pubspec.lock").exists());
    assert_eq!(
        get(&["--offline", &format!("--directory={directory}")]).ok(),
        Some(0)
    );
    // The values `dart pub get` 3.6.2 writes for the example.
    let config = read_json(root.join("example/.dart_tool/package_config.json"));
    assert_eq!(config["packages"][0]["name"], "pkg");
    assert_eq!(config["packages"][0]["rootUri"], "../../");
    assert_eq!(config["packages"][1]["rootUri"], "../");
    let lock =
        manifest::yaml_json(&std::fs::read_to_string(root.join("example/pubspec.lock")).unwrap())
            .unwrap();
    assert_eq!(
        lock["packages"]["pkg"]["description"],
        serde_json::json!({"path": "..", "relative": true})
    );
    assert_eq!(get(&["--precompile"]).err().map(|e| e.code), Some(1));
    assert_eq!(get(&["--upgrade"]).err().map(|e| e.code), Some(64));
}

#[test]
fn enforce_lockfile_fails_without_or_with_a_stale_lock() {
    let f = Fixture::new();
    let root = f.package("app", "name: app\nenvironment:\n  sdk: ^3.6.0\n");
    f.package(
        "dep",
        "name: dep\nversion: 1.0.0\nenvironment:\n  sdk: ^3.6.0\n",
    );
    let directory = root.to_str().unwrap();
    let error = get(&["--offline", "--enforce-lockfile", "-C", directory])
        .err()
        .unwrap();
    assert_eq!(error.code, 1);
    assert!(
        error
            .message
            .contains("Cannot do `--enforce-lockfile` without an existing `pubspec.lock`.")
    );
    assert_eq!(get(&["--offline", "-C", directory]).ok(), Some(0));
    // A lockfile written by `dart pub get` with the same content is kept as is.
    let pub_lock = "# Generated by pub\n# See https://dart.dev/tools/pub/glossary#lockfile\npackages: {}\nsdks:\n  dart: \">=3.6.0 <4.0.0\"\n";
    std::fs::write(root.join("pubspec.lock"), pub_lock).unwrap();
    assert_eq!(
        get(&["--offline", "--enforce-lockfile", "-C", directory]).ok(),
        Some(0)
    );
    assert_eq!(
        std::fs::read_to_string(root.join("pubspec.lock")).unwrap(),
        pub_lock
    );
    std::fs::write(
        root.join("pubspec.yaml"),
        "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  dep:\n    path: ../dep\n",
    )
    .unwrap();
    let error = get(&["--offline", "--enforce-lockfile", "-C", directory])
        .err()
        .unwrap();
    assert_eq!(error.code, 65);
    assert!(
        error
            .message
            .starts_with("Unable to satisfy `pubspec.yaml` using `pubspec.lock`.")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("pubspec.lock")).unwrap(),
        pub_lock
    );
    assert_eq!(get(&["--offline", "-C", directory]).ok(), Some(0));
    assert_eq!(
        get(&["--offline", "--enforce-lockfile", "-C", directory]).ok(),
        Some(0)
    );
}

fn git(repository: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=DartForge",
            "-c",
            "user.email=dartforge@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(repository)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn commit(repository: &std::path::Path, files: &[(&str, &str)]) -> String {
    for (path, text) in files {
        let path = repository.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(repository, &["add", "-A"]);
    git(repository, &["commit", "-q", "-m", "change"]);
    git(repository, &["rev-parse", "HEAD"])
}

#[test]
fn git_dependencies_resolve_commits_and_lay_out_the_cache_like_pub() {
    let f = Fixture::new();
    let repository = f.0.join("repos/dep_repo.git");
    std::fs::create_dir_all(&repository).unwrap();
    git(&repository, &["init", "-q"]);
    let first = commit(
        &repository,
        &[
            (
                "pubspec.yaml",
                "name: dep\nversion: 1.2.3\nenvironment:\n  sdk: ^3.0.0\n",
            ),
            (
                "sub/pkg/pubspec.yaml",
                "name: inner\nversion: 0.1.0\nenvironment:\n  sdk: ^3.0.0\n",
            ),
        ],
    );
    let url = repository.to_str().unwrap().to_owned();
    let root = f.package(
        "app",
        &format!(
            "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  dep:\n    git: {url}\n  inner:\n    git:\n      url: {url}\n      path: ./sub//pkg\n      ref: main\n"
        ),
    );
    let cache = f.0.join("cache");
    let resolve = |locked: bool| {
        let mut r = resolver::Resolver::new(
            hosted::Hosted::new(cache.clone(), false, "https://pub.dev".into()),
            semver::Version::new(3, 6, 2),
            if locked {
                output::read_lock(&root, "https://pub.dev").unwrap()
            } else {
                BTreeMap::new()
            },
        );
        r.locked_digests = output::read_lock_digests(&root).unwrap();
        let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
        let selected = r.resolve(&m, &root).unwrap();
        for value in selected.values() {
            if let manifest::Source::Git { url, path, .. } = &value.source {
                r.git
                    .materialize(url, path, value.candidate.digest.as_deref().unwrap())
                    .unwrap();
            }
        }
        output::write(&root, &m, &selected, &r.sdk).unwrap();
        selected
    };
    let selected = resolve(false);
    assert_eq!(
        selected["dep"].candidate.digest.as_deref(),
        Some(first.as_str())
    );
    let lock =
        manifest::yaml_json(&std::fs::read_to_string(root.join("pubspec.lock")).unwrap()).unwrap();
    // The fields and values `dart pub get` 3.6.2 writes for the same layout.
    assert_eq!(
        lock["packages"]["dep"],
        serde_json::json!({
            "dependency": "direct main",
            "description": {"path": ".", "ref": "HEAD", "resolved-ref": first, "url": url},
            "source": "git",
            "version": "1.2.3",
        })
    );
    assert_eq!(lock["packages"]["inner"]["description"]["path"], "sub/pkg");
    assert_eq!(lock["packages"]["inner"]["description"]["ref"], "main");
    let checkout = cache.join(format!("git/dep_repo-{first}"));
    assert!(checkout.join("sub/pkg/pubspec.yaml").is_file());
    let packages = std::fs::read_to_string(checkout.join(".git/pub-packages")).unwrap();
    assert_eq!(packages.split('\n').collect::<Vec<_>>(), [".", "sub/pkg"]);
    assert!(cache.join("git/cache").read_dir().unwrap().count() == 1);
    let config = read_json(root.join(".dart_tool/package_config.json"));
    let uri = |name: &str| {
        config["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap()["rootUri"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let base = url::Url::from_file_path(&checkout).unwrap().to_string();
    assert_eq!(uri("dep"), format!("{base}/"));
    assert_eq!(uri("inner"), format!("{base}/sub/pkg"));

    // A new commit is not taken while the lockfile pins the old one.
    let second = commit(
        &repository,
        &[(
            "pubspec.yaml",
            "name: dep\nversion: 1.3.0\nenvironment:\n  sdk: ^3.0.0\n",
        )],
    );
    assert_eq!(
        resolve(true)["dep"].candidate.digest.as_deref(),
        Some(first.as_str())
    );
    let selected = resolve(false);
    assert_eq!(
        selected["dep"].candidate.digest.as_deref(),
        Some(second.as_str())
    );
    assert_eq!(
        selected["dep"].candidate.manifest.version.to_string(),
        "1.3.0"
    );

    // Offline resolution cannot clone a repository that is not cached.
    let mut offline = resolver::Resolver::new(
        hosted::Hosted::new(f.0.join("empty-cache"), true, "https://pub.dev".into()),
        semver::Version::new(3, 6, 2),
        BTreeMap::new(),
    );
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    assert!(offline.resolve(&m, &root).unwrap_err().contains("offline"));
}

#[test]
fn git_descriptions_are_validated_like_pub() {
    let f = Fixture::new();
    for (description, message) in [
        ("git: relative/repo", "relative git repository paths"),
        (
            "git:\n      url: https://h/r\n      tag_pattern: v{{version}}",
            "\"tag_pattern\"",
        ),
        (
            "git:\n      url: https://h/r\n      path: ../out",
            "inside the repository",
        ),
        ("git:\n      ref: main", "requires a url"),
        (
            "git: https://h/r\n    hosted: x",
            "unsupported git dependency fields",
        ),
    ] {
        let root = f.package(
            "app",
            &format!(
                "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  dep:\n    {description}\n"
            ),
        );
        let error = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap_err();
        assert!(error.contains(message), "{description}: {error}");
    }
    let root = f.package(
        "app",
        "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  a:\n    git: git@github.com:org/a.git\n  b:\n    git: https://github.com/org/b\n    version: ^1.0.0\n",
    );
    let m = manifest::Manifest::read(&root, "https://pub.dev", true).unwrap();
    assert_eq!(
        m.dependencies["a"].source,
        manifest::Source::Git {
            url: "git@github.com:org/a.git".into(),
            reference: "HEAD".into(),
            path: ".".into(),
        }
    );
    assert_eq!(m.dependencies["b"].constraint.text, "^1.0.0");
}

#[test]
fn stale_locked_paths_from_other_systems_are_ignored() {
    let f = Fixture::new();
    let root = f.package(
        "app",
        "name: app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  dep:\n    path: ../dep\n",
    );
    f.package(
        "dep",
        "name: dep\nversion: 1.0.0\nenvironment:\n  sdk: ^3.6.0\n",
    );
    std::fs::write(
        root.join("pubspec.lock"),
        "packages:\n  dep:\n    dependency: \"direct main\"\n    description:\n      path: \"C:\\\\Projects\\\\dep\"\n      relative: false\n    source: path\n    version: \"1.0.0\"\nsdks:\n  dart: \">=3.6.0 <4.0.0\"\n",
    )
    .unwrap();
    assert_eq!(
        get(&["--offline", "-C", root.to_str().unwrap()]).ok(),
        Some(0)
    );
    let lock =
        manifest::yaml_json(&std::fs::read_to_string(root.join("pubspec.lock")).unwrap()).unwrap();
    assert_eq!(lock["packages"]["dep"]["description"]["path"], "../dep");
}
