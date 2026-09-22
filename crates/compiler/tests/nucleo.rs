//! Núcleo de `dart:core`: tipos base e membros de biblioteca.
//!
//! O oráculo é o **Dart 3.6.2** (`dart`) e o **Dart 3.13.4**
//! (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`), que produzem a mesma saída em
//! todos os programas deste arquivo. Cada `assert_eq!` de execução compara com o
//! texto copiado de `dart run`, **sem edição**. O contrato completo, a política de
//! cada decisão e a mensagem exata de cada recusa estão em `docs/NUCLEO.md`.
//!
//! Os casos negativos afirmam **mensagem e intervalo**, porque um diagnóstico com
//! o texto certo e o span errado aponta para o lugar errado no editor — e é essa
//! a única parte do diagnóstico que o usuário vê primeiro.
use dartforge_compiler::compile;
use dartforge_diagnostics::Span;

/// Compila uma fonte válida e devolve o módulo JavaScript emitido.
fn javascript(source: &str) -> String {
    compile(source).unwrap_or_else(|error| panic!("{source}: {}", error.message))
}

/// Executa o módulo gerado no Node e devolve stdout com quebras normalizadas.
fn execute(source: &str) -> String {
    let js = javascript(source);
    let result = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .expect("Node.js necessário");
    assert!(
        result.status.success(),
        "{}\n--- javascript ---\n{js}",
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

/// Confere só a mensagem exata, quando o span já é coberto por outro caso.
fn rejeita_mensagem(source: &str, message: &str) {
    let error = compile(source).expect_err(source);
    assert_eq!(error.message, message, "{source}");
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

/// Aceita a fonte e afirma que ela compila; usada nos casos só de resolução.
fn aceita(source: &str) {
    javascript(source);
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

/// Todo membro de `String` resolve e produz o tipo que o oráculo produz.
#[test]
fn string_members_resolve() {
    aceita(
        "void main() {
  var s = 'Dart Forge';
  int a = s.length;
  bool b = s.isEmpty;
  bool c = s.isNotEmpty;
  String d = s.substring(5);
  String e = s.substring(0, 4);
  int f = s.indexOf('r');
  int g = s.indexOf('r', 3);
  int h = s.lastIndexOf('r');
  bool i = s.contains('Forge');
  bool j = s.startsWith('Dart');
  bool k = s.endsWith('ge');
  String l = s.toLowerCase();
  String m = s.toUpperCase();
  String n = s.trim();
  String o = s.trimLeft();
  String p = s.trimRight();
  List<String> q = s.split(' ');
  String r = s.replaceAll('r', 'R');
  String t = s.replaceFirst('r', 'R');
  String u = s.replaceRange(0, 4, 'Pear');
  String v = s.padLeft(3, '0');
  String w = s.padRight(3, '.');
  int x = s.codeUnitAt(0);
  List<int> y = s.codeUnits;
  int z = s.compareTo('b');
  String aa = s.toString();
  int bb = s.hashCode;
  print(a + f + g + h + x + z + bb);
  print('$b$c$d$e$i$j$k$l$m$n$o$p$q$r$t$u$v$w$y$aa');
}",
    );
}

/// Saída copiada de `dart run`, sem edição.
#[test]
#[ignore = "requer Node.js no PATH"]
fn string_members_match_the_oracle() {
    let saida = execute(
        "void main() {
  var s = 'Dart Forge';
  print(s.length);
  print(s.isEmpty);
  print(s.isNotEmpty);
  print(s.substring(5));
  print(s.substring(0, 4));
  print(s.indexOf('r'));
  print(s.indexOf('r', 3));
  print(s.lastIndexOf('r'));
  print(s.contains('Forge'));
  print(s.startsWith('Dart'));
  print(s.endsWith('ge'));
  print(s.toLowerCase());
  print(s.toUpperCase());
  print('  x  '.trim());
  print('  x  '.trimLeft());
  print('  x  '.trimRight());
  print(s.split(' '));
  print(s.split(''));
  print(s.replaceAll('r', 'R'));
  print(s.replaceFirst('r', 'R'));
  print(s.replaceRange(0, 4, 'Pear'));
  print('7'.padLeft(3, '0'));
  print('7'.padRight(3, '.'));
  print('7'.padLeft(3));
  print(s.codeUnitAt(0));
  print(s.codeUnits);
  print('a'.compareTo('b'));
  print('b'.compareTo('a'));
  print('a'.compareTo('a'));
  print(s.toString());
  print(''.isEmpty);
  print('abc'.indexOf('z'));
}",
    );
    assert_eq!(
        saida,
        "10
false
true
Forge
Dart
2
7
7
true
true
true
dart forge
DART FORGE
x
x
  x
[Dart, Forge]
[D, a, r, t,  , F, o, r, g, e]
DaRt FoRge
DaRt Forge
Pear Forge
007
7..
  7
68
[68, 97, 114, 116, 32, 70, 111, 114, 103, 101]
-1
1
0
Dart Forge
true
-1
"
    );
}

/// `padLeft` repete a cadeia inteira `delta` vezes, ao contrário de `padStart`.
#[test]
#[ignore = "requer Node.js no PATH"]
fn pad_repeats_the_whole_padding_like_dart_and_not_like_pad_start() {
    let saida = execute(
        "void main() {
  print('7'.padLeft(5, 'ab'));
  print('7'.padRight(5, 'ab'));
  print('7'.padLeft(1, 'ab'));
}",
    );
    assert_eq!(saida, "abababab7\n7abababab\n7\n");
}

/// Casos de borda de `String` que o JavaScript resolveria de outro jeito.
#[test]
#[ignore = "requer Node.js no PATH"]
fn string_edge_cases_match_the_oracle() {
    let saida = execute(
        "void main() {
  print(''.split(''));
  print('a'.split('a'));
  print('ab'.replaceAll('', 'x'));
  print('abc'.contains('a', 1));
  print('abcabc'.replaceFirst('b', 'B', 2));
  print('abc'.substring(1, null));
  print('abc'.substring(3));
  print('abc'.replaceRange(1, null, 'Z'));
  print('abc'.lastIndexOf('c'));
  print('Ab'.compareTo('aB'));
  print('a'.hashCode == 'a'.hashCode);
}",
    );
    assert_eq!(
        saida,
        "[]
[, ]
xaxbx
false
abcaBc
bc

aZ
2
-1
true
"
    );
}

/// Índice fora de faixa lança, em vez de grampear como `slice`.
#[test]
#[ignore = "requer Node.js no PATH"]
fn string_range_checks_throw_like_the_oracle() {
    for (programa, esperado) in [
        (
            "void main() { try { print('abc'.substring(5)); } on Falha catch (e) { print(e); } }",
            "",
        ),
        ("", ""),
    ] {
        let _ = (programa, esperado);
    }
    // `catch (e)` liga `Object`, que este subconjunto não imprime; o que se
    // afirma aqui é o **lançamento**, observado pela falha do processo Node.
    let js = javascript("void main() { print('abc'.substring(5)); }");
    let resultado = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .expect("Node.js necessário");
    assert!(!resultado.status.success());
    let texto = String::from_utf8_lossy(&resultado.stderr);
    assert!(texto.contains("Not in inclusive range 0..3: 5"), "{texto}");
}

/// Um nome que não é membro de `String` nomeia o tipo no diagnóstico.
#[test]
fn unknown_string_member_names_the_type() {
    let fonte = "void main() { var s = 'a'; print(s.tamanho); }";
    rejeita(
        fonte,
        "'String' declares no member 'tamanho' in this subset; the recognized members are listed in docs/NUCLEO.md",
        trecho(fonte, "s.tamanho"),
    );
    let chamada = "void main() { var s = 'a'; print(s.corta(1)); }";
    rejeita(
        chamada,
        "'String' declares no method 'corta' in this subset, and no extension on it declares one either; the recognized members are listed in docs/NUCLEO.md",
        trecho(chamada, "s.corta(1)"),
    );
}

/// Tear-off de membro de biblioteca é recusado com a alternativa.
#[test]
fn library_member_tear_off_is_refused() {
    let fonte = "void main() { var s = 'a'; var f = s.trim; print(f); }";
    rejeita(
        fonte,
        "'String.trim' is a method: a library member tear-off is unsupported in this subset; call it as 'trim(...)' or wrap it in a closure",
        trecho(fonte, "s.trim"),
    );
}

// ---------------------------------------------------------------------------
// int, double, num, bool
// ---------------------------------------------------------------------------

/// Saída copiada de `dart run`, sem edição.
#[test]
#[ignore = "requer Node.js no PATH"]
fn numeric_members_match_the_oracle() {
    let saida = execute(
        "void main() {
  int i = -7;
  print(i.abs());
  print(i.isEven);
  print(i.isOdd);
  print(i.isNegative);
  print(i.sign);
  print(i.toString());
  print(i.toDouble());
  print(i.toInt());
  print(i.round());
  print(i.floor());
  print(i.ceil());
  print(i.truncate());
  print(i.compareTo(3));
  print(3.compareTo(3));
  print(255.toRadixString(16));
  print(12.gcd(18));
  print(7.bitLength);
  print(0.bitLength);
  double d = -7.5;
  print(d.abs());
  print(d.isNaN);
  print(d.isFinite);
  print(d.isInfinite);
  print(d.isNegative);
  print(d.sign);
  print(d.toString());
  print(d.toDouble());
  print(d.toInt());
  print(d.round());
  print(d.floor());
  print(d.ceil());
  print(d.truncate());
  print(d.roundToDouble());
  print(d.floorToDouble());
  print(d.ceilToDouble());
  print(d.truncateToDouble());
  print(d.toStringAsFixed(2));
  print(d.compareTo(1.0));
  print(2.5.round());
  print(3.5.round());
  print((-2.5).round());
  print(1.0.toString());
  print(true.toString());
  print(false.toString());
  print(true.hashCode == true.hashCode);
}",
    );
    assert_eq!(
        saida,
        "7
false
true
true
-1
-7
-7.0
-7
-7
-7
-7
-7
-1
0
ff
6
3
0
7.5
false
true
false
true
-1.0
-7.5
-7.5
-7
-8
-8
-7
-7
-8.0
-8.0
-7.0
-7.0
-7.50
-1
3
4
-3
1.0
true
false
true
"
    );
}

/// As quatro divergências entre Dart e JavaScript, uma por linha.
#[test]
#[ignore = "requer Node.js no PATH"]
fn the_four_numeric_divergences_follow_dart() {
    let saida = execute(
        "void main() {
  print((-0.0).isNegative);
  print((0.0).isNegative);
  print((-0.0).sign);
  print((-0.5).truncateToDouble());
  print((-2.5).roundToDouble());
  print((2.5).roundToDouble());
  print((0.0).compareTo(-0.0));
  print((-1).gcd(6));
  print(0.gcd(0));
  print((-2).bitLength);
  print((-1).bitLength);
  print(255.bitLength);
  print((0.0).toStringAsFixed(1));
  print((-0.0).toStringAsFixed(1));
  print((1.005).toStringAsFixed(2));
  print(true.hashCode);
  print(false.hashCode);
}",
    );
    assert_eq!(
        saida,
        "true
false
-0.0
-0.0
-3.0
3.0
1
1
0
1
0
8
0.0
-0.0
1.00
1231
1237
"
    );
}

/// `round`/`floor`/`toInt` lançam sobre NaN e infinito, como no oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn to_int_on_nan_throws_like_the_oracle() {
    let js = javascript("void main() { double d = 0.0 / 0.0; print(d.round()); }");
    let resultado = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &js])
        .output()
        .expect("Node.js necessário");
    assert!(!resultado.status.success());
    let texto = String::from_utf8_lossy(&resultado.stderr);
    assert!(
        texto.contains("Unsupported operation: Infinity or NaN toInt"),
        "{texto}"
    );
}

/// A semântica de `int` do alvo web é preservada: nada vira `String(x)` do JS.
#[test]
fn int_to_string_keeps_the_subset_policy_and_never_uses_javascript_string() {
    let js = javascript("void main() { int x = 0; print((x * -1).toString()); }");
    assert!(js.contains("$dartforgeIntString("), "{js}");
    // Nenhum `String(x)` global do JavaScript: removidos o próprio auxiliar e
    // o `.toString(10)` que é a sua implementação documentada, nada com
    // `String(` pode restar.
    let sem_auxiliar = js.replace("$dartforgeIntString", "");
    let sem_metodo = sem_auxiliar.replace(".toString(", "");
    assert!(!sem_metodo.contains("String("), "{js}");
}

/// Zero negativo de `int` imprime `0`, como o Dart, e não `-0`.
#[test]
#[ignore = "requer Node.js no PATH"]
fn negative_zero_int_prints_as_zero() {
    // Oráculo: `dart run` imprime `0` para `(0 * -1).toString()`.
    assert_eq!(
        execute("void main() { int x = 0; print((x * -1).toString()); }"),
        "0\n"
    );
}

/// `num.toString()` é recusado porque o apagamento não distingue 1 de 1.0.
#[test]
fn num_to_string_is_refused_with_the_erasure_reason() {
    let fonte = "void main() { num n = 1; print(n.toString()); }";
    rejeita(
        fonte,
        "'num.toString()' is unsupported: the JavaScript Number erasure cannot tell 1 from 1.0, and the oracle prints '1' for the int and '1.0' for the double; call 'toInt().toString()' or 'toDouble().toString()' to state which text you mean",
        trecho(fonte, "n.toString()"),
    );
    // As duas formas explícitas continuam aceitas.
    aceita("void main() { num n = 1; print(n.toInt().toString()); }");
    aceita("void main() { num n = 1; print(n.toDouble().toString()); }");
}

/// `runtimeType` e `noSuchMethod` recusam com a razão e a alternativa.
#[test]
fn object_members_outside_the_subset_explain_themselves() {
    let fonte = "void main() { var s = 'a'; print(s.runtimeType); }";
    rejeita(
        fonte,
        "runtimeType is unsupported: reading it would force every class of the program to carry its Dart name into the emitted JavaScript, because the text is chosen by the object and not by the static type — the same cost that made this subset refuse \"Instance of 'Name'\"; use 'is' or 'as' to recover the concrete type",
        trecho(fonte, "s.runtimeType"),
    );
}

// ---------------------------------------------------------------------------
// Comparable e a ordenação de sort
// ---------------------------------------------------------------------------

/// Classe comparável reutilizada pelos casos de ordenação.
const VERSAO: &str = "
class Versao implements Comparable<Versao> {
  final int maior;
  final int menor;
  Versao(this.maior, this.menor);
  int compareTo(Versao outra) {
    if (maior != outra.maior) { return maior - outra.maior; }
    return menor - outra.menor;
  }
  String toString() { return '$maior.$menor'; }
}
";

/// `compareTo` declarado dirige `sort()` sem comparador. Saída do oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn declared_compare_to_drives_sort() {
    let fonte = format!(
        "{VERSAO}void main() {{
  var vs = <Versao>[Versao(1, 2), Versao(1, 0), Versao(0, 9)];
  vs.sort();
  print(vs);
  print(Versao(1, 0).compareTo(Versao(1, 2)));
}}"
    );
    assert_eq!(execute(&fonte), "[0.9, 1.0, 1.2]\n-2\n");
}

/// `sort()` de escalares e `sort(comparador)`. Saída do oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn sort_of_scalars_and_with_comparator_match_the_oracle() {
    let saida = execute(
        "void main() {
  var ys = <int>[3, 1, 2];
  ys.sort();
  print(ys);
  var zs = <int>[3, 1, 2];
  zs.sort((a, b) => b - a);
  print(zs);
  var ws = <String>['b', 'a', 'c'];
  ws.sort();
  print(ws);
}",
    );
    assert_eq!(saida, "[1, 2, 3]\n[3, 2, 1]\n[a, b, c]\n");
}

/// `sort()` sem `compareTo` é recusado em compilação, e não em execução.
#[test]
fn sort_without_compare_to_is_refused_at_compile_time() {
    let fonte = "class P { final int v; P(this.v); String toString() { return 'P'; } }
void main() { var xs = <P>[P(2), P(1)]; xs.sort(); print(xs); }";
    rejeita(
        fonte,
        "'sort()' without a comparator orders by 'compareTo', and 'P' declares none: declare 'int compareTo(P other)' and 'implements Comparable<P>', or call 'sort((a, b) => ...)'",
        trecho(fonte, "xs.sort()"),
    );
    // Com comparador o mesmo programa compila.
    aceita(
        "class P { final int v; P(this.v); String toString() { return 'P'; } }
void main() { var xs = <P>[P(2), P(1)]; xs.sort((a, b) => a.v - b.v); print(xs); }",
    );
}

/// `implements Comparable` exige o `compareTo` que dá sentido à declaração.
#[test]
fn implements_comparable_requires_compare_to() {
    let fonte = "class V implements Comparable<V> { final int v; V(this.v); }
void main() { print(V(1).v); }";
    rejeita_mensagem(
        fonte,
        "'V' declares 'implements Comparable' but no 'int compareTo(V other)': the ordering contract is what 'sort()' and 'compareTo' resolve against, so it cannot be left out",
    );
}

/// `Comparable` e `Comparator` valem como anotação de tipo.
#[test]
fn comparable_and_comparator_are_written_types() {
    aceita(
        "void main() {
  Comparator<int> c = (a, b) => a - b;
  var xs = <int>[2, 1];
  xs.sort(c);
  print(xs);
}",
    );
    let fonte = format!(
        "{VERSAO}void main() {{ Comparable<Versao> c = Versao(1, 0); print(c.compareTo(Versao(1, 1))); }}"
    );
    aceita(&fonte);
}

// ---------------------------------------------------------------------------
// Iterator, Iterable e Exception implementáveis
// ---------------------------------------------------------------------------

/// Uma classe do usuário implementa `Iterator<T>` e é percorrida à mão.
#[test]
#[ignore = "requer Node.js no PATH"]
fn user_class_implements_iterator() {
    let saida = execute(
        "class Contagem implements Iterator<int> {
  int _atual = 0;
  final int _limite;
  Contagem(this._limite);
  int get current { return _atual; }
  bool moveNext() {
    if (_atual >= _limite) { return false; }
    _atual = _atual + 1;
    return true;
  }
}
void main() {
  var c = Contagem(3);
  while (c.moveNext()) { print(c.current); }
}",
    );
    assert_eq!(saida, "1\n2\n3\n");
}

/// `implements Iterator` sem `moveNext` não passa pelo contrato.
#[test]
fn implements_iterator_requires_move_next() {
    let fonte = "class C implements Iterator<int> { int get current { return 0; } }
void main() { print(C().current); }";
    rejeita_mensagem(fonte, "Missing concrete implementation of 'moveNext'");
}

/// Uma classe do usuário implementa `Iterable<T>` declarando o iterator.
#[test]
fn user_class_implements_iterable() {
    aceita(
        "class Ite implements Iterator<int> {
  int get current { return 0; }
  bool moveNext() { return false; }
}
class Col implements Iterable<int> {
  Iterator<int> get iterator { return Ite(); }
}
void main() { var c = Col(); print(c.iterator.moveNext()); }",
    );
}

/// `for-in` sobre `Iterable` do usuário recusa com a razão do apagamento.
#[test]
fn for_in_over_a_user_iterable_explains_the_erasure() {
    let fonte = "class Ite implements Iterator<int> {
  int get current { return 0; }
  bool moveNext() { return false; }
}
class Col implements Iterable<int> {
  Iterator<int> get iterator { return Ite(); }
}
void main() { for (var x in Col()) { print(x); } }";
    rejeita(
        fonte,
        "'for-in' over a class that implements Iterable<T> is unsupported: this subset erases class type arguments, so the element would bind as Object?; iterate explicitly with 'var it = x.iterator; while (it.moveNext()) { ... it.current ... }', which keeps the type your own Iterator declares",
        trecho(fonte, "Col()) { print(x); } }").into_start_of("Col()"),
    );
}

/// `implements Exception` não exige membro nenhum; `throw`/`catch` já existiam.
#[test]
#[ignore = "requer Node.js no PATH"]
fn user_class_implements_exception() {
    let saida = execute(
        "class FalhaDeRede implements Exception {
  final String mensagem;
  FalhaDeRede(this.mensagem);
  String toString() { return 'FalhaDeRede: $mensagem'; }
}
void main() {
  try {
    throw FalhaDeRede('tempo esgotado');
  } on FalhaDeRede catch (e) {
    print(e);
  }
}",
    );
    assert_eq!(saida, "FalhaDeRede: tempo esgotado\n");
}

/// `Exception` também vale como anotação de tipo.
#[test]
fn exception_is_a_written_type() {
    aceita(
        "class F implements Exception { String toString() { return 'F'; } }
void main() { Exception e = F(); print(e is F); }",
    );
}

/// `implements Error` recusa com a razão conferida no próprio SDK.
#[test]
fn implements_error_is_refused_with_the_stack_trace_reason() {
    let fonte = "class E implements Error { String toString() { return 'E'; } }
void main() { print(E()); }";
    rejeita(
        fonte,
        "Error is unsupported: Dart's Error declares 'StackTrace? get stackTrace', and this subset has no StackTrace type to satisfy it — 'implements Error' without that getter is a compile error in the SDK too; use 'implements Exception', which declares no members, and 'on YourType catch (e)' to recover it",
        trecho(fonte, "Error"),
    );
}

/// Uma classe homônima do programa tem precedência sobre a interface reservada.
#[test]
fn a_user_class_named_exception_wins() {
    aceita(
        "class Exception { String toString() { return 'minha'; } }
class F implements Exception { String toString() { return 'F'; } }
void main() { print(F()); }",
    );
}

// ---------------------------------------------------------------------------
// StringBuffer
// ---------------------------------------------------------------------------

/// `StringBuffer` acumula texto e mede em unidades UTF-16. Saída do oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn string_buffer_matches_the_oracle() {
    let saida = execute(
        "void main() {
  var b = StringBuffer();
  print(b.isEmpty);
  b.write('a');
  b.write(1);
  b.write(true);
  b.writeln('!');
  print(b.length);
  print(b.isNotEmpty);
  print(b.toString());
  b.writeCharCode(66);
  print(b.toString());
  b.clear();
  print(b.isEmpty);
  print(b.length);
  print(b.toString());
}",
    );
    assert_eq!(
        saida,
        "true
8
true
a1true!

a1true!
B
true
0

"
    );
}

/// `StringBuffer.write` recusa valor sem representação textual definida.
#[test]
fn string_buffer_write_refuses_a_value_without_text() {
    let fonte = "class SemTexto { final int v; SemTexto(this.v); }
void main() { var b = StringBuffer(); b.write(SemTexto(1)); print(b); }";
    rejeita(
        fonte,
        "'StringBuffer.write' writes the value through its 'toString', and 'SemTexto' declares none: emitting the JavaScript \"[object Object]\" — or Dart's \"Instance of 'Name'\" — would be a plausible wrong answer; declare 'String toString()' on it",
        trecho(fonte, "SemTexto(1))").into_start_of("SemTexto(1)"),
    );
}

/// `StringBuffer` com valor inicial é recusado, com a alternativa.
#[test]
fn string_buffer_with_an_initial_value_is_refused() {
    let fonte = "void main() { var b = StringBuffer('x'); print(b.length); }";
    rejeita(
        fonte,
        "'StringBuffer(...)' with an initial value is unsupported in this subset: write 'StringBuffer()' and call 'write' once",
        trecho(fonte, "StringBuffer('x')"),
    );
}

/// Um local chamado `StringBuffer` tem precedência sobre o intrínseco.
#[test]
fn a_local_named_string_buffer_shadows_the_intrinsic() {
    rejeita_mensagem(
        "void main() { int StringBuffer = 1; var b = StringBuffer(); print(b); }",
        "Calling a non-function value is unsupported",
    );
}

// ---------------------------------------------------------------------------
// List, Set e Iterable
// ---------------------------------------------------------------------------

/// Saída copiada de `dart run`, sem edição.
#[test]
#[ignore = "requer Node.js no PATH"]
fn list_members_match_the_oracle() {
    let saida = execute(
        "void main() {
  var xs = <int>[3, 1, 2];
  print(xs.length);
  print(xs.first);
  print(xs.last);
  print(xs.isEmpty);
  print(xs.isNotEmpty);
  print(xs.reversed);
  print(xs.reversed.toList());
  print(xs.indexOf(1));
  print(xs.indexOf(9));
  print(xs.join());
  print(xs.join('-'));
  print(xs.elementAt(1));
  print(xs.skip(1));
  print(xs.take(2));
  print(xs.skip(1).toList());
  print(xs.every((x) => x > 0));
  print(xs.any((x) => x > 2));
  print(xs.fold(0, (a, b) => a + b));
  print(xs.reduce((a, b) => a + b));
  print(xs.expand((x) => <int>[x, x]));
  print(xs.expand((x) => <int>[x, x]).toList());
  print(xs.firstWhere((x) => x > 1));
  print(xs.sublist(1));
  print(xs.sublist(0, 2));
  print(xs.contains(2));
  print(xs.toSet());
  print(xs.map((x) => x * 2).toList());
  print(xs.toString());
  print(<int>[7].single);
  print(xs.skip(9).toList());
  print(xs.take(9).toList());
  print(xs.sublist(3));
}",
    );
    assert_eq!(
        saida,
        "3
3
2
false
true
(2, 1, 3)
[2, 1, 3]
1
-1
312
3-1-2
1
(1, 2)
(3, 1)
[1, 2]
true
true
6
6
(3, 3, 1, 1, 2, 2)
[3, 3, 1, 1, 2, 2]
3
[1, 2]
[3, 1]
true
{3, 1, 2}
[6, 2, 4]
[3, 1, 2]
7
[]
[1, 2, 3]
[]
"
    );
}

/// Mutação de `List` preserva a ordem e os resultados do oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn list_mutation_matches_the_oracle() {
    let saida = execute(
        "void main() {
  var g = <int>[1];
  g.addAll(<int>[2, 3]);
  print(g);
  g.insert(0, 0);
  print(g);
  print(g.remove(2));
  print(g);
  print(g.removeAt(0));
  print(g);
  g.clear();
  print(g);
  print(g.isEmpty);
}",
    );
    assert_eq!(
        saida,
        "[1, 2, 3]
[0, 1, 2, 3]
true
[0, 1, 3]
0
[1, 3]
[]
true
"
    );
}

/// `Iterable` preguiçoso recusa mutação, com a alternativa escrita.
#[test]
fn a_lazy_iterable_refuses_mutation() {
    let fonte =
        "void main() { var it = <int>[1, 2].where((x) => x > 0); it.insert(0, 3); print(it); }";
    rejeita(
        fonte,
        "'Iterable<int>' is a lazy sequence and declares no 'insert': materialize it with '.toList()' first",
        trecho(fonte, "it.insert(0, 3)"),
    );
    let ordena = "void main() { var s = <int>{2, 1}; s.sort(); print(s); }";
    rejeita(
        ordena,
        "'Set<int>' declares no 'sort': only List is ordered in place; call '.toList()' first",
        trecho(ordena, "s.sort()"),
    );
    let inverte = "void main() { var s = <int>{2, 1}; print(s.reversed); }";
    rejeita(
        inverte,
        "'Set<int>' declares no 'reversed': only List does; call '.toList().reversed'",
        trecho(inverte, "s.reversed"),
    );
}

/// `join` recusa elemento sem `toString`, em vez de emitir `[object Object]`.
#[test]
fn join_refuses_an_element_without_text() {
    let fonte = "class SemTexto { final int v; SemTexto(this.v); }
void main() { var xs = <SemTexto>[SemTexto(1)]; print(xs.join(',')); }";
    rejeita(
        fonte,
        "'join' on 'List<SemTexto>' has no defined text in this subset: some element type declares no 'String toString()', and emitting the JavaScript \"[object Object]\" — or Dart's \"Instance of 'Name'\" — would be a plausible wrong answer; declare 'String toString()' on it, or map the elements to String first",
        trecho(fonte, "xs.join(',')"),
    );
}

/// `hashCode` de coleção é recusado, porque o do `Object` é identidade.
#[test]
fn collection_hash_code_is_refused() {
    let fonte = "void main() { var xs = <int>[1]; print(xs.hashCode); }";
    rejeita(
        fonte,
        "'hashCode' on 'List<int>' is unsupported: collections inherit the identity hash of Object — two lists with the same elements have different hashes — and reproducing that would need an identity table kept only for this member; compare with 'identical', or hash the elements you care about",
        trecho(fonte, "xs.hashCode"),
    );
}

/// `cast` e `whereType` recusam por exigir método genérico.
#[test]
fn cast_and_where_type_are_refused() {
    let fonte = "void main() { var xs = <int>[1]; print(xs.cast()); }";
    rejeita_mensagem(
        fonte,
        "'cast' is unsupported: it needs a written type argument on a method, and generic methods are outside this subset; declare the target collection with its element type and copy into it",
    );
}

/// Ler `.iterator` de coleção interna recusa com a alternativa.
#[test]
fn reading_iterator_of_a_builtin_collection_is_refused() {
    let fonte = "void main() { var xs = <int>[1]; print(xs.iterator); }";
    rejeita(
        fonte,
        "reading '.iterator' of a built-in collection is unsupported in this subset: use 'for (var x in ...)', which lowers to the JavaScript iteration protocol; a user class can still declare 'Iterator<T> get iterator' and implement Iterator itself",
        trecho(fonte, "xs.iterator"),
    );
}

// ---------------------------------------------------------------------------
// Map e Set
// ---------------------------------------------------------------------------

/// A ordem de iteração de `Map` é a de inserção. Saída do oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn map_iteration_order_is_insertion_order() {
    let saida = execute(
        "void main() {
  var m = <String, int>{};
  m['b'] = 2;
  m['a'] = 1;
  m['c'] = 3;
  print(m);
  print(m.length);
  print(m.isEmpty);
  print(m.isNotEmpty);
  print(m.keys);
  print(m.values);
  print(m.keys.toList());
  print(m.values.toList());
  print(m.containsKey('a'));
  print(m.containsKey('z'));
  print(m.containsValue(3));
  print(m['z']);
  print(m.putIfAbsent('d', () => 4));
  print(m.putIfAbsent('a', () => 99));
  print(m);
  print(m.remove('b'));
  print(m.remove('zz'));
  print(m);
  m.addAll(<String, int>{'e': 5});
  print(m);
  m.forEach((k, v) { print('$k=$v'); });
  m.clear();
  print(m);
}",
    );
    assert_eq!(
        saida,
        "{b: 2, a: 1, c: 3}
3
false
true
(b, a, c)
(2, 1, 3)
[b, a, c]
[2, 1, 3]
true
false
true
null
4
1
{b: 2, a: 1, c: 3, d: 4}
2
null
{a: 1, c: 3, d: 4}
{a: 1, c: 3, d: 4, e: 5}
a=1
c=3
d=4
e=5
{}
"
    );
}

/// A ordem de iteração de `Set` é a de inserção. Saída do oráculo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn set_iteration_order_is_insertion_order() {
    let saida = execute(
        "void main() {
  var s = <int>{3, 1, 2};
  print(s);
  print(s.length);
  print(s.contains(1));
  print(s.add(1));
  print(s.add(9));
  print(s);
  print(s.remove(3));
  print(s);
  print(s.toList());
  print(s.isEmpty);
  s.addAll(<int>{7});
  print(s);
  s.clear();
  print(s);
  print(<int>{1, 2}.join(','));
}",
    );
    assert_eq!(
        saida,
        "{3, 1, 2}
3
true
false
true
{3, 1, 2, 9}
true
{1, 2, 9}
[1, 2, 9]
false
{1, 2, 9, 7}
{}
1,2
"
    );
}

/// `Map.remove` devolve valor anulável, mesmo com valor não anulável.
#[test]
fn map_remove_is_nullable() {
    aceita("void main() { var m = <String, int>{'a': 1}; int? v = m.remove('a'); print(v); }");
    rejeita_mensagem(
        "void main() { var m = <String, int>{'a': 1}; int v = m.remove('a'); print(v); }",
        "Type mismatch: expected Int, found NullableInt",
    );
}

/// `entries` recusa nomeando o tipo que falta e a alternativa.
#[test]
fn map_entries_is_refused() {
    let fonte = "void main() { var m = <String, int>{}; print(m.entries); }";
    rejeita(
        fonte,
        "'entries' is unsupported because its result type is MapEntry<K, V>, which this subset does not model; iterate 'keys' and index the map, or call 'forEach((k, v) { ... })'",
        trecho(fonte, "m.entries"),
    );
}

/// Um membro inexistente de `Map` nomeia o tipo com seus argumentos.
#[test]
fn unknown_map_member_names_the_type() {
    let fonte = "void main() { var m = <String, int>{}; print(m.tamanho); }";
    rejeita(
        fonte,
        "'Map<String, int>' declares no member 'tamanho' in this subset; the recognized members are listed in docs/NUCLEO.md",
        trecho(fonte, "m.tamanho"),
    );
}

// ---------------------------------------------------------------------------
// Formas cruas e emissão
// ---------------------------------------------------------------------------

/// `List`, `Set` e `Iterable` sem argumento de tipo são aceitos.
///
/// `Map` cru é a exceção honesta: a leitura seria `Map<Object?, Object?>`, mas
/// o subconjunto só emite mapas de chave `String` ("Only String Map keys are
/// supported"), então aceitar a anotação seria prometer o que a emissão não
/// cumpre. A forma crua de `Map` é recusada com a razão, não com "tipo
/// desconhecido".
#[test]
fn raw_collection_types_are_accepted_as_object_nullable() {
    aceita(
        "void main() {
  List xs = <int>[1];
  Set ys = <int>{1};
  Iterable it = <int>[1];
  print(xs.length + ys.length + it.length);
}",
    );
    let mapa_cru = "void main() {
  Map m = <String, int>{'a': 1};
  print(m.length);
}";
    rejeita_mensagem(mapa_cru, "Only String Map keys are supported");
    // A leitura é `Object?`, então o elemento precisa de `as` — mais restrita
    // que o `dynamic` do Dart, nunca mais permissiva.
    rejeita_mensagem(
        "void main() { List xs = <int>[1]; int a = xs.first; print(a); }",
        "Type mismatch: expected Int, found NullableObject",
    );
}

/// Programa sem membro de biblioteca sai sem auxiliar nenhum do núcleo.
#[test]
fn a_program_without_library_members_pays_nothing() {
    let js = javascript("void main() { var a = 1; var b = 2; print(a + b); }");
    assert!(!js.contains("$dartforge"), "{js}");
}

/// Cada auxiliar entra só quando é usado, e sempre na mesma ordem.
#[test]
fn helpers_are_emitted_only_when_used_and_in_a_stable_order() {
    let js = javascript("void main() { print('abc'.substring(1)); }");
    assert!(js.contains("$dartforgeSubstring"), "{js}");
    assert!(!js.contains("$dartforgeGcd"), "{js}");
    // A ordem alfabética mantém a saída byte a byte igual entre execuções.
    let de_novo = javascript("void main() { print('abc'.substring(1)); }");
    assert_eq!(js, de_novo);
    let faixa = js.find("$dartforgeFaixa(nome").expect("auxiliar de faixa");
    let sub = js
        .find("$dartforgeSubstring(s,")
        .expect("auxiliar de substring");
    assert!(faixa < sub, "{js}");
}

/// `s.length` sai como propriedade do JavaScript, sem prefixo `$df_`.
#[test]
fn string_length_is_a_plain_javascript_property() {
    let js = javascript("void main() { var s = 'a'; print(s.length); }");
    assert!(js.contains("$df_s.length"), "{js}");
    assert!(!js.contains("$df_length"), "{js}");
}

/// Extensão declarada sobre `String` continua resolvendo e tem precedência zero.
#[test]
fn an_extension_on_string_still_resolves_for_names_outside_the_table() {
    let js = javascript(
        "extension Grito on String { String bradar() { return this + '!'; } }
void main() { print('a'.bradar()); }",
    );
    assert!(js.contains("$dartforgeExtension"), "{js}");
}

/// Auxiliar de teste: recorta o span de um prefixo dentro de outro trecho.
trait InicioDe {
    /// Devolve o span de `needle` começando no início deste span.
    fn into_start_of(self, needle: &str) -> Span;
}

impl InicioDe for Span {
    fn into_start_of(self, needle: &str) -> Span {
        Span {
            start: self.start,
            end: self.start + needle.len(),
        }
    }
}
