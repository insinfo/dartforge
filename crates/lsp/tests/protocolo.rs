//! Aceite do protocolo LSP por bytes no `stdin`/`stdout` do servidor.
//!
//! Nenhum teste aqui chama o servidor por função: cada sessão gera um
//! processo filho (`CARGO_BIN_EXE_dartforge-lsp`), escreve quadros
//! `Content-Length` no `stdin` e lê quadros do `stdout`. O que vale é o fio,
//! não a API.
//!
//! O teste de platô (`plato_pelo_protocolo`) vive em
//! `tests/plato_protocolo.rs`, num binário separado: o alocador contador é
//! global ao processo e os testes de um mesmo binário rodam em threads
//! paralelas, então a medição de memória precisa do processo só para ela
//! (a razão documentada em `plato_documentos.rs`).

use dartforge_lsp::transporte::{enquadrar, ler_mensagem};
use serde_json::{Value, json};
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// Tempo máximo por espera: binário debug arranca devagar no Windows.
const TEMPO_LIMITE: Duration = Duration::from_secs(20);

/// Leitura da thread que drena o `stdout` do filho continuamente.
///
/// Drenar sem parar importa: se o filho encher o pipe de saída ele bloqueia
/// e a sessão trava — o teste travaria junto, em vez de medir.
enum Leitura {
    Quadro(Value),
    Erro(String),
    Fim,
}

/// Sessão com o servidor em processo filho.
struct Sessao {
    filho: Child,
    entrada: ChildStdin,
    leituras: Receiver<Leitura>,
    /// Notificações e respostas que chegaram mas ainda não foram consumidas.
    pendentes: Vec<Value>,
    /// Quadros com erro de enquadramento (o teste `stdout_limpo` os proíbe).
    erros: Vec<String>,
}

impl Sessao {
    /// Gera o servidor com as variáveis de ambiente dadas.
    fn nova(variaveis: &[(&str, &str)]) -> Self {
        let caminho = env!("CARGO_BIN_EXE_dartforge-lsp");
        let mut comando = Command::new(caminho);
        comando
            .args(["--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (chave, valor) in variaveis {
            comando.env(chave, valor);
        }
        let mut filho = comando.spawn().expect("gerar dartforge-lsp");
        let entrada = filho.stdin.take().expect("stdin do filho");
        let saida = filho.stdout.take().expect("stdout do filho");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut leitor = BufReader::new(saida);
            loop {
                match ler_mensagem(&mut leitor) {
                    Ok(Some(quadro)) => {
                        if tx.send(Leitura::Quadro(quadro)).is_err() {
                            return;
                        }
                    }
                    Ok(None) => {
                        let _ = tx.send(Leitura::Fim);
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(Leitura::Erro(e.to_string()));
                        return;
                    }
                }
            }
        });
        Self {
            filho,
            entrada,
            leituras: rx,
            pendentes: Vec::new(),
            erros: Vec::new(),
        }
    }

    /// Envia uma mensagem JSON como um quadro.
    fn enviar(&mut self, mensagem: &Value) {
        let quadro = enquadrar(mensagem);
        self.entrada
            .write_all(&quadro)
            .expect("escrever no stdin do filho");
        self.entrada.flush().expect("descarregar stdin do filho");
    }

    /// Bombeia leituras até `condicao` achar a mensagem, ou estoura o limite.
    fn esperar_por(&mut self, o_que: &str, mut condicao: impl FnMut(&Value) -> bool) -> Value {
        let inicio = Instant::now();
        loop {
            if let Some(pos) = self.pendentes.iter().position(&mut condicao) {
                return self.pendentes.remove(pos);
            }
            let restante = TEMPO_LIMITE.saturating_sub(inicio.elapsed());
            assert!(
                !restante.is_zero(),
                "tempo esgotado esperando {o_que}; pendentes: {:?}",
                self.pendentes
            );
            match self.leituras.recv_timeout(restante) {
                Ok(Leitura::Quadro(quadro)) => self.pendentes.push(quadro),
                Ok(Leitura::Erro(e)) => self.erros.push(e),
                Ok(Leitura::Fim) => {
                    panic!(
                        "stdout fechou esperando {o_que}; pendentes: {:?}",
                        self.pendentes
                    )
                }
                Err(_) => panic!(
                    "tempo esgotado esperando {o_que}; pendentes: {:?}",
                    self.pendentes
                ),
            }
        }
    }

    /// Resposta com o `id` dado.
    fn resposta(&mut self, id: i64) -> Value {
        self.esperar_por(&format!("resposta {id}"), |m| {
            m.get("id").is_some_and(|v| v.as_i64() == Some(id))
                && (m.get("result").is_some() || m.get("error").is_some())
        })
    }

    /// Última publicação de diagnósticos para a `uri` (descarta as anteriores).
    fn diagnosticos(&mut self, uri: &str) -> Value {
        // Dá ao servidor uma chance de publicar após a edição: espera até
        // haver ao menos uma publicação nova para a uri.
        self.esperar_por(&format!("diagnósticos de {uri}"), |m| {
            m.get("method")
                .is_some_and(|v| v.as_str() == Some("textDocument/publishDiagnostics"))
                && m.get("params")
                    .and_then(|p| p.get("uri"))
                    .and_then(Value::as_str)
                    == Some(uri)
        })
    }

    /// Encerra o protocolo (`shutdown` + `exit`) e devolve o código de saída.
    fn encerrar(mut self, id: i64) -> i32 {
        self.enviar(&json!({"jsonrpc": "2.0", "id": id, "method": "shutdown"}));
        let resposta = self.resposta(id);
        assert_eq!(resposta["result"], Value::Null, "shutdown responde nulo");
        self.enviar(&json!({"jsonrpc": "2.0", "method": "exit"}));
        drop(self.entrada);
        let saida = self.filho.wait().expect("esperar o filho");
        // Drena o que restou para não mascarar erro de enquadramento.
        while let Ok(leitura) = self.leituras.try_recv() {
            match leitura {
                Leitura::Quadro(quadro) => self.pendentes.push(quadro),
                Leitura::Erro(e) => self.erros.push(e),
                Leitura::Fim => break,
            }
        }
        assert!(
            self.erros.is_empty(),
            "stdout com bytes fora do enquadramento: {:?}",
            self.erros
        );
        saida.code().expect("código de saída")
    }
}

/// Aperto de mãos mínimo de toda sessão.
fn iniciar(sessao: &mut Sessao) {
    sessao.enviar(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"processId": null, "rootUri": "file:///tmp", "capabilities": {}},
    }));
    let resposta = sessao.resposta(1);
    assert_eq!(
        resposta["result"]["capabilities"]["textDocumentSync"], 2,
        "capacidade anunciada é sincronização incremental"
    );
    sessao.enviar(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
}

fn publicar(uri: &str, versao: i32, texto: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {"textDocument": {"uri": uri, "languageId": "dart", "version": versao, "text": texto}},
    })
}

fn editar(uri: &str, versao: i32, mudancas: Value) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {"textDocument": {"uri": uri, "version": versao}, "contentChanges": mudancas},
    })
}

fn fechar(uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "textDocument/didClose",
        "params": {"textDocument": {"uri": uri}},
    })
}

/// Lista de diagnósticos da publicação.
fn itens(publicacao: &Value) -> &Vec<Value> {
    publicacao["params"]["diagnostics"]
        .as_array()
        .expect("publishDiagnostics traz lista")
}

/// Transcrição completa pelo fio: dois erros, correção incremental de um,
/// fechamento com lista vazia e saída com código 0.
#[test]
fn sessao_completa() {
    let mut sessao = Sessao::nova(&[]);
    iniciar(&mut sessao);

    let uri = "file:///sessao/teste.dart";
    sessao.enviar(&publicar(
        uri,
        1,
        "void main() { int x = ; }\nvoid outra() { int y = ; }\n",
    ));
    let primeira = sessao.diagnosticos(uri);
    assert_eq!(
        itens(&primeira).len(),
        2,
        "dois erros de sintaxe: {primeira}"
    );
    assert_eq!(primeira["params"]["diagnostics"][0]["source"], "dartforge");
    assert_eq!(
        primeira["params"]["diagnostics"][0]["range"]["start"],
        json!({"line": 0, "character": 22}),
        "span de bytes vira posição UTF-16: {primeira}"
    );
    assert_eq!(
        primeira["params"]["diagnostics"][0]["severity"], 1,
        "sintaxe quebrada é erro: {primeira}"
    );

    // Correção incremental: troca o `;` da linha 0 por `1;` (intervalo UTF-16).
    sessao.enviar(&editar(
        uri,
        2,
        json!([{
            "range": {"start": {"line": 0, "character": 22}, "end": {"line": 0, "character": 23}},
            "text": "1;",
        }]),
    ));
    let segunda = sessao.diagnosticos(uri);
    assert_eq!(itens(&segunda).len(), 1, "um erro restante: {segunda}");
    assert_eq!(
        segunda["params"]["diagnostics"][0]["range"]["start"],
        json!({"line": 1, "character": 23}),
        "o erro que sobrou é o da segunda declaração: {segunda}"
    );

    sessao.enviar(&fechar(uri));
    let terceira = sessao.diagnosticos(uri);
    assert!(
        itens(&terceira).is_empty(),
        "documento fechado publica lista vazia: {terceira}"
    );

    assert_eq!(sessao.encerrar(10), 0, "exit após shutdown sai com 0");
}

/// Colunas contam unidades UTF-16 nos dois sentidos, com CRLF e escape de
/// surrogate solto no mesmo arquivo.
#[test]
fn utf16_correto() {
    let mut sessao = Sessao::nova(&[]);
    iniciar(&mut sessao);

    // `👭` ocupa 4 bytes e 2 unidades UTF-16; o `;` do erro está no byte 38
    // da linha 0 e na unidade UTF-16 36. O `\uD800` é ASCII (escape), e o
    // lexer o aceita como literal válido: não gera diagnóstico.
    let uri = "file:///utf16/emoji.dart";
    sessao.enviar(&publicar(
        uri,
        1,
        "var s = '👭'; void main() { int x = ; }\r\nvar e = '\\uD800';\r\n",
    ));
    let primeira = sessao.diagnosticos(uri);
    assert_eq!(itens(&primeira).len(), 1, "só o `;`: {primeira}");
    assert_eq!(
        primeira["params"]["diagnostics"][0]["range"]["start"],
        json!({"line": 0, "character": 36}),
        "coluna publicada é UTF-16, não byte: {primeira}"
    );
    assert_eq!(
        primeira["params"]["diagnostics"][0]["range"]["end"],
        json!({"line": 0, "character": 37}),
        "fim do span também é UTF-16: {primeira}"
    );

    // Edição incremental endereçada em UTF-16 depois do emoji: trocar o `;`
    // (unidades 36..37) por `1;` conserta; tratando coluna como byte, o corte
    // cairia no `=` e o erro continuaria.
    sessao.enviar(&editar(
        uri,
        2,
        json!([{
            "range": {"start": {"line": 0, "character": 36}, "end": {"line": 0, "character": 37}},
            "text": "1;",
        }]),
    ));
    let segunda = sessao.diagnosticos(uri);
    assert!(
        itens(&segunda).is_empty(),
        "edição UTF-16 acertou o byte: {segunda}"
    );

    assert_eq!(sessao.encerrar(10), 0);
}

/// Com log detalhado ligado, o `stdout` continua sendo só protocolo.
#[test]
fn stdout_limpo() {
    let mut sessao = Sessao::nova(&[("RUST_LOG", "debug")]);
    iniciar(&mut sessao);

    let uri = "file:///limpo/a.dart";
    sessao.enviar(&publicar(uri, 1, "void main() { int x = ; }\n"));
    let primeira = sessao.diagnosticos(uri);
    assert_eq!(itens(&primeira).len(), 1);
    sessao.enviar(&editar(
        uri,
        2,
        json!([{"text": "void main() { int x = 1; }\n"}]),
    ));
    let segunda = sessao.diagnosticos(uri);
    assert!(itens(&segunda).is_empty());
    sessao.enviar(&fechar(uri));
    let terceira = sessao.diagnosticos(uri);
    assert!(itens(&terceira).is_empty());

    assert_eq!(sessao.encerrar(10), 0);
    // `encerrar` já afirmou: nenhum byte fora de `Content-Length` no stdout.
}

/// Duas requisições enfileiradas e cancelamento da segunda antes de a
/// primeira terminar: a resposta dela é `RequestCancelled` (-32800).
#[test]
fn cancelamento() {
    let mut sessao = Sessao::nova(&[]);
    iniciar(&mut sessao);

    // As três mensagens saem em rajada: a leitora enfileira enquanto a
    // principal ainda executa o dormir, e o cancelamento vale de todo jeito
    // porque `bombear` aplica os cancels da fila inteira antes de executar.
    sessao.enviar(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "dartforge/dormir",
        "params": {"ms": 1500},
    }));
    sessao.enviar(&json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}));
    sessao.enviar(&json!({
        "jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 3},
    }));

    let dormir = sessao.resposta(2);
    assert_eq!(
        dormir["result"],
        Value::Null,
        "a primeira termina: {dormir}"
    );
    let cancelada = sessao.resposta(3);
    assert_eq!(
        cancelada["error"]["code"], -32800,
        "a segunda foi cancelada na fila: {cancelada}"
    );

    assert_eq!(sessao.encerrar(4), 0);
}
