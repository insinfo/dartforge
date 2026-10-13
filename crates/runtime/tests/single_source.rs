//! The runtime's single source: what the AOT compiles and what the JIT uses are the
//! same code, with the same semantics.

/// The text the AOT compiles contains, unchanged, the files this crate compiles
/// as `heap` and `abi`: `heap.rs` and then all the fragments concatenated in the
/// order of `FRAGMENTS`.
#[test]
fn aot_compiles_the_same_files() {
    let text = dartforge_runtime::RUNTIME_MAIN;
    assert!(text.contains(include_str!("../src/heap.rs")));
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut concatenated = String::new();
    for f in dartforge_runtime::symbols::FRAGMENTS {
        concatenated.push_str(
            &std::fs::read_to_string(src.join(format!("{f}.rs"))).expect("read fragment"),
        );
    }
    assert!(text.ends_with(&concatenated));
}

/// The generated table has unique names, real addresses and no C `main`.
#[test]
fn the_table_covers_runtime_symbols() {
    let table = dartforge_runtime::symbols::table();
    assert_eq!(table.len(), dartforge_runtime::symbols::NAMES.len());
    assert!(table.len() > 100, "{}", table.len());
    let mut names: Vec<&str> = table.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), table.len());
    assert!(
        table
            .iter()
            .all(|(n, a)| n.starts_with("dartforge_") && *a != 0)
    );
    assert!(!names.contains(&"main"));
    // Every announced name appears in the text the AOT compiles.
    for name in &names {
        assert!(
            dartforge_runtime::RUNTIME_MAIN.contains(&format!("fn {name}(")),
            "{name} is not in RUNTIME_MAIN"
        );
    }
}

/// @Native resolves the whole reserved catalog in the instance serving the call,
/// even when the executable does not export its functions to the process lookup.
#[test]
#[allow(unsafe_code)] // The proof calls the ABI whose contract requires valid bytes.
fn native_resolver_and_its_cache_keep_this_runtimes_addresses() {
    for (name, expected) in dartforge_runtime::symbols::table() {
        for _ in 0..2 {
            // SAFETY: the name is a static UTF-8 string and lives during the call.
            let address = unsafe {
                dartforge_runtime::abi::dartforge_ffi_native_symbol(
                    name.as_ptr(),
                    name.len() as i64,
                )
            };
            assert_eq!(address as usize, expected, "{name}");
        }
    }
}

/// This package's profile is the AOT one (`rustc -O`): integer overflow wraps
/// around, it does not `panic`. The `[profile.dev.package.dartforge-runtime]`
/// override in the root `Cargo.toml` applies to every target of the package,
/// including this test; without it, the `test` profile (which inherits from `dev`)
/// would panic here, and the JIT runtime would diverge from the AOT on every
/// overflowing computation.
#[test]
fn overflow_wraps_as_in_aot() {
    let r = std::panic::catch_unwind(|| std::hint::black_box(i64::MAX) + std::hint::black_box(1));
    assert_eq!(r.ok(), Some(i64::MIN));
}
