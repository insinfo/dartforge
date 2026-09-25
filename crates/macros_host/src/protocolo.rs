//! Handshake do serviço `macro.*` sobre o transporte comum de `crates/dfexec`
//! (docs/BUILD-PROTOCOLO.md §1–§2). É o mesmo canal dos builders (regra governante,
//! item 2); as macros são o serviço `macro.*` dele (§5 lá, e
//! docs/MACROS-PROTOCOLO.md §5 aqui).
use serde_json::{Value, json};
pub use dartforge_dfexec::{Canal, CanalDeProcesso, PROTOCOLO, escrever_quadro, ler_quadro};

/// A versão do hospedeiro no handshake.
pub const VERSAO_DO_HOSPEDEIRO: &str = concat!("dartforge-macros-host/", env!("CARGO_PKG_VERSION"));

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
