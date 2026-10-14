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
        "name: app\ndependency_overrides: {}\n",
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
fn irrelevant_unsupported_historical_versions_do_not_poison_supported_release() {
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
            .contains("SDK package sources are not yet supported")
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
