//! Mede a latência de `references` e `implementation` pelo protocolo
//! (`Servidor` em processo, SDK real), como o oráculo pede: todos os
//! arquivos `lib/` e `bin/` do projeto abertos, pedidos em identificadores
//! espalhados de um arquivo, com o texto parado e logo depois de uma edição
//! de corpo em outro arquivo.
//!
//! `cargo run -q --release -p dartforge-lsp --example latencia_referencias -- <projeto> <arquivo.dart> [posições]`
//!
//! Imprime a primeira requisição (que paga a carga ampla), a mediana e o
//! máximo das demais.
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn resumo(v: &[Duration]) -> String {
    let mut v = v.to_vec();
    v.sort();
    let m = v.get(v.len() / 2).copied().unwrap_or_default();
    let x = v.last().copied().unwrap_or_default();
    format!("{:.1} ms; {:.1} ms (n={})", m.as_secs_f64() * 1000.0, x.as_secs_f64() * 1000.0, v.len())
}

fn dart_em(d: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(r) = std::fs::read_dir(d) else { return };
    for e in r.flatten() {
        let p = e.path();
        if p.is_dir() {
            dart_em(&p, saida);
        } else if p.extension().is_some_and(|x| x == "dart") {
            saida.push(p);
        }
    }
}

fn uri_de(p: &Path) -> String {
    url::Url::from_file_path(dartforge_elements::config::sem_verbatim(p.to_path_buf())).unwrap().to_string()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(projeto), Some(arquivo)) = (args.next(), args.next()) else {
        eprintln!("uso: latencia_referencias <projeto> <arquivo.dart> [posições]");
        std::process::exit(2);
    };
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let raiz = std::fs::canonicalize(&projeto).expect("projeto");
    let caminho = std::fs::canonicalize(raiz.join(&arquivo)).expect("arquivo");
    let mut arquivos = Vec::new();
    dart_em(&raiz.join("lib"), &mut arquivos);
    dart_em(&raiz.join("bin"), &mut arquivos);
    arquivos.sort();
    let lib = SdkLayout::discover().expect("SDK do Dart não encontrado");
    let sdk = SdkLayout::load_como_analyzer(&lib).expect("SDK");
    let mut servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)));
    let mut id = 0;
    let mut pedir = |servidor: &mut Servidor<AnalisadorSemantico>, metodo: &str, params: Value| {
        id += 1;
        servidor.receber(json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params}));
        let inicio = Instant::now();
        servidor.bombear();
        inicio.elapsed()
    };
    servidor.receber(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
        "rootUri": uri_de(&raiz), "capabilities": {}}}));
    servidor.bombear();
    for a in &arquivos {
        let t = std::fs::read_to_string(a).expect("leitura");
        servidor.receber(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri_de(a), "languageId": "dart", "version": 1, "text": t}}}));
    }
    servidor.bombear();
    servidor.aguardar_diagnosticos(Duration::from_secs(600));

    let texto = std::fs::read_to_string(&caminho).expect("leitura");
    let uri = uri_de(&caminho);
    let tabela = dartforge_lsp::utf16::TabelaLinhas::construir(&texto);
    let tokens = dartforge_frontend::lexer::lex(&texto).unwrap_or_default();
    let idents: Vec<usize> =
        tokens.iter().filter(|t| t.kind == dartforge_frontend::token::Kind::Ident).map(|t| t.span.start).collect();
    let passo = (idents.len() / n.max(1)).max(1);
    let amostra: Vec<usize> = idents.iter().step_by(passo).take(n).copied().collect();
    let pos = |o: usize| {
        let (l, c) = tabela.posicao_de_offset(&texto, o);
        json!({"line": l, "character": c})
    };
    let doc = json!({"uri": uri});
    let (mut refs, mut imps) = (Vec::new(), Vec::new());
    for &a in &amostra {
        refs.push(pedir(
            &mut servidor,
            "textDocument/references",
            json!({"textDocument": doc, "position": pos(a + 1), "context": {"includeDeclaration": true}}),
        ));
        imps.push(pedir(&mut servidor, "textDocument/implementation", json!({"textDocument": doc, "position": pos(a + 1)})));
    }
    // Uma edição de corpo (um espaço no fim) em outro arquivo, seguida de
    // `references`: o que sobrevive à edição.
    let outro = arquivos.iter().find(|a| **a != caminho).cloned().unwrap_or(caminho.clone());
    let texto_outro = std::fs::read_to_string(&outro).expect("leitura");
    let ultima = texto_outro.lines().count() as u32;
    let mut apos_edicao = Vec::new();
    for v in 0..5u32 {
        servidor.receber(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri_de(&outro), "version": 2 + v},
            "contentChanges": [{"range": {"start": {"line": ultima, "character": 0}, "end": {"line": ultima, "character": 0}}, "text": " "}]}}));
        servidor.bombear();
        let a = amostra[v as usize % amostra.len().max(1)];
        apos_edicao.push(pedir(
            &mut servidor,
            "textDocument/references",
            json!({"textDocument": doc, "position": pos(a + 1), "context": {"includeDeclaration": true}}),
        ));
    }
    servidor.aguardar_diagnosticos(Duration::from_secs(600));
    println!("projeto: {} ({} arquivos); arquivo: {}", raiz.display(), arquivos.len(), caminho.display());
    println!("| pedido | primeira | mediana; máximo das demais |");
    println!("|---|---|---|");
    println!("| references | {:.1} ms | {} |", refs[0].as_secs_f64() * 1000.0, resumo(&refs[1..]));
    println!("| implementation | {:.1} ms | {} |", imps[0].as_secs_f64() * 1000.0, resumo(&imps[1..]));
    println!("| references depois de editar outro arquivo | {:.1} ms | {} |", apos_edicao[0].as_secs_f64() * 1000.0, resumo(&apos_edicao[1..]));
}
