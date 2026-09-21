//! Granularidade de tipos e declarações: genéricos escritos, `late` e `typedef`.
//!
//! O contrato de `late` neste subconjunto vem do oráculo, não de suposição: as
//! mensagens e a ordem de avaliação foram conferidas com `dart run` no SDK 3.6.2
//! (no PATH) e no 3.13.4 (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`), que
//! concordam em todos os casos deste arquivo. Os limites conhecidos — erasure de
//! classe genérica e recusa de `late` com inicializador — estão em docs/TIPOS.md.
use dartforge_compiler::compile;

/// Programa que exercita as três frentes de uma vez.
///
/// A saída `3 10 2 2 1 1 42 42 9 5 3` é a de `dart run` nos dois SDKs, copiada
/// sem edição.
const FIXTURE: &str = r#"class Caixa<T extends int> {
  T v;
  Caixa(this.v);
}
class Par<A extends int, B extends num> {
  A a;
  B b;
  Par(this.a, this.b);
}
typedef F = int Function(int);
typedef int G(int x);
typedef M = Map<String, List<int>>;
int aplica(F f, int x) => f(x);
void main() {
  late int x;
  late final int y;
  x = 1;
  y = 2;
  print(x + y);
  x = 10;
  print(x);
  Map<String, List<int>> m = {'a': [1, 2]};
  print(m['a']!.length);
  List<List<int>> ll = [[1], [2, 3]];
  print(ll[1].length);
  Set<Map<String, int>> s = {{'k': 1}};
  print(s.length);
  List<int>? opcional = [7];
  print(opcional!.length);
  F f = (int v) => v + 1;
  print(aplica(f, 41));
  G g = (int v) => v * 2;
  print(g(21));
  M mm = {'z': [9]};
  print(mm['z']!.first);
  print(Caixa<int>(5).v);
  print(Par<int, double>(3, 4.5).a);
}
"#;

/// Saída do oráculo Dart para [`FIXTURE`], nos SDKs 3.6.2 e 3.13.4.
const FIXTURE_STDOUT: &str = "3\n10\n2\n2\n1\n1\n42\n42\n9\n5\n3\n";

/// Executa um módulo em Node e devolve `(status, stdout, stderr)`.
fn node(js: &str) -> (bool, String, String) {
    let saida = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", js])
        .output()
        .expect("Node.js necessário");
    (
        saida.status.success(),
        String::from_utf8_lossy(&saida.stdout).replace("\r\n", "\n"),
        String::from_utf8_lossy(&saida.stderr).replace("\r\n", "\n"),
    )
}

/// Confere mensagem e intervalo exatos de um programa recusado.
fn rejeita(source: &str, message: &str, trecho: &str) {
    let erro = compile(source).expect_err(source);
    assert_eq!(erro.message, message, "{source}");
    let inicio = source.find(trecho).expect(trecho);
    assert_eq!(
        (erro.span.start, erro.span.end),
        (inicio, inicio + trecho.len()),
        "{source}"
    );
}

/// A fixture inteira produz em Node a saída que os dois SDKs Dart produzem.
#[test]
#[ignore = "requer Node.js no PATH"]
fn the_fixture_matches_the_dart_sdk_output_in_node() {
    let (ok, stdout, stderr) = node(&compile(FIXTURE).unwrap());
    assert!(ok, "{stderr}");
    assert_eq!(stdout, FIXTURE_STDOUT);
}

/// Genéricos escritos, aninhados e anuláveis chegam ao emissor sem recusa.
#[test]
fn written_generic_annotations_are_accepted_in_every_position() {
    for fonte in [
        "void main() { Map<String, List<int>> a = {}; print(a.length); }",
        "void main() { List<List<List<int>>> a = []; print(a.length); }",
        "void main() { Set<Map<String, int>> a = {}; print(a.length); }",
        "void main() { Iterable<Map<String, int>> a = []; print(a.length); }",
        "Map<String, List<int>> f() => {}; void main() { print(f().length); }",
        "int f(Map<String, List<int>> a) => a.length; void main() { print(f({})); }",
        "class C { Map<String, List<int>> v = {}; } void main() { print(C().v.length); }",
        "Future<List<int>> f() async => [1]; void main() { f(); }",
        "void main() { List<int>? a = [1]; print(a!.length); }",
    ] {
        compile(fonte).unwrap_or_else(|e| panic!("{fonte}: {}", e.message));
    }
}

/// `T?` é idempotente: o bound implícito de um parâmetro de classe já é `Object?`.
///
/// Sem isso, `class A<T> { T? v; }` — a forma mais comum de campo genérico
/// anulável — era recusada com "type cannot be nullable".
#[test]
fn a_nullable_class_type_parameter_is_idempotent() {
    compile("class A<T> { T? v; } class B<T> extends A<T> {} void main() { B<int>(); }").unwrap();
}

/// `late` sem inicializador nasce no sentinela e a leitura prova a inicialização.
#[test]
fn late_without_an_initializer_starts_at_the_sentinel() {
    let js = compile("void main(){late int x; x = 1; print(x);}").unwrap();
    assert!(js.contains("const $dartforgeLate = Symbol(\"late\")"));
    assert!(js.contains("let $df_x = $dartforgeLate;"));
    assert!(js.contains("$dartforgeLateRead($df_x, \"Local\", \"x\")"));
    // Um `late` não-final aceita qualquer número de escritas: o auxiliar existe
    // no prelúdio, mas nenhuma escrita o chama.
    assert!(!js.contains("= $dartforgeLateWrite("));
}

/// `late final` aceita a primeira atribuição e prova em execução que é a única.
///
/// Recusá-la em tempo de compilação — como este subconjunto fazia — negava a
/// única escrita que Dart permite numa declaração `late final` sem inicializador.
#[test]
fn late_final_accepts_exactly_one_assignment() {
    let js = compile("void main(){late final int x; x = 1; print(x);}").unwrap();
    assert!(js.contains("$dartforgeLateWrite($df_x, 1, \"Local\", \"x\")"));
    let campo =
        compile("class C { late final int v; } void main(){ var c = C(); c.v = 1; }").unwrap();
    // A escrita em `receptor.campo` passa o objeto e a chave: o receptor pode
    // ter efeito colateral e é avaliado uma única vez.
    assert!(campo.contains("$dartforgeLateSet("));
}

/// Nada é emitido para `late` num programa que não o declara.
#[test]
fn a_program_without_late_pays_nothing() {
    let js = compile("void main(){int x = 1; print(x);}").unwrap();
    assert!(!js.contains("$dartforgeLate"));
}

/// Ler um local `late` antes de escrever lança com a mensagem exata do SDK.
#[test]
#[ignore = "requer Node.js no PATH"]
fn reading_an_unwritten_late_local_throws_like_dart() {
    let (ok, _, stderr) = node(&compile("void main(){late int x; print(x);}").unwrap());
    assert!(!ok, "a leitura antes da escrita precisa lançar");
    assert!(
        stderr.contains("LateInitializationError: Local 'x' has not been initialized."),
        "{stderr}"
    );
}

/// Ler um campo `late` antes de escrever lança com a mensagem exata do SDK.
///
/// Oráculo: `LateInitializationError: Field 'campo' has not been initialized.`
#[test]
#[ignore = "requer Node.js no PATH"]
fn reading_an_unwritten_late_field_throws_like_dart() {
    let (ok, _, stderr) =
        node(&compile("class C { late int campo; } void main(){ print(C().campo); }").unwrap());
    assert!(!ok);
    assert!(
        stderr.contains("LateInitializationError: Field 'campo' has not been initialized."),
        "{stderr}"
    );
}

/// Uma variável de topo `late` usa `Field` na mensagem, como o SDK.
#[test]
#[ignore = "requer Node.js no PATH"]
fn an_unwritten_late_top_level_variable_says_field() {
    let (ok, _, stderr) = node(&compile("late int topo; void main(){ print(topo); }").unwrap());
    assert!(!ok);
    assert!(
        stderr.contains("LateInitializationError: Field 'topo' has not been initialized."),
        "{stderr}"
    );
}

/// `late final` atribuído duas vezes lança na segunda, depois de avaliar o valor.
///
/// Oráculo: o lado direito roda antes do lançamento — `dart run` imprime o
/// efeito de `rhs` e só então o erro.
#[test]
#[ignore = "requer Node.js no PATH"]
fn assigning_a_late_final_twice_throws_after_evaluating_the_value() {
    let fonte = "int rhs(int v) { print('rhs'); return v; }
                 class C { late final int campo; }
                 void main() { var c = C(); c.campo = rhs(1); print(c.campo); c.campo = rhs(2); }";
    let (ok, stdout, stderr) = node(&compile(fonte).unwrap());
    assert!(!ok);
    // `rhs` aparece duas vezes: a ordem de Dart avalia o valor e só então lança.
    assert_eq!(stdout, "rhs\n1\nrhs\n");
    assert!(
        stderr.contains("LateInitializationError: Field 'campo' has already been initialized."),
        "{stderr}"
    );
}

/// O receptor de `receptor.campo = valor` é avaliado uma única vez.
#[test]
#[ignore = "requer Node.js no PATH"]
fn the_receiver_of_a_late_final_write_is_evaluated_once() {
    let fonte = "class C { late final int campo; }
                 C registra(C v) { print('receptor'); return v; }
                 void main() { var c = C(); registra(c).campo = 7; print(c.campo); }";
    let (ok, stdout, stderr) = node(&compile(fonte).unwrap());
    assert!(ok, "{stderr}");
    assert_eq!(stdout, "receptor\n7\n");
}

/// `late` com inicializador é recusado: a célula preguiçosa não é emitida.
///
/// Em Dart o inicializador roda na **primeira leitura**, e uma escrita anterior
/// o cancela sem executá-lo. Avaliá-lo na declaração daria a ordem de efeitos
/// errada em silêncio, então a forma é recusada em local, campo e topo.
#[test]
fn late_with_an_initializer_is_rejected_everywhere() {
    const MENSAGEM: &str = "late with an initializer is not supported: in Dart the initializer runs on the first read and a write before that read cancels it; declare `late T name;` and assign before reading";
    rejeita("void main(){late int x = 1; print(x);}", MENSAGEM, "late");
    rejeita(
        "class C { late int v = 1; } void main(){ print(C().v); }",
        MENSAGEM,
        "late",
    );
    rejeita("late int g = 1; void main(){ print(g); }", MENSAGEM, "late");
}

/// `typedef` moderno e clássico resolvem para o tipo subjacente.
#[test]
fn both_typedef_forms_resolve_to_the_aliased_type() {
    for fonte in [
        "typedef F = int Function(int); void main(){ F f = (int x) => x + 1; print(f(1)); }",
        "typedef int G(int x); void main(){ G g = (int x) => x * 2; print(g(3)); }",
        "typedef M = Map<String, List<int>>; void main(){ M m = {}; print(m.length); }",
        "typedef F = List<int> Function(Map<String, int>); void main(){ F f = (Map<String, int> m) => [m.length]; print(f({}).length); }",
    ] {
        compile(fonte).unwrap_or_else(|e| panic!("{fonte}: {}", e.message));
    }
}

/// `C<int>(...)` é construção genérica, com os argumentos validados e apagados.
#[test]
fn explicit_type_arguments_on_a_construction_are_checked_then_erased() {
    let js = compile(
        "class Caixa<T extends int> { T v; Caixa(this.v); } void main(){ print(Caixa<int>(5).v); }",
    )
    .unwrap();
    // Erasure: a construção é a mesma de `Caixa(5)`, sem descritor de `int`.
    assert!(js.contains("$dartforgeNew0(5)"));
    assert_eq!(
        js,
        compile(
            "class Caixa<T extends int> { T v; Caixa(this.v); } void main(){ print(Caixa(5).v); }"
        )
        .unwrap()
    );
}

/// Argumento de tipo fora do bound é recusado, não apagado em silêncio.
///
/// Este é o ponto em que erasure deixaria passar um erro de programa: sem a
/// checagem, `Caixa<String>` compilaria com a mesma representação de
/// `Caixa<int>`.
#[test]
fn a_type_argument_outside_the_bound_is_rejected() {
    const CLASSE: &str = "class Caixa<T extends int> { T v; Caixa(this.v); } ";
    let erro = compile(&format!("{CLASSE}void main(){{ Caixa<String>(1); }}"))
        .expect_err("argumento fora do bound deve ser recusado");
    assert_eq!(erro.message, "Type mismatch: expected Int, found String");
    let aridade = compile(&format!("{CLASSE}void main(){{ Caixa<int, int>(1); }}"))
        .expect_err("aridade errada deve ser recusada");
    assert_eq!(aridade.message, "Incorrect class type argument count");
    let sem_generico = compile("class C { int v; C(this.v); } void main(){ C<int>(1); }")
        .expect_err("classe sem genéricos deve recusar argumentos de tipo");
    assert_eq!(
        sem_generico.message,
        "Class 'C' declares no type parameters"
    );
}

/// `late const` não existe em Dart e continua recusado.
#[test]
fn late_const_stays_rejected() {
    let erro = compile("void main(){late const int x = 1;}").expect_err("late const");
    assert_eq!(erro.message, "late const is not supported");
}
