//! Parâmetros nomeados, opcionais e valores padrão: contrato, limites e emissão.
//!
//! A saída esperada dos programas executáveis foi conferida com `dart run` do
//! SDK Dart 3.6.2 instalado nesta máquina. A única exceção é o parâmetro
//! nomeado privado (`this._x`), recurso de Dart 3.12 que o SDK local rejeita
//! com «A named parameter can't start with an underscore ('_')»: para ele não
//! há oráculo local e o comportamento segue a especificação da linguagem.
use dartforge_compiler::{compile, compile_llvm};

/// Programa de referência; a saída foi copiada de `dart run` sem edição.
const PROGRAMA: &str = r#"
int efeito(int v) {
  print(v);
  return v;
}

int soma(int a, {required int b, int c = 10}) => a + b + c;

void registrar(String m, [int nivel = 2, String? tag]) {
  print(m);
  print(nivel);
  print(tag);
}

class Cliente {
  final String host;
  final int porta;
  Cliente({required this.host, this.porta = 80});
  int descrever({int extra = 1}) => porta + extra;
}

class Base {
  int calcular({required int a, int b = 3}) => a + b;
}

class Derivada extends Base {
  @override
  int calcular({required int a, int b = 3}) => a * b;
}

void main() {
  print(soma(1, b: 2));
  print(soma(1, c: 5, b: 2));
  registrar('a');
  registrar('b', 7);
  registrar('c', 7, 'd');
  var cliente = Cliente(host: 'h');
  print(cliente.host);
  print(cliente.porta);
  print(cliente.descrever());
  print(Cliente(porta: 443, host: 'i').descrever(extra: 2));
  print(Derivada().calcular(a: 4));
  print(soma(efeito(1), c: efeito(2), b: efeito(3)));
}
"#;

/// Saída exata de `dart run` para PROGRAMA, com a ordem de avaliação escrita.
const ESPERADO: &str = "13\n8\na\n2\nnull\nb\n7\nnull\nc\n7\nd\nh\n80\n81\n445\n12\n1\n2\n3\n6\n";

/// Despacho virtual com nomeados: a convenção do objeto vale em toda a cadeia.
const POLIMORFICO: &str = r#"
abstract class Forma {
  int area({int escala = 1});
  int lados([int extra = 0]);
}

class Quadrado extends Forma {
  final int lado;
  Quadrado({required this.lado});
  @override
  int area({int escala = 1}) => lado * lado * escala;
  @override
  int lados([int extra = 0]) => 4 + extra;
}

class Retangulo extends Forma {
  final int a;
  final int b;
  Retangulo({required this.a, this.b = 2});
  @override
  int area({int escala = 1}) => a * b * escala;
  @override
  int lados([int extra = 0]) => 4 + extra;
}

int total(List<Forma> fs, {int fator = 1}) {
  var soma = 0;
  for (var i = 0; i < fs.length; i++) {
    soma += fs[i].area(escala: fator);
  }
  return soma;
}

void main() {
  Forma q = Quadrado(lado: 3);
  Forma r = Retangulo(b: 5, a: 4);
  print(q.area());
  print(q.area(escala: 2));
  print(r.area());
  print(r.lados());
  print(r.lados(3));
  print(total([q, r]));
  print(total([q, r], fator: 10));
}
"#;

/// Saída exata de `dart run` para POLIMORFICO.
const ESPERADO_POLIMORFICO: &str = "9\n18\n20\n4\n7\n29\n290\n";

/// Confere mensagem e span de um programa rejeitado antes da emissão.
///
/// `ancora` é um trecho único da fonte e `trecho` é o texto que o span precisa
/// cobrir, procurado a partir do início da âncora.
fn rejeita_apos(source: &str, message: &str, ancora: &str, trecho: &str) {
    let error = match compile(source) {
        Ok(_) => panic!("deveria falhar: {source}"),
        Err(error) => error,
    };
    assert_eq!(error.message, message, "fonte: {source}");
    let base = source
        .find(ancora)
        .unwrap_or_else(|| panic!("âncora ausente na fonte: {ancora}"));
    let start = base
        + source[base..]
            .find(trecho)
            .unwrap_or_else(|| panic!("trecho ausente na fonte: {trecho}"));
    assert_eq!(
        (error.span.start, error.span.end),
        (start, start + trecho.len()),
        "span apontou {:?} em {source}",
        source.get(error.span.start..error.span.end)
    );
}

/// Caso comum em que a âncora já é o próprio trecho esperado.
fn rejeita(source: &str, message: &str, trecho: &str) {
    rejeita_apos(source, message, trecho, trecho);
}

#[test]
fn nomeados_com_e_sem_padrao_emitem_objeto_desestruturado() {
    let js = compile(
        "int f(int a, {required int b, String c = 'x', int? d}) => a + b;\
         void main(){ print(f(1, c: 'y', b: 2)); }",
    )
    .unwrap();
    assert!(
        js.contains(
            "function $df_f($df_a, {$dfn$b: $df_b, $dfn$c: $df_c = \"x\", $dfn$d: $df_d = null} = {})"
        ),
        "{js}"
    );
    assert!(js.contains("$df_f(1, {$dfn$c: \"y\", $dfn$b: 2})"), "{js}");
}

#[test]
fn posicionais_opcionais_recebem_padrao_ou_null() {
    let js = compile(
        "void f(int a, [int b = 2, int? c]) { print(a); print(b); print(c); }\
         void main(){ f(1); }",
    )
    .unwrap();
    assert!(
        js.contains("function $df_f($df_a, $df_b = 2, $df_c = null)"),
        "{js}"
    );
}

#[test]
fn nomeado_privado_usa_rotulo_sem_sublinhado() {
    // Dart 3.12; o SDK 3.6.2 local rejeita a declaração e não serve de oráculo.
    let js = compile(
        "class Servico { final String _apiKey; Servico({required this._apiKey});\
         String chave() => _apiKey; }\
         void main(){ print(Servico(apiKey: 'k').chave()); }",
    )
    .unwrap();
    assert!(js.contains("$dfn$apiKey: $dartforgeArgument0"), "{js}");
    assert!(js.contains("{$dfn$apiKey: \"k\"}"), "{js}");
}

#[test]
fn fabrica_nomeada_aceita_nomeados() {
    let js = compile(
        "class C { final int v; C(this.v); factory C.criar({int v = 3}) => C(v); }\
         void main(){ print(C.criar().v); print(C.criar(v: 9).v); }",
    )
    .unwrap();
    assert!(js.contains("{$dfn$v: $df_v = 3} = {}"), "{js}");
}

#[test]
fn construtor_com_corpo_recebe_nomeado_na_posicao_declarada() {
    let js = compile(
        "class C { final int a; int b = 0; C(this.a, {int extra = 2}) { b = a + extra; } }         void main(){ print(C(1).b); print(C(1, extra: 5).b); }",
    )
    .unwrap();
    assert!(
        js.contains("function $dartforgeNew0($dartforgeArgument0,{$dfn$extra: $dartforgeArgument1 = 2} = {})"),
        "{js}"
    );
    assert!(
        js.contains("$dartforgeArgument0, $dartforgeArgument1"),
        "{js}"
    );
}

#[test]
fn ordem_de_avaliacao_segue_a_ordem_escrita() {
    // Espelha `print(soma(efeito(1), c: efeito(2), b: efeito(3)))`: 1, 2, 3, 6.
    let js = compile(
        "int efeito(int v){ print(v); return v; }\
         int soma(int a, {required int b, int c = 10}) => a + b + c;\
         void main(){ print(soma(efeito(1), c: efeito(2), b: efeito(3))); }",
    )
    .unwrap();
    let chamada = js
        .split("console.log($df_soma(")
        .nth(1)
        .expect("chamada emitida")
        .to_owned();
    let primeiro = chamada.find("$df_efeito(1)").expect("primeiro efeito");
    let segundo = chamada.find("$df_efeito(2)").expect("segundo efeito");
    let terceiro = chamada.find("$df_efeito(3)").expect("terceiro efeito");
    assert!(primeiro < segundo && segundo < terceiro, "{chamada}");
}

#[test]
fn required_ausente_repetido_e_desconhecido_sao_erros() {
    rejeita(
        "void f({required int a}) {} void main(){ f(); }",
        "Missing required named argument 'a'",
        "f()",
    );
    rejeita(
        "void f({int? a}) {} void main(){ f(a: 1, a: 2); }",
        "Duplicate named argument 'a'",
        "a: 2",
    );
    rejeita(
        "void f({int? a}) {} void main(){ f(b: 1); }",
        "Unknown named argument 'b'",
        "b: 1",
    );
}

#[test]
fn nomeados_precisam_vir_depois_dos_posicionais() {
    rejeita(
        "void f(int a, {int? b}) {} void main(){ f(b: 1, 2); }",
        "positional arguments must precede named arguments",
        "2",
    );
}

#[test]
fn opcional_nao_anulavel_sem_padrao_e_erro_na_declaracao() {
    rejeita(
        "void f({int a}) {} void main(){ f(a: 1); }",
        "Optional named parameter requires a default value or a nullable type",
        "int a",
    );
    rejeita(
        "void f([int a]) {} void main(){ f(1); }",
        "Optional positional parameter requires a default value or a nullable type",
        "int a",
    );
}

#[test]
fn padrao_precisa_ser_constante_escalar_de_um_grupo_opcional() {
    rejeita_apos(
        "int g() => 1; void f({int a = g()}) {} void main(){ f(); }",
        "Const expression: calls, getters and this expression are unsupported",
        "a = g()",
        "g()",
    );
    rejeita_apos(
        "void f({List<int> a = const [1]}) {} void main(){ f(); }",
        "Const expression: list element type must be resolved before evaluation",
        "= const [1]",
        "[1]",
    );
    rejeita(
        "void f({int a = 1 + 2}) {} void main(){ f(); }",
        "A default value must be a literal scalar constant",
        "1 + 2",
    );
    rejeita(
        "void f(int a = 1) {} void main(){ f(1); }",
        "Only optional or named parameters accept a default value",
        "int a = 1",
    );
    rejeita(
        "void f({required int a = 1}) {} void main(){ f(a: 2); }",
        "A required named parameter cannot have a default value",
        "int a = 1",
    );
    rejeita(
        "void f({String a = 1}) {} void main(){ f(); }",
        "Type mismatch: expected String, found Int",
        "1",
    );
}

#[test]
fn rotulo_externo_repetido_na_declaracao_e_erro() {
    rejeita(
        "class C { final int _a; C({required this._a, int a = 0}); int v() => _a; }\
         void main(){ print(C(a: 1).v()); }",
        "Duplicate named parameter 'a'",
        "int a = 0",
    );
}

#[test]
fn sobrescrita_precisa_manter_os_nomeados_do_supertipo() {
    rejeita(
        "class B { int m({required int a}) => a; }\
         class D extends B { @override int m() => 1; }\
         void main(){ print(D().m(a: 1)); }",
        "Override drops required named parameter 'a'",
        "@override int m() => 1;",
    );
    rejeita(
        "class B { int m({int? a}) => 1; }\
         class D extends B { @override int m({int? a, required int b}) => 1; }\
         void main(){ print(D().m()); }",
        "Override adds required named parameter 'b'",
        "@override int m({int? a, required int b}) => 1;",
    );
}

#[test]
fn contextos_restritos_rejeitam_opcionais_e_nomeados() {
    rejeita_apos(
        "void main(){ var f = ({int a = 1}) { print(a); }; f(); }",
        "closures support only required positional parameters",
        "({int a = 1})",
        "{",
    );
    rejeita(
        "extension E on int { int d({int a = 1}) => this + a; }\
         void main(){ print(1.d()); }",
        "Extension methods support only required positional parameters",
        "int a = 1",
    );
    rejeita(
        "T f<T>(T a, {int b = 1}) => a; void main(){ print(f<int>(1)); }",
        "Generic functions support only required positional parameters",
        "int b = 1",
    );
    rejeita_apos(
        "void f(int a, [int b = 1], {int c = 2}) {} void main(){ f(1); }",
        "a signature accepts a single optional or named group",
        "{int c = 2}",
        "{",
    );
}

#[test]
fn backend_nativo_rejeita_parametros_e_argumentos_nomeados() {
    let error = compile_llvm("void f({int a = 1}) { print(a); } void main(){ f(); }").unwrap_err();
    assert_eq!(
        error.message,
        "LLVM AOT ainda não suporta parâmetros opcionais ou nomeados no backend nativo"
    );
}

#[test]
#[ignore = "requer Node.js no PATH"]
fn programa_completo_reproduz_a_saida_do_dart_3_6_2() {
    assert_eq!(executar(&compile(PROGRAMA).unwrap()), ESPERADO);
}

/// Despacho virtual, construtores nomeados e opcionais na mesma execução.
#[test]
#[ignore = "requer Node.js no PATH"]
fn despacho_virtual_com_nomeados_reproduz_a_saida_do_dart_3_6_2() {
    assert_eq!(
        executar(&compile(POLIMORFICO).unwrap()),
        ESPERADO_POLIMORFICO
    );
}

/// Executa o módulo emitido em Node e devolve a saída normalizada em LF.
fn executar(js: &str) -> String {
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", js])
        .output()
        .expect("executar node");
    assert!(
        output.status.success(),
        "{}\n{js}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
}
