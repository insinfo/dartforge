//! The same program, emitted several times — sequentially and on threads — must
//! come out **byte for byte** identical. The chosen programs have `noSuchMethod`
//! forwarders, whose order once came from iterating a `HashMap`
//! (`Ctx::unimplemented_abstract`) and changed from one run of the same binary to
//! the next. Each new `HashMap` randomizes its keys (`RandomState`), and each
//! thread has its own seed: repeating and spreading over threads is what exposes
//! an unordered iteration that reaches the text.

use std::path::{Path, PathBuf};

fn programs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inputs = vec![
        root.join("tests/programs/p8_nosuchmethod_order.dart"),
        root.join("../../corpus/js/57_nosuchmethod.dart"),
    ];
    for path in &inputs {
        assert!(
            path.is_file(),
            "required determinism input missing: {}",
            path.display()
        );
    }
    inputs
}

fn emit(p: &Path) -> Vec<(String, String)> {
    let mut m = dartforge_emit_js::compile(p, None, None)
        .expect("compile")
        .modules;
    assert!(
        !m.is_empty(),
        "{}: compilation produced no modules",
        p.display()
    );
    m.sort();
    m
}

const SEQUENTIAL: usize = 6;
const THREADS: usize = 4;

#[test]
#[ignore = "compiles real programs against the SDK; run with --ignored"]
fn emission_with_nosuchmethod_is_identical_byte_for_byte() {
    for p in programs() {
        // The first compilation also prepares the SDK artifact, before the
        // threads compete for it.
        let reference = emit(&p);
        for i in 1..SEQUENTIAL {
            assert!(
                emit(&p) == reference,
                "{}: run {i} diverges from the first",
                p.display()
            );
        }
        let results: Vec<Vec<(String, String)>> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..THREADS).map(|_| s.spawn(|| emit(&p))).collect();
            hs.into_iter().map(|h| h.join().expect("thread")).collect()
        });
        for (i, r) in results.iter().enumerate() {
            assert!(
                *r == reference,
                "{}: thread {i} diverges from the sequential run",
                p.display()
            );
        }
    }
}

/// The order of the forwarders is the CFE's (`ClassMembersNodeBuilder.build`):
/// superclass, mixins in `with` order, interfaces in `implements` order, getter
/// before the setter with the same name.
#[test]
#[ignore = "compiles real programs against the SDK; run with --ignored"]
fn forwarders_come_out_in_cfe_order() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/p8_nosuchmethod_order.dart");
    let modules = emit(&p);
    let text = &modules
        .iter()
        .find(|(c, _)| c.ends_with("p8_nosuchmethod_order.js"))
        .expect("program module")
        .1;
    let class = &text[text.find("class Fantasma").expect("class Fantasma")..];
    let expected = [
        "incrementa() {",
        "get dobro()",
        "get historico()",
        "registra(evento) {",
        "get total()",
        "soma(a, b) {",
        "area() {",
        "perimetro() {",
        "get nome()",
        "set nome(v)",
        "get lados()",
        "get cor()",
        "set cor(v)",
        "pinta(tinta, opts) {",
    ];
    let mut last = 0;
    for m in expected {
        let pos = class
            .find(m)
            .unwrap_or_else(|| panic!("forwarder `{m}` not found"));
        assert!(pos >= last, "forwarder `{m}` out of CFE order");
        last = pos;
    }
}
