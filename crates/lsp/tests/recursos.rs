//! Recursos além de definição/hover/referências, por JSON-RPC sobre um
//! projeto temporário com SDK mínimo: ajuda de assinatura, destaques,
//! implementações, definição do tipo, dobras, faixas de seleção e
//! hierarquia de tipos (regras em `docs/LSP.md`, "Paridade com o servidor
//! do Dart").

mod comum;

use comum::Projeto;
use serde_json::{Value, json};

/// Linha e coluna UTF-16 do início da `n`-ésima ocorrência de `agulha`,
/// mais `desloc` colunas.
fn onde(texto: &str, agulha: &str, n: usize, desloc: u32) -> (u32, u32) {
    let i = texto.match_indices(agulha).nth(n).unwrap_or_else(|| panic!("sem {agulha:?} #{n}")).0;
    let antes = &texto[..i];
    (antes.matches('\n').count() as u32, antes.rsplit('\n').next().unwrap().encode_utf16().count() as u32 + desloc)
}

const MODELO: &str = "/// Uma caixa de [T].
class Caixa<T> {
  T valor;
  Caixa(this.valor);
  /// Troca o valor.
  void trocar(T novo, {bool avisar = false}) {}
}

abstract class Forma {
  double area();
}

class Quadrado extends Forma {
  @override
  double area() => 1;
}

class Circulo implements Forma {
  @override
  double area() => 3;
}

/// Soma dois números.
int soma(int a, [int b = 0]) => a + b;
";

const USO: &str = "import 'modelo.dart';

void usar(List<int> lista, Forma f) {
  lista.add(1);
  soma(1, 2);
  final c = Caixa<String>('a');
  c.trocar('b', avisar: true);
  var total = soma(3);
  total = total + 1;
  print(total);
  [1].map((x) => soma(x));
  f.area();
}
";

fn projeto(nome: &str, capacidades: Value) -> Projeto {
    let mut p = Projeto::com_capacidades(nome, capacidades);
    p.gravar("lib/modelo.dart", MODELO);
    p.abrir("lib/modelo.dart", MODELO);
    p.abrir("lib/uso.dart", USO);
    p
}

fn assinatura(p: &mut Projeto, agulha: &str, n: usize, desloc: u32, contexto: Value) -> Value {
    let (l, c) = onde(USO, agulha, n, desloc);
    p.na_posicao("textDocument/signatureHelp", "lib/uso.dart", l, c, json!({"context": contexto}))["result"].clone()
}

#[test]
fn ajuda_de_assinatura_como_o_dart() {
    let mut p = projeto("assinatura", json!({"textDocument": {"signatureHelp": {"signatureInformation": {"documentationFormat": ["markdown"]}}}}));
    let invocada = json!({"triggerKind": 1});
    // Membro de receptor genérico: o tipo já substituído.
    let r = assinatura(&mut p, "add(", 0, 4, invocada.clone());
    assert_eq!(r["signatures"][0]["label"], "add(int value)", "{r}");
    assert_eq!(r["activeParameter"], 0);
    // Função de topo com opcional e padrão; o segundo argumento ativo.
    let r = assinatura(&mut p, "soma(1, 2)", 0, 8, invocada.clone());
    assert_eq!(r["signatures"][0]["label"], "soma(int a, [int b = 0])", "{r}");
    assert_eq!(r["signatures"][0]["parameters"][1]["label"], "int b = 0");
    assert_eq!(r["activeParameter"], 1);
    assert_eq!(r["signatures"][0]["documentation"]["value"], "Soma dois números.");
    // Construtor com os argumentos de tipo da criação.
    let r = assinatura(&mut p, "Caixa<String>(", 0, 14, invocada.clone());
    assert_eq!(r["signatures"][0]["label"], "Caixa(String valor)", "{r}");
    // Método genérico substituído, nomeado ativo pelo nome.
    let r = assinatura(&mut p, "avisar: true", 0, 2, invocada.clone());
    assert_eq!(r["signatures"][0]["label"], "trocar(String novo, {bool avisar = false})", "{r}");
    assert_eq!(r["activeParameter"], 1);
    // Disparo automático só no `(` que abre a lista.
    let auto = json!({"triggerKind": 2, "triggerCharacter": "(", "isRetrigger": false});
    let r = assinatura(&mut p, "soma(3)", 0, 5, auto.clone());
    assert_eq!(r["signatures"][0]["label"], "soma(int a, [int b = 0])", "{r}");
    let r = assinatura(&mut p, "soma(1, 2)", 0, 8, auto);
    assert!(r.is_null(), "{r}");
    // Dentro de um closure passado como argumento: a chamada interna, nunca
    // a de fora.
    let r = assinatura(&mut p, "soma(x)", 0, 5, invocada.clone());
    assert_eq!(r["signatures"][0]["label"], "soma(int a, [int b = 0])", "{r}");
    let r = assinatura(&mut p, "(x) =>", 0, 6, invocada);
    assert!(r.is_null(), "{r}");
}

#[test]
fn destaques_implementacoes_e_tipo() {
    let mut p = projeto("navegar_mais", json!({}));
    // Destaques: declaração e usos do local no arquivo.
    let (l, c) = onde(USO, "total", 1, 1);
    let r = p.na_posicao("textDocument/documentHighlight", "lib/uso.dart", l, c, json!({}))["result"].clone();
    let linhas: Vec<u64> = r.as_array().unwrap().iter().map(|h| h["range"]["start"]["line"].as_u64().unwrap()).collect();
    assert_eq!(linhas, vec![7, 8, 8, 9], "{r}");
    // Implementações de uma classe abstrata e de um método dela.
    let (l, c) = onde(MODELO, "Forma {", 0, 1);
    let r = p.na_posicao("textDocument/implementation", "lib/modelo.dart", l, c, json!({}))["result"].clone();
    let mut nomes: Vec<u64> = r.as_array().unwrap().iter().map(|x| x["range"]["start"]["line"].as_u64().unwrap()).collect();
    nomes.sort();
    assert_eq!(nomes, vec![12, 17], "{r}");
    let (l, c) = onde(USO, "area()", 0, 1);
    let r = p.na_posicao("textDocument/implementation", "lib/uso.dart", l, c, json!({}))["result"].clone();
    let mut linhas: Vec<u64> = r.as_array().unwrap().iter().map(|x| x["range"]["start"]["line"].as_u64().unwrap()).collect();
    linhas.sort();
    assert_eq!(linhas, vec![14, 19], "{r}");
    // Definição do tipo: a classe do tipo estático.
    let (l, c) = onde(USO, "c.trocar", 0, 0);
    let r = p.na_posicao("textDocument/typeDefinition", "lib/uso.dart", l, c, json!({}))["result"].clone();
    assert!(r["uri"].as_str().unwrap().ends_with("modelo.dart"), "{r}");
    assert_eq!(r["range"]["start"], json!({"line": 1, "character": 6}));
}

#[test]
fn definicao_de_tipo_como_o_handler_do_dart() {
    let mut p = Projeto::novo("tipo-def");
    let texto = "class A {\n  int x = 0;\n  A(int v) : x = v;\n  @override\n  String toString() => '';\n  @Deprecated('m')\n  void m() {\n    x = 1;\n  }\n}\n";
    p.abrir("lib/td.dart", texto);
    let mut td = |agulha: &str| {
        let (l, c) = onde(texto, agulha, 0, 0);
        p.na_posicao("textDocument/typeDefinition", "lib/td.dart", l, c, json!({}))["result"].clone()
    };
    let vazio = |r: &Value| r.is_null() || r.as_array().is_some_and(|a| a.is_empty());
    // O `returnType` da declaração de construtor: a classe.
    assert_eq!(td("A(int")["range"]["start"], json!({"line": 0, "character": 6}));
    // O campo do inicializador e `@override`: nada.
    let r = td("x = v");
    assert!(vazio(&r), "{r}");
    let r = td("override");
    assert!(vazio(&r), "{r}");
    // `@Deprecated(…)`: a classe.
    let r = td("Deprecated");
    assert!(r["uri"].as_str().is_some_and(|u| u.contains("core")), "{r}");
    // O alvo de uma escrita de campo: o tipo do campo.
    let r = td("x = 1");
    assert!(r["uri"].as_str().is_some_and(|u| u.contains("core")), "{r}");
}

#[test]
fn hierarquia_de_tipos() {
    let mut p = projeto("hierarquia", json!({}));
    let (l, c) = onde(MODELO, "Quadrado", 0, 1);
    let r = p.na_posicao("textDocument/prepareTypeHierarchy", "lib/modelo.dart", l, c, json!({}))["result"].clone();
    let item = r[0].clone();
    assert_eq!(item["name"], "Quadrado", "{r}");
    let sup = p.requisitar("typeHierarchy/supertypes", json!({"item": item}))["result"].clone();
    let nomes: Vec<&str> = sup.as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(nomes, vec!["Forma"], "{sup}");
    let forma = sup[0].clone();
    let sub = p.requisitar("typeHierarchy/subtypes", json!({"item": forma}))["result"].clone();
    let mut nomes: Vec<&str> = sub.as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    nomes.sort();
    assert_eq!(nomes, vec!["Circulo", "Quadrado"], "{sub}");
}

#[test]
fn dobras_e_selecao() {
    let mut p = projeto("dobras", json!({"textDocument": {"foldingRange": {"lineFoldingOnly": true}}}));
    let r = p.requisitar("textDocument/foldingRange", json!({"textDocument": {"uri": p.uri("lib/modelo.dart")}}))["result"].clone();
    let regioes: Vec<(u64, u64)> = r
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["startLine"].as_u64().unwrap(), d["endLine"].as_u64().unwrap()))
        .collect();
    // Classe Caixa (1–6), Forma (8–10), Quadrado (12–15), Circulo (17–20).
    assert_eq!(regioes, vec![(1, 6), (8, 10), (12, 15), (17, 20)], "{r}");
    let (l, c) = onde(USO, "soma(1, 2)", 0, 1);
    let r = p.requisitar(
        "textDocument/selectionRange",
        json!({"textDocument": {"uri": p.uri("lib/uso.dart")}, "positions": [{"line": l, "character": c}]}),
    )["result"]
        .clone();
    // O identificador, a chamada, ... até a função inteira.
    let primeiro = &r[0]["range"];
    assert_eq!(primeiro["start"], json!({"line": l, "character": c - 1}));
    assert_eq!(primeiro["end"], json!({"line": l, "character": c + 3}));
    let mut nivel = &r[0];
    let mut n = 0;
    while nivel.get("parent").is_some() {
        nivel = &nivel["parent"];
        n += 1;
    }
    assert!(n >= 3, "{r}");
    assert_eq!(nivel["range"]["start"], json!({"line": 2, "character": 0}));
}

#[test]
fn hierarquia_de_chamadas() {
    let mut p = projeto("chamadas", json!({}));
    // `soma` é chamada três vezes por `usar` (uma dentro de um closure).
    let (l, c) = onde(MODELO, "soma(int", 0, 1);
    let r = p.na_posicao("textDocument/prepareCallHierarchy", "lib/modelo.dart", l, c, json!({}))["result"].clone();
    let item = r[0].clone();
    assert_eq!(item["name"], "soma", "{r}");
    assert_eq!(item["kind"], 12);
    let recebidas = p.requisitar("callHierarchy/incomingCalls", json!({"item": item}))["result"].clone();
    let chamadores: Vec<(&str, usize)> = recebidas
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["from"]["name"].as_str().unwrap(), c["fromRanges"].as_array().unwrap().len()))
        .collect();
    assert_eq!(chamadores, vec![("usar", 3)], "{recebidas}");
    // `usar` chama `add`, `soma`, o construtor de `Caixa`, `trocar`, `print`, `map` e `area`.
    let (l, c) = onde(USO, "usar", 0, 1);
    let r = p.na_posicao("textDocument/prepareCallHierarchy", "lib/uso.dart", l, c, json!({}))["result"].clone();
    let feitas = p.requisitar("callHierarchy/outgoingCalls", json!({"item": r[0].clone()}))["result"].clone();
    let mut nomes: Vec<&str> = feitas.as_array().unwrap().iter().map(|c| c["to"]["name"].as_str().unwrap()).collect();
    nomes.sort();
    for esperado in ["Caixa", "add", "area", "soma", "trocar"] {
        assert!(nomes.contains(&esperado), "{nomes:?}");
    }
}
