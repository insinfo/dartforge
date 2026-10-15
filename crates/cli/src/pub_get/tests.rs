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
    for text in [
        "name: app\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  foo:\n    git: https://example.com/foo\n",
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
    assert_eq!(lock["packages"]["b"]["dependency"], "direct overridden");
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
    use std::io::{Read, Write};
    let f = Fixture::new();
    let bytes = archive("lib/a.dart", tar::EntryType::Regular);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        stream.read(&mut request).unwrap();
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
    assert!(text.contains("description: \"dart\"\n    source: sdk"));
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
        url::Url::from_directory_path(sdk.join("declared/one"))
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
    let outside = fixture.package("outside", "name: vendor_one\nversion: 1.2.3\nenvironment:\n  sdk: ^3.4.0\n");
    std::fs::remove_dir_all(root.join("declared/one")).unwrap();
    std::os::unix::fs::symlink(outside, root.join("declared/one")).unwrap();
    let error = sdk::candidate(Some(&root), "dart", "vendor_one", &constraints::parse_version("3.6.2").unwrap(), "https://pub.dev").unwrap_err();
    assert!(error.contains("escapes the selected SDK"));
}
