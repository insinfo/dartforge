//! Package URI resolution follows directory identity, including unsaved files and invalidations.
#![cfg(any(unix, windows))]

use dartforge_elements::{PackageConfig, config::canonicalize, fs_cache, generated, unit_cache};
use std::path::{Path, PathBuf};
use url::Url;

static CACHE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct CacheScope {
    _guard: std::sync::MutexGuard<'static, ()>,
}
impl CacheScope {
    fn new() -> Self {
        let guard = CACHE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        fs_cache::trust(false);
        Self { _guard: guard }
    }
}
impl Drop for CacheScope {
    fn drop(&mut self) {
        fs_cache::trust(false);
    }
}
fn link_directory(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        // Directory junctions do not require the symlink privilege.
        let output = std::process::Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
fn unlink_directory(link: &Path) {
    #[cfg(unix)]
    std::fs::remove_file(link).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(link).unwrap();
}

fn configure(root: &Path, root_uri: &str, package_uri: &str) -> PackageConfig {
    let config = root.join(".dart_tool/package_config.json");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, serde_json::to_vec(&serde_json::json!({
            "configVersion": 2,
            "packages": [{"name":"app", "rootUri":root_uri, "packageUri":package_uri, "languageVersion":"3.6"}]
        })).unwrap()).unwrap();
    PackageConfig::load(&config).unwrap()
}
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let real = temp.path().join("real");
    std::fs::create_dir_all(real.join("lib")).unwrap();
    std::fs::write(real.join("lib/value.dart"), "int value = 1;\n").unwrap();
    let alias = temp.path().join("alias");
    link_directory(&real, &alias);
    (temp, canonicalize(&real).unwrap(), alias)
}
#[test]
fn absolute_alias_matches_entry_file_identity() {
    let _cache = CacheScope::new();
    let (_temp, real, alias) = setup();
    let config = configure(
        &real,
        Url::from_directory_path(&alias).unwrap().as_str(),
        "lib/",
    );
    assert_eq!(
        config
            .resolve_package_uri("package:app/value.dart")
            .unwrap(),
        real.join("lib/value.dart")
    );
}
#[test]
fn canonical_and_relative_configuration_stay_equivalent() {
    let _cache = CacheScope::new();
    let (_temp, real, _alias) = setup();
    for root_uri in [
        Url::from_directory_path(&real).unwrap().to_string(),
        "../".to_string(),
    ] {
        let config = configure(&real, &root_uri, "lib/");
        assert_eq!(
            config
                .resolve_package_uri("package:app/value.dart")
                .unwrap(),
            real.join("lib/value.dart")
        );
    }
}
#[test]
fn linked_package_directory_uses_the_same_file_key() {
    let _cache = CacheScope::new();
    let (temp, real, _alias) = setup();
    let shared = temp.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::write(shared.join("value.dart"), "int value = 2;\n").unwrap();
    link_directory(&shared, &real.join("linked"));
    let config = configure(
        &real,
        Url::from_directory_path(&real).unwrap().as_str(),
        "linked/",
    );
    assert_eq!(
        config
            .resolve_package_uri("package:app/value.dart")
            .unwrap(),
        canonicalize(&shared).unwrap().join("value.dart")
    );
}
#[test]
fn unsaved_overlay_child_has_the_canonical_package_key() {
    let _cache = CacheScope::new();
    let (_temp, real, alias) = setup();
    let mut config = configure(
        &real,
        Url::from_directory_path(&alias).unwrap().as_str(),
        "lib/",
    );
    let child = real.join("lib/unsaved.dart");
    let mut generation = generated::GenerationBuilder::new();
    generation.put(child.clone(), "int unsaved = 1;".to_string(), "lsp", vec![]);
    config.generated_sources = Some(generation.finish(1).unwrap());
    assert!(!child.exists());
    assert_eq!(
        config
            .resolve_package_uri("package:app/unsaved.dart")
            .unwrap(),
        child
    );
}
#[test]
fn alias_retarget_is_seen_after_filesystem_invalidation() {
    let _cache = CacheScope::new();
    let (temp, real, alias) = setup();
    let other = temp.path().join("other");
    std::fs::create_dir_all(other.join("lib")).unwrap();
    std::fs::write(other.join("lib/value.dart"), "int value = 2;\n").unwrap();
    let uri = Url::from_directory_path(&alias).unwrap();
    let config = configure(&real, uri.as_str(), "lib/");
    assert_eq!(
        std::fs::read_to_string(
            config
                .resolve_package_uri("package:app/value.dart")
                .unwrap()
        )
        .unwrap(),
        "int value = 1;\n"
    );
    fs_cache::trust(true);
    unlink_directory(&alias);
    link_directory(&other, &alias);
    fs_cache::forget(std::slice::from_ref(&alias));
    let config = PackageConfig::load(&real.join(".dart_tool/package_config.json")).unwrap();
    let actual = std::fs::read_to_string(
        config
            .resolve_package_uri("package:app/value.dart")
            .unwrap(),
    )
    .unwrap();
    fs_cache::trust(false);
    assert_eq!(actual, "int value = 2;\n");
}

#[test]
fn same_generation_reuses_resolved_package_directories() {
    let _cache = CacheScope::new();
    let (_temp, real, alias) = setup();
    configure(
        &real,
        Url::from_directory_path(&alias).unwrap().as_str(),
        "lib/",
    );
    let path = real.join(".dart_tool/package_config.json");
    fs_cache::trust(true);
    PackageConfig::load(&path).unwrap();
    let before = fs_cache::stats();
    let config = PackageConfig::load(&path).unwrap();
    let after = fs_cache::stats();
    fs_cache::trust(false);
    assert_eq!(
        canonicalize(
            &config
                .resolve_package_uri("package:app/value.dart")
                .unwrap()
        )
        .unwrap(),
        real.join("lib/value.dart")
    );
    assert_eq!(before.misses, after.misses);
}
#[test]
fn preserved_stamp_configuration_edit_reloads_after_forget() {
    let _cache = CacheScope::new();
    let (_temp, real, _alias) = setup();
    for name in ["first", "other"] {
        std::fs::create_dir_all(real.join(name).join("lib")).unwrap();
        std::fs::write(real.join(name).join("lib/value.dart"), name).unwrap();
    }
    configure(&real, "../first/", "lib/");
    let path = real.join(".dart_tool/package_config.json");
    let before = std::fs::metadata(&path).unwrap();
    fs_cache::trust(true);
    let initial = PackageConfig::load(&path).unwrap();
    assert_eq!(
        std::fs::read_to_string(
            initial
                .resolve_package_uri("package:app/value.dart")
                .unwrap()
        )
        .unwrap(),
        "first"
    );
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("../first/", "../other/");
    std::fs::write(&path, text).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    assert_eq!(
        unit_cache::FileStamp::of(&before),
        unit_cache::FileStamp::of(&std::fs::metadata(&path).unwrap())
    );
    fs_cache::forget(std::slice::from_ref(&path));
    let updated = PackageConfig::load(&path).unwrap();
    fs_cache::trust(false);
    assert_eq!(
        std::fs::read_to_string(
            updated
                .resolve_package_uri("package:app/value.dart")
                .unwrap()
        )
        .unwrap(),
        "other"
    );
}

#[test]
fn missing_package_directory_keeps_unsaved_overlay_identity() {
    let _cache = CacheScope::new();
    let (_temp, real, alias) = setup();
    std::fs::remove_file(real.join("lib/value.dart")).unwrap();
    std::fs::remove_dir(real.join("lib")).unwrap();
    let mut config = configure(
        &real,
        Url::from_directory_path(&alias).unwrap().as_str(),
        "lib/generated/",
    );
    let child = real.join("lib/generated/unsaved.dart");
    let mut generation = generated::GenerationBuilder::new();
    generation.put(child.clone(), "int unsaved = 1;".to_string(), "lsp", vec![]);
    config.generated_sources = Some(generation.finish(1).unwrap());
    assert!(!child.parent().unwrap().exists());
    assert_eq!(
        config
            .resolve_package_uri("package:app/unsaved.dart")
            .unwrap(),
        child
    );
}
