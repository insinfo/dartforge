//! Records originais: ordem total, imutabilidade e igualdade conforme SDK 3.6.2.
use dartforge_diagnostics::Diagnostic;

const SOURCE: &str = r"
int effect(int value) { print(value); return value; }
T identity<T>(T value) { return value; }
bool accepts<T>(Object? value) { return value is T; }
(int, {String name}) make() { print('once'); return (7,name:'seven'); }
void main() {
  var record = (effect(1), z: effect(2), effect(3), a: effect(4));
  print(record);
  print(record.$2);
  print(record.z);
  final (n, name: label) = make();
  print(n);
  print(label);
  print((1, b: true, a: 'x') == (a: 'x', 1, b: true));
  print(((1,2),) == ((1,2),));
  print((<int>[1],) == (<int>[1],));
  var list = <int>[1];
  print((list,) == (list,));
  print(() == ());
  Object erased = (identity<Object>(1),);
  print(accepts<(int,)>(erased));
  print(accepts<(String,)>(erased));
  print((1,));
  print((z: 1, a: (2,)));
  print(<(int,)>[(3,)]);
}
";
const EXPECTED: &str = "1\n2\n3\n4\n(1, 3, a: 4, z: 2)\n3\n2\nonce\n7\nseven\ntrue\ntrue\nfalse\ntrue\ntrue\ntrue\nfalse\n(1)\n(a: (2), z: 1)\n[(3)]\n";

/// Conserva resolução de campos e tipos estruturais obtida pelo frontend real.
fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    Ok(dartforge_codegen::emit(&dartforge_hir::lower_resolved(
        program, resolution,
    )))
}

/// Destructuring deve introduzir um temporário e acessar o valor original somente uma vez.
#[test]
fn records_emit_shape_and_single_initializer() {
    let output = compile(SOURCE).unwrap();
    assert!(output.contains("const $dartforgeRecordTemp0 = $df_make();"));
    assert!(output.contains("$dartforgeEqual("));
    assert!(output.contains("$dartforgeRecord(["));
}

/// Confere stdout do Dart VM 3.6.2, inclusive named fields intercalados com posicionais.
#[test]
#[ignore = "requer Node.js no PATH"]
fn records_match_dart_3_6_2() {
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(SOURCE).unwrap()])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        EXPECTED
    );
}

/// Campos do record são congelados, mas listas contidas conservam mutabilidade e identidade.
#[test]
#[ignore = "requer Node.js no PATH"]
fn record_runtime_is_shallowly_immutable_and_preserves_real_types() {
    let javascript = format!(
        "{}\n{}\n{}\n{}",
        include_str!("../src/types.js"),
        include_str!("../src/records.js"),
        include_str!("../src/core.js"),
        r"
const list = new $dartforgeList([1],['int']);
const value = $dartforgeRecord([[null,list],['a',1]]);
console.log(Object.isFrozen(value));
try { value.$df_a = 2; } catch (_) {}
console.log(value.$df_a);
list.$df_add(2);
console.log(value.$df_$1.values.length);
console.log($dartforgeEqual(value,$dartforgeRecord([['a',1],[null,list]])));
console.log($dartforgeIs(value,['record',[['list',['object']]],[['a',['int']]]]));
console.log($dartforgeIs(value,['record',[['list',['string']]],[['a',['int']]]]));
"
    );
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "true\n1\n2\ntrue\ntrue\nfalse\n"
    );
}
