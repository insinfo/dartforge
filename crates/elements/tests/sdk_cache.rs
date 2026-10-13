//! Cache of the analyzed SDK (`sdk_cache.rs`): loading through the cache
//! produces the same `Program` as loading from the files and is faster. It
//! needs SDK 3.6.2; without it the test is skipped.

use dartforge_elements::load::{load_lenient, load_lenient_com_cache};
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::SdkCache;
use dartforge_intern::Interner;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::tempdir;

fn sdk_real() -> Option<SdkLayout> {
    let lib = SdkLayout::discover().or_else(|| {
        let p = PathBuf::from("C:/tools/dartsdk-3.6.2/lib");
        p.join("libraries.json").exists().then_some(p)
    })?;
    SdkLayout::load(&lib, "dartdevc").ok()
}

#[test]
fn sdk_cache_reproduces_the_program_and_is_faster() {
    let Some(sdk) = sdk_real() else {
        eprintln!("SDK not found; test skipped");
        return;
    };
    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(
        &entry,
        "import 'dart:async';\nimport 'dart:convert';\nimport 'dart:collection';\nimport 'dart:math';\nimport 'dart:typed_data';\n\
         part 'part.dart';\nclass C { int x = 1; }\nvoid main() { print(json.encode([C().x, max(1, 2)])); }\n",
    )
    .unwrap();
    fs::write(tmp.path().join("part.dart"), "part of 'main.dart';\nint dobro(int a) => a * 2;\n").unwrap();

    // 1. From the files.
    let mut i1 = Interner::new();
    let (p1, d1) = load_lenient(&entry, &sdk, None, &mut i1);
    assert!(d1.is_empty(), "{d1:?}");

    // 2. Builds the cache (once) and loads through it.
    let caminho = tmp.path().join("sdk.bin");
    let t1 = Instant::now();
    SdkCache::build_and_write(&sdk, &caminho).expect("builds the cache");
    let t_build = t1.elapsed();
    assert!(caminho.exists());
    let cache = SdkCache::abrir(&caminho).expect("opens the cache");
    let mut i2 = Interner::new();
    let (p2, d2) = load_lenient_com_cache(&entry, &sdk, None, &mut i2, Some(cache));
    assert!(d2.is_empty(), "{d2:?}");

    // 3. Time: the minimum of N runs of each path, interleaved (the runner's
    // noise only adds; the minimum is the cost of the work). In the test's
    // `dev` profile, the cache with parallel decoding takes ~45% of the files
    // path (75 ms × 165 ms on the development machine; serially it tied,
    // 150 × 155 ms, and the test failed at random); in `release`, 15 × 44 ms
    // (serially, 22 ms). The margin requires it to stay clearly faster.
    const N: usize = 5;
    let mut t_files = std::time::Duration::MAX;
    let mut t_cache = std::time::Duration::MAX;
    for _ in 0..N {
        let t = Instant::now();
        let _ = load_lenient(&entry, &sdk, None, &mut Interner::new());
        t_files = t_files.min(t.elapsed());
        let t = Instant::now();
        let cache = SdkCache::abrir(&caminho).expect("opens the cache");
        let _ = load_lenient_com_cache(&entry, &sdk, None, &mut Interner::new(), Some(cache));
        t_cache = t_cache.min(t.elapsed());
    }

    eprintln!(
        "minimum of {N}: files {t_files:.1?}; open+load through the cache {t_cache:.1?} ({:.0}%); build cache {t_build:.1?}; \
         units {} / libraries {} / classes {} / functions {} / variables {}",
        100.0 * t_cache.as_secs_f64() / t_files.as_secs_f64(),
        p2.units.len(), p2.libraries.len(), p2.classes.len(), p2.functions.len(), p2.variables.len()
    );

    // Same program: counts, URIs and roles of the units in the same order, and
    // the same class/function names in the same order.
    assert_eq!(p1.libraries.len(), p2.libraries.len());
    assert_eq!(p1.units.len(), p2.units.len());
    assert_eq!(p1.classes.len(), p2.classes.len());
    assert_eq!(p1.functions.len(), p2.functions.len());
    assert_eq!(p1.variables.len(), p2.variables.len());
    assert_eq!(p1.extensions.len(), p2.extensions.len());
    assert_eq!(p1.typedefs.len(), p2.typedefs.len());
    for (u1, u2) in p1.units.iter().zip(&p2.units) {
        assert_eq!(u1.uri, u2.uri);
        assert_eq!(u1.role, u2.role);
        assert_eq!(u1.source, u2.source);
        assert_eq!(u1.ast.exprs.len(), u2.ast.exprs.len(), "{}", u1.uri);
        assert_eq!(u1.unit.declarations.len(), u2.unit.declarations.len(), "{}", u1.uri);
    }
    for (c1, c2) in p1.classes.iter().zip(&p2.classes) {
        assert_eq!(i1.resolve(c1.name), i2.resolve(c2.name));
        assert_eq!(c1.instance_members.len(), c2.instance_members.len());
    }
    for (f1, f2) in p1.functions.iter().zip(&p2.functions) {
        assert_eq!(i1.resolve(f1.name), i2.resolve(f2.name));
    }
    let sdk_units = p2.units.iter().filter(|u| p2.library(u.library).is_sdk).count();
    assert!(sdk_units > 100, "the SDK has more than 100 units: {sdk_units}");
    // The user's unit and its part still come from the files.
    assert!(p2.units.iter().any(|u| u.uri.ends_with("/part.dart") && !p2.library(u.library).is_sdk));

    assert!(
        t_cache.as_secs_f64() < 0.75 * t_files.as_secs_f64(),
        "open+load through the cache ({t_cache:?}, minimum of {N}) should cost less than 75% of loading from the files ({t_files:?})"
    );
}

#[test]
fn invalid_cache_is_ignored() {
    let Some(sdk) = sdk_real() else {
        return;
    };
    let tmp = tempdir().unwrap();
    let caminho = tmp.path().join("sdk.bin");
    fs::write(&caminho, b"garbage").unwrap();
    assert!(SdkCache::abrir(&caminho).is_none());
    assert!(!SdkCache::hash(&sdk, "dartdevc").is_empty());
    assert_ne!(SdkCache::hash(&sdk, "dartdevc"), SdkCache::hash(&sdk, "dart2js"));
}
