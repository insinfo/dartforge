//! Generated async frames retain the resume ABI of their suspended generation.
use dartforge_emit_native::{CompileOptions, contracts::ContractMode};
use dartforge_jit::JitSession;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires LLVM and the matching source-built Dart SDK"]
fn suspended_frame_resumes_old_body_after_frame_size_changes() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../target/jit-async-frame-reload-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            let directory = directory.canonicalize().unwrap();
            let signal = directory.join("suspended");
            let output = directory.join("result");
            let release = directory.join("release");
            for path in [&signal, &output, &release] {
                let _ = std::fs::remove_file(path);
            }
            let source = directory.join("main.dart");
            let sdk = std::env::var_os("DARTFORGE_TEST_SDK_LIB").map(PathBuf::from);
            let options = CompileOptions {
                contracts: ContractMode::Verify,
                sdk: sdk.as_deref(),
                packages: None,
                timings: false,
                optimize: false,
                language_version: None,
                experiments: Vec::new(),
                debug: false,
                generator: None,
                cpu: None,
            };
            let program = |new: bool| {
                format!(
                    r#"
import 'dart:async';
import 'dart:io';
Future<void> worker() async {{
  final value = 37;
  {extra}
  File(r'{signal}').writeAsStringSync('suspended');
  while (!File(r'{release}').existsSync()) {{
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }}
  File(r'{output}').writeAsStringSync('{version}:$value{suffix}\n', mode: FileMode.append);
}}
Future<void> main() async {{
  await worker();
  await worker();
}}
"#,
                    signal = signal.display(),
                    output = output.display(),
                    release = release.display(),
                    extra = if new {
                        "final extra = DateTime.now().millisecondsSinceEpoch;"
                    } else {
                        ""
                    },
                    version = if new { "new" } else { "old" },
                    suffix = if new { ":${extra > 0}" } else { "" }
                )
            };
            std::fs::write(&source, program(false)).unwrap();
            let first = dartforge_emit_native::emit_reloadable_ir(&source, &options, None).unwrap();
            std::fs::write(directory.join("before.ll"), &first.text).unwrap();
            std::fs::write(&source, program(true)).unwrap();
            let second =
                dartforge_emit_native::emit_reloadable_ir(&source, &options, Some(&first.text))
                    .unwrap();
            std::fs::write(directory.join("after.ll"), &second.text).unwrap();
            let markers = |ir: &str| {
                ir.lines()
                    .filter(|line| line.starts_with("; df.async-frame "))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            assert!(!markers(&first.text).is_empty());
            assert_ne!(markers(&first.text), markers(&second.text));
            // The second source genuinely needs another value across the suspension.
            let sizes = |ir: &str| {
                ir.lines()
                    .filter(|line| line.starts_with("; df.async-frame "))
                    .map(|line| line.rsplit_once(' ').unwrap().1.parse::<usize>().unwrap())
                    .collect::<Vec<_>>()
            };
            assert_ne!(sizes(&first.text), sizes(&second.text));
            let dll = dartforge_emit_native::sdk_module::source_sdk_dll().unwrap();
            let mut session = JitSession::new_for_ir_with(&first.text, Some(&dll)).unwrap();
            session.add_reloadable_module("app", &first.text).unwrap();
            struct ReleaseOnDrop<'a>(&'a std::path::Path);
            impl Drop for ReleaseOnDrop<'_> {
                fn drop(&mut self) {
                    let _ = std::fs::write(self.0, "resume");
                }
            }
            session
                .with_live_program(|live| {
                    let _release_on_unwind = ReleaseOnDrop(&release);
                    live.run_main().unwrap();
                    let deadline = Instant::now() + Duration::from_secs(15);
                    while !signal.exists() {
                        assert!(
                            Instant::now() < deadline,
                            "worker did not reach its suspension"
                        );
                        assert!(live.finished().is_none(), "program ended before suspension");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    let publication = live.publish("app", &second.text);
                    // Always release the running program before surfacing a refusal;
                    // otherwise scoped-thread cleanup would wait on the fixture forever.
                    std::fs::write(&release, "resume").unwrap();
                    publication.expect("compatible per-body frame ABI");
                    assert_eq!(live.wait_end().unwrap().exit_code, 0);
                })
                .unwrap();
            assert_eq!(
                std::fs::read_to_string(&output).unwrap(),
                "old:37\nnew:37:true\n"
            );
            drop(session);
        })
        .unwrap()
        .join()
        .unwrap();
}
