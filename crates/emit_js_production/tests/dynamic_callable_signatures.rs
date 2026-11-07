//! Callable objects need signatures even when dispatch reaches them through a getter.
use std::process::Command;

const PROGRAM: &str = r#"
class Callable {
  int call(int value) => value * 2;
}
class Child extends Callable {}
class Generic {
  T call<T>(T value) => value;
}
class Holder {
  final Callable value;
  Holder(this.value);
}
int throughGetter(dynamic holder) => holder.value(2);
void main() {
  if (throughGetter(Holder(Callable())) != 4) throw 'field call';
  if (throughGetter(Holder(Child())) != 4) throw 'inherited call';
  dynamic callable = Callable();
  if (callable(3) != 6) throw 'dynamic object call';
  dynamic generic = Generic();
  if (generic<int>(7) != 7) throw 'generic object call';
  var badType = false, badArity = false;
  try { callable('wrong'); } on TypeError { badType = true; }
  try { callable(); } on NoSuchMethodError { badArity = true; }
  if (!badType || !badArity) throw 'call checks were removed';
  print('callable signatures ok');
}
"#;

#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn dynamic_getter_and_object_calls_keep_signatures_and_checks() {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-callable-signatures-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("main.dart");
    std::fs::write(&source, PROGRAM).unwrap();
    let vm = Command::new("dart").arg(&source).output().unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&vm.stdout).replace("\r\n", "\n"),
        "callable signatures ok\n"
    );
    let compiler = std::env::var_os("DARTFORGE_JSPROD")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge-jsprod").into());
    for level in ["-O1", "-O2"] {
        let artifact = directory.join("main.js");
        let mut command = Command::new(&compiler);
        command.arg(&source).arg("-o").arg(&artifact).arg(level);
        if let Some(sdk) = std::env::var_os("DARTFORGE_SDK_LIB") {
            command.arg("--sdk").arg(sdk);
        }
        let compilation = command.output().unwrap();
        assert!(
            compilation.status.success(),
            "{level}: {}",
            String::from_utf8_lossy(&compilation.stderr)
        );
        let output = Command::new("node")
            .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{level}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
            "callable signatures ok\n"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
