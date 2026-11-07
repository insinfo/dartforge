//! Production entry points must use the host event loop when one is supplied.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "dartforge-host-event-loop-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not reserve {}: {error}", path.display()),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("host event-loop artifacts retained at {}", self.0.display());
        } else {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn successful(directory: &Path, phase: &str, output: Output) -> Vec<u8> {
    std::fs::write(directory.join(format!("{phase}.stdout")), &output.stdout).unwrap();
    std::fs::write(directory.join(format!("{phase}.stderr")), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{phase}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if phase != "official-compile" {
        assert!(output.stderr.is_empty(), "{phase}: {:?}", output.stderr);
    }
    output.stdout
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node"]
fn host_and_default_event_loops_complete_async_tests_after_minification() {
    let scratch = Scratch::new();
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("Dart 3.6.2 oracle SDK"));
    let preambles = sdk.join("lib/_internal/js_runtime/lib/preambles");
    let source = scratch.0.join("main.dart");
    std::fs::write(
        &source,
        r#"import 'dart:async';
class HostName {
  String dartMainRunner() => 'member';
}
void main() {
  print(HostName().dartMainRunner());
  print('unittest-suite-wait-for-done');
  final events = <String>[];
  scheduleMicrotask(() { events.add('micro'); });
  final cancelled = Timer(Duration.zero, () { events.add('cancelled'); });
  cancelled.cancel();
  Timer(Duration.zero, () { events.add('zero'); });
  Timer(const Duration(milliseconds: 2), () {
    print(events.join(','));
    print('unittest-suite-success');
  });
}
"#,
    )
    .unwrap();
    let host = scratch.0.join("host.cjs");
    let preambles = serde_json::to_string(&[
        preambles.join("seal_native_object.js"),
        preambles.join("d8.js"),
    ])
    .unwrap();
    std::fs::write(
        &host,
        format!(
            "const fs = require('node:fs');\nconst vm = require('node:vm');\n\
             const context = vm.createContext({{ console, print: console.log, performance: {{}}, arguments: [] }});\n\
             context.load = (filename) => vm.runInContext(fs.readFileSync(filename, 'utf8'), context, {{ filename }});\n\
             for (const preamble of {preambles}) context.load(preamble);\ncontext.load(process.argv[2]);\n"
        ),
    )
    .unwrap();
    let official = scratch.0.join("official.js");
    let dart = sdk.join(if cfg!(windows) {
        "bin/dart.exe"
    } else {
        "bin/dart"
    });
    successful(
        &scratch.0,
        "official-compile",
        Command::new(dart)
            .args(["compile", "js", "-O2"])
            .arg(&source)
            .arg("-o")
            .arg(&official)
            .output()
            .unwrap(),
    );
    let expected = successful(
        &scratch.0,
        "official-host",
        Command::new("node")
            .arg(&host)
            .arg(&official)
            .output()
            .unwrap(),
    );
    assert_eq!(
        String::from_utf8_lossy(&expected).replace("\r\n", "\n"),
        "member\nunittest-suite-wait-for-done\nmicro,zero\nunittest-suite-success\n"
    );
    for minify in [false, true] {
        let artifact = scratch.0.join(format!("candidate-{minify}.js"));
        let compiled = dartforge_emit_js_production::compile_own_sdk(
            &source,
            Some(&sdk.join("lib")),
            None,
            dartforge_emit_js_production::Options {
                minify,
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();
        std::fs::write(&artifact, compiled.js).unwrap();
        for supplied_host in [false, true] {
            let mut command = Command::new("node");
            if supplied_host {
                command.arg(&host);
            }
            assert_eq!(
                successful(
                    &scratch.0,
                    &format!("candidate-{minify}-host-{supplied_host}"),
                    command.arg(&artifact).output().unwrap(),
                ),
                expected
            );
        }
    }
}
