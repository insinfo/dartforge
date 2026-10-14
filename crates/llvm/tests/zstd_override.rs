#[path = "../build_support/zstd.rs"]
mod zstd;

#[test]
fn explicit_zstd_override_selects_static_or_shared_and_rejects_bad_paths() {
    let directory =
        std::env::temp_dir().join(format!("dartforge-zstd-override-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for (name, kind) in [
        ("libzstd.a", "static"),
        ("libzstd.dylib", "dylib"),
        ("libzstd.so", "dylib"),
    ] {
        let path = directory.join(name);
        std::fs::write(&path, []).unwrap();
        let directives = zstd::override_directives(&path).unwrap();
        assert!(directives.contains(&format!("cargo::rustc-link-lib={kind}=zstd")));
        assert!(directives.contains(&format!("cargo::rerun-if-changed={}", path.display())));
    }
    let wrong = directory.join("unrelated.a");
    std::fs::write(&wrong, []).unwrap();
    assert!(
        zstd::override_directives(&wrong)
            .unwrap_err()
            .contains("must name libzstd")
    );
    assert!(
        zstd::override_directives(&directory.join("missing.a"))
            .unwrap_err()
            .contains("not a library file")
    );
    std::fs::remove_dir_all(directory).unwrap();
}
