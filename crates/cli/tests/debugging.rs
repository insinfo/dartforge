//! J05: the native debugger stops at a `file.dart:line` of the executable
//! compiled with `--debug` and shows the stack with the Dart names and lines.
//! Runs with `gdb` (Linux); without it, the test warns and passes.
#![cfg(feature = "native")]

use std::process::Command;

const PROGRAM: &str = "int twice(int x) {
  final y = x * 2;
  return y;
}

class Account {
  int balance = 0;
  void deposit(int v) {
    balance += v;
    print('balance $balance');
  }
}

void main() {
  var total = 0;
  for (var i = 0; i < 3; i++) {
    total += twice(i);
  }
  final c = Account();
  c.deposit(total);
  print(total);
}
";

#[test]
#[ignore = "compiles with the source-built SDK and links with Clang; runs in the Linux CI (gdb)"]
fn gdb_stops_at_the_dart_line_and_shows_the_stack() {
    if Command::new("gdb").arg("--version").output().is_err() {
        eprintln!("no gdb: debugging test does not run");
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/tmp-depuracao-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("test directory");
    let source = dir.join("main.dart");
    std::fs::write(&source, PROGRAM).unwrap();
    let exe = dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    let compilation = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("compile-native")
        .arg(&source)
        .arg("-o")
        .arg(&exe)
        .arg("--debug")
        .output()
        .expect("CLI");
    assert!(compilation.status.success(), "{}", String::from_utf8_lossy(&compilation.stderr));

    // Two breakpoints: at the `return` of `twice` (without an instruction of
    // its own) and in the method body; the stack of each and the program output.
    let gdb = Command::new("gdb")
        .args(["-batch", "-nx"])
        .args(["-ex", "break main.dart:3", "-ex", "break main.dart:9", "-ex", "run", "-ex", "bt 2"])
        .args(["-ex", "delete 1", "-ex", "continue", "-ex", "bt 2", "-ex", "continue"])
        .arg(&exe)
        .output()
        .expect("gdb");
    let _ = std::fs::remove_dir_all(&dir);
    let output = String::from_utf8_lossy(&gdb.stdout).replace("\r\n", "\n");
    let frames: Vec<&str> = output.lines().filter(|l| l.starts_with('#')).collect();
    let frame = |l: &str| {
        // `#0  0x… in twice () at main.dart:3` or `#0  twice () at main.dart:3`.
        let after = l.split_once(" in ").map_or_else(|| l.split_once("  ").map_or(l, |(_, rest)| rest), |(_, r)| r);
        let name = after.split(" (").next().unwrap_or("");
        let location = after.rsplit(" at ").next().unwrap_or("");
        format!("{name} {location}")
    };
    let view: Vec<String> = frames.iter().map(|l| frame(l)).collect();
    assert_eq!(
        view,
        ["twice main.dart:3", "main main.dart:17", "Account.deposit main.dart:9", "main main.dart:20"],
        "gdb output:\n{output}\n{}",
        String::from_utf8_lossy(&gdb.stderr)
    );
    assert!(output.contains("balance 6\n6\n"), "the program output:\n{output}");
}
