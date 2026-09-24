//! O transporte do `dfexec/1` (docs/BUILD-PROTOCOLO.md §1–§2): quadros
//! `u32` big-endian com o tamanho, seguidos de JSON em UTF-8, e o handshake.
//! É o mesmo protocolo e o mesmo executor dos builders (regra governante,
//! item 2); as macros são o serviço `macro.*` dele (§5 lá, e
//! docs/MACROS-PROTOCOLO.md §5 aqui).
use serde_json::{Value, json};
use std::io::{Read, Write};

/// Nome e versão do protocolo.
pub const PROTOCOLO: &str = "dfexec/1";

/// A versão do hospedeiro no handshake.
pub const VERSAO_DO_HOSPEDEIRO: &str = concat!("dartforge-macros-host/", env!("CARGO_PKG_VERSION"));

/// Maior quadro aceito (64 MiB): um quadro maior é defeito do outro lado, não
/// modelo legítimo.
const MAIOR_QUADRO: u32 = 64 << 20;

/// Escreve `m` como um quadro.
pub fn escrever_quadro(w: &mut dyn Write, m: &Value) -> std::io::Result<()> {
    let corpo = serde_json::to_vec(m).map_err(std::io::Error::other)?;
    let n = u32::try_from(corpo.len()).map_err(std::io::Error::other)?;
    w.write_all(&n.to_be_bytes())?;
    w.write_all(&corpo)?;
    w.flush()
}

/// Lê um quadro; `Ok(None)` no fim do fluxo entre quadros.
pub fn ler_quadro(r: &mut dyn Read) -> std::io::Result<Option<Value>> {
    let mut n = [0u8; 4];
    let mut lidos = 0;
    while lidos < 4 {
        match r.read(&mut n[lidos..])? {
            0 if lidos == 0 => return Ok(None),
            0 => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "quadro cortado no tamanho")),
            k => lidos += k,
        }
    }
    let n = u32::from_be_bytes(n);
    if n > MAIOR_QUADRO {
        return Err(std::io::Error::other(format!("quadro de {n} bytes excede o limite")));
    }
    let mut corpo = vec![0u8; n as usize];
    r.read_exact(&mut corpo)?;
    serde_json::from_slice(&corpo).map(Some).map_err(std::io::Error::other)
}

/// Um canal de mensagens JSON já enquadradas (processo, memória).
pub trait Canal: Send {
    fn enviar(&mut self, m: &Value) -> Result<(), String>;
    /// A próxima mensagem; `Err` se o outro lado sumiu.
    fn receber(&mut self) -> Result<Value, String>;
}

/// O `ola` do hospedeiro.
pub fn ola_do_hospedeiro() -> Value {
    json!({"t": "ola", "protocolo": PROTOCOLO, "servicos": ["macro"], "motor": VERSAO_DO_HOSPEDEIRO})
}

/// O que o executor respondeu no handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apresentacao {
    pub executor: String,
    pub abi: String,
    /// `uri#Classe` → nomes de construtor disponíveis no bootstrap.
    pub macros: Vec<(String, Vec<String>)>,
}

/// Confere a resposta ao `ola`: mesmo protocolo e o serviço `macro`.
pub fn conferir_ola(v: &Value) -> Result<Apresentacao, String> {
    if v.get("t").and_then(Value::as_str) != Some("ola") {
        return Err(format!("o executor não respondeu ao handshake: {v}"));
    }
    let p = v.get("protocolo").and_then(Value::as_str).unwrap_or("");
    if p != PROTOCOLO {
        return Err(format!("o executor fala {p}, o hospedeiro fala {PROTOCOLO}"));
    }
    let servicos: Vec<&str> =
        v.get("servicos").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !servicos.contains(&"macro") {
        return Err(format!("o executor não oferece o serviço macro (oferece {servicos:?})"));
    }
    let macros = v
        .get("macros")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|m| {
                    let nome = m.get("macro").and_then(Value::as_str).unwrap_or("").to_string();
                    let cs = m
                        .get("construtores")
                        .and_then(Value::as_array)
                        .map(|c| c.iter().filter_map(Value::as_str).map(str::to_string).collect())
                        .unwrap_or_default();
                    (nome, cs)
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Apresentacao {
        executor: v.get("executor").and_then(Value::as_str).unwrap_or("").to_string(),
        abi: v.get("abi").and_then(Value::as_str).unwrap_or("").to_string(),
        macros,
    })
}

/// Um processo filho que fala `dfexec/1` por stdin/stdout. stderr fica
/// livre (log humano) e é herdado.
pub struct CanalDeProcesso {
    filho: std::process::Child,
    entrada: std::process::ChildStdin,
    saida: std::process::ChildStdout,
}

impl CanalDeProcesso {
    pub fn iniciar(mut comando: std::process::Command) -> Result<Self, String> {
        comando.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped());
        let mut filho = comando.spawn().map_err(|e| format!("não foi possível iniciar o executor: {e}"))?;
        let entrada = filho.stdin.take().ok_or("executor sem stdin")?;
        let saida = filho.stdout.take().ok_or("executor sem stdout")?;
        Ok(CanalDeProcesso { filho, entrada, saida })
    }
}

impl Canal for CanalDeProcesso {
    fn enviar(&mut self, m: &Value) -> Result<(), String> {
        escrever_quadro(&mut self.entrada, m).map_err(|e| format!("executor: {e}"))
    }
    fn receber(&mut self) -> Result<Value, String> {
        match ler_quadro(&mut self.saida) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err("o executor terminou no meio da sessão".into()),
            Err(e) => Err(format!("executor: {e}")),
        }
    }
}

impl Drop for CanalDeProcesso {
    fn drop(&mut self) {
        // Encerramento educado já mandou `fim`; aqui só garante que o filho
        // não fica órfão.
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn quadro_ida_e_volta() {
        let mut buf = Vec::new();
        let m = json!({"t": "macro.executar", "id": 7, "texto": "ação ✓"});
        escrever_quadro(&mut buf, &m).unwrap();
        assert_eq!(&buf[..4], &(buf.len() as u32 - 4).to_be_bytes());
        let mut r = &buf[..];
        assert_eq!(ler_quadro(&mut r).unwrap(), Some(m));
        assert_eq!(ler_quadro(&mut r).unwrap(), None);
    }

    #[test]
    fn handshake_recusa_outro_protocolo() {
        assert!(conferir_ola(&json!({"t": "ola", "protocolo": "dfexec/2", "servicos": ["macro"]})).is_err());
        assert!(conferir_ola(&json!({"t": "ola", "protocolo": "dfexec/1", "servicos": ["build"]})).is_err());
        let a = conferir_ola(&json!({"t": "ola", "protocolo": "dfexec/1", "servicos": ["macro"], "executor": "x",
            "macros": [{"macro": "package:json/json.dart#JsonCodable", "construtores": [""]}]}))
        .unwrap();
        assert_eq!(a.macros, vec![("package:json/json.dart#JsonCodable".to_string(), vec![String::new()])]);
    }
}
