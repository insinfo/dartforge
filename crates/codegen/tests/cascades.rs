//! Cascades originais comparados ao Dart SDK 3.6.2, sem depender de valores retornados pelos métodos.
use dartforge_diagnostics::Diagnostic;

const SOURCE: &str = r"
int effect(int value) { print(value); return value; }
class Box {
  int value = 0;
  Box? child;
  Box other(int argument) { print(argument); return Box(); }
  void callbacks() {
    var callbacks = <int Function()>[]..add(() => value);
    print(callbacks[0]());
  }
}
extension BoxExtra on Box {
  void tick() { this.value = this.value + 1; }
}
Box make() { print('receiver'); return Box(); }
Box? absent() { print('absent'); return null; }
void main() {
  var box = make()..value = effect(1)..other(effect(2))..value = effect(3);
  print(box.value);
  var same = box..other(effect(4));
  print(same == box);
  box..child = (Box()..value = effect(5))..value = effect(6)..tick();
  print(box.value);
  print(box.child!.value);
  box..callbacks();
  var skipped = absent()?..value = effect(99)..other(effect(98));
  print(skipped == null);
  List<int>? missing = null;
  missing?..[effect(90)] = effect(91)..add(effect(92));
  var list = <int>[0]..[effect(0)] = effect(8)..add(effect(9));
  print(list);
}
";
const EXPECTED: &str =
    "receiver\n1\n2\n2\n3\n3\n4\n4\ntrue\n5\n6\n7\n5\n7\nabsent\ntrue\n0\n8\n9\n[8, 9]\n";

/// Conserva resolução de extensions e contexto do receiver sintético até a emissão.
fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    Ok(dartforge_codegen::emit(&dartforge_hir::lower_resolved(
        program, resolution,
    )))
}

/// Cada cascade ganha um parâmetro lexical; o null guard antecede suas seções.
#[test]
fn cascades_emit_once_and_preserve_nested_receivers() {
    let output = compile(SOURCE).unwrap();
    assert!(output.contains("$dartforgeCascade0"));
    assert!(output.contains(" === null) return $dartforgeCascade"));
    assert!(output.contains("$dartforgeExtension"));
}

/// Null evita argumentos/índices/RHS; chamadas e nested cascades preservam identidade e this lexical.
#[test]
#[ignore = "requer Node.js no PATH"]
fn cascades_match_dart_3_6_2() {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(SOURCE).unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        EXPECTED
    );
}

/// Closures criadas em seções conservam this lexical e leem seu estado após o cascade terminar.
#[test]
#[ignore = "requer Node.js no PATH"]
fn nested_cascade_callbacks_keep_lexical_this_after_return() {
    let source = r"
class Holder {
  int value = 1;
  Holder? child;
  List<int Function()> callbacks = <int Function()>[];
  void setup() {
    this..callbacks.add(() => value)
        ..child = (Holder()..callbacks.add(() => this.value))
        ..value = 7;
  }
}
void main() {
  var owner = Holder()..setup();
  owner.value = 11;
  owner.child!.value = 99;
  print(owner.callbacks[0]());
  print(owner.child!.callbacks[0]());
  print(owner.child!.value);
}
";
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(source).unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "11\n11\n99\n"
    );
}
