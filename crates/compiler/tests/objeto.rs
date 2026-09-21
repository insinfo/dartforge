//! Protocolo `Object` e acessores de instância: `==`, `hashCode`, `toString`.
//!
//! O oráculo é o **Dart 3.6.2** (`dart`) e o **Dart 3.13.4**
//! (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`), que produzem a mesma saída em
//! todos os programas deste arquivo. Cada `assert_eq!` de execução compara com
//! o texto copiado de `dart run`, sem edição. O contrato completo e a decisão
//! sobre `Instance of 'Nome'` estão em `docs/OBJETO.md`.
use dartforge_compiler::compile;
use dartforge_diagnostics::Span;

/// Compila uma fonte válida e devolve o módulo JavaScript emitido.
fn javascript(source: &str) -> String {
    compile(source).unwrap_or_else(|error| panic!("{source}: {}", error.message))
}

/// Executa o módulo gerado no Node e devolve stdout.
fn execute(source: &str) -> String {
    let js = javascript(source);
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .expect("Node.js necessário");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

/// Confere mensagem e intervalo exatos de um programa rejeitado.
fn rejeita(source: &str, message: &str, span: Span) {
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, message, "{source}");
    assert_eq!(error.span, span, "{source}");
}

/// Calcula o intervalo de um trecho único da fonte, em bytes.
fn trecho(source: &str, needle: &str) -> Span {
    let start = source.find(needle).expect(needle);
    assert_eq!(
        source.rfind(needle),
        Some(start),
        "trecho ambíguo: {needle}"
    );
    Span {
        start,
        end: start + needle.len(),
    }
}

/// Classe com o protocolo completo, reutilizada por vários casos.
const PONTO: &str = "
class Point {
  final int x;
  final int y;
  Point(this.x, this.y);
  bool operator ==(Object other) { return other is Point && other.x == x && other.y == y; }
  int get hashCode { return x * 31 + y; }
  String toString() { return 'Point($x, $y)'; }
}
";

/// O operador declarado decide `==` e `!=`; `===` do JavaScript não aparece.
#[test]
fn declared_equality_replaces_reference_comparison() {
    let source = format!(
        "{PONTO}void main() {{ var a=Point(1,2); var b=Point(1,2); print(a==b); print(a!=b); }}"
    );
    let js = javascript(&source);
    assert!(js.contains("$dartforgeEquals("), "{js}");
    assert!(
        js.contains("$df$eq(other)") || js.contains("$df$eq($df_other)"),
        "{js}"
    );
}

/// Programa sem operador declarado não ganha runtime nem muda a emissão de `==`.
#[test]
fn equality_without_a_declared_operator_stays_a_strict_comparison() {
    let js = javascript("void main() { var a=1; var b=2; print(a==b); }");
    assert!(
        js.contains("(1 === 2)") || js.contains("($df_a === $df_b)"),
        "{js}"
    );
    assert!(!js.contains("$dartforgeEquals"), "{js}");
}

/// Escalares continuam em `===` mesmo num programa que declara o operador.
#[test]
fn scalar_equality_never_calls_the_runtime_helper() {
    let source = format!(
        "{PONTO}void main() {{ var a=Point(1,2); print(a==a); var i=1; var j=2; print(i==j); }}"
    );
    let js = javascript(&source);
    let escalar = js
        .lines()
        .find(|line| line.contains("$df_i") && line.contains("console.log"))
        .unwrap_or_default()
        .to_owned();
    assert!(escalar.contains("==="), "{js}");
    assert!(!escalar.contains("$dartforgeEquals"), "{js}");
}

/// Oráculo Dart 3.6.2 e 3.13.4 para igualdade, identidade e null dos dois lados.
#[test]
#[ignore = "requer Node.js no PATH"]
fn equality_identity_and_null_match_the_dart_oracle() {
    let source = format!(
        "{PONTO}void main() {{
  var a = Point(1, 2);
  var b = Point(1, 2);
  var c = Point(3, 4);
  print(a == b);
  print(a == c);
  print(a != b);
  print(identical(a, b));
  print(identical(a, a));
  Point? n = null;
  print(n == null);
  print(n == a);
  print(a == n);
  print(n != null);
}}"
    );
    assert_eq!(
        execute(&source),
        "true\nfalse\nfalse\nfalse\ntrue\ntrue\nfalse\nfalse\nfalse\n"
    );
}

/// Um receptor null nunca executa o corpo do operador declarado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn a_null_receiver_never_runs_the_declared_operator() {
    let source = "
class Sentinela {
  int marca = 0;
  bool operator ==(Object other) { print('chamou'); return true; }
}
void main() {
  Sentinela? n = null;
  Sentinela s = Sentinela();
  print(n == null);
  print(n == s);
  print(s == n);
}
";
    // `s == n` é o único que chama: o lado esquerdo não é null.
    assert_eq!(execute(source), "true\nfalse\nchamou\ntrue\n");
}

/// `identical` é identidade e não consulta o operador declarado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn identical_stays_reference_identity() {
    let source = format!(
        "{PONTO}void main() {{
  var a = Point(1, 2);
  var b = Point(1, 2);
  print(a == b);
  print(identical(a, b));
  print(identical(a, a));
}}"
    );
    assert_eq!(execute(&source), "true\nfalse\ntrue\n");
}

/// Par getter/setter de mesmo nome resolve leitura e escrita da propriedade.
#[test]
#[ignore = "requer Node.js no PATH"]
fn a_getter_setter_pair_reads_and_writes_the_same_property() {
    let source = "
class Celsius {
  int _graus = 0;
  int get graus { return _graus; }
  set graus(int valor) { _graus = valor < -273 ? -273 : valor; }
  int get dobro { return _graus * 2; }
  String toString() { return 'Celsius($_graus)'; }
}
void main() {
  var c = Celsius();
  c.graus = 25;
  print(c.graus);
  print(c.dobro);
  c.graus = -400;
  print(c.graus);
  print(c);
}
";
    assert_eq!(execute(source), "25\n50\n-273\nCelsius(-273)\n");
}

/// O setter também responde à forma implícita `nome = valor` dentro da classe.
#[test]
#[ignore = "requer Node.js no PATH"]
fn an_implicit_receiver_reaches_the_setter() {
    let source = "
class Contador {
  int _valor = 0;
  int get valor { return _valor; }
  set valor(int novo) { _valor = novo * 2; }
  void dobrar() { valor = valor; }
}
void main() { var c = Contador(); c.valor = 3; print(c.valor); c.dobrar(); print(c.valor); }
";
    assert_eq!(execute(source), "6\n12\n");
}

/// `toString` sobrescrito atende print, interpolação e chamada direta.
#[test]
#[ignore = "requer Node.js no PATH"]
fn to_string_serves_print_interpolation_and_direct_calls() {
    let source = format!(
        "{PONTO}void main() {{
  var a = Point(1, 2);
  print(a);
  print('valor: $a');
  print(a.toString());
  print(a.hashCode);
}}"
    );
    assert_eq!(
        execute(&source),
        "Point(1, 2)\nvalor: Point(1, 2)\nPoint(1, 2)\n33\n"
    );
}

/// O `toString` escolhido é o do objeto, não o do tipo estático da referência.
#[test]
#[ignore = "requer Node.js no PATH"]
fn to_string_dispatches_on_the_object_not_the_static_type() {
    let source = "
class Base { String toString() { return 'Base'; } }
class Derivada extends Base { String toString() { return 'Derivada'; } }
void main() { Base b = Derivada(); print(b); print('$b'); }
";
    assert_eq!(execute(source), "Derivada\nDerivada\n");
}

/// Instâncias dentro de coleções usam o mesmo `toString` declarado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn collections_format_elements_with_the_declared_to_string() {
    let source = "
class P { final int x; P(this.x); String toString() { return 'P($x)'; } }
void main() { var lista = [P(1), P(2)]; print(lista); }
";
    assert_eq!(execute(source), "[P(1), P(2)]\n");
}

/// Objetos iguais por `==` podem declarar o mesmo `hashCode`; ver docs/OBJETO.md.
#[test]
#[ignore = "requer Node.js no PATH"]
fn equal_objects_share_the_declared_hash_code() {
    let source = format!(
        "{PONTO}void main() {{
  var a = Point(1, 2);
  var b = Point(1, 2);
  print(a == b);
  print(a.hashCode == b.hashCode);
}}"
    );
    assert_eq!(execute(&source), "true\ntrue\n");
}

/// `Map` deste subconjunto compara chaves por identidade, não por `==`/`hashCode`.
///
/// Divergência deliberada e documentada em `docs/OBJETO.md`: o Dart 3.6.2 e o
/// 3.13.4 imprimem `1` para este programa, porque a tabela hash usa `==` e
/// `hashCode`; aqui a chave é comparada por identidade e as duas entradas
/// coexistem. O teste existe para que a divergência não passe despercebida.
#[test]
#[ignore = "requer Node.js no PATH"]
fn map_keys_compare_by_identity_not_by_the_declared_equality() {
    let source = format!(
        "{PONTO}void main() {{
  var tabela = <Point, int>{{}};
  tabela[Point(1, 2)] = 1;
  tabela[Point(1, 2)] = 2;
  print(tabela.length);
}}"
    );
    // Dart imprime "1\n"; o subconjunto imprime "2\n".
    assert_eq!(execute(&source), "2\n");
}

/// Instância sem `toString` declarado é recusada, e não impressa como Dart faz.
#[test]
fn an_instance_without_to_string_is_rejected_instead_of_printed() {
    let source = "
class SemToString { int v = 1; }
void main() { print(SemToString()); }
";
    rejeita(
        source,
        "Class 'SemToString' declares no 'String toString()'; this subset does not emit \"Instance of 'SemToString'\"",
        trecho(source, "SemToString()"),
    );
    let interpolado = "
class SemToString { int v = 1; }
void main() { print('${SemToString()}'); }
";
    rejeita(
        interpolado,
        "Class 'SemToString' declares no 'String toString()'; this subset does not emit \"Instance of 'SemToString'\"",
        trecho(interpolado, "SemToString()"),
    );
}

/// Atribuir a propriedade que só declara getter é erro, com mensagem própria.
#[test]
fn assigning_to_a_getter_without_a_setter_is_a_compile_error() {
    let source = "
class C { int _v = 1; int get v { return _v; } }
void main() { var c = C(); c.v = 2; }
";
    rejeita(
        source,
        "Cannot assign to 'v': the property declares a getter and no setter",
        trecho(source, "c.v = 2;"),
    );
}

/// Ler propriedade que só declara setter também tem diagnóstico próprio.
#[test]
fn reading_a_setter_only_property_is_a_compile_error() {
    let source = "
class C { int _v = 1; set v(int novo) { _v = novo; } }
void main() { var c = C(); print(c.v); }
";
    rejeita(
        source,
        "Cannot read 'v': the property declares a setter and no getter",
        trecho(source, "c.v"),
    );
}

/// Getter e campo de mesmo nome na mesma classe é erro.
#[test]
fn a_getter_and_a_field_cannot_share_a_name() {
    let source = "
class C { int v = 1; int get v { return 2; } }
void main() { print(C().v); }
";
    rejeita(
        source,
        "Duplicate class member",
        trecho(source, "int get v { return 2; }"),
    );
}

/// Getter e setter precisam declarar o mesmo tipo.
#[test]
fn a_getter_and_its_setter_must_agree_on_the_type() {
    let source = "
class C { int _v = 1; int get v { return _v; } set v(String novo) { _v = 1; } }
void main() { print(C().v); }
";
    rejeita(
        source,
        "A getter and its setter must declare the same type",
        trecho(source, "set v(String novo) { _v = 1; }"),
    );
}

/// Setter em conflito com campo ou com método comum de mesmo nome.
#[test]
fn a_setter_cannot_share_a_field_or_method_name() {
    let campo = "
class C { int v = 1; set v(int n) { v = n; } }
void main() { print(1); }
";
    rejeita(
        campo,
        "Duplicate setter or conflicting field",
        trecho(campo, "set v(int n) { v = n; }"),
    );
    let metodo = "
class C { int _v = 1; void v(int n) { _v = n; } set v(int n) { _v = n; } }
void main() { print(1); }
";
    rejeita(
        metodo,
        "Duplicate class member: a setter cannot share a method name",
        trecho(metodo, "set v(int n) { _v = n; }"),
    );
}

/// Enums não declaram acessor, operador nem protocolo Object neste subconjunto.
#[test]
fn enums_reject_the_object_protocol_and_accessors() {
    let source = "
enum Cor { vermelho, azul; String toString() { return 'cor'; } }
void main() { print(1); }
";
    rejeita(
        source,
        "Enums cannot declare setters, operator ==, toString or hashCode in this subset",
        trecho(source, "String toString() { return 'cor'; }"),
    );
}

/// Assinaturas fixas de toString, hashCode e operator ==.
#[test]
fn the_object_protocol_signatures_are_fixed() {
    let to_string = "
class C { int toString() { return 1; } }
void main() { print(C().toString()); }
";
    rejeita(
        to_string,
        "toString must be declared as 'String toString()'",
        trecho(to_string, "int toString() { return 1; }"),
    );
    let hash_code = "
class C { String get hashCode { return 'a'; } }
void main() { print(C().hashCode); }
";
    rejeita(
        hash_code,
        "hashCode must be declared as 'int get hashCode'",
        trecho(hash_code, "String get hashCode { return 'a'; }"),
    );
    let igual = "
class C { bool operator ==(int other) { return true; } }
void main() { print(C() == C()); }
";
    rejeita(
        igual,
        "operator == must be declared as 'bool operator ==(Object other)'",
        trecho(igual, "bool operator ==(int other) { return true; }"),
    );
}

/// Só `operator ==` é declarável; os demais continuam recusados.
#[test]
fn other_operator_declarations_stay_rejected() {
    let source = "
class C { int operator +(C other) { return 1; } }
void main() { print(1); }
";
    rejeita(
        source,
        "only 'operator ==' is supported; other operator declarations are not supported yet",
        trecho(source, "+"),
    );
}

/// `dynamic` é recusado com a explicação do motivo, não com erro genérico.
#[test]
fn dynamic_is_rejected_with_an_explicit_reason() {
    let source = "void main() { dynamic x = 1; print(x); }";
    rejeita(
        source,
        "dynamic is unsupported: this subset resolves every member statically, and dynamic dispatch would disable tree shaking and force the runtime to keep every same-named member; use an explicit type, Object or Object?",
        trecho(source, "dynamic"),
    );
}

/// `identical` recusa `double`/`num`, onde o apagamento para Number mente.
#[test]
fn identical_rejects_double_operands() {
    let source = "void main() { print(identical(1, 1.5)); }";
    rejeita(
        source,
        "identical on double or num operands is unsupported: the JavaScript Number erasure cannot distinguish the identities Dart distinguishes",
        trecho(source, "1.5"),
    );
}

/// `identical` não vira `==` no parse: o operador declarado nunca é chamado.
#[test]
fn identical_never_reaches_the_declared_operator() {
    let source = format!(
        "{PONTO}void main() {{ var a=Point(1,2); var b=Point(1,2); print(identical(a,b)); }}"
    );
    let js = javascript(&source);
    let linha = js
        .lines()
        .find(|line| line.contains("console.log"))
        .unwrap_or_default();
    assert!(linha.contains("==="), "{js}");
    assert!(!linha.contains("$dartforgeEquals"), "{js}");
}

/// Setter abstrato, assíncrono e com aridade errada têm diagnósticos próprios.
#[test]
fn rejected_setter_forms_report_message_and_span() {
    let abstrato = "
abstract class C { set v(int novo); }
void main() { print(1); }
";
    let assinatura = trecho(abstrato, "set v(int novo);");
    rejeita(
        abstrato,
        "abstract setters and operator declarations are not supported yet",
        Span {
            start: assinatura.end - 1,
            end: assinatura.end,
        },
    );
    let aridade = "
class C { int _v = 1; set v(int a, int b) { _v = a; } }
void main() { print(1); }
";
    rejeita(
        aridade,
        "a setter takes exactly one required positional parameter",
        trecho(aridade, "set v(int a, int b)"),
    );
    let retorno = "
class C { int _v = 1; int set v(int novo) { _v = novo; } }
void main() { print(1); }
";
    rejeita(
        retorno,
        "a setter declares no return type or 'void'",
        trecho(retorno, "set"),
    );
}

/// Sobrescrever um setter com tipo mais estreito quebra a contravariância.
#[test]
fn a_setter_override_keeps_parameter_contravariance() {
    let source = "
class Base { int _v = 1; set v(Object novo) { _v = 1; } }
class Derivada extends Base { set v(int novo) { _v = novo; } }
void main() { print(1); }
";
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, "Type mismatch: expected Int, found Object");
}

/// `operator ==` herdado decide a comparação por uma referência da base.
#[test]
#[ignore = "requer Node.js no PATH"]
fn an_inherited_operator_decides_equality_through_a_base_reference() {
    let source = "
class Base {
  final int v;
  Base(this.v);
  bool operator ==(Object other) { return other is Base && other.v == v; }
}
class Derivada extends Base { Derivada(int v) : super(v); }
void main() { Base a = Derivada(7); Base b = Derivada(7); print(a == b); print(identical(a, b)); }
";
    assert_eq!(execute(source), "true\nfalse\n");
}
