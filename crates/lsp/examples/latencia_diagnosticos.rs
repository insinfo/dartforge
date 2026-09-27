//! Mede a latência dos diagnósticos do LSP (imediatos e tipados) por arquivo.
//!
//! `cargo run -q --release -p dartforge-lsp --example latencia_diagnosticos -- <arquivo.dart>...`
//!
//! Para cada arquivo, pelo protocolo (`didOpen`/`didChange` em JSON, como
//! um editor): abre, espera a publicação tipada, aplica 10 edições
//! incrementais esperando cada uma, e uma rajada de 20 edições sem esperar.
//! O SDK é o de `SdkLayout::discover` (`DARTFORGE_SDK_LIB`, `DART_SDK`,
//! `PATH`). Imprime:
//!
//! * `imediato`: tempo do `bombear` do `didChange` (parser + verificadores
//!   locais), a publicação que sai na hora;
//! * `tipado`: do `didChange` até a publicação tipada (análise do
//!   `dartforge analyze` em segundo plano), mediana e máximo;
//! * a rajada: quantas publicações tipadas saíram (a coalescência quer 1) e
//!   quanto tempo até a última;
//! * memória viva acima da base depois de fechar tudo (nada retido).
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_lsp::{AnalisadorSemantico, Servidor};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const LIMITE: Duration = Duration::from_secs(600);

fn edicao(uri: &str, versao: i64) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {
            "textDocument": {"uri": uri, "version": versao},
            "contentChanges": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "text": " ",
            }],
        },
    })
}

/// Espera a publicação tipada de `uri` (as imediatas saem no `bombear`).
fn esperar_tipada(servidor: &mut Servidor<AnalisadorSemantico>, uri: &str) -> Option<Value> {
    servidor
        .aguardar_diagnosticos(LIMITE)
        .into_iter()
        .rev()
        .find(|m| m["params"]["uri"] == uri)
}

/// Compara a publicação tipada com o que o `dartforge analyze` publica para
/// o mesmo arquivo (mesmo caminho de código: `Motor` + `diagnosticos_json`),
/// por (código, linha, coluna).
fn conferir_com_analyze(arquivo: &std::path::Path, publicacao: &Value) -> String {
    let mut raiz = arquivo.parent().unwrap().to_path_buf();
    while !raiz.join("pubspec.yaml").is_file() {
        match raiz.parent() {
            Some(p) => raiz = p.to_path_buf(),
            None => {
                raiz = arquivo.parent().unwrap().to_path_buf();
                break;
            }
        }
    }
    let motor = dartforge_paridade::analise::Motor::descobrir().expect("SDK");
    let config = raiz.join(".dart_tool/package_config.json");
    let config = config.is_file().then_some(config);
    let opcoes = dartforge_paridade::filtros::Opcoes::ler(&raiz);
    let analise = motor.analisar(&raiz, &[arquivo.to_path_buf()], config.as_deref());
    let mut cli: Vec<(String, u64, u64)> =
        dartforge_paridade::diagnosticos_json(&analise, &raiz, &opcoes, true)
            .iter()
            .map(|d| {
                (
                    d.code.to_lowercase(),
                    d.location.range.start.line as u64 - 1,
                    d.location.range.start.column as u64 - 1,
                )
            })
            .collect();
    let mut lsp: Vec<(String, u64, u64)> = publicacao["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"]
                    .as_str()
                    .unwrap_or("dartforge_sem_codigo")
                    .to_string(),
                d["range"]["start"]["line"].as_u64().unwrap(),
                d["range"]["start"]["character"].as_u64().unwrap(),
            )
        })
        .collect();
    cli.sort();
    lsp.sort();
    let semanticos = lsp
        .iter()
        .filter(|(c, _, _)| dartforge_analise::publicacao::verificados().contains(&c.as_str()))
        .count();
    let so_cli: Vec<_> = cli.iter().filter(|x| !lsp.contains(x)).take(5).collect();
    let so_lsp: Vec<_> = lsp.iter().filter(|x| !cli.contains(x)).take(5).collect();
    format!(
        "paridade com `dartforge analyze`: analyze {} / LSP {} ({} semânticos publicados); só no analyze {:?}; só no LSP {:?}",
        cli.len(),
        lsp.len(),
        semanticos,
        so_cli,
        so_lsp
    )
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn main() {
    let arquivos: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    let base = dartforge_instrument::live_bytes();
    let mut servidor = Servidor::com_analisador(AnalisadorSemantico::descobrir());
    servidor.receber(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
    servidor.bombear();
    for arquivo in &arquivos {
        let arquivo = std::fs::canonicalize(arquivo).expect("arquivo existe");
        let texto = std::fs::read_to_string(&arquivo).expect("texto");
        let uri = url::Url::from_file_path(&arquivo).unwrap().to_string();
        let linhas = texto.lines().count();

        let t = Instant::now();
        servidor.receber(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": 1, "text": texto}},
        }));
        servidor.bombear();
        let abrir_imediato = t.elapsed();
        let tipada = esperar_tipada(&mut servidor, &uri);
        let abrir_tipado = t.elapsed();
        let n = tipada
            .as_ref()
            .and_then(|p| p["params"]["diagnostics"].as_array().map(Vec::len));
        let paridade = tipada.as_ref().map(|p| conferir_com_analyze(&arquivo, p));

        let mut imediatos = Vec::new();
        let mut tipados = Vec::new();
        for v in 2..12 {
            let t = Instant::now();
            servidor.receber(edicao(&uri, v));
            servidor.bombear();
            imediatos.push(t.elapsed());
            esperar_tipada(&mut servidor, &uri).expect("publicação tipada");
            tipados.push(t.elapsed());
        }
        imediatos.sort();
        tipados.sort();

        // Rajada: 20 edições seguidas, sem esperar a análise.
        let t = Instant::now();
        let mut publicadas = 0;
        for v in 12..32 {
            servidor.receber(edicao(&uri, v));
            publicadas += servidor
                .bombear()
                .iter()
                .filter(|m| m["params"]["uri"] == uri)
                .count()
                .saturating_sub(1);
        }
        let finais = servidor.aguardar_diagnosticos(LIMITE);
        publicadas += finais.iter().filter(|m| m["params"]["uri"] == uri).count();
        let rajada = t.elapsed();
        let ultima_versao = finais
            .last()
            .map(|m| m["params"]["version"].clone())
            .unwrap_or(Value::Null);

        println!(
            "{} ({} bytes, {linhas} linhas)",
            arquivo.display(),
            texto.len()
        );
        println!(
            "  abrir: imediato {:.1} ms; tipado {:.1} ms ({} diagnósticos publicados)",
            ms(abrir_imediato),
            ms(abrir_tipado),
            n.map_or("?".to_string(), |n| n.to_string())
        );
        if let Some(p) = paridade {
            println!("  {p}");
        }
        println!(
            "  10 edições: imediato mediana {:.1} ms (máx {:.1}); tipado mediana {:.1} ms (mín {:.1}, máx {:.1})",
            ms(imediatos[imediatos.len() / 2]),
            ms(imediatos[imediatos.len() - 1]),
            ms(tipados[tipados.len() / 2]),
            ms(tipados[0]),
            ms(tipados[tipados.len() - 1])
        );
        println!(
            "  rajada de 20 edições: {publicadas} publicação(ões) tipada(s), última da versão {ultima_versao}, {:.1} ms até ela; {} resultados velhos descartados até aqui",
            ms(rajada),
            servidor.tipados_descartados()
        );
        servidor.receber(json!({
            "jsonrpc": "2.0", "method": "textDocument/didClose",
            "params": {"textDocument": {"uri": uri}},
        }));
        servidor.bombear();
        servidor.aguardar_diagnosticos(LIMITE);
    }
    println!(
        "vivo acima da base com o servidor ainda aberto e os documentos fechados: {:.2} MiB",
        dartforge_instrument::live_bytes().saturating_sub(base) as f64 / (1024.0 * 1024.0)
    );
}
