//! Handshake of the `macro.*` service over the common transport of
//! `crates/dfexec` (docs/BUILD-PROTOCOLO.md §1–§2). It is the same channel as
//! the builders' (governing rule, item 2); macros are its `macro.*` service
//! (§5 there, and docs/MACROS-PROTOCOLO.md §5 here).
use serde_json::{Value, json};
pub use dartforge_dfexec::{Channel, ProcessChannel, PROTOCOL, write_frame, read_frame};

/// The host version announced in the handshake.
pub const HOST_VERSION: &str = concat!("dartforge-macros-host/", env!("CARGO_PKG_VERSION"));

/// The host's `ola` (hello) message.
pub fn host_hello() -> Value {
    json!({"t": "ola", "protocolo": PROTOCOL, "servicos": ["macro"], "motor": HOST_VERSION})
}

/// What the executor answered in the handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub executor: String,
    pub abi: String,
    /// `uri#Class` → constructor names available in the bootstrap.
    pub macros: Vec<(String, Vec<String>)>,
}

/// Checks the reply to `ola`: same protocol and the `macro` service.
pub fn check_hello(v: &Value) -> Result<Handshake, String> {
    if v.get("t").and_then(Value::as_str) != Some("ola") {
        return Err(format!("the executor did not answer the handshake: {v}"));
    }
    let p = v.get("protocolo").and_then(Value::as_str).unwrap_or("");
    if p != PROTOCOL {
        return Err(format!("the executor speaks {p}, the host speaks {PROTOCOL}"));
    }
    let services: Vec<&str> =
        v.get("servicos").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !services.contains(&"macro") {
        return Err(format!("the executor does not offer the macro service (it offers {services:?})"));
    }
    let macros = v
        .get("macros")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|m| {
                    let name = m.get("macro").and_then(Value::as_str).unwrap_or("").to_string();
                    let constructors = m
                        .get("construtores")
                        .and_then(Value::as_array)
                        .map(|c| c.iter().filter_map(Value::as_str).map(str::to_string).collect())
                        .unwrap_or_default();
                    (name, constructors)
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Handshake {
        executor: v.get("executor").and_then(Value::as_str).unwrap_or("").to_string(),
        abi: v.get("abi").and_then(Value::as_str).unwrap_or("").to_string(),
        macros,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip() {
        let mut buf = Vec::new();
        let m = json!({"t": "macro.executar", "id": 7, "texto": "ação ✓"});
        write_frame(&mut buf, &m).unwrap();
        assert_eq!(&buf[..4], &(buf.len() as u32 - 4).to_be_bytes());
        let mut r = &buf[..];
        assert_eq!(read_frame(&mut r).unwrap(), Some(m));
        assert_eq!(read_frame(&mut r).unwrap(), None);
    }

    #[test]
    fn handshake_rejects_other_protocol() {
        assert!(check_hello(&json!({"t": "ola", "protocolo": "dfexec/2", "servicos": ["macro"]})).is_err());
        assert!(check_hello(&json!({"t": "ola", "protocolo": "dfexec/1", "servicos": ["build"]})).is_err());
        let a = check_hello(&json!({"t": "ola", "protocolo": "dfexec/1", "servicos": ["macro"], "executor": "x",
            "macros": [{"macro": "package:json/json.dart#JsonCodable", "construtores": [""]}]}))
        .unwrap();
        assert_eq!(a.macros, vec![("package:json/json.dart#JsonCodable".to_string(), vec![String::new()])]);
    }
}
