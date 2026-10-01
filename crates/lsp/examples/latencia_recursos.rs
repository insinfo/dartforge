//! Mede a latência dos recursos de edição pelo protocolo (`Servidor` em
//! processo, SDK real): `hover`, `completion`, `signatureHelp`,
//! `documentHighlight` e `foldingRange`, com o texto parado e logo depois de
//! cada edição (o caso de quem digita: a sessão semântica é descartada).
//!
//! `cargo run -q --release -p dartforge-lsp --example latencia_recursos -- <arquivo.dart> [posições]`
//!
//! Para cada uma das `posições` (padrão 20) primeiras posições de
//! identificador: `hover`; `completion` com o prefixo de até dois
//! caracteres do identificador; `signatureHelp` depois de cada `(` de
//! chamada; `documentHighlight`. Depois, 10 vezes: uma edição incremental
//! (um espaço no fim do arquivo) seguida de `hover` e de `completion` (a
//! primeira consulta paga a carga da biblioteca). Imprime mediana e máximo.
use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn resumo(v: &mut [Duration]) -> String {
    v.sort();
    let m = v.get(v.len() / 2).copied().unwrap_or_default();
    let x = v.last().copied().unwrap_or_default();
    format!("{:.1} ms; {:.1} ms (n={})", m.as_secs_f64() * 1000.0, x.as_secs_f64() * 1000.0, v.len())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(arquivo) = args.next() else {
        eprintln!("uso: latencia_recursos <arquivo.dart> [posições]");
        std::process::exit(2);
    };
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let caminho = std::fs::canonicalize(&arquivo).expect("arquivo");
    let texto = std::fs::read_to_string(&caminho).expect("leitura");
    let uri = url::Url::from_file_path(dartforge_elements::config::sem_verbatim(caminho.clone())).unwrap().to_string();
    let lib = SdkLayout::discover().expect("SDK do Dart não encontrado");
    let sdk = SdkLayout::load(&lib, "dartdevc").expect("SDK");
    let mut servidor = Servidor::com_analisador(AnalisadorSemantico::novo(Some(sdk)));
    let mut id = 0;
    let mut pedir = |servidor: &mut Servidor<AnalisadorSemantico>, metodo: &str, params: Value| {
        id += 1;
        servidor.receber(json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params}));
        let inicio = Instant::now();
        servidor.bombear();
        inicio.elapsed()
    };
    servidor.receber(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"capabilities": {
        "textDocument": {"completion": {"completionItem": {"snippetSupport": true}}, "foldingRange": {"lineFoldingOnly": true}}}}}));
    servidor.bombear();
    servidor.receber(json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
        "textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": texto}}}));
    servidor.bombear();
    servidor.aguardar_diagnosticos(Duration::from_secs(600));

    let tabela = dartforge_lsp::utf16::TabelaLinhas::construir(&texto);
    let tokens = dartforge_frontend::lexer::lex(&texto).unwrap_or_default();
    let idents: Vec<(usize, usize)> = tokens
        .iter()
        .filter(|t| t.kind == dartforge_frontend::token::Kind::Ident)
        .map(|t| (t.span.start, t.span.end))
        .collect();
    let passo = (idents.len() / n.max(1)).max(1);
    let amostra: Vec<(usize, usize)> = idents.iter().step_by(passo).take(n).copied().collect();
    let pos = |o: usize| {
        let (l, c) = tabela.posicao_de_offset(&texto, o);
        json!({"line": l, "character": c})
    };
    let doc = json!({"uri": uri});
    let (mut hover, mut completar, mut assinatura, mut destaque) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &(a, b) in &amostra {
        hover.push(pedir(&mut servidor, "textDocument/hover", json!({"textDocument": doc, "position": pos(a + 1)})));
        completar.push(pedir(
            &mut servidor,
            "textDocument/completion",
            json!({"textDocument": doc, "position": pos(a + (b - a).min(2))}),
        ));
        destaque.push(pedir(&mut servidor, "textDocument/documentHighlight", json!({"textDocument": doc, "position": pos(a + 1)})));
    }
    for (i, _) in texto.match_indices('(').filter(|(i, _)| *i > 0 && texto.as_bytes()[i - 1].is_ascii_alphanumeric()).take(n) {
        assinatura.push(pedir(
            &mut servidor,
            "textDocument/signatureHelp",
            json!({"textDocument": doc, "position": pos(i + 1), "context": {"triggerKind": 2, "triggerCharacter": "(", "isRetrigger": false}}),
        ));
    }
    let mut dobras = vec![pedir(&mut servidor, "textDocument/foldingRange", json!({"textDocument": doc}))];
    let (mut hover_ed, mut completar_ed) = (Vec::new(), Vec::new());
    let ultima = tabela.total_linhas().saturating_sub(1) as u32;
    for v in 0..10 {
        servidor.receber(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri, "version": 2 + v},
            "contentChanges": [{"range": {"start": {"line": ultima, "character": 0}, "end": {"line": ultima, "character": 0}}, "text": " "}]}}));
        servidor.bombear();
        let (a, b) = amostra[v as usize % amostra.len().max(1)];
        hover_ed.push(pedir(&mut servidor, "textDocument/hover", json!({"textDocument": doc, "position": pos(a + 1)})));
        servidor.receber(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri, "version": 100 + v},
            "contentChanges": [{"range": {"start": {"line": ultima, "character": 0}, "end": {"line": ultima, "character": 0}}, "text": " "}]}}));
        servidor.bombear();
        completar_ed.push(pedir(&mut servidor, "textDocument/completion", json!({"textDocument": doc, "position": pos(a + (b - a).min(2))})));
        dobras.push(pedir(&mut servidor, "textDocument/foldingRange", json!({"textDocument": doc})));
    }
    servidor.aguardar_diagnosticos(Duration::from_secs(600));
    println!("arquivo: {} ({} bytes)", caminho.display(), texto.len());
    println!("| recurso | mediana; máximo |");
    println!("|---|---|");
    println!("| hover (texto parado) | {} |", resumo(&mut hover));
    println!("| completion (texto parado) | {} |", resumo(&mut completar));
    println!("| signatureHelp no `(` | {} |", resumo(&mut assinatura));
    println!("| documentHighlight | {} |", resumo(&mut destaque));
    println!("| foldingRange | {} |", resumo(&mut dobras));
    println!("| hover logo após edição | {} |", resumo(&mut hover_ed));
    println!("| completion logo após edição | {} |", resumo(&mut completar_ed));
}
