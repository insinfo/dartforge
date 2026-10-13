//! Common `dfexec/1` transport: one JSON frame per message, in the same
//! persistent process that serves the `build.*` and `macro.*` services.
use serde_json::Value;
use std::io::{Read, Write};

pub const PROTOCOLO: &str = "dfexec/1";

/// Frames larger than 64 MiB are a defect on the other side, not a valid model.
const LARGEST_FRAME: u32 = 64 << 20;

pub fn escrever_quadro(w: &mut dyn Write, m: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(m).map_err(std::io::Error::other)?;
    let n = u32::try_from(body.len()).map_err(std::io::Error::other)?;
    if n > LARGEST_FRAME { return Err(std::io::Error::other(format!("frame of {n} bytes exceeds the limit"))); }
    w.write_all(&n.to_be_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

pub fn ler_quadro(r: &mut dyn Read) -> std::io::Result<Option<Value>> {
    let mut n = [0u8; 4];
    let mut read_list = 0;
    while read_list < 4 {
        match r.read(&mut n[read_list..])? {
            0 if read_list == 0 => return Ok(None),
            0 => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "frame truncated in the size")),
            k => read_list += k,
        }
    }
    let n = u32::from_be_bytes(n);
    if n > LARGEST_FRAME { return Err(std::io::Error::other(format!("frame of {n} bytes exceeds the limit"))); }
    let mut body = vec![0u8; n as usize];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(std::io::Error::other)
}

pub trait Canal: Send {
    fn enviar(&mut self, m: &Value) -> Result<(), String>;
    fn receber(&mut self) -> Result<Value, String>;
}

/// Child process that speaks `dfexec/1` over stdin/stdout. stderr is inherited.
///
/// The child's frames are read by a thread, and `receber` waits for them at most
/// the deadline ([`CanalDeProcesso::com_prazo`]): a stuck executor (an infinite
/// loop in a builder, an `await` that never completes) becomes a channel error
/// instead of holding the host, and the process is terminated.
pub struct CanalDeProcesso {
    child: std::process::Child,
    input: std::process::ChildStdin,
    frames: std::sync::mpsc::Receiver<std::io::Result<Option<Value>>>,
    deadline: Option<std::time::Duration>,
    shut: bool,
}

impl CanalDeProcesso {
    pub fn iniciar(mut command: std::process::Command) -> Result<Self, String> {
        command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(|e| format!("could not start the executor: {e}"))?;
        let input = child.stdin.take().ok_or("executor without stdin")?;
        let mut out = child.stdout.take().ok_or("executor without stdout")?;
        let (tx, frames) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("dfexec-leitor".into())
            .spawn(move || loop {
                let q = ler_quadro(&mut out);
                let end = !matches!(q, Ok(Some(_)));
                if tx.send(q).is_err() || end {
                    break;
                }
            })
            .map_err(|e| format!("could not read from the executor: {e}"))?;
        Ok(Self { child, input, frames, deadline: None, shut: false })
    }

    /// Waits at most `deadline` for each message from the executor (`None`: no
    /// limit). When it runs out, the process is terminated and `receber` fails.
    pub fn com_prazo(mut self, deadline: Option<std::time::Duration>) -> Self {
        self.deadline = deadline;
        self
    }
}

impl Canal for CanalDeProcesso {
    fn enviar(&mut self, m: &Value) -> Result<(), String> {
        if self.shut {
            return Err("executor encerrado".into());
        }
        escrever_quadro(&mut self.input, m).map_err(|e| format!("executor: {e}"))
    }
    fn receber(&mut self) -> Result<Value, String> {
        if self.shut {
            return Err("executor encerrado".into());
        }
        let q = match self.deadline {
            Some(p) => match self.frames.recv_timeout(p) {
                Ok(q) => q,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.shut = true;
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(format!(
                        "the executor did not respond within {} s (hung?); process terminated",
                        p.as_secs_f64()
                    ));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(None),
            },
            None => self.frames.recv().unwrap_or(Ok(None)),
        };
        match q {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err("the executor ended in the middle of the session".into()),
            Err(e) => Err(format!("executor: {e}")),
        }
    }
}

impl Drop for CanalDeProcesso {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests_mod {
    use super::*;
    use serde_json::json;

    #[cfg(unix)]
    #[test]
    fn expired_deadline_ends_the_process() {
        let mut c = std::process::Command::new("sleep");
        c.arg("30");
        let t0 = std::time::Instant::now();
        let mut chan = CanalDeProcesso::iniciar(c)
            .unwrap()
            .com_prazo(Some(std::time::Duration::from_millis(200)));
        let e = chan.receber().unwrap_err();
        assert!(e.contains("did not respond"), "{e}");
        assert!(t0.elapsed() < std::time::Duration::from_secs(10));
        assert!(chan.receber().is_err());
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
