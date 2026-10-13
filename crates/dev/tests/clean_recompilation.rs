//! Invariant of the resident session: the output of an incremental
//! recompilation is **byte for byte** equal to that of a clean compilation
//! of the same final source.
//!
//! The session keeps the `Interner` across edits, so the names the edits
//! bring are interned in the order they arrive — not in source order, as in
//! a compilation from scratch. Any walk ordered by `SymbolId` (or by another
//! id that depends on arrival order) that reaches the emitted text makes the
//! two outputs diverge. The edits below intern the constructor names in the
//! **reverse** order of the final declaration (`gamma`, then `beta`, then
//! `alpha`), which is what exposed the iteration over
//! `ClassElement::constructors` (a `BTreeMap<SymbolId, _>`) in the tearoffs
//! and in the forwarding constructors of mixin applications. Named fields of
//! function types and records, private members and `noSuchMethod`
//! forwarders also arrive with their names interned in a swapped order.

use dartforge_dev::Sessao;
use std::collections::BTreeMap;
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

/// The three revisions of the project. The final one declares `alpha`,
/// `beta`, `gamma` in that order; the earlier ones bring the names in reverse
/// order.
const SUPPORT: [&str; 3] = [
    "int uses() => 1;\nvoid names({int bravo = 0, int alpha = 0}) {}\nint _zulu = 0;\nint _yankee = 0;\n",
    "int omega() => 3;\nint uses() => omega();\n",
    "int delta() => 4;\nint omega() => 3;\nint uses() => omega() + delta();\n",
];

const MAIN: [&str; 3] = [
    r#"import 'support.dart';
mixin Mark {}
class Base { final int v; Base.gamma(this.v); }
class Child extends Base with Mark { Child.gamma(int v) : super.gamma(v); }
class Point { final int x; Point.gamma(this.x); }
void main() {
  print(Point.gamma(1).x);
  print(Child.gamma(2).v);
  print(uses());
}
"#,
    r#"import 'support.dart';
mixin Mark {}
class Base { final int v; Base.beta(this.v); Base.gamma(this.v); }
class Child extends Base with Mark { Child.beta(int v) : super.beta(v); Child.gamma(int v) : super.gamma(v); }
class Point { final int x; Point.beta(this.x); Point.gamma(this.x); }
void main() {
  print(Point.beta(1).x);
  print(Child.gamma(2).v);
  print(uses());
}
"#,
    r#"import 'support.dart';
mixin Mark {}
class Base { final int v; Base.alpha(this.v); Base.beta(this.v); Base.gamma(this.v); }
class Child extends Base with Mark {
  Child.alpha(int v) : super.alpha(v);
  Child.beta(int v) : super.beta(v);
  Child.gamma(int v) : super.gamma(v);
}
class Point { final int x; Point.alpha(this.x); Point.beta(this.x); Point.gamma(this.x); }
typedef Signature = int Function({int alpha, int bravo});
abstract class Service { int invoke({int alpha, int bravo}); int get yankee; }
class Fake implements Service {
  int _yankee = 1;
  int _zulu = 2;
  @override
  dynamic noSuchMethod(Invocation i) => _yankee + _zulu;
}
({int alpha, int bravo}) pair() => (alpha: 1, bravo: 2);
void main() {
  Signature? g;
  print(g);
  print(pair());
  final Service s = Fake();
  print(s.invoke(bravo: 1));
  print(s.yankee);
  final f = Point.alpha;
  print(f(1).x);
  print(Point.beta(2).x);
  print(Child.gamma(3).v);
  print(uses());
}
"#,
];

/// All the files of an output directory (relative path → bytes).
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    collect(root, root, &mut out);
    out
}

#[test]
fn incremental_recompilation_equals_clean_compilation() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = simulated_sdk(tmp.path());
    let proj = tmp.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let support = proj.join("support.dart");
    let main = proj.join("main.dart");

    // A long session: three revisions, each new name interned when it arrives.
    let session_output = tmp.path().join("js_session");
    std::fs::write(&support, SUPPORT[0]).unwrap();
    std::fs::write(&main, MAIN[0]).unwrap();
    let mut session = Sessao::nova(&main, Some(&sdk), None, &session_output).expect("session");
    session.compilar().expect("first compilation");
    for i in 1..3 {
        std::fs::write(&support, SUPPORT[i]).unwrap();
        std::fs::write(&main, MAIN[i]).unwrap();
        session.arquivo_mudou(&support);
        session.arquivo_mudou(&main);
        let r = session.compilar().expect("recompilation");
        assert!(!r.first);
    }

    // A clean compilation of the same final source, with a new name arena.
    let clean_output = tmp.path().join("js_clean");
    let mut clean = Sessao::nova(&main, Some(&sdk), None, &clean_output).expect("clean session");
    clean.compilar().expect("clean compilation");

    let a = files(&session_output);
    let b = files(&clean_output);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>(), "different sets of files");
    for (path, clean_bytes) in &b {
        let incremental = &a[path];
        if incremental != clean_bytes {
            let (ti, tc) = (String::from_utf8_lossy(incremental), String::from_utf8_lossy(clean_bytes));
            let line = ti.lines().zip(tc.lines()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "{path}: the incremental recompilation diverges from the clean compilation at line {}:\n  incremental: {}\n  clean:       {}",
                line + 1,
                ti.lines().nth(line).unwrap_or("<end>"),
                tc.lines().nth(line).unwrap_or("<end>")
            );
        }
    }

    // The test only holds if the final text has the constructors in source order.
    let module = b.iter().find(|(c, _)| c.ends_with("main.js")).map(|(_, t)| String::from_utf8_lossy(t).into_owned()).expect("main module");
    let pos = |s: &str| module.find(s).unwrap_or_else(|| panic!("`{s}` missing from the module"));
    assert!(pos("_#alpha#tearOff") < pos("_#beta#tearOff") && pos("_#beta#tearOff") < pos("_#gamma#tearOff"), "tearoffs out of declaration order");
}
