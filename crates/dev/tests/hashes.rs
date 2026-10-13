//! The two hashes per library separate the body from the public API, and the
//! session reuses what did not change.

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

#[test]
fn private_body_does_not_change_the_api_and_signature_does() {
    let tmp = tempfile::tempdir().unwrap();
    let sdk = simulated_sdk(tmp.path());
    let proj = tmp.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let support = proj.join("support.dart");
    let main = proj.join("main.dart");
    std::fs::write(&support, "int twice(int x) { var s = 1; return x + s; }\n").unwrap();
    std::fs::write(&main, "import 'support.dart';\nvoid main() { print(twice(2)); }\n").unwrap();
    let output = tmp.path().join("js");

    let mut session = Session::new(&main, Some(&sdk), None, &output).expect("session");
    let r = session.compile().expect("first");
    assert!(r.first);
    assert!(r.modules >= 2, "one library per file: {}", r.modules);
    let units = session.cached_units();
    assert!(units >= 3, "input, support and the SDK core: {units}");

    // 1. Body edit: the value changes, the signature does not.
    std::fs::write(&support, "int twice(int x) { var s = 2; return x + s; }\n").unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("body edit");
    assert_eq!(r.body_changed.len(), 1, "only `support.dart` changed: {:?}", r.body_changed);
    assert!(r.api_changed.is_empty(), "a body is not API: {:?}", r.api_changed);
    assert_eq!(r.invalidated_dependents, 0);
    assert_eq!(r.reanalyzed_units, 1, "only the edited file is reanalyzed");
    assert_eq!(r.written_modules, 1, "only the `support` module changes text");

    // 2. API edit: a new public function.
    std::fs::write(&support, "int twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\n").unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("API edit");
    assert_eq!(r.api_changed.len(), 1, "the API of `support` changed: {:?}", r.api_changed);
    assert_eq!(r.invalidated_dependents, 1, "`main.dart` imports `support.dart`");

    // 3. A new private function: changes the library's body, not the API.
    std::fs::write(
        &support,
        "int twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\nint _hidden(int x) => x;\n",
    )
    .unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("private edit");
    assert_eq!(r.body_changed.len(), 1);
    assert!(r.api_changed.is_empty(), "a private declaration is not API: {:?}", r.api_changed);

    // 3b. A new statement inside a class method: it is the most common edit
    // (a keystroke in a body) and must not touch the API — otherwise every
    // keystroke would invalidate all the dependents.
    std::fs::write(
        &support,
        "class Account {\n  int balance = 0;\n  int add(int x) { balance = balance + x; return balance; }\n}\nint twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\nint _hidden(int x) => x;\n",
    )
    .unwrap();
    session.file_changed(&support);
    session.compile().expect("new class");
    std::fs::write(
        &support,
        "class Account {\n  int balance = 0;\n  int add(int x) { print(x); balance = balance + x; return balance; }\n}\nint twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\nint _hidden(int x) => x;\n",
    )
    .unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("new statement in the method");
    assert_eq!(r.body_changed.len(), 1, "the body changed: {:?}", r.body_changed);
    assert!(
        r.api_changed.is_empty(),
        "inserting a statement in a method is not an API change: {:?}",
        r.api_changed
    );
    assert_eq!(r.invalidated_dependents, 0);

    // 3c. A new parameter on a public method: now the API did change, and
    // the dependent counts.
    std::fs::write(
        &support,
        "class Account {\n  int balance = 0;\n  int add(int x, int y) { print(x); balance = balance + x; return balance; }\n}\nint twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\nint _hidden(int x) => x;\n",
    )
    .unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("new parameter");
    assert_eq!(r.api_changed.len(), 1, "a new parameter is API: {:?}", r.api_changed);
    assert_eq!(r.invalidated_dependents, 1, "`main.dart` depends on `support.dart`");

    // 3d. Renaming a public method changes the API.
    std::fs::write(
        &support,
        "class Account {\n  int balance = 0;\n  int deposit(int x, int y) { print(x); balance = balance + x; return balance; }\n}\nint twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\nint _hidden(int x) => x;\n",
    )
    .unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("rename method");
    assert_eq!(r.api_changed.len(), 1, "renaming is API: {:?}", r.api_changed);

    // 3e. Comments and whitespace are neither API nor emitted body.
    std::fs::write(
        &support,
        "// a new comment\nclass Account {\n  int balance = 0;\n\n  int deposit(int x, int y) { print(x); balance = balance + x; return balance; }\n}\nint twice(int x) { var s = 2; return x + s; }\nint thrice(int x) => x;\nint _hidden(int x) => x;\n",
    )
    .unwrap();
    session.file_changed(&support);
    let r = session.compile().expect("comment");
    assert!(r.api_changed.is_empty(), "a comment is not API: {:?}", r.api_changed);
    assert_eq!(r.written_modules, 0, "the JS did not change, nothing to rewrite");

    // 4. Nothing changes: no library reports a change and nothing is rewritten.
    let r = session.compile().expect("no edit");
    assert!(r.body_changed.is_empty(), "{:?}", r.body_changed);
    assert!(r.api_changed.is_empty());
    assert_eq!(r.reanalyzed_units, 0, "nothing to reanalyze");
    assert_eq!(r.written_modules, 0, "no module changed text");
}
