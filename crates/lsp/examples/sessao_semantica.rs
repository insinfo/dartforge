//! Mede latência e memória das consultas semânticas com e sem a sessão
//! limitada (`crates/lsp/src/sessao.rs`), juntas, pelo protocolo.
//!
//! `cargo run -q --release -p dartforge-lsp --example sessao_semantica -- <arquivo.dart> [consultas]`
//!
//! Com o SDK de `SdkLayout::discover`, abre o arquivo (o projeto é o
//! diretório com `pubspec.yaml`) e pede, nas primeiras `consultas` posições
//! de identificador (padrão 20), `hover` e `definition` (escopo da
//! biblioteca), e três `references` (projeto inteiro). Mede cada pedido,
//! depois aplica uma edição e mede de novo a primeira consulta. Roda duas
//! vezes: sessão desligada (orçamento 0) e ligada (padrão). Imprime, para
//! cada modo, a mediana e o máximo das latências, o vivo retido durante as
//! consultas (alocador contador) e o vivo depois da edição.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_elements::sdk::SdkLayout;
use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn mediana(v: &mut [Duration]) -> Duration {
    v.sort();
    v.get(v.len() / 2).copied().unwrap_or_default()
}

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

fn mib(b: usize) -> String {
    format!("{:.1} MiB", b as f64 / (1024.0 * 1024.0))
}

/// Posições (linha, coluna UTF-16) dos primeiros identificadores do texto.
fn posicoes(texto: &str, n: usize) -> Vec<(u32, u32)> {
    let Ok(tokens) = dartforge_frontend::lexer::lex(texto) else {
        return Vec::new();
    };
    let tabela = dartforge_lsp::utf16::TabelaLinhas::construir(texto);
    tokens
        .iter()
        .filter(|t| t.kind == dartforge_frontend::token::Kind::Ident)
        .take(n)
        .map(|t| tabela.posicao_de_offset(texto, t.span.start))
        .collect()
}

struct Medida {
    hover: Vec<Duration>,
    definicao: Vec<Duration>,
    referencias: Vec<Duration>,
    primeira_depois_da_edicao: Duration,
    retido: usize,
    depois_da_edicao: usize,
    reaproveitadas: u64,
    carregadas: u64,
    fonte_retida: usize,
}

fn medir(sdk: SdkLayout, uri: &str, texto: &str, n: usize, orcamento: Option<usize>) -> Medida {
    let analisador = AnalisadorSemantico::novo(Some(sdk));
    let analisador = match orcamento {
        Some(mib) => analisador.com_orcamento_de_sessao(mib),
        None => analisador,
    };
    let mut servidor = Servidor::com_analisador(analisador);
    let mut id = 0;
    let mut pedir = |servidor: &mut Servidor<AnalisadorSemantico>,
                     metodo: &str,
                     (l, c): (u32, u32),
                     extra: Value| {
        id += 1;
        let mut params =
            json!({"textDocument": {"uri": uri}, "position": {"line": l, "character": c}});
        if let Value::Object(m) = extra {
            for (k, v) in m {
                params[k] = v;
            }
        }
        servidor.receber(json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params}));
        let inicio = Instant::now();
        servidor.bombear();
        inicio.elapsed()
    };
    servidor.receber(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"capabilities": {}}}),
    );
    servidor.bombear();
    servidor.receber(
        json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
        "textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": texto}}}),
    );
    servidor.bombear();
    servidor.aguardar_diagnosticos(Duration::from_secs(600));
    let base = dartforge_instrument::live_bytes();
    let pos = posicoes(texto, n);
    let mut m = Medida {
        hover: Vec::new(),
        definicao: Vec::new(),
        referencias: Vec::new(),
        primeira_depois_da_edicao: Duration::ZERO,
        retido: 0,
        depois_da_edicao: 0,
        reaproveitadas: 0,
        carregadas: 0,
        fonte_retida: 0,
    };
    for &p in &pos {
        m.hover
            .push(pedir(&mut servidor, "textDocument/hover", p, json!({})));
        m.definicao.push(pedir(
            &mut servidor,
            "textDocument/definition",
            p,
            json!({}),
        ));
        m.retido = m
            .retido
            .max(dartforge_instrument::live_bytes().saturating_sub(base));
    }
    for &p in pos.iter().take(3) {
        let extra = json!({"context": {"includeDeclaration": true}});
        m.referencias
            .push(pedir(&mut servidor, "textDocument/references", p, extra));
        m.retido = m
            .retido
            .max(dartforge_instrument::live_bytes().saturating_sub(base));
    }
    m.fonte_retida = servidor.analisador().estatisticas_da_sessao().fonte_retida;
    servidor.receber(json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {
        "textDocument": {"uri": uri, "version": 2},
        "contentChanges": [{"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "text": " "}]}}));
    servidor.bombear();
    servidor.aguardar_diagnosticos(Duration::from_secs(600));
    m.depois_da_edicao = dartforge_instrument::live_bytes().saturating_sub(base);
    if let Some(&(l, c)) = pos.first() {
        m.primeira_depois_da_edicao = pedir(
            &mut servidor,
            "textDocument/hover",
            (l, c + u32::from(l == 0)),
            json!({}),
        );
    }
    let e = servidor.analisador().estatisticas_da_sessao();
    m.reaproveitadas = e.reaproveitadas;
    m.carregadas = e.carregadas;
    m
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(arquivo) = args.next() else {
        eprintln!("uso: sessao_semantica <arquivo.dart> [consultas]");
        std::process::exit(2);
    };
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(20);
    let caminho = std::fs::canonicalize(&arquivo).expect("arquivo");
    let texto = std::fs::read_to_string(&caminho).expect("leitura");
    let uri = url::Url::from_file_path(&caminho).unwrap().to_string();
    let lib = SdkLayout::discover().expect("SDK do Dart não encontrado");
    let carregar = || SdkLayout::load(&lib, "dartdevc").expect("SDK");
    println!(
        "arquivo: {} ({} bytes), {n} posições",
        caminho.display(),
        texto.len()
    );
    println!(
        "| modo | hover (mediana; máx) | definition (mediana; máx) | references (mediana; máx) | 1ª consulta após edição | fonte retida | vivo retido nas consultas | vivo após a edição | cargas / reaproveitadas |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for (nome, orcamento) in [
        ("sem sessão (orçamento 0)", Some(0)),
        ("com sessão (padrão)", None),
    ] {
        let mut m = medir(carregar(), &uri, &texto, n, orcamento);
        let (hm, hx) = (
            mediana(&mut m.hover),
            m.hover.iter().max().copied().unwrap_or_default(),
        );
        let (dm, dx) = (
            mediana(&mut m.definicao),
            m.definicao.iter().max().copied().unwrap_or_default(),
        );
        let (rm, rx) = (
            mediana(&mut m.referencias),
            m.referencias.iter().max().copied().unwrap_or_default(),
        );
        println!(
            "| {nome} | {}; {} | {}; {} | {}; {} | {} | {} | {} | {} | {} / {} |",
            ms(hm),
            ms(hx),
            ms(dm),
            ms(dx),
            ms(rm),
            ms(rx),
            ms(m.primeira_depois_da_edicao),
            mib(m.fonte_retida),
            mib(m.retido),
            mib(m.depois_da_edicao),
            m.carregadas,
            m.reaproveitadas
        );
    }
}
