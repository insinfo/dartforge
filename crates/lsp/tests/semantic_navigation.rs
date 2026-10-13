//! `definition`, `hover` e `references` pela identidade semântica
//! (`crates/lsp/src/project.rs`), por JSON-RPC: matriz por forma (local,
//! parâmetro, função local, membro herdado, campo genérico, getter,
//! construtores, prefixo, enum, extensão, tipo, parâmetro de tipo, SDK,
//! documentação), com comentários, partes e textos abertos ainda não salvos.

mod common;

use common::Project;
use serde_json::{Value, json};

/// Linha e coluna UTF-16 do início da `n`-ésima ocorrência de `needle`.
fn location(text: &str, needle: &str, n: usize) -> (u32, u32) {
    let i = text
        .match_indices(needle)
        .nth(n)
        .unwrap_or_else(|| panic!("sem {needle:?} #{n}"))
        .0;
    let before = &text[..i];
    (
        before.matches('\n').count() as u32,
        before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    )
}

fn start(l: u32, c: u32) -> Value {
    json!({"line": l, "character": c})
}

fn request(
    p: &mut Project,
    method: &str,
    rel: &str,
    text: &str,
    needle: &str,
    n: usize,
    extra: Value,
) -> Value {
    let (l, c) = location(text, needle, n);
    p.in_position(method, rel, l, c, extra)["result"].clone()
}

/// (uri, início) da definição.
fn definition(p: &mut Project, rel: &str, text: &str, needle: &str, n: usize) -> (String, Value) {
    let r = request(
        p,
        "textDocument/definition",
        rel,
        text,
        needle,
        n,
        json!({}),
    );
    assert!(r.is_object(), "sem definição em {needle:?}: {r}");
    (
        r["uri"].as_str().unwrap().to_string(),
        r["range"]["start"].clone(),
    )
}

fn hover(p: &mut Project, rel: &str, text: &str, needle: &str, n: usize) -> String {
    let r = request(p, "textDocument/hover", rel, text, needle, n, json!({}));
    r["contents"]
        .as_str()
        .unwrap_or_else(|| panic!("sem hover em {needle:?}: {r}"))
        .to_string()
}

/// (uri relativa ao projeto, linha, coluna) de cada referência.
fn references(
    p: &mut Project,
    rel: &str,
    text: &str,
    needle: &str,
    n: usize,
    include: bool,
) -> Vec<(String, u32, u32)> {
    let r = request(
        p,
        "textDocument/references",
        rel,
        text,
        needle,
        n,
        json!({"context": {"includeDeclaration": include}}),
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

const MODEL: &str = "/// Algo que se compara.
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

const USE: &str = "import 'modelo.dart';
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
fn definition_by_shape() {
    let mut p = Project::new("nav-definicao");
    // O modelo fica só no disco; o uso está aberto.
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    let model = p.uri("lib/modelo.dart");
    let usage = p.uri("lib/uso.dart");
    let em = |t: &str, a: &str, n: usize| {
        let (l, c) = location(t, a, n);
        start(l, c)
    };
    let cases: &[(&str, usize, &str, &str, usize)] = &[
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
    for &(needle, n, file, target, dn) in cases {
        let (uri, pos) = definition(&mut p, "lib/uso.dart", USE, needle, n);
        let (expected_uri, text) = if file == "modelo" {
            (&model, MODEL)
        } else {
            (&usage, USE)
        };
        assert_eq!(&uri, expected_uri, "{needle}");
        assert_eq!(pos, em(text, target, dn), "{needle}");
    }
    // Do modelo: parâmetro de tipo e referência de documentação.
    p.open("lib/modelo.dart", MODEL);
    let (uri, pos) = definition(&mut p, "lib/modelo.dart", MODEL, "T valor", 0);
    assert_eq!((uri, pos), (model.clone(), em(MODEL, "T> extends", 0)));
    let (uri, pos) = definition(&mut p, "lib/modelo.dart", MODEL, "x].", 0);
    assert_eq!((uri, pos), (model.clone(), em(MODEL, "x, {int y", 0)));
    let (uri, pos) = definition(&mut p, "lib/modelo.dart", MODEL, "T].", 0);
    assert_eq!((uri, pos), (model.clone(), em(MODEL, "T> extends", 0)));
    // Elemento do SDK: o arquivo do SDK.
    let (uri, _) = definition(&mut p, "lib/uso.dart", USE, "print(n", 0);
    assert!(uri.ends_with("sdk/lib/core/core.dart"), "{uri}");
    // Palavra-chave e espaço: nada.
    assert_eq!(
        request(
            &mut p,
            "textDocument/definition",
            "lib/uso.dart",
            USE,
            "var total",
            0,
            json!({})
        ),
        Value::Null
    );
}

#[test]
fn hover_documentation_like_compute_documentation() {
    let mut p = Project::new("nav-hover-doc");
    let text = "class Base {\n  /// Faz algo com [x].\n  void met(int x) {}\n  final String campo = '';\n}\nclass Filho extends Base {\n  /// O valor.\n  final int valor;\n  Filho(this.valor);\n  @override\n  void met(int x) {}\n  @override\n  String get campo => '';\n}\n";
    p.open("lib/a.dart", text);
    // O parâmetro mostra a documentação do método dono, herdada.
    assert_eq!(hover(&mut p, "lib/a.dart", text, "x) {}", 1), "int x\nType: int\n\nFaz algo com [x].\n\nCopied from `Base`.");
    // `this.x`: o tipo e a documentação do campo.
    assert_eq!(hover(&mut p, "lib/a.dart", text, "valor);", 0), "int valor\nType: int\n\nO valor.");
    // O getter que sobrescreve um campo: o acessor sintético não tem
    // documentação.
    assert_eq!(hover(&mut p, "lib/a.dart", text, "campo =>", 0), "String get campo");
    // O método lido pelo receptor é o `Member` substituído.
    let usage = "void f(List<String> l) {\n  l.add('a');\n}\n";
    p.open("lib/b.dart", usage);
    assert_eq!(hover(&mut p, "lib/b.dart", usage, "add(", 0), "void add(String value)\nType: void Function(String)");
    // O nome da classe em `A.n(…)` mostra o construtor; a declaração de um
    // parâmetro de tipo não tem hover; o alvo de `x += …` mostra o
    // `writeType`.
    let usage = "class A<T> {\n  A.n(T v);\n}\nvoid g<V>(V v) {\n  var a = A.n(1);\n  num x = 1;\n  x += 2;\n  print([a, x]);\n}\n";
    p.open("lib/c.dart", usage);
    assert_eq!(hover(&mut p, "lib/c.dart", usage, "A.n(1", 0), "(new) A<int> A.n(int v)");
    assert_eq!(
        request(&mut p, "textDocument/hover", "lib/c.dart", usage, "V>(V", 0, json!({})),
        Value::Null
    );
    assert_eq!(hover(&mut p, "lib/c.dart", usage, "x += 2", 0), "num x\nType: num");
}

#[test]
fn hover_by_shape_with_documentation() {
    let mut p = Project::new("nav-hover");
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    let cases: &[(&str, usize, &str)] = &[
        // A sobrescrita sem documentação herda a do membro sobrescrito.
        (
            "met(1",
            0,
            // Três parâmetros: um por linha, como o `multiline` do analyzer;
            // numa chamada, o tipo da invocação.
            "void met(\n  int x, {\n  int y = 0,\n  required String nome,\n})\nType: void Function(int, {required String nome, int y})\n\nFaz algo com [x].\n\nCopied from `Base`.",
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
    for &(needle, n, expected) in cases {
        assert_eq!(
            hover(&mut p, "lib/uso.dart", USE, needle, n),
            expected,
            "{needle}"
        );
    }
    let constructor = hover(&mut p, "lib/uso.dart", USE, "vazia(3", 0);
    // O `ConstructorMember`: os argumentos do tipo criado.
    assert_eq!(constructor, "(new) Caixa<int> Caixa.vazia(int v)");
    // Markdown: bloco de código, tipo e documentação depois de `---`.
    p.restart(json!({"capabilities": {"textDocument": {"hover": {"contentFormat": ["markdown"]}}}}));
    let r = request(
        &mut p,
        "textDocument/hover",
        "lib/uso.dart",
        USE,
        "valor + c",
        0,
        json!({}),
    );
    assert_eq!(
        r["contents"]["value"],
        "```dart\nT valor\n```\nType: `int`\n\n*package:projeto/modelo.dart*\n\n---\nO valor guardado."
    );
    // O intervalo é o do nome sob o cursor.
    let (l, c) = location(USE, "valor + c", 0);
    assert_eq!(r["range"]["start"], start(l, c));
    assert_eq!(r["range"]["end"], start(l, c + 5));
    // Prefixo e palavra-chave não têm hover.
    assert_eq!(
        request(
            &mut p,
            "textDocument/hover",
            "lib/uso.dart",
            USE,
            "m.contador",
            0,
            json!({})
        ),
        Value::Null
    );
    assert_eq!(
        request(
            &mut p,
            "textDocument/hover",
            "lib/uso.dart",
            USE,
            "var k",
            0,
            json!({})
        ),
        Value::Null
    );
}

#[test]
fn open_texts_and_parts() {
    let mut p = Project::new("nav-abertos");
    p.write("lib/modelo.dart", MODEL);
    p.open("lib/uso.dart", USE);
    // O modelo aberto e alterado sem salvar: a definição e o hover seguem
    // o buffer, não o disco.
    let changed = MODEL.replace("int contador = 0;", "\n/// Quantos.\ndouble contador = 0;");
    p.open("lib/modelo.dart", &changed);
    p.write("lib/modelo.dart", MODEL);
    let (_, pos) = definition(&mut p, "lib/uso.dart", USE, "contador++", 0);
    let (l, c) = location(&changed, "contador =", 0);
    assert_eq!(pos, start(l, c));
    assert_eq!(
        hover(&mut p, "lib/uso.dart", USE, "contador++", 0),
        "double contador\nType: double\n\nQuantos."
    );
    // Parte e biblioteca dona.
    let owning = "library partes;\npart 'parte.dart';\n/// Base comum.\nint base = 1;\nint dobro() => metade() * 4;\n";
    let part = "part of 'partes.dart';\nint metade() => base ~/ 2;\n";
    p.write("lib/partes.dart", owning);
    p.open("lib/parte.dart", part);
    let (uri, pos) = definition(&mut p, "lib/parte.dart", part, "base ~/", 0);
    assert_eq!(uri, p.uri("lib/partes.dart"));
    let (l, c) = location(owning, "base =", 0);
    assert_eq!(pos, start(l, c));
    assert_eq!(
        hover(&mut p, "lib/parte.dart", part, "base ~/", 0),
        "int base\nType: int\n\nBase comum."
    );
    p.open("lib/partes.dart", owning);
    let (uri, pos) = definition(&mut p, "lib/partes.dart", owning, "metade()", 0);
    assert_eq!(uri, p.uri("lib/parte.dart"));
    let (l, c) = location(part, "metade", 0);
    assert_eq!(pos, start(l, c));
}

#[test]
fn references_without_false_positives_from_shadow_homonym_and_library() {
    let mut p = Project::new("nav-referencias");
    let a = "int f() => 1;\nvoid g() { f(); f(); }\n/// Veja [f].\nvoid h() {}\n";
    let b = "import 'a.dart';\nvoid x() => print(f());\n";
    // Homônimo noutra biblioteca, que não importa `a.dart`.
    let c = "int f() => 2;\nvoid y() => print(f());\n";
    // Prefixo, `show` e um local homônimo que sombreia.
    let d = "import 'a.dart' as p;\nimport 'a.dart' show f;\nvoid z() { p.f(); int f = 0; print(f); }\n";
    p.write("lib/b.dart", b);
    p.write("lib/c.dart", c);
    p.write("lib/d.dart", d);
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "f()", 0, true);
    // A ordem do servidor do Dart: os outros arquivos pelo caminho, o que
    // declara e a declaração no fim.
    let expected: Vec<(String, u32, u32)> = vec![
        ("lib/b.dart".into(), 1, 18),
        ("lib/d.dart".into(), 1, 21),
        ("lib/d.dart".into(), 2, 13),
        ("lib/a.dart".into(), 1, 11),
        ("lib/a.dart".into(), 1, 16),
        ("lib/a.dart".into(), 2, 10),
        ("lib/a.dart".into(), 0, 4),
    ];
    assert_eq!(refs, expected);
    // Sem a declaração.
    let sem = references(&mut p, "lib/a.dart", a, "f()", 0, false);
    assert_eq!(sem, expected[..expected.len() - 1].to_vec());
    // A partir de um uso num arquivo fechado dá o mesmo conjunto.
    p.open("lib/b.dart", b);
    assert_eq!(
        references(&mut p, "lib/b.dart", b, "f()", 0, true),
        expected
    );
    // O homônimo de `c.dart` só vê a si mesmo.
    p.open("lib/c.dart", c);
    assert_eq!(
        references(&mut p, "lib/c.dart", c, "f()", 1, true),
        vec![
            ("lib/c.dart".to_string(), 1, 18),
            ("lib/c.dart".to_string(), 0, 4)
        ]
    );
    // O local que sombreia só vê a si mesmo.
    p.open("lib/d.dart", d);
    assert_eq!(
        references(&mut p, "lib/d.dart", d, "f = 0", 0, true),
        vec![
            ("lib/d.dart".to_string(), 2, 35),
            ("lib/d.dart".to_string(), 2, 22)
        ]
    );
    // O prefixo: o uso e a declaração, que o servidor do Dart 3.6.2 dá no
    // começo do arquivo (0:0).
    assert_eq!(
        references(&mut p, "lib/d.dart", d, "p.f", 0, true),
        vec![
            ("lib/d.dart".to_string(), 2, 11),
            ("lib/d.dart".to_string(), 0, 0)
        ]
    );
}

#[test]
fn documentation_references_like_the_index() {
    // `[A.n]` resolve para o construtor: o índice do analyzer não cita o
    // `A` dela; `[A.+]` (operador) e `[A]` citam.
    let mut p = Project::new("nav-referencias-doc");
    let a = "class A {\n  A.n();\n  A operator +(A o) => this;\n}\n/// [A.n], [A.+] e [A].\nvoid h() {}\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "A {", 0, false);
    assert!(refs.contains(&("lib/a.dart".to_string(), 4, 12)), "{refs:?}");
    assert!(refs.contains(&("lib/a.dart".to_string(), 4, 20)), "{refs:?}");
    assert!(!refs.contains(&("lib/a.dart".to_string(), 4, 5)), "{refs:?}");
    // O construtor: do fim do prefixo ao fim.
    let refs = references(&mut p, "lib/a.dart", a, "n();", 0, false);
    assert!(refs.contains(&("lib/a.dart".to_string(), 4, 6)), "{refs:?}");
}

#[test]
fn class_references_in_new_and_const_creations() {
    let mut p = Project::new("nav-referencias-criacao");
    let a = "class A {\n  const A.n();\n}\nvoid f() {\n  new A.n();\n  const A.n();\n  A.n();\n}\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "A {", 0, false);
    assert!(refs.contains(&("lib/a.dart".to_string(), 4, 6)), "{refs:?}");
    assert!(refs.contains(&("lib/a.dart".to_string(), 5, 8)), "{refs:?}");
    assert!(refs.contains(&("lib/a.dart".to_string(), 6, 2)), "{refs:?}");
}

#[test]
fn member_references_through_family_and_sdk() {
    let mut p = Project::new("nav-ref-membro");
    let a = "class A { void m() {} }\nclass B extends A { void m() {} }\nclass D { void m() {} }\nvoid t(A a, B b, D d) { a.m(); b.m(); d.m(); print(1); }\n";
    p.open("lib/a.dart", a);
    let refs = references(&mut p, "lib/a.dart", a, "m()", 0, true);
    assert_eq!(
        refs,
        vec![
            ("lib/a.dart".to_string(), 3, 26),
            ("lib/a.dart".to_string(), 3, 33),
            ("lib/a.dart".to_string(), 0, 15),
        ]
    );
    // Elemento do SDK: a declaração no SDK e os usos só no projeto.
    let refs = references(&mut p, "lib/a.dart", a, "print", 0, true);
    assert_eq!(refs.len(), 2, "{refs:?}");
    assert!(refs[1].0.ends_with("sdk/lib/core/core.dart"), "{refs:?}");
    assert_eq!(refs[0], ("lib/a.dart".to_string(), 3, 45));
}

#[test]
fn workspace_symbols_include_closed_project_files() {
    let mut p = Project::new("nav-workspace");
    p.write(
        "lib/fechado.dart",
        "class CarregadorRemoto {}\nvoid carregarTudo() {}\n",
    );
    p.write("lib/outro.dart", "int nada = 0;\n");
    p.open("lib/aberto.dart", "void carregarDados() {}\n");
    // Só os do projeto (o SDK e os pacotes também entram, depois).
    let of_project = |r: &serde_json::Value| -> Vec<String> {
        r["result"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["location"]["uri"].as_str().unwrap().contains("nav-workspace"))
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect()
    };
    let r = p.request("workspace/symbol", json!({"query": "carregar"}));
    // Na ordem dos arquivos do projeto e, por arquivo, classes antes de
    // funções; executáveis com o sufixo `()`.
    assert_eq!(of_project(&r), vec!["carregarDados()", "CarregadorRemoto", "carregarTudo()"]);
    // Aproximado: iniciais de palavras.
    let r = p.request("workspace/symbol", json!({"query": "cR"}));
    assert_eq!(of_project(&r), vec!["CarregadorRemoto"]);
}

/// Num sistema de arquivos que não distingue caixa (macOS, Windows), o
/// editor pode mandar a URI com outra caixa que a do disco: a unidade do
/// documento continua achada (o caminho canônico em toda carga). Onde a
/// caixa conta, o teste não se aplica.
#[test]
fn uri_with_different_case_than_disk() {
    let mut p = Project::new("nav-caixa");
    let text = "void f() {\n  var s = 'a';\n  var n = s.length;\n}\n";
    p.write("lib/a.dart", text);
    let real = p.path("lib/a.dart");
    let other = std::path::PathBuf::from(real.to_string_lossy().replace("/lib/a.dart", "/LIB/a.dart"));
    if !other.exists() {
        return;
    }
    let uri = url::Url::from_file_path(&other).unwrap().to_string();
    p.server.receive(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{
        "textDocument":{"uri":uri,"languageId":"dart","version":1,"text":text}
    }}));
    p.server.pump();
    // O membro pelo tipo do receptor só sai da análise semântica.
    let (l, c) = location(text, "length", 0);
    let r = p.request("textDocument/hover", json!({"textDocument": {"uri": uri}, "position": {"line": l, "character": c + 1}}));
    assert!(r["result"]["contents"].to_string().contains("int get length"), "{r}");
}
