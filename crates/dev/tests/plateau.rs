//! Memory plateau of the resident session (PLANO.md, item 1 of "O que
//! falta").
//!
//! Twenty successive edits — body, public signature and import — and the
//! live bytes must settle on a plateau instead of growing with N. Growth
//! here means some live structure keeps the model of the previous edit: it
//! is the defect that makes Dart's analyzer and `webdev` reach gigabytes.
//!
//! **A single `#[test]` in this binary**: the counting allocator is global
//! and measures the whole process; two tests in parallel would measure each
//! other.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_dev::Session;
use std::path::{Path, PathBuf};

/// Simulated SDK: `dart:core` with the minimum to type the test project.
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

/// Project: a support library and the input that imports it.
fn project(dir: &Path) -> (PathBuf, PathBuf) {
    let proj = dir.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let support = proj.join("support.dart");
    std::fs::write(&support, "int twice(int x) => x + x;\n").unwrap();
    let main = proj.join("main.dart");
    std::fs::write(
        &main,
        "import 'support.dart';\nvoid main() { print(twice(2)); }\n",
    )
    .unwrap();
    (main, support)
}

/// Three kinds of edit, which invalidate different things.
fn edit(support: &Path, main: &Path, i: usize) {
    match i % 3 {
        // Body: only the function's text changes; the API stays the same.
        0 => std::fs::write(support, format!("int twice(int x) {{ var y = {i:03}; return x + x; }}\n")).unwrap(),
        // Public signature: a top-level function comes and goes.
        1 => std::fs::write(
            support,
            format!("int twice(int x) => x + x;\nint thrice{i:03}(int x) => x + x + x;\n"),
        )
        .unwrap(),
        // Import: the input starts (and stops) importing dart:core
        // explicitly, changing the graph.
        _ => std::fs::write(
            main,
            format!("import 'support.dart';\nimport 'dart:core';\nvoid main() {{ print(twice({i:03})); }}\n"),
        )
        .unwrap(),
    }
}

#[test]
fn successive_edits_settle_on_a_plateau() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = simulated_sdk(tmp.path());
    let (main, support) = project(tmp.path());
    let output = tmp.path().join("js");

    let mut session = Session::new(&main, Some(&sdk), None, &output).expect("session");
    session.compile().expect("first compilation");

    // Warm-up: the first edits still intern new names and grow the arenas;
    // the plateau is measured once the shape of the project repeats.
    for i in 0..4 {
        edit(&support, &main, i);
        session.file_changed(&support);
        session.file_changed(&main);
        session.compile().expect("warm-up compilation");
    }

    let base = dartforge_instrument::live_bytes();
    let mut largest = base;
    for i in 4..24 {
        edit(&support, &main, i);
        session.file_changed(&support);
        session.file_changed(&main);
        let r = session.compile().expect("compilation of the edit");
        assert!(r.modules >= 2, "the project has at least two modules");
        largest = largest.max(dartforge_instrument::live_bytes());
    }
    let end = dartforge_instrument::live_bytes();

    // Tolerance: the text of the edits varies in size and the `Interner`
    // keeps the new names of each revision (`thrice007`…), which are few and
    // short.
    let limit = base + 256 * 1024;
    assert!(
        end <= limit && largest <= limit,
        "memory grew with the edits: base {base} B, largest {largest} B, end {end} B (limit {limit} B)"
    );
}
