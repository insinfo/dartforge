//! Regressões originais para descritores genéricos, identidade e escritas covariantes.
use dartforge_diagnostics::Diagnostic;

/// Analisa e emite o mesmo módulo sem substituir a resolução por tipos apagados.
fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let program = dartforge_parser::parse(&tokens, source.len())?;
    let resolution = dartforge_semantic::analyze(&program)?;
    let module = dartforge_hir::lower_resolved(program, resolution);
    dartforge_codegen::validate_javascript(&module)?;
    Ok(dartforge_codegen::emit(&module))
}

const SOURCE: &str = r"
bool test<T>(Object? value) { return value is T; }
bool forward<T>(Object? value) { return test<T>(value); }
T cast<T>(Object? value) { return value as T; }
T identity<T>(T value) { return value; }
class Service { int answer() { return 42; } }
class Child extends Service {}
int answer<T extends Service>(T service) { return service.answer(); }
int fn(Object value) { return 7; }
void main() {
  print(test<int>(1));
  print(test<String>(1));
  print(forward<int?>(null));
  print(forward<int>(null));
  print(cast<String>('ok'));
  print(identity(4));
  print(answer(Child()));
  print(test<Service>(Child()));
  print(test<Child>(Service()));
  Object value = <int>[1];
  print(value is List<int>);
  print(value is List<String>);
  print(value is Iterable<Object>);
  var mapped = <int>[1,2].map((int n) => 'x');
  print(mapped is Iterable<String>);
  print(mapped.toList() is List<String>);
  Object function = fn;
  print(function is int Function(String));
  print(function is String Function(Object));
  print(test<int Function(String)>(fn));
  var closure = (String s) => s;
  print(test<String Function(String)>(closure));
  print(cast<int Function(String)>(fn) == fn);
}
";
const EXPECTED: &str = "true\nfalse\ntrue\nfalse\nok\n4\n42\ntrue\nfalse\ntrue\nfalse\ntrue\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\ntrue\n";

/// Chamadas inferidas e explícitas devem receber o ambiente reificado.
#[test]
fn generic_calls_emit_runtime_descriptors() {
    let output = compile(SOURCE).unwrap();
    assert!(output.contains("$dartforgeTypes"));
    assert!(output.contains("$dartforgeIs("));
    assert!(output.contains("$dartforgeCast("));
}

/// Confere testes de tipos, casts, bounds nominais e assinaturas contra stdout SDK 3.6.2.
#[test]
#[ignore = "requer Node.js no PATH"]
fn reified_generics_match_dart_3_6_2() {
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

/// A covariância não altera o tipo real de uma lista compartilhada; falha antes da mutação.
#[test]
#[ignore = "requer Node.js no PATH"]
fn covariant_list_writes_and_failed_casts_throw() {
    for tail in [
        "values.add('bad');",
        "values[0] = 'bad';",
        "Object x = 1; print(x as String);",
    ] {
        let source = format!(
            "void main(){{List<Object> values=<int>[1];print('before');{tail}print('after');}}"
        );
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &compile(&source).unwrap()])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "before\n"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("TypeError"));
    }
}

/// Substituição de T normaliza Null?; falhas de tipo precedem limites e não alteram a lista.
#[test]
#[ignore = "requer Node.js no PATH"]
fn runtime_nullable_substitution_and_write_error_order() {
    let javascript = format!(
        "{}\n{}\n{}",
        include_str!("../src/types.js"),
        include_str!("../src/core.js"),
        r"
const list = new $dartforgeList([], ['nullable',['null']]);
console.log($dartforgeIs(list,['list',['null']]));
console.log($dartforgeSubtype(['nullable',['nullable',['int']]],['nullable',['object']]));
const integers = new $dartforgeList([1], ['int']);
try { $dartforgeIndexSet(integers, 10, 'bad'); }
catch(error) { console.log(error instanceof TypeError); }
console.log(integers.values.length);
console.log(integers.values[0]);
const fn = $dartforgeTyped(x => 1,['function',['int'],[['object']]]);
console.log($dartforgeCast(fn,['function',['void'],[['string']]]) === fn);
const oldShape = new $dartforgeList([1]);
oldShape.$df_add('valid Object?');
console.log(oldShape.values.length);
"
    );
    let output = std::process::Command::new("node")
        .args(["--eval", &javascript])
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
        "true\ntrue\ntrue\n1\n1\ntrue\n2\n"
    );
}
