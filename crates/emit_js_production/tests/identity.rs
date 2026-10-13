//! With `filter: None` the emitter must be **byte for byte** the usual one —
//! this is what guarantees that the development profile and `dartforge
//! serve` do not feel step 5 (`docs/JS-PRODUCAO.md` §1.7). The same program
//! is compiled through three paths and the modules are compared:
//!
//! 1. `dartforge_emit_js::compile` (the `compile-js` path);
//! 2. `Analysis::emit(None)`;
//! 3. `Analysis::emit(Some(&AllLive))` — the filter that says everything
//!    is live.
//!
//! If `DARTFORGE_IDENTITY_REF` points to a directory with the output of
//! `dartforge compile-js` from **before** the change (`<name>/…js`), path 1 is
//! also compared against it.

use std::path::{Path, PathBuf};

fn programs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut v: Vec<PathBuf> = ["p1_basic", "p2_functions", "p3_classes", "p3_generics", "p4_collections", "p5_exceptions", "p6_async", "p7_dynamic", "p7_patterns", "p7_libraries/main", "p8_nosuchmethod_order"]
        .iter()
        .map(|n| root.join(format!("crates/emit_js/tests/programs/{n}.dart")))
        .collect();
    for n in ["51_mixins", "57_nosuchmethod", "90_dynamic_chamadas", "207_factory_redirecionada_tearoff", "209_js_interop/main", "212_rti_de_superclasse_generica"] {
        let p = root.join(format!("corpus/js/{n}.dart"));
        if p.is_file() {
            v.push(p);
        }
    }
    v
}

fn modules(e: &dartforge_emit_js::Emitted) -> Vec<(String, String)> {
    let mut m = e.modules.clone();
    m.sort();
    m
}

#[test]
#[ignore = "compiles real programs against the SDK; run with --ignored"]
fn no_filter_and_all_live_emit_the_same_text() {
    let reference = std::env::var("DARTFORGE_IDENTITY_REF").ok().map(PathBuf::from);
    for p in programs() {
        let packages = p.parent().map(|d| d.join(".dart_tool/package_config.json")).filter(|x| x.is_file());
        let a = dartforge_emit_js::compile(&p, None, packages.as_deref()).expect("compilar");
        let ((b, c), _) = dartforge_emit_js::compile_with(&p, None, packages.as_deref(), &Default::default(), |an| {
            let b = an.emit(None)?;
            let c = an.emit(Some(&dartforge_emit_js::filter::AllLive))?;
            Ok((b, c))
        })
        .expect("compilar_com");
        let (ma, mb, mc) = (modules(&a), modules(&b), modules(&c));
        assert_eq!(ma, mb, "{}: `compilar` and `emitir(None)` diverge", p.display());
        assert_eq!(mb, mc, "{}: `emitir(None)` and `emitir(TudoVivo)` diverge", p.display());
        assert_eq!(a.entry, c.entry);
        if let Some(r) = &reference {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            let dir = r.join(&name);
            if dir.is_dir() {
                for (path, text) in &ma {
                    let old = std::fs::read_to_string(dir.join(path)).unwrap_or_else(|_| panic!("{}: {path} missing from the reference", p.display()));
                    assert_eq!(&old.replace("\r\n", "\n"), text, "{}: {path} differs from the output before", p.display());
                }
            }
        }
    }
}
