//! Transporte comum do `dfexec/1`: um quadro JSON por mensagem, no mesmo
//! processo persistente que atende os serviços `build.*` e `macro.*`.
use serde_json::Value;
use std::io::{Read, Write};

pub const PROTOCOLO: &str = "dfexec/1";

/// Quadros maiores que 64 MiB são defeito do outro lado, não um modelo válido.
const MAIOR_QUADRO: u32 = 64 << 20;

pub fn escrever_quadro(w: &mut dyn Write, m: &Value) -> std::io::Result<()> {
    let corpo = serde_json::to_vec(m).map_err(std::io::Error::other)?;
    let n = u32::try_from(corpo.len()).map_err(std::io::Error::other)?;
    if n > MAIOR_QUADRO { return Err(std::io::Error::other(format!("quadro de {n} bytes excede o limite"))); }
    w.write_all(&n.to_be_bytes())?;
    w.write_all(&corpo)?;
    w.flush()
}

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
    if n > MAIOR_QUADRO { return Err(std::io::Error::other(format!("quadro de {n} bytes excede o limite"))); }
    let mut corpo = vec![0u8; n as usize];
    r.read_exact(&mut corpo)?;
    serde_json::from_slice(&corpo).map(Some).map_err(std::io::Error::other)
}

pub trait Canal: Send {
    fn enviar(&mut self, m: &Value) -> Result<(), String>;
    fn receber(&mut self) -> Result<Value, String>;
}

/// Processo filho que fala `dfexec/1` por stdin/stdout. stderr é herdado.
///
/// Os quadros do filho são lidos por uma thread, e `receber` espera por eles
/// no máximo o prazo ([`CanalDeProcesso::com_prazo`]): um executor travado
/// (laço infinito num builder, um `await` que nunca completa) vira erro do
/// canal em vez de prender o hospedeiro, e o processo é encerrado.
pub struct CanalDeProcesso {
    filho: std::process::Child,
    entrada: std::process::ChildStdin,
    quadros: std::sync::mpsc::Receiver<std::io::Result<Option<Value>>>,
    prazo: Option<std::time::Duration>,
    encerrado: bool,
}

impl CanalDeProcesso {
    pub fn iniciar(mut comando: std::process::Command) -> Result<Self, String> {
        comando.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped());
        let mut filho = comando.spawn().map_err(|e| format!("não foi possível iniciar o executor: {e}"))?;
        let entrada = filho.stdin.take().ok_or("executor sem stdin")?;
        let mut saida = filho.stdout.take().ok_or("executor sem stdout")?;
        let (tx, quadros) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("dfexec-leitor".into())
            .spawn(move || loop {
                let q = ler_quadro(&mut saida);
                let fim = !matches!(q, Ok(Some(_)));
                if tx.send(q).is_err() || fim {
                    break;
                }
            })
            .map_err(|e| format!("não foi possível ler o executor: {e}"))?;
        Ok(Self { filho, entrada, quadros, prazo: None, encerrado: false })
    }

    /// Espera no máximo `prazo` por cada mensagem do executor (`None`: sem
    /// limite). Esgotado, o processo é encerrado e `receber` falha.
    pub fn com_prazo(mut self, prazo: Option<std::time::Duration>) -> Self {
        self.prazo = prazo;
        self
    }
}

impl Canal for CanalDeProcesso {
    fn enviar(&mut self, m: &Value) -> Result<(), String> {
        if self.encerrado {
            return Err("executor encerrado".into());
        }
        escrever_quadro(&mut self.entrada, m).map_err(|e| format!("executor: {e}"))
    }
    fn receber(&mut self) -> Result<Value, String> {
        if self.encerrado {
            return Err("executor encerrado".into());
        }
        let q = match self.prazo {
            Some(p) => match self.quadros.recv_timeout(p) {
                Ok(q) => q,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.encerrado = true;
                    let _ = self.filho.kill();
                    let _ = self.filho.wait();
                    return Err(format!(
                        "o executor não respondeu em {} s (travado?); processo encerrado",
                        p.as_secs_f64()
                    ));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(None),
            },
            None => self.quadros.recv().unwrap_or(Ok(None)),
        };
        match q {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err("o executor terminou no meio da sessão".into()),
            Err(e) => Err(format!("executor: {e}")),
        }
    }
}

impl Drop for CanalDeProcesso {
    fn drop(&mut self) {
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[cfg(unix)]
    #[test]
    fn prazo_esgotado_encerra_o_processo() {
        let mut c = std::process::Command::new("sleep");
        c.arg("30");
        let t0 = std::time::Instant::now();
        let mut canal = CanalDeProcesso::iniciar(c)
            .unwrap()
            .com_prazo(Some(std::time::Duration::from_millis(200)));
        let e = canal.receber().unwrap_err();
        assert!(e.contains("não respondeu"), "{e}");
        assert!(t0.elapsed() < std::time::Duration::from_secs(10));
        assert!(canal.receber().is_err());
    }

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
}
