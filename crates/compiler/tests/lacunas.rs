//! Lacunas fechadas contra o corpus de produção, e os limites que sobraram.
//!
//! Este arquivo cobre as construções que a aferição de `docs/CORPUS-REAL.md`
//! apontou no pacote `pdf` 3.13.1 e que este subconjunto passou a aceitar:
//! `const` sem anotação, anotação em parâmetro, `mixin on`, `assert` na lista de
//! inicialização, construtores redirecionadores, `factory` com metadados e
//! incremento/decremento em posição de expressão.
//!
//! O oráculo é o **Dart SDK 3.6.2** (`dart` no PATH) e o **Dart SDK 3.13.4**
//! (`D:/DartSDKs/3.13.4/dart-sdk/bin/dart.exe`). [`FIXTURE`] foi executada com
//! `dart run --enable-asserts` nos dois, que produzem o mesmo texto; a saída em
//! [`FIXTURE_STDOUT`] é esse texto copiado sem edição.
//!
//! O contrato completo e cada limite que permanece estão em `docs/LACUNAS.md`.
use dartforge_compiler::compile;

/// Programa que exercita as sete lacunas de uma vez.
///
/// Cada construção é uma das que o aferidor de `docs/CORPUS-REAL.md` contou
/// contra o pacote `pdf`, na forma em que ele as encontrou — `const kIndentSize
/// = 2;` inclusive, que é o exemplo citado naquele documento.
const FIXTURE: &str = r#"const kIndentSize = 2;
const kName = 'pdf';
const kFlag = true;
const kRatio = 1.5;
const kSum = kIndentSize + 3;

class Base {
  int valor = 21;
  int descreve() => valor;
}

mixin Medida on Base {
  int dobro() => valor * 2;
  int viaSuper() => descreve() + 1;
}

class Derivada extends Base with Medida {}

class Positivo {
  final int x;
  Positivo(this.x) : assert(x > 0);
  Positivo.um() : this(1);
  Positivo.doisPassos() : this.um();
}

class Alvo {
  final int y;
  Alvo.direto() : y = 7;
  factory Alvo.via() = Alvo;
  Alvo() : y = 9;
}

class ComCorpo {
  int z;
  ComCorpo(this.z) {
    print('corpo $z');
  }
  ComCorpo.redir() : this(5);
}

class Estatico {
  static const kMax = 255;
  static const kNome = 'estatico';
}

void anotado(@Deprecated('use outro') String? x, {@Deprecated('idem') int n = 0}) {
  print('anotado $x $n');
}

void main() {
  print(kIndentSize);
  print(kName);
  print(kFlag);
  print(kRatio);
  print(kSum);
  print(Estatico.kMax);
  print(Estatico.kNome);
  print(Derivada().dobro());
  print(Derivada().viaSuper());
  List<int> a = [10, 20, 30];
  int i = 0;
  print(a[i++]);
  print(i);
  int y = 5;
  int x = y++;
  print('$x $y');
  int w = 5;
  int v = ++w;
  print('$v $w');
  int d = 3;
  print(a[--d]);
  print(d);
  int e = 0;
  a[e++] = 99;
  print(a[0]);
  print(e);
  print(Positivo(3).x);
  print(Positivo.um().x);
  print(Positivo.doisPassos().x);
  print(Alvo.via().y);
  print(Alvo.direto().y);
  print(ComCorpo.redir().z);
  anotado(null);
  anotado('ok', n: 4);
}
"#;

/// Saída de `dart run --enable-asserts` para [`FIXTURE`], nos SDKs 3.6.2 e 3.13.4.
const FIXTURE_STDOUT: &str = "2\npdf\ntrue\n1.5\n5\n255\nestatico\n42\n22\n10\n1\n5 6\n6 6\n30\n2\n99\n1\n3\n1\n1\n9\n7\ncorpo 5\n5\nanotado null 0\nanotado ok 4\n";

/// Executa um módulo em Node e devolve `(sucesso, stdout, stderr)`.
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

/// Compila uma fonte aceita e devolve o módulo JavaScript emitido.
fn javascript(source: &str) -> String {
    compile(source).unwrap_or_else(|erro| panic!("{source}: {}", erro.message))
}

/// Confere mensagem e intervalo exatos de um programa recusado.
///
/// O intervalo é conferido pelo **texto** que ele cobre: `trecho` é procurado na
/// fonte e o span precisa casar com ele byte a byte. Assim o teste falha quando
/// o span escorrega, e não apenas quando a mensagem muda.
fn rejeita(source: &str, message: &str, trecho: &str) {
    let erro = compile(source).expect_err(source);
    assert_eq!(erro.message, message, "{source}");
    let inicio = source.find(trecho).unwrap_or_else(|| panic!("{trecho}"));
    assert_eq!(
        (erro.span.start, erro.span.end),
        (inicio, inicio + trecho.len()),
        "span cobre {:?} em {source}",
        &source[erro.span.start.min(source.len())..erro.span.end.min(source.len())]
    );
}

/// A fixture inteira produz em Node exatamente a saída dos dois SDKs Dart.
#[test]
#[ignore = "requer Node.js no PATH"]
fn the_fixture_matches_the_dart_sdk_output_in_node() {
    let (ok, stdout, stderr) = node(&javascript(FIXTURE));
    assert!(ok, "{stderr}");
    assert_eq!(stdout, FIXTURE_STDOUT);
}

// --- `const` e `static const` sem anotação de tipo -------------------------

/// O tipo de um `const` sem anotação vem do inicializador constante.
#[test]
fn an_untyped_const_takes_its_type_from_the_initializer() {
    for fonte in [
        "const k = 2; void main(){ int v = k; print(v); }",
        "const k = 'pdf'; void main(){ String v = k; print(v); }",
        "const k = true; void main(){ bool v = k; print(v); }",
        "const k = 1.5; void main(){ double v = k; print(v); }",
        "const a = 2; const b = a + 3; void main(){ int v = b; print(v); }",
        "const k = <int>[1, 2]; void main(){ print(k.length); }",
        "class E { static const kMax = 255; } void main(){ int v = E.kMax; print(v); }",
        "class E { static const kNome = 'e'; } void main(){ String v = E.kNome; print(v); }",
    ] {
        javascript(fonte);
    }
}

/// O tipo deduzido é o do inicializador, não `dynamic`: o uso errado é recusado.
#[test]
fn the_inferred_const_type_still_checks_its_uses() {
    let erro = compile("const k = 2; void main(){ String v = k; print(v); }")
        .expect_err("int não é String");
    assert_eq!(erro.message, "Type mismatch: expected String, found Int");
}

// --- anotação em parâmetro -------------------------------------------------

/// `@Deprecated` e os metadados sem semântica são aceitos em parâmetro.
#[test]
fn a_parameter_accepts_deprecated_and_semantics_free_metadata() {
    for fonte in [
        "void f(@Deprecated('use outro') String? x) { print(x); } void main(){ f(null); }",
        "void f({@Deprecated('idem') int n = 0}) { print(n); } void main(){ f(); }",
        "void f(@internal int n) { print(n); } void main(){ f(1); }",
        "class C { void m(@Deprecated('x') int n) { print(n); } } void main(){ C().m(1); }",
    ] {
        javascript(fonte);
    }
}

// --- `mixin on` ------------------------------------------------------------

/// Dentro de `mixin M on Base`, os membros de `Base` estão visíveis.
#[test]
fn a_mixin_on_constraint_exposes_the_members_of_the_constraint() {
    let js = javascript(
        "class Base { int valor = 21; int descreve() => valor; }
         mixin M on Base { int dobro() => valor * 2; int viaMetodo() => descreve() + 1; }
         class C extends Base with M {}
         void main(){ print(C().dobro()); print(C().viaMetodo()); }",
    );
    assert!(js.contains("$df_valor"), "{js}");
}

/// A restrição `on` é cumprida pela cadeia de superclasses, não por `implements`.
#[test]
fn a_mixin_on_constraint_is_checked_against_the_superclass_chain() {
    javascript(
        "class Base { int v = 1; }
         mixin M on Base { int d() => v; }
         class C extends Base with M {}
         void main(){ print(C().d()); }",
    );
    // Uma classe que apenas declara a mesma forma não satisfaz a restrição: a
    // garantia de `on` é que os membros de `Base` existem em execução.
    rejeita(
        "class Base { int v = 1; }
class Outra { int v = 1; }
mixin M on Base { int d() => v; }
class C extends Outra with M {}
void main(){ print(C().d()); }",
        "Mixin 'M' constrains on 'Base', which is not in the superclass chain here",
        "class C extends Outra with M {}",
    );
}

// --- `assert` na lista de inicialização ------------------------------------

/// A asserção da lista roda antes do corpo e antes de `super`.
#[test]
fn an_initializer_list_assert_runs_before_the_body_and_before_super() {
    let js = javascript(
        "class C { final int x; C(this.x) : assert(x > 0) { print('corpo'); } }
         void main(){ print(C(1).x); }",
    );
    let asercao = js
        .find("throw $dartforgeAssertionError")
        .expect("asserção emitida");
    let corpo = js.find("corpo").expect("corpo emitido");
    assert!(asercao < corpo, "{js}");
}

/// A asserção da lista dispara quando a condição é falsa, e passa quando é verdadeira.
#[test]
#[ignore = "requer Node.js no PATH"]
fn an_initializer_list_assert_throws_in_node_exactly_when_it_fails() {
    let fonte = "class C { final int x; C(this.x) : assert(x > 0); }
                 void main(){ print(C(3).x); }";
    let (ok, stdout, stderr) = node(&javascript(fonte));
    assert!(ok, "{stderr}");
    assert_eq!(stdout, "3\n");
    let falha = "class C { final int x; C(this.x) : assert(x > 0); }
                 void main(){ print(C(0).x); }";
    let (ok, _, stderr) = node(&javascript(falha));
    assert!(!ok, "a asserção tinha de disparar");
    assert!(stderr.contains("Failed assertion"), "{stderr}");
}

/// A asserção lê o formal `this.campo` pelo nome escrito, como no Dart.
#[test]
fn an_initializer_list_assert_reads_an_initializing_formal_by_its_written_name() {
    let js = javascript(
        "class C { final int x; C(this.x) : assert(x > 0); } void main(){ print(C(1).x); }",
    );
    assert!(js.contains("const $df_x = $dartforgeFormal0;"), "{js}");
}

/// `assert` e `campo = valor` são entradas da mesma lista, avaliadas na ordem escrita.
#[test]
fn asserts_and_field_entries_keep_the_written_order() {
    let js = javascript(
        "class C { int a; int b; C(int v) : a = v, assert(v > 0), b = v + 1; }
         void main(){ print(C(1).b); }",
    );
    // Cada campo é escrito duas vezes: o `null` da declaração e a entrada da
    // lista. A última escrita de `a` precede a asserção e a última de `b` a
    // segue, que é a ordem escrita.
    let primeiro = js.rfind("$dartforgeThis.$df_a").expect("campo a");
    let asercao = js.find("throw $dartforgeAssertionError").expect("asserção");
    let segundo = js.rfind("$dartforgeThis.$df_b").expect("campo b");
    assert!(primeiro < asercao && asercao < segundo, "{js}");
}

// --- construtores redirecionadores -----------------------------------------

/// `C.nomeado() : this(0)` delega e não executa corpo próprio.
#[test]
fn a_redirecting_constructor_delegates_without_running_its_own_body() {
    let js = javascript(
        "class C { int z; C(this.z) { print('corpo'); } C.redir() : this(5); }
         void main(){ print(C.redir().z); }",
    );
    // A função de inicialização do redirecionador chama a do alvo com o
    // argumento escrito e não faz mais nada: nem campo, nem corpo.
    let inicio = js
        .find("$df_redir($dartforgeThis) {")
        .expect("função de inicialização do redirecionador");
    let trecho = &js[inicio..];
    let trecho = &trecho[..trecho
        .find(
            "
}
",
        )
        .expect("fim da função")];
    assert!(trecho.contains("($dartforgeThis, 5);"), "{trecho}");
    assert!(!trecho.contains("$dartforgeThis.$df_z"), "{trecho}");
    assert!(!trecho.contains("corpo"), "{trecho}");
}

/// A cadeia de redirecionamentos chega ao construtor designado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn a_redirection_chain_reaches_the_designated_constructor_in_node() {
    let js = javascript(
        "class C { final int x; C(this.x); C.um() : this(1); C.dois() : this.um(); }
         void main(){ print(C.dois().x); print(C.um().x); print(C(7).x); }",
    );
    let (ok, stdout, stderr) = node(&js);
    assert!(ok, "{stderr}");
    assert_eq!(stdout, "1\n1\n7\n");
}

/// `factory C.x() = Outra;` repassa os argumentos e devolve a instância do alvo.
#[test]
#[ignore = "requer Node.js no PATH"]
fn a_redirecting_factory_forwards_to_its_target_in_node() {
    let js = javascript(
        "class A { final int y; A(this.y); A.direto() : y = 7; factory A.via(int v) = A; }
         void main(){ print(A.via(3).y); print(A.direto().y); }",
    );
    let (ok, stdout, stderr) = node(&js);
    assert!(ok, "{stderr}");
    assert_eq!(stdout, "3\n7\n");
}

// --- `factory` com metadados -----------------------------------------------

/// `@Deprecated` e os metadados sem semântica são aceitos numa fábrica.
#[test]
fn a_factory_accepts_deprecated_and_semantics_free_metadata() {
    for fonte in [
        "class A { final int y; A(this.y); @Deprecated('use A') factory A.via(int v) = A; }
         void main(){ print(A.via(1).y); }",
        "class A { final int y; A(this.y); @protected factory A.via(int v) = A; }
         void main(){ print(A.via(1).y); }",
        "class A { final int y; A(this.y); @visibleForTesting factory A.via(int v) = A; }
         void main(){ print(A.via(1).y); }",
    ] {
        javascript(fonte);
    }
}

// --- incremento e decremento como expressão --------------------------------

/// `a[i++]` avalia `i` uma única vez e indexa com o valor **anterior**.
#[test]
fn an_index_with_a_postfix_increment_uses_the_previous_value_once() {
    let js =
        javascript("void main(){ List<int> a = [10, 20]; int i = 0; print(a[i++]); print(i); }");
    // `i` aparece uma vez só dentro do índice: nada reavalia o operando.
    assert!(js.contains("$dartforgeIndex($df_a,($df_i++))"), "{js}");
}

/// A forma pós-fixa produz o valor anterior e a prefixa o já atualizado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn postfix_yields_the_previous_value_and_prefix_the_updated_one_in_node() {
    let js = javascript(
        "void main(){
           List<int> a = [10, 20, 30];
           int i = 0;
           print(a[i++]);
           print(i);
           int y = 5;
           int x = y++;
           print('$x $y');
           int w = 5;
           int v = ++w;
           print('$v $w');
           int d = 3;
           print(a[--d]);
           print(d);
           int e = 0;
           a[e++] = 99;
           print(a[0]);
           print(e);
         }",
    );
    let (ok, stdout, stderr) = node(&js);
    assert!(ok, "{stderr}");
    // Os mesmos 8 valores que `dart run` imprime para este trecho.
    assert_eq!(stdout, "10\n1\n5 6\n6 6\n30\n2\n99\n1\n");
}

// --- limites que permanecem, com mensagem e span exatos ---------------------

/// Um `const` sem anotação cujo inicializador não decide o tipo é recusado.
#[test]
fn an_untyped_const_that_the_initializer_does_not_decide_is_rejected() {
    rejeita(
        "int leia(int n) => n; const k = leia(1); void main(){ print(k); }",
        "a const declaration without a type annotation takes its type from the initializer, and this initializer does not decide it syntactically: only literals, constant operators over them, collection literals with a written or uniform element type, constructor invocations and references to const declarations written earlier are inferred here; write the type before the name",
        "leia(1)",
    );
}

/// `on` só existe em declaração de mixin, e só com uma restrição.
#[test]
fn the_on_constraint_belongs_to_a_mixin_and_accepts_exactly_one_supertype() {
    rejeita(
        "class Base { int v = 1; } class C on Base {} void main(){}",
        "only a mixin declaration accepts an on constraint",
        "on",
    );
    rejeita(
        "class A {} class B {} mixin M on A, B {} void main(){}",
        "this subset accepts a single on constraint per mixin: several constraints would need a synthesized intersection type to resolve members against; declare the shared supertype and constrain on it",
        ",",
    );
}

/// Um redirecionador delega inteiramente: nada mais cabe na mesma declaração.
#[test]
fn a_redirecting_constructor_refuses_a_body_fields_super_and_const() {
    rejeita(
        "class C { int x = 0; C(); C.r() : this() { print('x'); } } void main(){ C.r(); }",
        "a redirecting constructor delegates entirely and cannot declare a body",
        "this()",
    );
    rejeita(
        "class C { int x = 0; C(); C.r() : x = 1, this(); } void main(){ C.r(); }",
        "a redirecting constructor delegates entirely: it cannot also initialize fields or call super; move those to the target constructor",
        "this()",
    );
    rejeita(
        "class C { C(); C.r() : this(), this(); } void main(){ C.r(); }",
        "the redirection must be the last entry of an initializer list",
        "this()",
    );
    rejeita(
        "class C { final int x; const C(this.x); const C.r() : this(1); } void main(){}",
        "a const redirecting constructor is unsupported: the canonical instance is built from a single recipe of fields, and following a redirection would need a second recipe per target; declare the const constructor that initializes the fields directly",
        "this(1)",
    );
    rejeita(
        "class C { int x; C(int v) : x = v; C.r(this.x) : this(1); } void main(){ C.r(2); }",
        "A redirecting constructor cannot declare the initializing formal 'this.x': it delegates the whole initialization to the target; declare a plain parameter and pass it in the redirection",
        "this.x",
    );
}

/// Um redirecionamento precisa de alvo existente, sem ciclo e com aridade certa.
#[test]
fn a_redirection_requires_an_existing_acyclic_target() {
    rejeita(
        "class C { C(); C.r() : this.ausente(); } void main(){ C.r(); }",
        "Unknown redirected constructor 'C.ausente'",
        "this.ausente()",
    );
    rejeita(
        "class C { C(); C.a() : this.b(); C.b() : this.a(); } void main(){ C.a(); }",
        "Redirecting constructors form a cycle",
        "this.b()",
    );
    rejeita(
        "class C { C(int a); C.r() : this(); } void main(){ C.r(); }",
        "Incorrect redirected constructor argument count",
        "this()",
    );
}

/// Anotação fora da lista tolerada é recusada onde está escrita.
#[test]
fn an_annotation_outside_the_tolerated_list_is_rejected_where_it_is_written() {
    rejeita(
        "void f(@override int x) { print(x); } void main(){ f(1); }",
        "this annotation targets a declaration, not a parameter; a parameter accepts Deprecated and the semantics-free annotations of package:meta",
        "@override",
    );
    rejeita(
        "class A { final int y; A(this.y); @override factory A.via(int v) = A; }
         void main(){ print(A.via(1).y); }",
        "this annotation targets a class or a method, not a factory; a factory accepts Deprecated and the semantics-free annotations of package:meta",
        "@override",
    );
}

/// `@pragma` dirige o compilador: tolerá-la em silêncio prometeria honrá-la.
#[test]
fn pragma_keeps_its_own_diagnostic_instead_of_being_ignored() {
    rejeita(
        "@pragma('vm:prefer-inline') int f() => 1; void main(){ print(f()); }",
        "@pragma directs the compiler and cannot be ignored; it is not implemented",
        "@pragma('vm:prefer-inline')",
    );
}

/// `++` e `--` exigem um nome simples com tipo numérico e gravável.
///
/// Os casos usam `print(...)` porque só a **posição de expressão** chega a
/// `ExprKind::Increment`: `x++;` como instrução isolada é reescrito pelo parser
/// como `x = x + 1` e tem os diagnósticos de atribuição.
#[test]
fn increment_requires_a_writable_numeric_simple_target() {
    rejeita(
        "void main(){ List<int> a = [1]; print(a[0]++); }",
        "`++` accepts only a simple variable as target: updating `a[i]` or `o.field` has to evaluate receiver and index exactly once, which needs temporaries this subset does not emit yet; write `a[i] = a[i] + 1` as a statement instead",
        "a[0]",
    );
    rejeita(
        "void main(){ String s = 'a'; print(s++); }",
        "`++` requires an int, double or num target: Dart defines it as `s = s + 1`, which needs the numeric operator",
        "s++",
    );
    rejeita(
        "void main(){ final int x = 1; print(x++); }",
        "Cannot apply `++` to final variable 'x'",
        "x++",
    );
    rejeita(
        "void main(){ late int x; x = 1; print(x++); }",
        "`++` on the late declaration 'x' is unsupported: it reads and writes the same name, and a late read is a checked call rather than an assignment target; write `x = x + 1` as a statement",
        "x++",
    );
    // A forma prefixa recusa o mesmo alvo composto, com o operador na mensagem.
    let erro = compile("void main(){ List<int> v = [1]; print(--v[0]); }")
        .expect_err("alvo composto recusado");
    assert_eq!(
        erro.message,
        "`--` accepts only a simple variable as target: updating `a[i]` or `o.field` has to evaluate receiver and index exactly once, which needs temporaries this subset does not emit yet; write `a[i] = a[i] - 1` as a statement instead"
    );
}
