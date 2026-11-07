//! Symbolic traces must cross generated frames with either exception protocol.
#![cfg(all(
    feature = "native",
    any(
        all(windows, target_arch = "x86_64"),
        all(unix, any(target_arch = "x86_64", target_arch = "aarch64"))
    )
))]

use std::process::Command;

const PROGRAM: &str = r#"
import 'forwarder.dart';
import 'interface.dart';
part 'trace_part.dart';
@pragma('vm:never-inline')
String currentTrace() => StackTrace.current.toString();
@pragma('vm:never-inline')
void leaf() { throw StateError('trace-probe'); }
@pragma('vm:never-inline')
void caller() { leaf(); }
@pragma('vm:never-inline')
void scalarLeaf() { try { throw 42; } finally {} }
@pragma('vm:never-inline')
void scalarCaller() {
  try { scalarLeaf(); } finally { try { throw 'inner'; } catch (_) {} }
}
@pragma('vm:never-inline')
void defaultMissingLeaf() { dynamic object = PlainReceiver(); object.absent(); }
@pragma('vm:never-inline')
void defaultMissingCaller() { defaultMissingLeaf(); }
void checkRuntimeMissingFrames(String leaf, String caller, void Function() invoke) {
  try {
    invoke();
    throw 'expected default noSuchMethod error';
  } on NoSuchMethodError catch (error, trace) {
    final text = trace.toString();
    final frames = RegExp(r'#\d+\s+(\S+) \(file:').allMatches(text)
        .map((match) => match.group(1))
        .where((name) => name == leaf || name == caller || name == 'main').join(',');
    if (frames != '$leaf,$caller,main') throw 'runtime reentry lost caller frames: $text';
    if (error.stackTrace.toString() != text) throw 'NoSuchMethodError lost its original trace';
  }
}
class TraceConstructor {
  TraceConstructor() { throw 1; }
  TraceConstructor.named() { throw 2; }
  factory TraceConstructor.factoryBody() { throw 3; }
}
void checkConstructor(String expected, void Function() create) {
  try { create(); throw 'constructor did not throw'; } catch (error, trace) {
    if (!trace.toString().contains('$expected (file:')) throw 'constructor name: $trace';
  }
}
void main() { runTraceChecks(); }
@pragma('vm:never-inline')
void runTraceChecks() {
  final current = currentTrace();
  if (!current.contains('currentTrace (file:')) throw 'missing current frame';
  try {
    caller();
    throw 'expected StateError';
  } on StateError catch (error, trace) {
    final text = trace.toString();
    final frames = RegExp(r'#\d+\s+(leaf|caller|main) \(file:.*main\.dart:\d+:\d+\)')
        .allMatches(text).map((match) => match.group(1)).join(',');
    if (frames != 'leaf,caller,main') throw 'missing or unordered frames: $text';
    if (error.stackTrace.toString() != text) throw 'Error lost its original trace';
  }
  try { scalarCaller(); throw 'expected scalar exception'; } catch (error, trace) {
    final text = trace.toString();
    if (error != 42 || !text.contains('scalarLeaf (file:') ||
        !text.contains('scalarCaller (file:')) throw 'finally lost the trace: $text';
  }
  checkConstructor('new TraceConstructor', () { TraceConstructor(); });
  checkConstructor('new TraceConstructor.named', () { TraceConstructor.named(); });
  checkConstructor('new TraceConstructor.factoryBody', () { TraceConstructor.factoryBody(); });
  try {
    invokePrivate(Forwarder());
    throw 'expected private member error';
  } on NoSuchMethodError catch (error, trace) {
    final text = trace.toString();
    if (!text.contains('Forwarder._hidden (file:') ||
        !text.contains('/forwarder.dart:')) throw 'missing forwarder source: $text';
  }
  checkRuntimeMissingFrames('defaultMissingLeaf', 'defaultMissingCaller', defaultMissingCaller);
  checkRuntimeMissingFrames('partMissingLeaf', 'partMissingCaller', partMissingCaller);
  print('symbolic trace ok');
}
"#;

#[test]
#[ignore = "requires the source SDK, LLVM and a current preserved native compiler"]
fn symbolic_traces_cross_checked_and_table_exception_frames() {
    for parameters in ["", "List<String> arguments"] {
        check_symbolic_traces(parameters);
    }
}

fn check_symbolic_traces(parameters: &str) {
    let directory =
        std::env::temp_dir().join(format!("dartforge-symbolic-trace-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("main.dart");
    std::fs::write(
        &source,
        PROGRAM.replacen("void main()", &format!("void main({parameters})"), 1),
    )
    .unwrap();
    std::fs::write(
        directory.join("forwarder.dart"),
        r#"
import 'interface.dart';
class Forwarder extends Fallback implements Interface {}
class PlainReceiver { final _myPrecious = 'secret'; }
"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("interface.dart"),
        r#"
class Fallback {
  dynamic noSuchMethod(Invocation invocation) => null;
}
class Interface {
  void _hidden() {}
}
@pragma('vm:never-inline')
void invokePrivate(Interface target) { target._hidden(); }
"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("trace_part.dart"),
        r#"
part of 'main.dart';
@pragma('vm:never-inline')
void partMissingLeaf() { dynamic object = PlainReceiver(); print(object._myPrecious); }
@pragma('vm:never-inline')
void partMissingCaller() { partMissingLeaf(); }
"#,
    )
    .unwrap();
    let vm = Command::new("dart").arg(&source).output().expect("Dart VM");
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&vm.stdout).replace("\r\n", "\n"),
        "symbolic trace ok\n"
    );
    let compiler = std::env::var_os("DARTFORGE_COMPILER")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge").into());
    for optimize in [false, true] {
        for exceptions in ["check", "tables"] {
            let executable = directory.join(if cfg!(windows) { "main.exe" } else { "main" });
            let mut command = Command::new(&compiler);
            command.arg("aot").arg(&source).arg(&executable).args([
                "--stack-trace",
                "symbolic",
                "--exceptions",
                exceptions,
            ]);
            if optimize {
                command.arg("--optimize");
            }
            let compilation = command.output().expect("native compiler");
            assert!(
                compilation.status.success(),
                "optimize={optimize}, exceptions={exceptions}: {}",
                String::from_utf8_lossy(&compilation.stderr)
            );
            let output = Command::new(&executable).output().expect("native program");
            assert!(
                output.status.success(),
                "optimize={optimize}, exceptions={exceptions}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
                "symbolic trace ok\n"
            );
            if optimize && exceptions == "check" {
                let stressed = Command::new(&executable)
                    .env("DARTFORGE_GC_STRESS", "1")
                    .output()
                    .expect("native program under collection pressure");
                assert!(
                    stressed.status.success(),
                    "saved traces must survive collections in finally: {}",
                    String::from_utf8_lossy(&stressed.stderr)
                );
                assert_eq!(
                    String::from_utf8_lossy(&stressed.stdout).replace("\r\n", "\n"),
                    "symbolic trace ok\n"
                );
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
