//! Structural gate of the zero-cost rule (PLANO.md, "geração de código e
//! macros: rápidas, e custo zero para quem não usa"): in a project without
//! builders, the session builds no engine, the report has no engine, and an
//! edit allocates **exactly** the same as in a session built without stages
//! (the engine disabled by construction).
//!
//! **A single `#[test]` in this binary**: the counting allocator is global.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_dev::Session;
use std::path::{Path, PathBuf};

fn simulated_sdk(dir: &Path) -> PathBuf {
    let lib = dir.join("sdk/lib");
    std::fs::create_dir_all(lib.join("core")).unwrap();
    std::fs::write(
        lib.join("libraries.json"),
        r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
    )
    .unwrap();
    std::fs::write(
        lib.join("core/core.dart"),
        r#"
        library dart.core;
        class Object { const Object(); String toString() => ""; }
        class num extends Object {}
        class int extends num { int operator +(int o) => this; }
        class double extends num {}
        class String extends Object { int get length => 0; }
        class bool extends Object {}
        class Null extends Object {}
        class Function extends Object {}
        class Record extends Object {}
        class Iterable<E> extends Object {}
        class List<E> extends Object implements Iterable<E> {}
        class Map<K, V> extends Object {}
        class Set<E> extends Object implements Iterable<E> {}
        class Type extends Object {}
        class Symbol extends Object {}
        class Enum extends Object {}
        class StackTrace extends Object {}
        class Invocation extends Object {}
        class Comparable<T> extends Object {}
        void print(Object? o) {}
        "#,
    )
    .unwrap();
    lib
}

/// A package without `build_runner`: `pubspec.yaml`, a `package_config.json`
/// with only the root, and the input importing a support library.
fn project(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let proj = dir.join("proj");
    std::fs::create_dir_all(proj.join("lib")).unwrap();
    std::fs::create_dir_all(proj.join(".dart_tool")).unwrap();
    std::fs::write(
        proj.join("pubspec.yaml"),
        "name: proj\nenvironment:\n  sdk: ^3.6.0\n",
    )
    .unwrap();
    // Empty definition sections are valid configuration, not a generation
    // stage. No lockfile or engine is needed for this project.
    std::fs::write(
        proj.join("build.yaml"),
        "builders:\npost_process_builders:\n",
    )
    .unwrap();
    let cfg = proj.join(".dart_tool/package_config.json");
    std::fs::write(
        &cfg,
        r#"{"configVersion":2,"packages":[{"name":"proj","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    )
    .unwrap();
    let support = proj.join("lib/support.dart");
    std::fs::write(&support, "int twice(int x) => x + x;\n").unwrap();
    let main = proj.join("main.dart");
    std::fs::write(
        &main,
        "import 'package:proj/support.dart';\nvoid main() { print(twice(2)); }\n",
    )
    .unwrap();
    (main, support, cfg)
}

#[test]
fn project_without_builders_costs_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = simulated_sdk(tmp.path());
    let (main, support, cfg) = project(tmp.path());
    // Same output path length in both sessions.
    let (s1, s2) = (tmp.path().join("output1"), tmp.path().join("output2"));
    let mut with_detection =
        Session::new(&main, Some(&sdk), Some(&cfg), &s1).expect("session with detection");
    let mut without_stages = Session::with_stages(&main, Some(&sdk), Some(&cfg), &s2, Vec::new())
        .expect("session without stages");
    assert!(
        !with_detection.has_stages(),
        "a project without build_runner got a generation stage"
    );
    let r1 = with_detection.compile().expect("first compilation");
    let r2 = without_stages.compile().expect("first compilation");
    assert!(
        r1.engine.is_none() && r2.engine.is_none(),
        "report with an engine in a project without builders"
    );
    assert_eq!(r1.modules, r2.modules);
    for i in 0..4 {
        std::fs::write(
            &support,
            format!("int twice(int x) {{ var y = {i:03}; return x + x; }}\n"),
        )
        .unwrap();
        let mut counts = Vec::new();
        for s in [&mut with_detection, &mut without_stages] {
            let before = dartforge_instrument::allocation_count();
            for p in s.changes() {
                s.file_changed(&p);
            }
            s.file_changed(&support);
            let r = s.compile().expect("edit");
            counts.push(dartforge_instrument::allocation_count() - before);
            assert!(r.engine.is_none());
            assert_eq!(
                r.reanalyzed_units, 1,
                "a body edit reanalyzes only the edited unit"
            );
        }
        assert_eq!(
            counts[0], counts[1],
            "the edit allocated differently with engine detection ({}) and without stages ({})",
            counts[0], counts[1]
        );
    }
    assert_eq!(
        dartforge_build::instance_count(),
        0,
        "a build engine was built"
    );
}
