//! `definition`, `hover` e `references` pela identidade semântica
//! (`crates/lsp/src/projeto.rs`), por JSON-RPC: matriz por forma (local,
//! parâmetro, função local, membro herdado, campo genérico, getter,
//! construtores, prefixo, enum, extensão, tipo, parâmetro de tipo, SDK,
//! documentação), com comentários, partes e textos abertos ainda não salvos.

mod comum;

use comum::Projeto;
use serde_json::{Value, json};

/// Linha e coluna UTF-16 do início da `n`-ésima ocorrência de `agulha`.
fn onde(texto: &str, agulha: &str, n: usize) -> (u32, u32) {
    let i = texto
        .match_indices(agulha)
        .nth(n)
        .unwrap_or_else(|| panic!("sem {agulha:?} #{n}"))
        .0;
    let antes = &texto[..i];
    (
        antes.matches('\n').count() as u32,
        antes.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

fn inicio(l: u32, c: u32) -> Value {
    json!({"line": l, "character": c})
}

fn pedir(
    p: &mut Projeto,
    metodo: &str,
    rel: &str,
    texto: &str,
    agulha: &str,
    n: usize,
    extra: Value,
) -> Value {
    let (l, c) = onde(texto, agulha, n);
    p.na_posicao(metodo, rel, l, c, extra)["result"].clone()
}

/// (uri, início) da definição.
fn definicao(p: &mut Projeto, rel: &str, texto: &str, agulha: &str, n: usize) -> (String, Value) {
    let r = pedir(
        p,
        "textDocument/definition",
        rel,
        texto,
        agulha,
        n,
        json!({}),
    );
    assert!(r.is_object(), "sem definição em {agulha:?}: {r}");
    (
        r["uri"].as_str().unwrap().to_string(),
        r["range"]["start"].clone(),
    )
}

fn hover(p: &mut Projeto, rel: &str, texto: &str, agulha: &str, n: usize) -> String {
    let r = pedir(p, "textDocument/hover", rel, texto, agulha, n, json!({}));
    r["contents"]
        .as_str()
        .unwrap_or_else(|| panic!("sem hover em {agulha:?}: {r}"))
        .to_string()
}

/// (uri relativa ao projeto, linha, coluna) de cada referência.
fn referencias(
    p: &mut Projeto,
    rel: &str,
    texto: &str,
    agulha: &str,
    n: usize,
    incluir: bool,
) -> Vec<(String, u32, u32)> {
    let r = pedir(
        p,
        "textDocument/references",
        rel,
        texto,
        agulha,
        n,
        json!({"context": {"includeDeclaration": incluir}}),
    );
    let base = format!("{}/", p.uri(""));
    r.as_array()
        .unwrap_or_else(|| panic!("sem referências: {r}"))
        .iter()
        .map(|l| {
            (
                l["uri"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches(&base)
                    .to_string(),
                l["range"]["start"]["line"].as_u64().unwrap() as u32,
                l["range"]["start"]["character"].as_u64().unwrap() as u32,
            )
        })
        .collect()
}

const MODELO: &str = "/// Algo que se compara.
abstract class Comparavel {}

class Base {
  /// Faz algo com [x].
  void met(int x, {int y = 0, required String nome}) {}
}

/// Uma caixa de [T].
class Caixa<T> extends Base implements Comparavel {
  /// O valor guardado.
  T valor;
  Caixa(this.valor);
  Caixa.vazia(T v) : this(v);
  int get dobro => 2;
  @override
  void met(int x, {int y = 0, required String nome}) {}
}

enum Cor { azul, verde }

extension Ext on int {
  int get triplo => this * 3;
}

int contador = 0;
";

const USO: &str = "import 'modelo.dart';
import 'modelo.dart' as m;

void usar(Caixa<int> c, num n) {
  c.met(1, nome: 'a');
  var total = c.valor + c.dobro;
  if (n is int) print(n);
  int dobro(int v) => v * 2;
  print(dobro(total));
  var k = Caixa.vazia(3);
  var k2 = Caixa(4);
  m.contador++;
  print(Cor.azul);
  print(5.triplo);
  print(k.valor + k2.valor);
}
";

#[test]
fn definicao_por_forma() {
    let mut p = Projeto::novo("nav-definicao");
    // O modelo fica só no disco; o uso está aberto.
    p.gravar("lib/modelo.dart", MODELO);
    p.abrir("lib/uso.dart", USO);
    let modelo = p.uri("lib/modelo.dart");
    let uso = p.uri("lib/uso.dart");
    let em = |t: &str, a: &str, n: usize| {
        let (l, c) = onde(t, a, n);
        inicio(l, c)
    };
    let casos: &[(&str, usize, &str, &str, usize)] = &[
        // (agulha no uso, ocorrência, arquivo esperado, agulha no destino, ocorrência)
        // Membro herdado e sobrescrito: vai ao da classe do receptor.
        ("met(1", 0, "modelo", "met(int x", 1),
        // Campo de classe genérica.
        ("valor + c", 0, "modelo", "valor;", 0),
        // Getter (há uma função local homônima: não confunde).
        ("dobro;", 0, "modelo", "dobro =>", 0),
        // Função local.
        ("dobro(total", 0, "uso", "dobro(int v", 0),
        // Local.
        ("total));", 0, "uso", "total =", 0),
        // Parâmetro promovido.
        ("n);", 0, "uso", "n) {", 0),
        // Construtor nomeado e construtor escrito sem nome.
        ("vazia(3", 0, "modelo", "vazia(T", 0),
        ("Caixa(4", 0, "modelo", "Caixa(this", 0),
        // Prefixo de import e elemento pelo prefixo.
        ("m.contador", 0, "uso", "m;", 0),
        ("contador++", 0, "modelo", "contador =", 0),
        // Constante de enum, getter de extensão, tipo anotado.
        ("azul", 0, "modelo", "azul", 0),
        ("triplo", 0, "modelo", "triplo =>", 0),
        ("Caixa<int>", 0, "modelo", "Caixa<T>", 0),
    ];
    for &(agulha, n, arquivo, destino, dn) in casos {
        let (uri, pos) = definicao(&mut p, "lib/uso.dart", USO, agulha, n);
        let (esperado_uri, texto) = if arquivo == "modelo" {
            (&modelo, MODELO)
        } else {
            (&uso, USO)
        };
        assert_eq!(&uri, esperado_uri, "{agulha}");
        assert_eq!(pos, em(texto, destino, dn), "{agulha}");
    }
    // Do modelo: parâmetro de tipo e referência de documentação.
    p.abrir("lib/modelo.dart", MODELO);
    let (uri, pos) = definicao(&mut p, "lib/modelo.dart", MODELO, "T valor", 0);
    assert_eq!((uri, pos), (modelo.clone(), em(MODELO, "T> extends", 0)));
    let (uri, pos) = definicao(&mut p, "lib/modelo.dart", MODELO, "x].", 0);
    assert_eq!((uri, pos), (modelo.clone(), em(MODELO, "x, {int y", 0)));
    let (uri, pos) = definicao(&mut p, "lib/modelo.dart", MODELO, "T].", 0);
    assert_eq!((uri, pos), (modelo.clone(), em(MODELO, "T> extends", 0)));
    // Elemento do SDK: o arquivo do SDK.
    let (uri, _) = definicao(&mut p, "lib/uso.dart", USO, "print(n", 0);
    assert!(uri.ends_with("sdk/lib/core/core.dart"), "{uri}");
    // Palavra-chave e espaço: nada.
    assert_eq!(
        pedir(
            &mut p,
            "textDocument/definition",
            "lib/uso.dart",
            USO,
            "var total",
            0,
            json!({})
        ),
        Value::Null
    );
}

#[test]
fn hover_por_forma_com_documentacao() {
    let mut p = Projeto::novo("nav-hover");
    p.gravar("lib/modelo.dart", MODELO);
    p.abrir("lib/uso.dart", USO);
    let casos: &[(&str, usize, &str)] = &[
        // A sobrescrita sem documentação herda a do membro sobrescrito.
        (
            "met(1",
            0,
            // Três parâmetros: um por linha, como o `multiline` do analyzer;
            // numa chamada, o tipo da invocação.
            "void met(\n  int x, {\n  int y = 0,\n  required String nome,\n})\nType: void Function(int, {required String nome, int y})\n\nFaz algo com [x].",
        ),
        ("valor + c", 0, "T valor\nType: int\n\nO valor guardado."),
        ("dobro;", 0, "int get dobro\nType: int"),
        ("dobro(total", 0, "int dobro(int v)\nType: int Function(int)"),
        ("total));", 0, "int total\nType: int"),
        // O tipo mostrado é o promovido; a declaração, a escrita.
        ("n);", 0, "num n\nType: int"),
        ("azul", 0, "Cor azul\nType: Cor"),
        ("contador++", 0, "int contador\nType: int"),
        (
            "Caixa<int>",
            0,
            "class Caixa<T> extends Base implements Comparavel\n\nUma caixa de [T].",
        ),
        ("print(n", 0, "void print(Object? object)\nType: void Function(Object?)"),
    ];
    for &(agulha, n, esperado) in casos {
        assert_eq!(
            hover(&mut p, "lib/uso.dart", USO, agulha, n),
            esperado,
            "{agulha}"
        );
    }
    let construtor = hover(&mut p, "lib/uso.dart", USO, "vazia(3", 0);
    assert!(construtor.ends_with("Caixa.vazia(T v)"), "{construtor}");
    // Markdown: bloco de código, tipo e documentação depois de `---`.
    p.reiniciar(json!({"capabilities": {"textDocument": {"hover": {"contentFormat": ["markdown"]}}}}));
    let r = pedir(
        &mut p,
        "textDocument/hover",
        "lib/uso.dart",
        USO,
        "valor + c",
        0,
        json!({}),
    );
    assert_eq!(
        r["contents"]["value"],
        "```dart\nT valor\n```\nType: `int`\n\n*package:projeto/modelo.dart*\n\n---\nO valor guardado."
    );
    // O intervalo é o do nome sob o cursor.
    let (l, c) = onde(USO, "valor + c", 0);
    assert_eq!(r["range"]["start"], inicio(l, c));
    assert_eq!(r["range"]["end"], inicio(l, c + 5));
    // Prefixo e palavra-chave não têm hover.
    assert_eq!(
        pedir(
            &mut p,
            "textDocument/hover",
            "lib/uso.dart",
            USO,
            "m.contador",
            0,
            json!({})
        ),
        Value::Null
    );
    assert_eq!(
        pedir(
            &mut p,
            "textDocument/hover",
            "lib/uso.dart",
            USO,
            "var k",
            0,
            json!({})
        ),
        Value::Null
    );
}

#[test]
fn textos_abertos_e_partes() {
    let mut p = Projeto::novo("nav-abertos");
    p.gravar("lib/modelo.dart", MODELO);
    p.abrir("lib/uso.dart", USO);
    // O modelo aberto e alterado sem salvar: a definição e o hover seguem
    // o buffer, não o disco.
    let alterado = MODELO.replace("int contador = 0;", "\n/// Quantos.\ndouble contador = 0;");
    p.abrir("lib/modelo.dart", &alterado);
    p.gravar("lib/modelo.dart", MODELO);
    let (_, pos) = definicao(&mut p, "lib/uso.dart", USO, "contador++", 0);
    let (l, c) = onde(&alterado, "contador =", 0);
    assert_eq!(pos, inicio(l, c));
    assert_eq!(
        hover(&mut p, "lib/uso.dart", USO, "contador++", 0),
        "double contador\nType: double\n\nQuantos."
    );
    // Parte e biblioteca dona.
    let dona = "library partes;\npart 'parte.dart';\n/// Base comum.\nint base = 1;\nint dobro() => metade() * 4;\n";
    let parte = "part of 'partes.dart';\nint metade() => base ~/ 2;\n";
    p.gravar("lib/partes.dart", dona);
    p.abrir("lib/parte.dart", parte);
    let (uri, pos) = definicao(&mut p, "lib/parte.dart", parte, "base ~/", 0);
    assert_eq!(uri, p.uri("lib/partes.dart"));
    let (l, c) = onde(dona, "base =", 0);
    assert_eq!(pos, inicio(l, c));
    assert_eq!(
        hover(&mut p, "lib/parte.dart", parte, "base ~/", 0),
        "int base\nType: int\n\nBase comum."
    );
    p.abrir("lib/partes.dart", dona);
    let (uri, pos) = definicao(&mut p, "lib/partes.dart", dona, "metade()", 0);
    assert_eq!(uri, p.uri("lib/parte.dart"));
    let (l, c) = onde(parte, "metade", 0);
    assert_eq!(pos, inicio(l, c));
}

#[test]
fn referencias_sem_falsos_por_sombra_homonimo_e_biblioteca() {
    let mut p = Projeto::novo("nav-referencias");
    let a = "int f() => 1;\nvoid g() { f(); f(); }\n/// Veja [f].\nvoid h() {}\n";
    let b = "import 'a.dart';\nvoid x() => print(f());\n";
    // Homônimo noutra biblioteca, que não importa `a.dart`.
    let c = "int f() => 2;\nvoid y() => print(f());\n";
    // Prefixo, `show` e um local homônimo que sombreia.
    let d = "import 'a.dart' as p;\nimport 'a.dart' show f;\nvoid z() { p.f(); int f = 0; print(f); }\n";
    p.gravar("lib/b.dart", b);
    p.gravar("lib/c.dart", c);
    p.gravar("lib/d.dart", d);
    p.abrir("lib/a.dart", a);
    let refs = referencias(&mut p, "lib/a.dart", a, "f()", 0, true);
    // A ordem do servidor do Dart: os outros arquivos pelo caminho, o que
    // declara e a declaração no fim.
    let esperado: Vec<(String, u32, u32)> = vec![
        ("lib/b.dart".into(), 1, 18),
        ("lib/d.dart".into(), 1, 21),
        ("lib/d.dart".into(), 2, 13),
        ("lib/a.dart".into(), 1, 11),
        ("lib/a.dart".into(), 1, 16),
        ("lib/a.dart".into(), 2, 10),
        ("lib/a.dart".into(), 0, 4),
    ];
    assert_eq!(refs, esperado);
    // Sem a declaração.
    let sem = referencias(&mut p, "lib/a.dart", a, "f()", 0, false);
    assert_eq!(sem, esperado[..esperado.len() - 1].to_vec());
    // A partir de um uso num arquivo fechado dá o mesmo conjunto.
    p.abrir("lib/b.dart", b);
    assert_eq!(
        referencias(&mut p, "lib/b.dart", b, "f()", 0, true),
        esperado
    );
    // O homônimo de `c.dart` só vê a si mesmo.
    p.abrir("lib/c.dart", c);
    assert_eq!(
        referencias(&mut p, "lib/c.dart", c, "f()", 1, true),
        vec![
            ("lib/c.dart".to_string(), 1, 18),
            ("lib/c.dart".to_string(), 0, 4)
        ]
    );
    // O local que sombreia só vê a si mesmo.
    p.abrir("lib/d.dart", d);
    assert_eq!(
        referencias(&mut p, "lib/d.dart", d, "f = 0", 0, true),
        vec![
            ("lib/d.dart".to_string(), 2, 35),
            ("lib/d.dart".to_string(), 2, 22)
        ]
    );
    // O prefixo: o uso e a declaração, que o servidor do Dart 3.6.2 dá no
    // começo do arquivo (0:0).
    assert_eq!(
        referencias(&mut p, "lib/d.dart", d, "p.f", 0, true),
        vec![
            ("lib/d.dart".to_string(), 2, 11),
            ("lib/d.dart".to_string(), 0, 0)
        ]
    );
}

#[test]
fn referencias_de_membro_pela_familia_e_do_sdk() {
    let mut p = Projeto::novo("nav-ref-membro");
    let a = "class A { void m() {} }\nclass B extends A { void m() {} }\nclass D { void m() {} }\nvoid t(A a, B b, D d) { a.m(); b.m(); d.m(); print(1); }\n";
    p.abrir("lib/a.dart", a);
    let refs = referencias(&mut p, "lib/a.dart", a, "m()", 0, true);
    assert_eq!(
        refs,
        vec![
            ("lib/a.dart".to_string(), 3, 26),
            ("lib/a.dart".to_string(), 3, 33),
            ("lib/a.dart".to_string(), 0, 15),
        ]
    );
    // Elemento do SDK: a declaração no SDK e os usos só no projeto.
    let refs = referencias(&mut p, "lib/a.dart", a, "print", 0, true);
    assert_eq!(refs.len(), 2, "{refs:?}");
    assert!(refs[1].0.ends_with("sdk/lib/core/core.dart"), "{refs:?}");
    assert_eq!(refs[0], ("lib/a.dart".to_string(), 3, 45));
}

#[test]
fn simbolos_do_workspace_incluem_arquivos_fechados_do_projeto() {
    let mut p = Projeto::novo("nav-workspace");
    p.gravar(
        "lib/fechado.dart",
        "class CarregadorRemoto {}\nvoid carregarTudo() {}\n",
    );
    p.gravar("lib/outro.dart", "int nada = 0;\n");
    p.abrir("lib/aberto.dart", "void carregarDados() {}\n");
    // Só os do projeto (o SDK e os pacotes também entram, depois).
    let do_projeto = |r: &serde_json::Value| -> Vec<String> {
        r["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["location"]["uri"].as_str().unwrap().contains("nav-workspace"))
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect()
    };
    let r = p.requisitar("workspace/symbol", json!({"query": "carregar"}));
    // Na ordem dos arquivos do projeto e, por arquivo, classes antes de
    // funções; executáveis com o sufixo `()`.
    assert_eq!(do_projeto(&r), vec!["carregarDados()", "CarregadorRemoto", "carregarTudo()"]);
    // Aproximado: iniciais de palavras.
    let r = p.requisitar("workspace/symbol", json!({"query": "cR"}));
    assert_eq!(do_projeto(&r), vec!["CarregadorRemoto"]);
}
