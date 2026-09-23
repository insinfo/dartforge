//! Servidor de desenvolvimento: serve a saída, recarrega o navegador.
//!
//! É o que o `webdev` faz, sem o `build_runner` e sem o `build_web_compilers`
//! (a compilação é nossa). Escrito sobre `std::net`, sem dependência nova:
//! HTTP/1.1 com uma thread por conexão e a recarga por **WebSocket**, como o
//! auto-refresh do `webdev` (o `dwds` é agnóstico de transporte e aceita
//! WebSocket ou SSE — `dwds/lib/src/handlers/socket_connections.dart`); o
//! protocolo está em [`crate::ws`].
//!
//! As regras de serviço são as que o `scripts/fluxo.mjs` já validou contra a
//! aplicação real: recuo para `index.html` **só em navegação** (o navegador
//! manda `Accept: text/html`), para que `fetch` de API continue recebendo 404,
//! que é o que a aplicação veria em produção.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Versão do protocolo de desenvolvimento; o cliente confere ao conectar.
pub const PROTOCOLO: u32 = 1;

/// Mensagem do servidor para o navegador.
///
/// O protocolo é **separado do transporte**: hoje as mensagens saem em
/// quadros de texto de WebSocket, e trocar por SSE (ou acrescentar) não
/// mexeria nem no compilador nem na sessão. O `hot-update` e o `debugger`
/// entram aqui quando existirem — por isso o canal é bidirecional desde já,
/// como no `webdev`/`dwds`, no Vite e no webpack-dev-server.
#[derive(Debug, Clone)]
pub enum Mensagem {
    /// Aperto de mão: versão e geração atual da saída.
    Ola { geracao: u64 },
    /// A compilação terminou e mudou módulos: recarregue.
    Recarregar { geracao: u64 },
    /// A compilação falhou; a mensagem vai para o console do navegador.
    ErroDeCompilacao { mensagem: String },
}

impl Mensagem {
    /// Serializa em JSON à mão (o crate não tem `serde`, e o protocolo tem
    /// três mensagens).
    pub fn json(&self) -> String {
        match self {
            Mensagem::Ola { geracao } => {
                format!("{{\"tipo\":\"ola\",\"protocolo\":{PROTOCOLO},\"geracao\":{geracao}}}")
            }
            Mensagem::Recarregar { geracao } => {
                format!("{{\"tipo\":\"recarregar\",\"geracao\":{geracao}}}")
            }
            Mensagem::ErroDeCompilacao { mensagem } => {
                format!("{{\"tipo\":\"erro\",\"mensagem\":{}}}", texto_json(mensagem))
            }
        }
    }
}

/// Escapa um texto como literal JSON.
fn texto_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Navegadores abertos: cada aba tem um canal, como no modelo do Vite.
#[derive(Default)]
pub struct Recarga {
    inscritos: Mutex<Vec<Sender<Mensagem>>>,
    geracao: Mutex<u64>,
}

impl Recarga {
    /// Avisa todas as abas; as que fecharam saem da lista.
    pub fn enviar(&self, m: Mensagem) {
        let mut v = self.inscritos.lock().unwrap_or_else(|e| e.into_inner());
        v.retain(|s| s.send(m.clone()).is_ok());
    }
    /// Uma compilação mudou módulos: nova geração e recarga.
    pub fn disparar(&self) {
        let g = {
            let mut g = self.geracao.lock().unwrap_or_else(|e| e.into_inner());
            *g += 1;
            *g
        };
        self.enviar(Mensagem::Recarregar { geracao: g });
    }
    /// A compilação falhou: o navegador mostra no console e continua ligado.
    pub fn erro(&self, mensagem: &str) {
        self.enviar(Mensagem::ErroDeCompilacao { mensagem: mensagem.to_string() });
    }
    fn geracao_atual(&self) -> u64 {
        *self.geracao.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn inscrever(&self) -> Receiver<Mensagem> {
        let (tx, rx) = channel();
        self.inscritos.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }
    /// Abas abertas no momento.
    pub fn abertos(&self) -> usize {
        self.inscritos.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Sobe o servidor numa thread e devolve a porta efetiva.
///
/// `saida` é o diretório dos módulos emitidos; `web` é o diretório do projeto
/// com `index.html` e `assets/` (opcional). `porta` 0 escolhe uma livre.
pub fn servir(
    saida: &Path,
    web: Option<&Path>,
    porta: u16,
    recarga: Arc<Recarga>,
) -> Result<u16, String> {
    servir_com_gerados(saida, web, porta, recarga, None)
}

/// Como [`servir`], procurando antes nas saídas do motor de build: um `.css`
/// de `web/` pedido pelo navegador sai da geração corrente (e é a demanda que
/// materializa a ação do `sass_builder`).
pub fn servir_com_gerados(
    saida: &Path,
    web: Option<&Path>,
    porta: u16,
    recarga: Arc<Recarga>,
    gerados: Option<crate::geracao::Provedor>,
) -> Result<u16, String> {
    let ouvinte = TcpListener::bind(("127.0.0.1", porta)).map_err(|e| format!("porta {porta}: {e}"))?;
    let porta = ouvinte.local_addr().map_err(|e| e.to_string())?.port();
    let saida = saida.to_path_buf();
    let web = web.map(|p| p.to_path_buf());
    std::thread::Builder::new()
        .name("dartforge-serve".into())
        .spawn(move || {
            for conexao in ouvinte.incoming() {
                let Ok(fluxo) = conexao else { continue };
                let (saida, web, recarga, gerados) = (saida.clone(), web.clone(), recarga.clone(), gerados.clone());
                // Uma thread por conexão: o WebSocket fica aberto enquanto a aba viver.
                let _ = std::thread::Builder::new()
                    .stack_size(256 * 1024)
                    .spawn(move || atender(fluxo, &saida, web.as_deref(), &recarga, gerados.as_ref()));
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(porta)
}

fn atender(
    mut fluxo: TcpStream,
    saida: &Path,
    web: Option<&Path>,
    recarga: &Recarga,
    gerados: Option<&crate::geracao::Provedor>,
) {
    let mut leitor = BufReader::new(match fluxo.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    let mut pedido = String::new();
    if leitor.read_line(&mut pedido).is_err() {
        return;
    }
    let mut aceita_html = false;
    let mut chave_ws: Option<String> = None;
    loop {
        let mut linha = String::new();
        match leitor.read_line(&mut linha) {
            Ok(0) => break,
            Ok(_) => {
                if linha.trim().is_empty() {
                    break;
                }
                let baixa = linha.to_ascii_lowercase();
                if let Some(v) = baixa.strip_prefix("accept:") {
                    aceita_html = v.contains("text/html");
                }
                if baixa.starts_with("sec-websocket-key:") {
                    chave_ws = linha.split(':').nth(1).map(|v| v.trim().to_string());
                }
            }
            Err(_) => return,
        }
    }
    let mut partes = pedido.split_whitespace();
    let metodo = partes.next().unwrap_or("");
    let alvo = partes.next().unwrap_or("/");
    if metodo != "GET" && metodo != "HEAD" {
        let _ = responder(&mut fluxo, 405, "text/plain; charset=utf-8", b"metodo nao suportado");
        return;
    }
    let caminho = alvo.split(['?', '#']).next().unwrap_or("/");

    // Canal de recarga: WebSocket aberto enquanto a aba viver, um quadro de
    // texto por compilação (o auto-refresh do `webdev` usa o mesmo desenho).
    if caminho == ROTA_RECARGA {
        let Some(chave) = chave_ws else {
            let _ = responder(&mut fluxo, 400, "text/plain; charset=utf-8", b"esperava WebSocket");
            return;
        };
        if crate::ws::apertar_mao(&mut fluxo, &chave).is_err() {
            return;
        }
        let rx = recarga.inscrever();
        let ola = Mensagem::Ola { geracao: recarga.geracao_atual() };
        if crate::ws::quadro_texto(&mut fluxo, &ola.json()).is_err() {
            return;
        }
        while let Ok(m) = rx.recv() {
            if crate::ws::quadro_texto(&mut fluxo, &m.json()).is_err() {
                return;
            }
        }
        return;
    }

    let rel = caminho.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    // `..` no caminho não sai do diretório servido.
    if rel.split('/').any(|p| p == "..") {
        let _ = responder(&mut fluxo, 400, "text/plain; charset=utf-8", b"caminho invalido");
        return;
    }

    if rel == "index.html" {
        match index_html(saida, web) {
            Some(html) => {
                let _ = responder(&mut fluxo, 200, "text/html; charset=utf-8", html.as_bytes());
            }
            None => {
                let _ = responder(&mut fluxo, 404, "text/plain; charset=utf-8", b"sem index.html");
            }
        }
        return;
    }

    // Saída gerada pelo motor de build (caminho natural sob `web/`).
    if let (Some(pv), Some(w)) = (gerados, web) {
        let natural = w.join(rel);
        if let Some(dados) = pv(&natural) {
            let _ = responder(&mut fluxo, 200, tipo_mime(&natural), &dados);
            return;
        }
    }
    // O arquivo vem da saída; se não estiver lá, do diretório `web` do projeto.
    let mut arquivo = saida.join(rel);
    if !arquivo.is_file() {
        if let Some(w) = web {
            let alternativo = w.join(rel);
            if alternativo.is_file() {
                arquivo = alternativo;
            }
        }
    }
    if !arquivo.is_file() {
        // Recuo de SPA só para navegação: `fetch` de API continua com 404.
        let sem_extensao = !rel.rsplit('/').next().unwrap_or("").contains('.');
        if aceita_html && sem_extensao {
            if let Some(html) = index_html(saida, web) {
                let _ = responder(&mut fluxo, 200, "text/html; charset=utf-8", html.as_bytes());
                return;
            }
        }
        let _ = responder(&mut fluxo, 404, "text/plain; charset=utf-8", b"nao encontrado");
        return;
    }
    let mut dados = Vec::new();
    if std::fs::File::open(&arquivo).and_then(|mut f| f.read_to_end(&mut dados)).is_err() {
        let _ = responder(&mut fluxo, 500, "text/plain; charset=utf-8", b"erro de leitura");
        return;
    }
    let _ = responder(&mut fluxo, 200, tipo_mime(&arquivo), &dados);
}

/// Rota do canal de recarga (fora do espaço de nomes da aplicação).
const ROTA_RECARGA: &str = "/__dartforge_recarga";

/// Script injetado no `index.html`: recarrega a página a cada compilação.
const SCRIPT_RECARGA: &str = "<script>
(function () {
  var geracao = null;
  function conectar() {
    var p = location.protocol === 'https:' ? 'wss://' : 'ws://';
    var s = new WebSocket(p + location.host + '/__dartforge_recarga');
    s.onmessage = function (ev) {
      var m; try { m = JSON.parse(ev.data); } catch (e) { return; }
      if (m.tipo === 'ola') { if (geracao !== null && m.geracao !== geracao) location.reload(); geracao = m.geracao; }
      else if (m.tipo === 'recarregar') location.reload();
      else if (m.tipo === 'erro') console.error('[dartforge] ' + m.mensagem);
    };
    s.onclose = function () { setTimeout(conectar, 500); };
  }
  conectar();
})();
</script>
";

/// `index.html` servido: o do projeto com o `main.dart.js` trocado pelo nosso
/// módulo e o script de recarga; ou, se o projeto não tiver um, um mínimo.
fn index_html(saida: &Path, web: Option<&Path>) -> Option<String> {
    let proprio = saida.join("index.html");
    let fonte = web
        .map(|w| w.join("index.html"))
        .filter(|p| p.is_file())
        .or(Some(proprio).filter(|p| p.is_file()))
        .and_then(|p| std::fs::read_to_string(p).ok());
    let Some(fonte) = fonte else {
        if !saida.join("main.mjs").is_file() {
            return None;
        }
        return Some(format!(
            "<!DOCTYPE html>\n<html lang=\"pt-br\">\n<head>\n<meta charset=\"utf-8\">\n<title>DartForge</title>\n{SCRIPT_RECARGA}<script type=\"module\" src=\"main.mjs\"></script>\n</head>\n<body></body>\n</html>\n"
        ));
    };
    // O bootstrap oficial sai; entra o nosso módulo.
    let mut html = String::with_capacity(fonte.len() + 256);
    let mut resto = fonte.as_str();
    while let Some(i) = resto.find("<script") {
        let fim = match resto[i..].find("</script>") {
            Some(f) => i + f + "</script>".len(),
            None => break,
        };
        let tag = &resto[i..fim];
        if tag.contains("main.dart.js") {
            html.push_str(&resto[..i]);
            resto = &resto[fim..];
        } else {
            html.push_str(&resto[..fim]);
            resto = &resto[fim..];
        }
    }
    html.push_str(resto);
    let entrada = format!("{SCRIPT_RECARGA}<script type=\"module\" src=\"main.mjs\"></script>\n");
    match html.find("</head>") {
        Some(i) => html.insert_str(i, &entrada),
        None => html.push_str(&entrada),
    }
    Some(html)
}

fn responder(fluxo: &mut TcpStream, codigo: u16, tipo: &str, corpo: &[u8]) -> std::io::Result<()> {
    let razao = match codigo {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    let cabecalho = format!(
        "HTTP/1.1 {codigo} {razao}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        corpo.len()
    );
    fluxo.write_all(cabecalho.as_bytes())?;
    fluxo.write_all(corpo)?;
    fluxo.flush()
}

fn tipo_mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O `index.html` do projeto perde o bootstrap oficial e ganha o nosso
    /// módulo mais o script de recarga.
    #[test]
    fn index_troca_bootstrap_e_injeta_recarga() {
        // `tempfile` apaga o diretório no `Drop`, inclusive quando uma asserção falha.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(
            dir.join("index.html"),
            "<html><head><title>x</title><script defer src=\"main.dart.js\"></script></head><body></body></html>",
        )
        .unwrap();
        let html = index_html(dir, Some(dir)).expect("index");
        assert!(!html.contains("main.dart.js"), "{html}");
        assert!(html.contains("src=\"main.mjs\""), "{html}");
        assert!(html.contains(ROTA_RECARGA), "{html}");
        assert!(html.contains("<title>x</title>"));
    }

    /// Sem `index.html` e sem `main.mjs` não há o que servir.
    #[test]
    fn sem_saida_nao_ha_index() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(index_html(tmp.path(), None).is_none());
    }
}
