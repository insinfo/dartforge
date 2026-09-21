//! Infraestrutura real para código expandido de macros: Maps tipados e fábricas nomeadas.
use dartforge_diagnostics::Diagnostic;

const SOURCE: &str = r"
class User {
  final String name;
  final int age;
  final bool? active;
  User(this.name, this.age, this.active);
  factory User.fromJson(Map<String, Object?> json) {
    return User(json['name']! as String, json['age'] as int, json['active'] as bool?);
  }
  Map<String,Object?> toJson() {
    return <String,Object?>{'name':name, 'age':age, 'active':active};
  }
}
String key(String name) { print(name); return name; }
int value(int number) { print(number); return number; }
void main() {
  var user = User.fromJson(<String,Object?>{'name':'á🦀', 'age':28});
  print(user.name);
  print(user.age);
  print(user.active);
  var json = user.toJson();
  print(json['name'] as String);
  print(json.length);
  var map = <String,int>{key('b'):value(1),key('__proto__'):value(2),key('b'):value(3)};
  print(map.length);
  print(map['b']);
  print(map['missing']);
  print(map['__proto__']);
  map['c'] = 4;
  print(map);
  Object erased = map;
  print(erased is Map<String,Object?>);
  print(erased is Map<String,String>);
}
";
const EXPECTED: &str = "á🦀\n28\nnull\ná🦀\n3\nb\n1\n__proto__\n2\nb\n3\n2\n3\nnull\n2\n{b: 3, __proto__: 2, c: 4}\ntrue\nfalse\n";

/// Compila factories comuns; nenhuma inspeção de anotação acontece no runtime JS.
fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    Ok(dartforge_codegen::emit(&dartforge_hir::lower_resolved(
        program, resolution,
    )))
}

/// A fábrica é uma função distinta do construtor generativo e preserva a asserção de null.
#[test]
fn map_and_named_factory_emit_real_operations() {
    let output = compile(SOURCE).unwrap();
    assert!(output.contains("function $dartforgeFactory0$df_fromJson("));
    assert!(output.contains("new $dartforgeMap("));
    assert!(output.contains("function $dartforgeNullAssert("));
}

/// Confere Unicode, chaves especiais, duplicatas, ausência e descritores com Dart VM 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn map_and_factory_match_dart_3_6_2() {
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

/// Uma visão covariante não permite trocar o tipo real dos valores armazenados.
#[test]
#[ignore = "requer Node.js no PATH"]
fn covariant_map_rejects_incompatible_write() {
    let source = "void main(){Map<String,Object?> m=<String,int>{'a':1};print('before');m['a']='wrong';print('after');}";
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &compile(source).unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TypeError"));
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "before\n"
    );
}
