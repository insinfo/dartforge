//! `textDocument/prepareRename` e `textDocument/rename` pelas mensagens
//! JSON-RPC: locais, parâmetros, membros com sobrescritas, topo entre
//! arquivos, recusas e código incompleto.

mod comum;

use comum::{Projeto, aplicar, marcar};
use serde_json::{Value, json};

/// Abre `texto` (com o marcador `▮`) e devolve (texto sem marcador, linha, coluna).
fn abrir(p: &mut Projeto, relativo: &str, texto: &str) -> (String, u32, u32) {
    let (texto, linha, coluna) = marcar(texto);
    p.abrir(relativo, &texto);
    (texto, linha, coluna)
}

fn renomear(p: &mut Projeto, relativo: &str, linha: u32, coluna: u32, novo: &str) -> Value {
    p.na_posicao(
        "textDocument/rename",
        relativo,
        linha,
        coluna,
        json!({"newName": novo}),
    )
}

fn erro(resposta: &Value) -> String {
    assert_eq!(resposta["error"]["code"], -32010, "{resposta}");
    resposta["error"]["message"].as_str().unwrap().to_string()
}

#[test]
fn anuncia_prepare_quando_o_cliente_aceita() {
    let mut p = Projeto::novo("renomear-cap");
    let r = p.reiniciar(json!({"capabilities": {"textDocument": {"rename": {"prepareSupport": true}}}}));
    assert_eq!(
        r["result"]["capabilities"]["renameProvider"],
        json!({"prepareProvider": true})
    );
    let r = p.reiniciar(json!({"capabilities": {}}));
    assert_eq!(r["result"]["capabilities"]["renameProvider"], json!(true));
}

#[test]
fn local_e_parametro() {
    let mut p = Projeto::novo("renomear-local");
    let fonte = "int f(int par) {\n  var lo▮cal = par + 1;\n  local += 2;\n  print('$local ${local}');\n  return local;\n}\nint g() { var local = 0; return local; }\n";
    let (texto, l, c) = abrir(&mut p, "lib/a.dart", fonte);
    let uri = p.uri("lib/a.dart");
    let prep = p.na_posicao("textDocument/prepareRename", "lib/a.dart", l, c, json!({}));
    assert_eq!(prep["result"]["placeholder"], "local");
    assert_eq!(
        prep["result"]["range"],
        json!({"start": {"line": 1, "character": 6}, "end": {"line": 1, "character": 11}})
    );
    let r = renomear(&mut p, "lib/a.dart", l, c, "total");
    assert_eq!(
        aplicar(&r["result"], &uri, &texto),
        "int f(int par) {\n  var total = par + 1;\n  total += 2;\n  print('$total ${total}');\n  return total;\n}\nint g() { var local = 0; return local; }\n"
    );
    // Parâmetro, a partir de um uso.
    let r = renomear(&mut p, "lib/a.dart", 1, 17, "base");
    assert!(
        aplicar(&r["result"], &uri, &texto)
            .starts_with("int f(int base) {\n  var local = base + 1;")
    );
    // Espaço em branco: nada a renomear.
    let prep = p.na_posicao("textDocument/prepareRename", "lib/a.dart", 1, 0, json!({}));
    assert_eq!(prep["result"], Value::Null);
}

#[test]
fn parametro_nomeado_renomeia_os_rotulos() {
    let mut p = Projeto::novo("renomear-nomeado");
    let fonte = "void f({int? li▮mite}) { print(limite); }\nvoid g() { f(limite: 1); f(); }\n";
    let (texto, l, c) = abrir(&mut p, "lib/a.dart", fonte);
    let uri = p.uri("lib/a.dart");
    let r = renomear(&mut p, "lib/a.dart", l, c, "teto");
    assert_eq!(
        aplicar(&r["result"], &uri, &texto),
        "void f({int? teto}) { print(teto); }\nvoid g() { f(teto: 1); f(); }\n"
    );
}

#[test]
fn membro_com_sobrescritas_e_usos_por_instancia_em_outro_arquivo() {
    let mut p = Projeto::novo("renomear-membro");
    let base = "class A {\n  void met() {}\n  void usa() { met(); this.met(); }\n}\nclass B extends A {\n  @override\n  void met() { super.met(); }\n}\nclass C implements A {\n  void met() {}\n  void usa() {}\n}\nclass D { void met() {} }\n";
    // O arquivo que usa fica só no disco (fechado).
    let usos =
        "import 'a.dart';\nvoid f(A a, B b, D d) { a.met(); b.met(); d.met(); a..met()..usa(); }\n";
    p.gravar("lib/usos.dart", usos);
    let (texto, _, _) = abrir(
        &mut p,
        "lib/a.dart",
        &base.replacen("void met() { super", "void m▮et() { super", 1),
    );
    // Renomeia a partir da sobrescrita em B.
    let r = renomear(&mut p, "lib/a.dart", 6, 8, "executar");
    let novo = aplicar(&r["result"], &p.uri("lib/a.dart"), &texto);
    assert_eq!(
        novo,
        "class A {\n  void executar() {}\n  void usa() { executar(); this.executar(); }\n}\nclass B extends A {\n  @override\n  void executar() { super.executar(); }\n}\nclass C implements A {\n  void executar() {}\n  void usa() {}\n}\nclass D { void met() {} }\n"
    );
    assert_eq!(
        aplicar(&r["result"], &p.uri("lib/usos.dart"), usos),
        "import 'a.dart';\nvoid f(A a, B b, D d) { a.executar(); b.executar(); d.met(); a..executar()..usa(); }\n"
    );
    // Conflito com um membro existente na família.
    let r = renomear(&mut p, "lib/a.dart", 6, 8, "usa");
    assert_eq!(erro(&r), "Renamed method will shadow method 'A.usa'.", "{r}");
}

#[test]
fn campo_com_getter_setter_this_e_inicializador() {
    let mut p = Projeto::novo("renomear-campo");
    let fonte = "class P {\n  int va▮lor;\n  final int outro;\n  P(this.valor) : outro = 0;\n  P.nomeado({required this.valor}) : outro = 1;\n  P.init(int v) : valor = v, outro = 2;\n}\nvoid f(P p) { p.valor = p.valor + 1; P.nomeado(valor: 3); }\n";
    let (texto, l, c) = abrir(&mut p, "lib/a.dart", fonte);
    let r = renomear(&mut p, "lib/a.dart", l, c, "quantia");
    assert_eq!(
        aplicar(&r["result"], &p.uri("lib/a.dart"), &texto),
        "class P {\n  int quantia;\n  final int outro;\n  P(this.quantia) : outro = 0;\n  P.nomeado({required this.quantia}) : outro = 1;\n  P.init(int v) : quantia = v, outro = 2;\n}\nvoid f(P p) { p.quantia = p.quantia + 1; P.nomeado(quantia: 3); }\n"
    );
    // A partir do parâmetro `this.valor` o alvo é o mesmo campo.
    let r2 = renomear(&mut p, "lib/a.dart", 3, 10, "quantia");
    assert_eq!(r2["result"], r["result"]);
}

#[test]
fn topo_entre_arquivos_com_tipos_construtores_e_show() {
    let mut p = Projeto::novo("renomear-topo");
    let a = "class Caixa {\n  Caixa();\n  Caixa.vazia();\n  static Caixa criar() => Caixa();\n}\nint contador = 0;\n";
    let b = "import 'a.dart' show Caixa, contador;\nimport 'a.dart' as pa;\nCaixa f(List<Caixa> xs) {\n  Caixa c = new Caixa();\n  pa.Caixa d = pa.Caixa.vazia();\n  contador++;\n  return xs.first is Caixa ? c : d;\n}\n";
    p.gravar("lib/a.dart", a);
    p.abrir("lib/b.dart", b);
    // A partir de um uso num arquivo que importa; a declaração está fechada.
    let r = renomear(&mut p, "lib/b.dart", 2, 1, "Pacote");
    assert_eq!(
        aplicar(&r["result"], &p.uri("lib/a.dart"), a),
        "class Pacote {\n  Pacote();\n  Pacote.vazia();\n  static Pacote criar() => Pacote();\n}\nint contador = 0;\n"
    );
    assert_eq!(
        aplicar(&r["result"], &p.uri("lib/b.dart"), b),
        "import 'a.dart' show Pacote, contador;\nimport 'a.dart' as pa;\nPacote f(List<Pacote> xs) {\n  Pacote c = new Pacote();\n  pa.Pacote d = pa.Pacote.vazia();\n  contador++;\n  return xs.first is Pacote ? c : d;\n}\n"
    );
    // Variável de topo: leitura, escrita e `show`.
    let r = renomear(&mut p, "lib/b.dart", 5, 3, "total");
    assert!(aplicar(&r["result"], &p.uri("lib/b.dart"), b).contains("show Caixa, total;"));
    assert!(aplicar(&r["result"], &p.uri("lib/b.dart"), b).contains("  total++;"));
    assert_eq!(
        aplicar(&r["result"], &p.uri("lib/a.dart"), a)
            .lines()
            .last(),
        Some("int total = 0;")
    );
    // Nome de classe não pode ser palavra-chave (mensagem do Dart 3.6.2).
    assert_eq!(erro(&renomear(&mut p, "lib/b.dart", 2, 1, "dynamic")), "Class name must not be a keyword.");
}

#[test]
fn recusas() {
    let mut p = Projeto::novo("renomear-recusas");
    p.gravar(
        "lib/outra.dart",
        "import 'a.dart';\nvoid g() => publica();\n",
    );
    let fonte = "class A {\n  @override\n  String toString() => '';\n}\nvoid publica() {}\nvoid f(int x) {\n  var y = 1;\n  print(x + y);\n}\n";
    abrir(&mut p, "lib/a.dart", &format!("{fonte}▮"));
    // As mensagens do `dart language-server` 3.6.2 nos mesmos pedidos.
    // Elemento do SDK.
    let prep = p.na_posicao("textDocument/prepareRename", "lib/a.dart", 7, 3, json!({}));
    assert_eq!(prep["error"]["message"], "The function 'print' is defined in the SDK, so cannot be renamed.", "{prep}");
    assert_eq!(erro(&renomear(&mut p, "lib/a.dart", 7, 3, "imprimir")), "The function 'print' is defined in the SDK, so cannot be renamed.");
    // A sobrescrita de um membro do SDK renomeia só ela.
    let r = renomear(&mut p, "lib/a.dart", 2, 10, "texto");
    let edicoes = r["result"]["changes"].as_object().expect("changes").values().next().unwrap().clone();
    assert_eq!(edicoes, json!([{"newText": "texto", "range": {"start": {"line": 2, "character": 9}, "end": {"line": 2, "character": 17}}}]), "{r}");
    // Nomes inválidos.
    assert_eq!(erro(&renomear(&mut p, "lib/a.dart", 6, 7, "class")), "Variable name must not be a keyword.");
    assert_eq!(erro(&renomear(&mut p, "lib/a.dart", 6, 7, "1y")), "Variable name must begin with a lowercase letter or underscore.");
    assert_eq!(erro(&renomear(&mut p, "lib/a.dart", 6, 7, "a-b")), "Variable name must not contain '-'.");
    // Conflito com parâmetro no mesmo corpo.
    assert_eq!(
        erro(&renomear(&mut p, "lib/a.dart", 6, 7, "x")),
        "Usage of parameter \"x\" declared in \"a.dart\" will be shadowed by renamed local variable."
    );
    // Público que viraria privado com uso em outra biblioteca.
    assert_eq!(
        erro(&renomear(&mut p, "lib/a.dart", 4, 6, "_privada")),
        format!("Renamed function will be invisible in '{}'.", std::path::Path::new("lib").join("outra.dart").display())
    );
    // Palavra-chave sob o cursor: nada a renomear.
    let prep = p.na_posicao("textDocument/prepareRename", "lib/a.dart", 5, 1, json!({}));
    assert_eq!(prep["result"], Value::Null);
}

#[test]
fn codigo_incompleto_no_arquivo() {
    let mut p = Projeto::novo("renomear-incompleto");
    // Um comando quebrado noutro ponto do corpo não impede renomear.
    let fonte = "void f(int x) {\n  var so▮ma = x;\n  x.\n  print(soma);\n}\n";
    let (texto, l, c) = abrir(&mut p, "lib/a.dart", fonte);
    let r = renomear(&mut p, "lib/a.dart", l, c, "total");
    let novo = aplicar(&r["result"], &p.uri("lib/a.dart"), &texto);
    // `x.` seguido de `print(soma);` analisa como `x.print(soma);`: o
    // argumento continua sendo um uso do local.
    assert_eq!(
        novo,
        "void f(int x) {\n  var total = x;\n  x.\n  print(total);\n}\n"
    );
    // O operando que falta vira um identificador sintético (o
    // `ensureIdentifier` do fasta): o comando fica na árvore e o uso dentro
    // dele também é renomeado.
    let fonte = "void f(int x) {\n  var so▮ma = x;\n  var q = soma + ;\n  print(soma);\n}\n";
    let (texto, l, c) = abrir(&mut p, "lib/b.dart", fonte);
    let r = renomear(&mut p, "lib/b.dart", l, c, "total");
    let novo = aplicar(&r["result"], &p.uri("lib/b.dart"), &texto);
    assert_eq!(
        novo,
        "void f(int x) {\n  var total = x;\n  var q = total + ;\n  print(total);\n}\n"
    );
}

#[test]
fn import_de_pacote_a_partir_de_bin() {
    let mut p = Projeto::novo("renomear-pacote");
    let a = "String saudar(String nome) => nome;\n";
    let main = "import 'package:projeto/a.dart';\nvoid main() { print(saudar('x')); }\n";
    p.gravar("lib/a.dart", a);
    p.abrir("bin/main.dart", main);
    let r = renomear(&mut p, "bin/main.dart", 1, 22, "cumprimentar");
    assert_eq!(
        aplicar(&r["result"], &p.uri("lib/a.dart"), a),
        "String cumprimentar(String nome) => nome;\n"
    );
    assert_eq!(
        aplicar(&r["result"], &p.uri("bin/main.dart"), main),
        "import 'package:projeto/a.dart';\nvoid main() { print(cumprimentar('x')); }\n"
    );
}

#[test]
fn enum_extensao_getter_e_setter() {
    let mut p = Projeto::novo("renomear-diversos");
    let fonte = "enum Cor { azul, verde }\nextension Ext on int {\n  int get dobro => this * 2;\n  int triplo() => this * 3;\n}\nclass K {\n  int _v = 0;\n  int get v => _v;\n  set v(int n) { _v = n; }\n}\nint get topo => 1;\nset topo(int x) {}\nvoid f(K k) {\n  var c = Cor.azul;\n  print(c == Cor.azul);\n  print(2.dobro + 3.triplo());\n  k.v = k.v + 1;\n  topo = topo;\n}\n";
    p.abrir("lib/a.dart", fonte);
    let uri = p.uri("lib/a.dart");
    // Constante de enum, a partir de um uso.
    let r = renomear(&mut p, "lib/a.dart", 13, 14, "celeste");
    let novo = aplicar(&r["result"], &uri, fonte);
    assert!(novo.starts_with("enum Cor { celeste, verde }"), "{r}");
    assert!(
        novo.contains("var c = Cor.celeste;") && novo.contains("c == Cor.celeste"),
        "{novo}"
    );
    // Getter de extensão e método de extensão.
    let r = renomear(&mut p, "lib/a.dart", 15, 10, "duplo");
    let novo = aplicar(&r["result"], &uri, fonte);
    assert!(
        novo.contains("int get duplo") && novo.contains("2.duplo + 3.triplo()"),
        "{r}"
    );
    let r = renomear(&mut p, "lib/a.dart", 3, 7, "vezesTres");
    assert!(
        aplicar(&r["result"], &uri, fonte).contains("3.vezesTres()"),
        "{r}"
    );
    // Getter e setter de instância juntos, a partir da escrita.
    let r = renomear(&mut p, "lib/a.dart", 16, 4, "valor");
    let novo = aplicar(&r["result"], &uri, fonte);
    assert!(
        novo.contains("int get valor => _v;") && novo.contains("set valor(int n)"),
        "{r}"
    );
    assert!(novo.contains("k.valor = k.valor + 1;"), "{novo}");
    // Getter e setter de topo juntos.
    let r = renomear(&mut p, "lib/a.dart", 10, 9, "cima");
    let novo = aplicar(&r["result"], &uri, fonte);
    assert!(
        novo.contains("int get cima => 1;\nset cima(int x) {}") && novo.contains("cima = cima;"),
        "{r}"
    );
}

/// Linha e coluna UTF-16 do início da `n`-ésima ocorrência de `agulha`.
fn onde(texto: &str, agulha: &str, n: usize) -> (u32, u32) {
    let i = texto.match_indices(agulha).nth(n).unwrap_or_else(|| panic!("sem {agulha:?}")).0;
    let antes = &texto[..i];
    (
        antes.matches('\n').count() as u32,
        antes.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

fn renomear_em(p: &mut Projeto, rel: &str, texto: &str, agulha: &str, n: usize, novo: &str) -> Value {
    let (l, c) = onde(texto, agulha, n);
    renomear(p, rel, l, c, novo)
}

#[test]
fn familia_pelo_extends_object_implicito() {
    // `getHierarchyMembers` sobe até `Object` e desce pelos subtipos que o
    // índice acha: a classe sem `extends` num arquivo que cita `Object`
    // entra; a de um arquivo que não cita, não.
    let mut p = Projeto::novo("renomear-object");
    p.gravar("lib/c.dart", "class C {\n  @override\n  String toString() => 'c';\n}\n");
    let a = "import 'c.dart';\nclass A {\n  @override\n  String toString() => 'a';\n}\nclass B {\n  @override\n  String toString() => 'b';\n}\nvoid f(A a, B b, C c, Object o) {\n  a.toString();\n  b.toString();\n  c.toString();\n}\n";
    p.abrir("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = renomear_em(&mut p, "lib/a.dart", a, "toString() => 'a'", 0, "texto");
    let novo = aplicar(&r["result"], &uri, a);
    assert!(novo.contains("String texto() => 'a'") && novo.contains("String texto() => 'b'"), "{novo}");
    assert!(novo.contains("a.texto();\n  b.texto();\n  c.toString();"), "{novo}");
}

#[test]
fn redirecionamento_de_fabrica_fora_dos_candidatos() {
    // O arquivo que só cita a classe no `ConstructorName` de um
    // redirecionamento de fábrica não é candidato da busca (o Dart deixa a
    // referência como está).
    let mut p = Projeto::novo("renomear-redirecionamento");
    p.gravar("lib/x.dart", "import 'b.dart';\nabstract class X {\n  factory X() = B;\n}\n");
    let b = "import 'x.dart';\nclass B implements X {\n  B();\n}\n";
    p.abrir("lib/b.dart", b);
    let r = renomear_em(&mut p, "lib/b.dart", b, "B implements", 0, "C");
    assert!(aplicar(&r["result"], &p.uri("lib/b.dart"), b).contains("class C implements X"), "{r}");
    assert!(!r["result"].to_string().contains("x.dart"), "{r}");
}

#[test]
fn campo_com_argumento_posicional_de_this_e_docs_por_declaracao() {
    // O argumento posicional de um `this.x` é referência de comprimento 0 no
    // índice: o rename do campo insere o nome novo antes dele (como o Dart).
    let mut p = Projeto::novo("renomear-campo-posicional");
    let a = "class A {\n  final int? x;\n  A([this.x]);\n}\nvoid f(A a) {\n  A(a.x);\n}\n";
    p.abrir("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = renomear_em(&mut p, "lib/a.dart", a, "x;", 0, "y");
    assert_eq!(aplicar(&r["result"], &uri, a), "class A {\n  final int? y;\n  A([this.y]);\n}\nvoid f(A a) {\n  A(ya.y);\n}\n", "{r}");
    // O `[c]` da documentação só muda no comentário da declaração que
    // declara o parâmetro renomeado.
    let b = "class B {\n  /// Usa [c].\n  B.um(int c);\n  /// Usa [c].\n  B.dois(int c);\n}\n";
    p.abrir("lib/b.dart", b);
    let uri_b = p.uri("lib/b.dart");
    let r = renomear_em(&mut p, "lib/b.dart", b, "c);", 1, "d");
    assert_eq!(aplicar(&r["result"], &uri_b, b), "class B {\n  /// Usa [c].\n  B.um(int c);\n  /// Usa [d].\n  B.dois(int d);\n}\n", "{r}");
}

#[test]
fn construtor_nomeado() {
    let mut p = Projeto::novo("renomear-construtor");
    let a = "/// Crie com [Caixa.vazia].\nclass Caixa {\n  final int v;\n  Caixa(this.v);\n  Caixa.vazia() : this(0);\n  Caixa.outra() : this.vazia();\n  factory Caixa.fab() = Caixa.vazia;\n}\nclass Sub extends Caixa {\n  Sub() : super.vazia();\n}\nvoid f() {\n  var x = Caixa.vazia();\n  var y = new Caixa.vazia();\n  print([x, y, Caixa(1)]);\n}\n";
    p.abrir("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let prep = {
        let (l, c) = onde(a, "vazia() :", 0);
        p.na_posicao("textDocument/prepareRename", "lib/a.dart", l, c, json!({}))
    };
    assert_eq!(prep["result"]["placeholder"], "vazia", "{prep}");
    let r = renomear_em(&mut p, "lib/a.dart", a, "vazia() :", 0, "nova");
    assert_eq!(aplicar(&r["result"], &uri, a), a.replace("vazia", "nova"), "{r}");
    // A partir de um uso, o mesmo resultado.
    let r2 = renomear_em(&mut p, "lib/a.dart", a, "vazia();\n  var y", 0, "nova");
    assert_eq!(r2["result"], r["result"]);
    // Conflito com outro construtor (a mensagem do servidor do Dart 3.6.2).
    assert_eq!(
        erro(&renomear_em(&mut p, "lib/a.dart", a, "vazia() :", 0, "outra")),
        "Class 'Caixa' already declares constructor with name 'outra'."
    );
}

#[test]
fn prefixo_de_import() {
    let mut p = Projeto::novo("renomear-prefixo");
    p.gravar("lib/a.dart", "const anot = 0;\nclass Caixa {}\nCaixa criar() => Caixa();\n");
    let b = "import 'a.dart' as pa;\n@pa.anot\npa.Caixa f(pa.Caixa c) => pa.criar();\nint pb = 0;\n";
    p.abrir("lib/b.dart", b);
    let uri = p.uri("lib/b.dart");
    let r = renomear_em(&mut p, "lib/b.dart", b, "pa.Caixa f", 0, "q");
    assert_eq!(aplicar(&r["result"], &uri, b), b.replace("pa", "q").replace("qb", "pb"), "{r}");
    // A partir da diretiva: o `RenameImportRefactoring` do Dart troca o
    // `pa.` inteiro de cada uso (o mesmo texto final).
    let r2 = renomear_em(&mut p, "lib/b.dart", b, "pa;", 0, "q");
    assert_eq!(aplicar(&r2["result"], &uri, b), aplicar(&r["result"], &uri, b));
    let mudancas = r2["result"]["changes"].as_object().unwrap().values().next().unwrap().clone();
    assert_eq!(mudancas[1], json!({"newText": "q.", "range": {"start": {"line": 1, "character": 1}, "end": {"line": 1, "character": 4}}}));
    // Pelo uso, o prefixo é membro da biblioteca: o nome de um topo dela
    // é conflito; pela diretiva, não (só a validação do nome do prefixo).
    assert_eq!(erro(&renomear_em(&mut p, "lib/b.dart", b, "pa.Caixa f", 0, "pb")), "Library already declares getter with name 'pb'.");
    assert!(renomear_em(&mut p, "lib/b.dart", b, "pa;", 0, "pb")["result"]["changes"].is_object());
    assert_eq!(erro(&renomear_em(&mut p, "lib/b.dart", b, "pa;", 0, "dynamic")), "Import prefix name must not be a keyword.");
    // Pelo uso numa expressão (`pa.criar()`), `getElementOfNode` dá o
    // import: o `pa.` de cada uso vira `q.`, e o `[pa.criar]` da
    // documentação também.
    let c = "import 'a.dart' as pa;\n/// Veja [pa.criar].\nvoid g() => pa.criar();\n";
    p.abrir("lib/c.dart", c);
    let uri_c = p.uri("lib/c.dart");
    let r3 = renomear_em(&mut p, "lib/c.dart", c, "pa.criar()", 0, "q");
    assert_eq!(aplicar(&r3["result"], &uri_c, c), c.replace("pa", "q"), "{r3}");
    let mudancas = r3["result"]["changes"].as_object().unwrap().values().next().unwrap().clone();
    assert!(mudancas.as_array().unwrap().iter().all(|e| e["newText"] == "q" || e["newText"] == "q."), "{r3}");
    assert!(mudancas.as_array().unwrap().iter().any(|e| e["newText"] == "q." && e["range"]["start"] == json!({"line": 1, "character": 10})), "{r3}");
}

#[test]
fn parametro_de_tipo_respeita_sombra() {
    let mut p = Projeto::novo("renomear-tipo");
    let a = "class Caixa<T> {\n  T v;\n  Caixa(this.v);\n  S pegar<S, T>(T x, S s) => s;\n  List<T> lista() => [v];\n}\n";
    p.abrir("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = renomear_em(&mut p, "lib/a.dart", a, "T v", 0, "E");
    assert_eq!(
        aplicar(&r["result"], &uri, a),
        "class Caixa<E> {\n  E v;\n  Caixa(this.v);\n  S pegar<S, T>(T x, S s) => s;\n  List<E> lista() => [v];\n}\n",
        "{r}"
    );
    // O `T` do método é outro parâmetro.
    let r = renomear_em(&mut p, "lib/a.dart", a, "T x", 0, "U");
    assert_eq!(
        aplicar(&r["result"], &uri, a),
        "class Caixa<T> {\n  T v;\n  Caixa(this.v);\n  S pegar<S, U>(U x, S s) => s;\n  List<T> lista() => [v];\n}\n"
    );
    // O irmão na mesma lista é conflito; o nome de um tipo usado no escopo
    // não (o servidor do Dart 3.6.2 renomeia `T` para `List`).
    assert_eq!(erro(&renomear_em(&mut p, "lib/a.dart", a, "T x", 0, "S")), "Duplicate type parameter 'S'.");
    let r = renomear_em(&mut p, "lib/a.dart", a, "T v", 0, "List");
    assert_eq!(
        aplicar(&r["result"], &uri, a),
        "class Caixa<List> {\n  List v;\n  Caixa(this.v);\n  S pegar<S, T>(T x, S s) => s;\n  List<List> lista() => [v];\n}\n",
        "{r}"
    );
}

#[test]
fn nomeado_de_sobrescritas_e_documentacao() {
    let mut p = Projeto::novo("renomear-sobrescrita");
    let a = "class A {\n  /// Usa [n] e [valor].\n  void f({int? n}) {}\n  int valor = 0;\n}\nclass B extends A {\n  @override\n  void f({int? n}) { print(n); }\n}\nvoid g(A a, B b) { a.f(n: 1); b.f(n: 2); print(a.valor); }\n";
    p.abrir("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    let r = renomear_em(&mut p, "lib/a.dart", a, "n})", 0, "m");
    assert_eq!(
        aplicar(&r["result"], &uri, a),
        "class A {\n  /// Usa [m] e [valor].\n  void f({int? m}) {}\n  int valor = 0;\n}\nclass B extends A {\n  @override\n  void f({int? m}) { print(m); }\n}\nvoid g(A a, B b) { a.f(m: 1); b.f(m: 2); print(a.valor); }\n",
        "{r}"
    );
    // Campo citado na documentação.
    let r = renomear_em(&mut p, "lib/a.dart", a, "valor = 0", 0, "total");
    let novo = aplicar(&r["result"], &uri, a);
    assert!(novo.contains("/// Usa [n] e [total].") && novo.contains("print(a.total)"), "{novo}");
}

#[test]
fn arquivo_da_classe_com_documentchanges_versionado() {
    let mut p = Projeto::novo("renomear-arquivo");
    let classe = "class MinhaClasse {}\n";
    let usa = "import 'minha_classe.dart';\nMinhaClasse? x;\n";
    let main = "import 'package:projeto/minha_classe.dart';\nvoid main() { MinhaClasse(); }\n";
    p.gravar("lib/minha_classe.dart", classe);
    p.gravar("bin/main.dart", main);
    p.abrir("lib/usa.dart", usa);
    // `dart.renameFilesWithClasses` é da configuração do cliente
    // (`LspClientConfiguration`), pedida por `workspace/configuration`.
    let capacidades = json!({"workspace": {"configuration": true, "workspaceEdit": {"documentChanges": true, "resourceOperations": ["create", "rename"]}}});
    p.reiniciar(json!({"capabilities": capacidades}));
    p.configurar(json!({"renameFilesWithClasses": "always"}));
    let r = renomear_em(&mut p, "lib/usa.dart", usa, "MinhaClasse?", 0, "OutraClasse");
    let mudancas = r["result"]["documentChanges"].as_array().unwrap_or_else(|| panic!("{r}")).clone();
    // A operação de arquivo vem depois das edições de texto.
    let ultima = mudancas.last().unwrap();
    assert_eq!(ultima["kind"], "rename", "{r}");
    assert_eq!(ultima["oldUri"], p.uri("lib/minha_classe.dart"));
    assert_eq!(ultima["newUri"], p.uri("lib/outra_classe.dart"));
    let edicoes = |uri: &str| -> (Value, Value) {
        let d = mudancas.iter().find(|d| d["textDocument"]["uri"] == uri).unwrap_or_else(|| panic!("sem {uri}"));
        (d["textDocument"]["version"].clone(), d["edits"].clone())
    };
    // Aberto leva a versão vigente; fechado, `null`.
    let uri_usa = p.uri("lib/usa.dart");
    let (versao, e) = edicoes(&uri_usa);
    assert_eq!(versao, json!(1));
    let mut mapa = serde_json::Map::new();
    mapa.insert(uri_usa.clone(), e);
    // O `rename` do servidor do Dart 3.6.2 não reescreve os `import` (o
    // cliente os pede no `workspace/willRenameFiles`).
    assert_eq!(
        aplicar(&json!({"changes": mapa}), &uri_usa, usa),
        "import 'minha_classe.dart';\nOutraClasse? x;\n"
    );
    let uri_main = p.uri("bin/main.dart");
    let (versao, e) = edicoes(&uri_main);
    assert_eq!(versao, Value::Null);
    let mut mapa = serde_json::Map::new();
    mapa.insert(uri_main.clone(), e);
    assert_eq!(
        aplicar(&json!({"changes": mapa}), &uri_main, main),
        "import 'package:projeto/minha_classe.dart';\nvoid main() { OutraClasse(); }\n"
    );
    // A ordem do `SourceChange`: o arquivo da declaração e os outros pelo
    // caminho; a operação de arquivo no fim.
    let ordem: Vec<String> = mudancas.iter().filter_map(|d| d["textDocument"]["uri"].as_str().map(str::to_string)).collect();
    assert_eq!(ordem, vec![p.uri("lib/minha_classe.dart"), uri_main.clone(), uri_usa.clone()]);
    // Sem a opção, a classe muda e o arquivo fica.
    p.reiniciar(json!({"capabilities": capacidades}));
    let r = renomear_em(&mut p, "lib/usa.dart", usa, "MinhaClasse?", 0, "OutraClasse");
    let mudancas = r["result"]["documentChanges"].as_array().unwrap();
    assert!(mudancas.iter().all(|m| m.get("kind").is_none()), "{r}");
    let texto_usa = mudancas.iter().find(|d| d["textDocument"]["uri"] == uri_usa).unwrap();
    assert_eq!(texto_usa["edits"].as_array().unwrap().len(), 1);
}

#[test]
fn conflitos_por_escopo() {
    let mut p = Projeto::novo("renomear-escopo");
    let a = "int topo = 0;\nint valor = 0;\nvoid f() {\n  { var a = 1; print(a); }\n  { var b = 2; print(b); }\n}\nvoid g() { var x = 1; print(topo + x); }\nvoid h() { var c = 1; { var d = 2; print(c + d); } }\nclass K {\n  int campo = 1;\n  int soma() => campo + valor;\n}\n";
    p.abrir("lib/a.dart", a);
    let uri = p.uri("lib/a.dart");
    // Blocos disjuntos: permitido.
    let r = renomear_em(&mut p, "lib/a.dart", a, "b = 2", 0, "a");
    assert!(aplicar(&r["result"], &uri, a).contains("{ var a = 2; print(a); }"), "{r}");
    // As respostas do `dart language-server` 3.6.2 nos mesmos pedidos.
    // Local que sombrearia o uso do topo.
    assert_eq!(
        erro(&renomear_em(&mut p, "lib/a.dart", a, "x = 1", 0, "topo")),
        "Usage of top level variable \"topo\" declared in \"a.dart\" will be shadowed by renamed local variable."
    );
    // Topo renomeado para o nome de um local: o Dart renomeia (a declaração
    // e o uso).
    let r = renomear_em(&mut p, "lib/a.dart", a, "topo = 0", 0, "x");
    assert_eq!(aplicar(&r["result"], &uri, a), a.replace("topo", "x"), "{r}");
    // Local que já existe num bloco aninhado do escopo.
    assert_eq!(erro(&renomear_em(&mut p, "lib/a.dart", a, "c = 1", 0, "d")), "Duplicate local variable 'd'.");
    // Campo com o nome de um topo usado na classe: renomeia.
    let r = renomear_em(&mut p, "lib/a.dart", a, "campo = 1", 0, "valor");
    assert_eq!(aplicar(&r["result"], &uri, a), a.replace("int campo = 1", "int valor = 1").replace("=> campo +", "=> valor +"), "{r}");
    // Topo que passaria a ser o membro dentro da classe.
    assert_eq!(
        erro(&renomear_em(&mut p, "lib/a.dart", a, "valor = 0", 0, "campo")),
        "Reference to renamed top level variable will be shadowed by getter 'campo'."
    );
}

#[test]
fn homonimo_de_outra_biblioteca_fica_intacto() {
    let mut p = Projeto::novo("renomear-homonimo");
    let a = "int f() => 1;\n";
    let b = "import 'a.dart';\nint g() => f();\n";
    let c = "int f() => 2;\nint h() => f();\n";
    p.gravar("lib/b.dart", b);
    p.gravar("lib/c.dart", c);
    p.abrir("lib/a.dart", a);
    let r = renomear(&mut p, "lib/a.dart", 0, 4, "um");
    assert_eq!(aplicar(&r["result"], &p.uri("lib/a.dart"), a), "int um() => 1;\n");
    assert_eq!(aplicar(&r["result"], &p.uri("lib/b.dart"), b), "import 'a.dart';\nint g() => um();\n");
    assert!(r["result"]["changes"].get(p.uri("lib/c.dart")).is_none(), "{r}");
}
