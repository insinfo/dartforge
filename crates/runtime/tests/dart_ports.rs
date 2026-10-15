//! The CI `grep` of contract C9 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
//! §4.6 and §13.15): with table-based exceptions, a Dart exception crossing a
//! Rust frame would be undefined behavior, and every point where the runtime
//! calls Dart code goes through the ports (`exceptions_tables.rs`,
//! `dart_r<n>`/`dart_v<n>`). The sign of a new point that calls directly is a
//! `transmute` to a function pointer outside that file.
//!
//! The remaining ones call C code (native finalizers and handlers of the
//! `dart:ffi` port API, system functions found at run time) or only convert the
//! address that goes on through the port; each one is in the list below with
//! the reason. A new `transmute` outside it fails the test: either it goes
//! through the port, or it enters the list with the reason.
//! Written without compiling or running (2026-10-05).

use std::path::Path;

/// (file, chunk of the line, reason).
const ALLOWED: &[(&str, &str, &str)] = &[
    ("arc_native_ports.rs", "let test_body: extern \"C\" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(target as usize) };", "cfg(test) only: the target is checked equal to the static extern C Rust body, which only retains the reference and neither throws nor calls Dart"),
    ("ffi_native_api.rs", "let f: HandleFinalizer = unsafe { std::mem::transmute(f) };", "the C `Dart_HandleFinalizer` of the native API caller"),
    ("ffi_native_api.rs", "let handler: NativePortHandler = unsafe { std::mem::transmute(handler) };", "the C `Dart_NativeMessageHandler`"),
    ("heap.rs", "let f: extern \"C\" fn(usize) = unsafe { std::mem::transmute(f) };", "the C `NativeFinalizerFunction` of `dart:ffi`"),
    ("io_watcher.rs", "std::mem::transmute::<*mut c_void, _>(s(c\"FSEvent", "CoreServices functions found at run time"),
    ("io_windows_events.rs", "std::mem::transmute::<usize, Fn", "the Winsock extensions (`AcceptEx`, `ConnectEx`…) found through `WSAIoctl`"),
    ("ports.rs", "std::mem::transmute::<usize, extern \"C\" fn()>(p)", "the trampolines of the generation published by the JIT, which has no table-based exceptions"),
];

#[test]
fn every_fn_transmute_outside_ports_is_listed() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut new_ones = Vec::new();
    for input in std::fs::read_dir(&source).expect("read crates/runtime/src") {
        let file_path = input.expect("entry").path();
        let Some(name) = file_path.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.ends_with(".rs") || name == "exceptions_tables.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&file_path).expect("read source");
        for (n, line) in text.lines().enumerate() {
            if !line.contains("transmute") || line.trim_start().starts_with("//") {
                continue;
            }
            let allowed = ALLOWED.iter().any(|(file, chunk, _)| *file == name && line.contains(chunk));
            if !allowed {
                new_ones.push(format!("{name}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        new_ones.is_empty(),
        "`transmute` outside the ports and outside the list (call the Dart code through `dart_r<n>`/`dart_v<n>`, or add the case with its reason to ALLOWED):\n{}",
        new_ones.join("\n")
    );
}

/// The list keeps no dead entry: each one still matches some line.
#[test]
fn the_list_has_no_dead_entry() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (file, chunk, reason) in ALLOWED {
        let text = std::fs::read_to_string(source.join(file)).unwrap_or_default();
        assert!(text.contains(chunk), "{file}: the entry ({reason}) no longer matches anything: remove it from the list");
    }
}
