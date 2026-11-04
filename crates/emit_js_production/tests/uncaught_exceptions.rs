//! The Node bootstrap keeps its host properties when Dart members use the
//! same names, and reports uncaught synchronous and asynchronous errors once.

use std::process::Command;

#[test]
#[ignore = "requires the Dart SDK sources and Node on PATH"]
fn uncaught_exceptions_keep_the_host_protocol() {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-uncaught-exceptions-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (name, declaration, wait) in [
        ("sync", "void main()", ""),
        (
            "async",
            "Future<void> main() async",
            "await Future<void>.value();",
        ),
    ] {
        let source = directory.join(format!("{name}.dart"));
        std::fs::write(
            &source,
            format!(
                "class HostNames {{
  int exit() => 1;
  int on() => 2;
  int stderr() => 3;
  int write() => 4;
}}
{declaration} {{
  final names = HostNames();
  print([names.exit(), names.on(), names.stderr(), names.write()]);
  {wait}
  throw 'production failure';
}}
"
            ),
        )
        .unwrap();
        for level in ["-O1", "-O2", "-O3", "-O4"] {
            let artifact = directory.join(format!("{name}{level}.js"));
            let compile = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
                .arg(&source)
                .arg("-o")
                .arg(&artifact)
                .arg(level)
                .output()
                .unwrap();
            assert!(
                compile.status.success(),
                "{name} {level}: {}",
                String::from_utf8_lossy(&compile.stderr)
            );
            let run = Command::new("node").arg(&artifact).output().unwrap();
            let stderr = String::from_utf8_lossy(&run.stderr).replace("\r\n", "\n");
            assert_eq!(run.status.code(), Some(255), "{name} {level}: {stderr}");
            assert_eq!(
                String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"),
                "[1, 2, 3, 4]\n",
                "{name} {level}"
            );
            assert_eq!(
                stderr, "Unhandled exception:\nError: production failure\n",
                "{name} {level}"
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
