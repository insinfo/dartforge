//! Sessão semântica limitada (`crates/lsp/src/sessao.rs`) pelo protocolo:
//! consultas sobre a mesma versão reaproveitam o programa carregado; uma
//! edição, um arquivo do disco alterado ou criado o descartam; o orçamento
//! zero nunca retém. A memória é medida à parte, em `sessao_memoria.rs`.

mod comum;

use comum::Projeto;
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, EstatisticasSessao, Servidor};
use serde_json::{Value, json};

fn est(p: &Projeto) -> EstatisticasSessao {
    p.servidor.analisador().estatisticas_da_sessao()
}

fn hover(p: &mut Projeto, rel: &str, l: u32, c: u32) -> Value {
    p.na_posicao("textDocument/hover", rel, l, c, json!({}))["result"].clone()
}

fn referencias(p: &mut Projeto, rel: &str, l: u32, c: u32) -> Vec<String> {
    let r = p.na_posicao(
        "textDocument/references",
        rel,
        l,
        c,
        json!({"context": {"includeDeclaration": true}}),
    );
    let base = format!("{}/", p.uri(""));
    r["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{r}"))
        .iter()
        .map(|l| {
            format!(
                "{}:{}:{}",
                l["uri"].as_str().unwrap().trim_start_matches(&base),
                l["range"]["start"]["line"],
                l["range"]["start"]["character"]
            )
        })
        .collect()
}

#[test]
fn reaproveita_entre_consultas_e_cai_na_edicao() {
    let mut p = Projeto::novo("sessao-reuso");
    let a = "int f() => 1;\nint g() => f() + f();\n";
    p.gravar("lib/b.dart", "import 'a.dart';\nint h() => f();\n");
    p.abrir("lib/a.dart", a);
    assert_eq!(est(&p), EstatisticasSessao::default());
    // Biblioteca: carrega uma vez, reaproveita na definição.
    assert_eq!(
        hover(&mut p, "lib/a.dart", 1, 11),
        json!({"contents": "int f()\nType: int Function()", "range": {"start": {"line": 1, "character": 11}, "end": {"line": 1, "character": 12}}})
    );
    p.na_posicao("textDocument/definition", "lib/a.dart", 1, 11, json!({}));
    let e = est(&p);
    assert_eq!((e.carregadas, e.reaproveitadas), (1, 1), "{e:?}");
    // Projeto inteiro (outro escopo): carrega; prepareRename, rename, hover
    // e codeAction na mesma versão reaproveitam.
    assert_eq!(referencias(&mut p, "lib/a.dart", 1, 11).len(), 4);
    p.na_posicao("textDocument/prepareRename", "lib/a.dart", 1, 11, json!({}));
    p.na_posicao(
        "textDocument/rename",
        "lib/a.dart",
        1,
        11,
        json!({"newName": "um"}),
    );
    hover(&mut p, "lib/a.dart", 1, 11);
    p.requisitar(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": p.uri("lib/a.dart")}, "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 0}}, "context": {"diagnostics": []}}),
    );
    let e = est(&p);
    assert_eq!((e.carregadas, e.reaproveitadas), (2, 5), "{e:?}");
    assert!(e.fonte_retida > 0);
    // A edição descarta na hora; a consulta seguinte vê o texto novo.
    let antes = est(&p).invalidadas;
    p.mudar("lib/a.dart", 2, "double f() => 1;\nint g() => 2;\n");
    let e = est(&p);
    assert_eq!((e.invalidadas, e.fonte_retida), (antes + 1, 0), "{e:?}");
    assert_eq!(hover(&mut p, "lib/a.dart", 0, 7)["contents"], "double f()");
    assert_eq!(est(&p).carregadas, 3);
}

#[test]
fn disco_alterado_ou_arquivo_novo_recarrega() {
    let mut p = Projeto::novo("sessao-disco");
    let a = "int f() => 1;\n";
    p.gravar("lib/b.dart", "import 'a.dart';\nint h() => f();\n");
    p.abrir("lib/a.dart", a);
    // A ordem do servidor do Dart: os outros arquivos, e o da declaração
    // (com ela por último) no fim.
    assert_eq!(
        referencias(&mut p, "lib/a.dart", 0, 4),
        vec!["lib/b.dart:1:11", "lib/a.dart:0:4"]
    );
    // O mesmo pedido reaproveita.
    referencias(&mut p, "lib/a.dart", 0, 4);
    assert_eq!(est(&p).reaproveitadas, 1);
    // Arquivo fechado alterado no disco (outra ferramenta, `git checkout`).
    p.gravar("lib/b.dart", "import 'a.dart';\nint h() => f() + f();\n");
    assert_eq!(
        referencias(&mut p, "lib/a.dart", 0, 4),
        vec!["lib/b.dart:1:11", "lib/b.dart:1:17", "lib/a.dart:0:4"]
    );
    // Arquivo novo no projeto.
    p.gravar("lib/c.dart", "import 'a.dart';\nvar x = f();\n");
    assert_eq!(
        referencias(&mut p, "lib/a.dart", 0, 4),
        vec![
            "lib/b.dart:1:11",
            "lib/b.dart:1:17",
            "lib/c.dart:1:8",
            "lib/a.dart:0:4"
        ]
    );
    let e = est(&p);
    assert_eq!((e.carregadas, e.reaproveitadas), (3, 1), "{e:?}");
}

#[test]
fn orcamento_zero_nunca_retem() {
    let mut p = Projeto::novo("sessao-orcamento");
    let sdk = SdkLayout::load(&p.raiz.join("sdk/lib"), "dartdevc").unwrap();
    p.servidor =
        Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)).com_orcamento_de_sessao(0));
    p.requisitar("initialize", json!({"capabilities": {}}));
    p.abrir("lib/a.dart", "int f() => 1;\nint g() => f();\n");
    for _ in 0..3 {
        assert_eq!(hover(&mut p, "lib/a.dart", 1, 11)["contents"], "int f()\nType: int Function()");
    }
    let e = est(&p);
    assert_eq!(
        (
            e.carregadas,
            e.reaproveitadas,
            e.acima_do_orcamento,
            e.fonte_retida
        ),
        (3, 0, 3, 0),
        "{e:?}"
    );
}
