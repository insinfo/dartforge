//! Client of the `build.*` service of `dfexec/1`. The channel and the framing
//! are the same ones used by macros; the real executor can serve both.
use crate::executor::{
    Disponibilidade, ErroExecutor, ExecutorDart, ExtensoesDeExecucao, Nivel, PedidoAcao,
    PedidoExtensoes, ResultadoAcao, ScriptDeBuilders, ServicoBuildStep,
};
use crate::grafo::AssetId;
use crate::valor::{Mapa, Valor};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use dartforge_dfexec::{Canal, CanalDeProcesso, PROTOCOLO};
use serde_json::{Value, json};
use std::sync::Arc;

pub struct ClienteBuild<C: Canal> {
    channel: C,
    next_id: u64,
    started: bool,
    loaded: bool,
    prepare_failure: Option<String>,
}

impl ClienteBuild<CanalDeProcesso> {
    /// Starts a compatible process. The handshake and the loading are left
    /// for the first Dart action; the engine should build this client on
    /// demand so that the process is kept warm only when needed.
    ///
    /// Each response from the process has the deadline of
    /// [`prazo_do_executor`]: a hung builder becomes a failure of the action
    /// (`Origem::Falha`), not a stuck `dartforge build`.
    pub fn iniciar(command: std::process::Command) -> Result<Self, String> {
        CanalDeProcesso::iniciar(command).map(|c| Self::novo(c.com_prazo(prazo_do_executor())))
    }
}

/// How long the host waits for each message from the Dart executor (the VM
/// or the native one) before considering it hung: `DARTFORGE_BUILD_PRAZO` in
/// seconds (`0` disables it), 600 s by default — slack for the SDK summary and
/// the resolution of the first builder on a loaded machine; the `build.*`
/// queries in the middle of an action restart the count.
pub fn prazo_do_executor() -> Option<std::time::Duration> {
    let s = match std::env::var("DARTFORGE_BUILD_PRAZO") {
        Ok(v) => v.trim().parse::<f64>().ok().filter(|s| s.is_finite() && *s >= 0.0).unwrap_or(600.0),
        Err(_) => 600.0,
    };
    (s > 0.0).then(|| std::time::Duration::from_secs_f64(s))
}

impl<C: Canal> ClienteBuild<C> {
    pub fn novo(channel: C) -> Self {
        Self {
            channel,
            next_id: 1,
            started: false,
            loaded: false,
            prepare_failure: None,
        }
    }

    pub fn canal(&self) -> &C {
        &self.channel
    }

    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn receive(
        &mut self,
        id: u64,
        expected: &str,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<Value, ErroExecutor> {
        loop {
            let message = self.channel.receber().map_err(ErroExecutor)?;
            let kind = message.get("t").and_then(Value::as_str).unwrap_or("");
            if matches!(
                kind,
                "build.ler"
                    | "build.existe"
                    | "build.glob"
                    | "build.digest"
                    | "build.escrever"
                    | "build.log"
                    | "build.resolver"
            ) || kind.starts_with("build.resolver.")
            {
                let response = match self.serve(&message, service) {
                    Ok(response) => response,
                    Err(error) => {
                        let request =
                            message.get("id").and_then(Value::as_u64).ok_or_else(|| {
                                ErroExecutor(format!("build query without id: {message}"))
                            })?;
                        json!({"t":"build.resposta","id":request,"erro":error.0})
                    }
                };
                self.channel.enviar(&response).map_err(ErroExecutor)?;
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                return Err(ErroExecutor(format!(
                    "out-of-order response from the executor: {message}"
                )));
            }
            if kind == "erro" {
                return Err(ErroExecutor(
                    message
                        .get("mensagem")
                        .and_then(Value::as_str)
                        .unwrap_or("error without a message")
                        .into(),
                ));
            }
            if kind != expected {
                return Err(ErroExecutor(format!(
                    "expected {expected}, got {message}"
                )));
            }
            return Ok(message);
        }
    }

    fn serve(&self, m: &Value, s: &mut dyn ServicoBuildStep) -> Result<Value, ErroExecutor> {
        let kind = m.get("t").and_then(Value::as_str).unwrap_or("");
        let id = m
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| ErroExecutor("build query without id".into()))?;
        let asset = || {
            m.get("asset")
                .and_then(Value::as_str)
                .and_then(AssetId::de_texto)
                .ok_or_else(|| ErroExecutor(format!("{kind}: invalid AssetId")))
        };
        let mut response = json!({"t":"build.resposta","id":id});
        match kind {
            "build.ler" => {
                let asset = asset()?;
                match s.ler(&asset) {
                    Some(bytes) => response["bytes_base64"] = json!(STANDARD.encode(bytes)),
                    None => response["erro"] = json!("AssetNotFound"),
                }
            }
            "build.existe" => response["sim"] = json!(s.can_read(&asset()?)),
            "build.glob" => {
                let pattern = m
                    .get("padrao")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ErroExecutor("glob without a pattern".into()))?;
                response["assets"] = json!(
                    s.find_assets(pattern)
                        .into_iter()
                        .map(|a| a.texto())
                        .collect::<Vec<_>>()
                );
            }
            "build.digest" => {
                response["hex"] = s
                    .digest(&asset()?)
                    .map(|d| d.iter().map(|b| format!("{b:02x}")).collect::<String>())
                    .map_or(Value::Null, Value::String);
            }
            "build.escrever" => {
                let asset = asset()?;
                let bytes = m
                    .get("bytes_base64")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ErroExecutor("write without bytes".into()))?;
                let bytes: Arc<[u8]> = STANDARD
                    .decode(bytes)
                    .map_err(|e| ErroExecutor(format!("invalid base64: {e}")))?
                    .into();
                if s.escrever(&asset, bytes).is_err() {
                    response["erro"] = json!("SaidaNaoPermitida");
                }
            }
            "build.log" => {
                let level = match m.get("nivel").and_then(Value::as_str).unwrap_or("info") {
                    "fino" => Nivel::Fino,
                    "aviso" => Nivel::Aviso,
                    "severo" => Nivel::Severo,
                    _ => Nivel::Info,
                };
                s.log(
                    level,
                    m.get("mensagem").and_then(Value::as_str).unwrap_or(""),
                );
            }
            "build.resolver" => response["erro"] = json!("indisponivel"),
            t if t.starts_with("build.resolver.") => response["erro"] = json!("indisponivel"),
            _ => return Err(ErroExecutor(format!("unknown query: {kind}"))),
        }
        Ok(response)
    }

    /// `build.executar` and `build.posprocessar`: the same request and the
    /// same `build.resultado` response; the post-processor adds `apagados`.
    fn action(
        &mut self,
        kind: &str,
        p: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        if !self.loaded {
            return Err(ErroExecutor(format!("{kind} before build.carregar")));
        }
        let id = self.id();
        self.channel.enviar(&json!({"t":kind,"id":id,"fase":p.fase,"chave":p.chave,
            "fabrica":p.fabrica,"opcoes":map_json(&p.opcoes),"isRoot":p.raiz,"entrada":p.entrada.texto(),
            "saidas_permitidas":p.saidas_permitidas.iter().map(AssetId::texto).collect::<Vec<_>>()
        })).map_err(ErroExecutor)?;
        let response = self.receive(id, "build.resultado", service)?;
        let mut outputs = Vec::new();
        let list = response
            .get("saidas")
            .and_then(Value::as_array)
            .ok_or_else(|| ErroExecutor("build.resultado without a list of outputs".into()))?;
        for output in list {
            let asset = output
                .get("asset")
                .and_then(Value::as_str)
                .and_then(AssetId::de_texto)
                .ok_or_else(|| ErroExecutor("result with an invalid AssetId".into()))?;
            let bytes = output
                .get("bytes_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| ErroExecutor("result without bytes".into()))?;
            outputs.push((
                asset,
                Arc::from(
                    STANDARD
                        .decode(bytes)
                        .map_err(|e| ErroExecutor(e.to_string()))?,
                ),
            ));
        }
        let logs = response
            .get("logs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|log| {
                (
                    log.get("nivel")
                        .and_then(Value::as_str)
                        .unwrap_or("info")
                        .to_string(),
                    log.get("mensagem")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect();
        let failed = response
            .get("falhou")
            .and_then(Value::as_bool)
            .ok_or_else(|| ErroExecutor("build.resultado without the falhou flag".into()))?;
        let mut deleted = Vec::new();
        for a in response
            .get("apagados")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            deleted.push(
                a.as_str()
                    .and_then(AssetId::de_texto)
                    .ok_or_else(|| ErroExecutor("result with an invalid deleted AssetId".into()))?,
            );
        }
        Ok(ResultadoAcao {
            saidas: outputs,
            logs,
            falhou: failed,
            apagados: deleted,
        })
    }

    fn handshake(&mut self) -> Result<(), ErroExecutor> {
        self.channel.enviar(&json!({"t":"ola","protocolo":PROTOCOLO,"servicos":["build"],"motor":crate::VERSAO})).map_err(ErroExecutor)?;
        let response = self.channel.receber().map_err(ErroExecutor)?;
        let services = response.get("servicos").and_then(Value::as_array);
        if response.get("t").and_then(Value::as_str) != Some("ola")
            || response.get("protocolo").and_then(Value::as_str) != Some(PROTOCOLO)
            || !services.is_some_and(|s| s.iter().any(|v| v.as_str() == Some("build")))
        {
            return Err(ErroExecutor(format!(
                "incompatible build handshake: {response}"
            )));
        }
        self.started = true;
        Ok(())
    }
}

struct NoService;
impl ServicoBuildStep for NoService {
    fn can_read(&mut self, _: &AssetId) -> bool {
        false
    }
    fn ler(&mut self, _: &AssetId) -> Option<Arc<[u8]>> {
        None
    }
    fn find_assets(&mut self, _: &str) -> Vec<AssetId> {
        Vec::new()
    }
    fn digest(&mut self, _: &AssetId) -> Option<[u8; 32]> {
        None
    }
    fn escrever(
        &mut self,
        id: &AssetId,
        _: Arc<[u8]>,
    ) -> Result<(), crate::executor::SaidaNaoPermitida> {
        Err(crate::executor::SaidaNaoPermitida(id.clone()))
    }
    fn resolver(&mut self) -> Option<&mut dyn crate::executor::ServicoResolver> {
        None
    }
    fn log(&mut self, _: Nivel, _: &str) {}
}

impl<C: Canal> ExecutorDart for ClienteBuild<C> {
    fn disponibilidade(&self) -> Disponibilidade {
        self.prepare_failure
            .as_ref()
            .map_or(Disponibilidade::Disponivel, |e| {
                Disponibilidade::Indisponivel(e.clone())
            })
    }

    fn preparar(&mut self, script: &ScriptDeBuilders) -> Result<(), ErroExecutor> {
        if let Some(error) = &self.prepare_failure {
            return Err(ErroExecutor(error.clone()));
        }
        let result: Result<(), ErroExecutor> = (|| {
            if !self.started {
                self.handshake()?;
            }
            let id = self.id();
            let applications: Vec<_> = script.aplicacoes.iter().map(|(key, import, factories)|
                json!({"chave":key,"import":import,"fabricas":factories})).collect();
            self.channel.enviar(&json!({"t":"build.carregar","id":id,"script":{"aplicacoes":applications,"chave_de_cache":script.chave_de_cache}})).map_err(ErroExecutor)?;
            self.receive(id, "build.carregado", &mut NoService)?;
            self.loaded = true;
            Ok(())
        })();
        if let Err(e) = &result {
            self.prepare_failure = Some(e.0.clone());
        }
        result
    }

    fn nova_rodada(&mut self) {
        if self.loaded {
            // Notification without a response: the executor resets its
            // per-build state before the next action, which comes through the
            // same ordered channel.
            if let Err(e) = self.channel.enviar(&json!({"t":"build.rodada"})) {
                self.prepare_failure = Some(e);
                self.loaded = false;
            }
        }
    }

    fn extensoes(
        &mut self,
        p: &PedidoExtensoes,
    ) -> Result<Option<ExtensoesDeExecucao>, ErroExecutor> {
        if !self.loaded {
            return Err(ErroExecutor(
                "build.extensoes before build.carregar".into(),
            ));
        }
        let id = self.id();
        self.channel
            .enviar(
                &json!({"t":"build.extensoes","id":id,"chave":p.chave,"fabrica":p.fabrica,
            "opcoes":map_json(&p.opcoes),"isRoot":p.raiz}),
            )
            .map_err(ErroExecutor)?;
        let response = self.receive(id, "build.extensoes", &mut NoService)?;
        let invalid = || ErroExecutor(format!("invalid build.extensoes: {response}"));
        let pairs = response
            .get("extensoes")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        let mut v = Vec::with_capacity(pairs.len());
        for pair in pairs {
            let input = pair.get(0).and_then(Value::as_str).ok_or_else(invalid)?;
            let outputs = pair
                .get(1)
                .and_then(Value::as_array)
                .ok_or_else(invalid)?
                .iter()
                .map(|s| s.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(invalid)?;
            v.push((input.to_string(), outputs));
        }
        Ok(Some(v))
    }

    fn executar(
        &mut self,
        p: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        self.action("build.executar", p, service)
    }

    fn entradas_pos(&mut self, p: &PedidoExtensoes) -> Result<Option<Vec<String>>, ErroExecutor> {
        if !self.loaded {
            return Err(ErroExecutor(
                "build.entradas_pos before build.carregar".into(),
            ));
        }
        let id = self.id();
        self.channel
            .enviar(
                &json!({"t":"build.entradas_pos","id":id,"chave":p.chave,"fabrica":p.fabrica,
            "opcoes":map_json(&p.opcoes),"isRoot":p.raiz}),
            )
            .map_err(ErroExecutor)?;
        let response = self.receive(id, "build.entradas_pos", &mut NoService)?;
        let invalid = || ErroExecutor(format!("invalid build.entradas_pos: {response}"));
        let v = response
            .get("entradas")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?
            .iter()
            .map(|s| s.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(invalid)?;
        Ok(Some(v))
    }

    fn pos_processar(
        &mut self,
        p: &PedidoAcao,
        service: &mut dyn ServicoBuildStep,
    ) -> Result<ResultadoAcao, ErroExecutor> {
        self.action("build.posprocessar", p, service)
    }

    fn encerrar(&mut self) {
        if self.started {
            let _ = self.channel.enviar(&json!({"t":"fim"}));
            let _ = self.channel.receber();
            self.started = false;
            self.loaded = false;
            self.prepare_failure = None;
        }
    }
}

fn map_json(map: &Mapa) -> Value {
    value_json(&Valor::Mapa(map.clone()))
}

fn value_json(value: &Valor) -> Value {
    match value {
        Valor::Nulo => Value::Null,
        Valor::Bool(v) => json!(v),
        Valor::Int(v) => json!(v),
        Valor::Real(v) => serde_json::from_str(v).unwrap_or_else(|_| json!(v)),
        Valor::Texto(v) => json!(v),
        Valor::Lista(v) => Value::Array(v.iter().map(value_json).collect()),
        Valor::Mapa(v) => Value::Object(
            v.0.iter()
                .filter_map(|(k, v)| k.como_texto().map(|k| (k.to_string(), value_json(v))))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_dfexec::Canal;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeChannel {
        received: VecDeque<Value>,
        sent: Arc<Mutex<Vec<Value>>>,
    }
    impl Canal for FakeChannel {
        fn enviar(&mut self, m: &Value) -> Result<(), String> {
            self.sent.lock().unwrap().push(m.clone());
            Ok(())
        }
        fn receber(&mut self) -> Result<Value, String> {
            self.received
                .pop_front()
                .ok_or_else(|| "fake executor finished".into())
        }
    }

    struct FakeService {
        writes: Vec<(AssetId, Arc<[u8]>)>,
    }
    impl ServicoBuildStep for FakeService {
        fn can_read(&mut self, id: &AssetId) -> bool {
            id.texto() == "p|lib/a.dart"
        }
        fn ler(&mut self, id: &AssetId) -> Option<Arc<[u8]>> {
            self.can_read(id).then(|| Arc::from(&b"source"[..]))
        }
        fn find_assets(&mut self, _: &str) -> Vec<AssetId> {
            vec![AssetId::novo("p", "lib/a.dart")]
        }
        fn digest(&mut self, _: &AssetId) -> Option<[u8; 32]> {
            None
        }
        fn escrever(
            &mut self,
            id: &AssetId,
            bytes: Arc<[u8]>,
        ) -> Result<(), crate::executor::SaidaNaoPermitida> {
            if id.texto() != "p|lib/a.g.dart" {
                return Err(crate::executor::SaidaNaoPermitida(id.clone()));
            }
            self.writes.push((id.clone(), bytes));
            Ok(())
        }
        fn resolver(&mut self) -> Option<&mut dyn crate::executor::ServicoResolver> {
            None
        }
        fn log(&mut self, _: Nivel, _: &str) {}
    }

    #[test]
    fn handshake_load_and_build_queries_on_the_same_channel() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let received = [
            json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"],"executor":"test","abi":"a"}),
            json!({"t":"build.carregado","id":1}),
            json!({"t":"build.ler","id":9,"asset":"p|lib/a.dart"}),
            json!({"t":"build.escrever","id":10,"asset":"p|lib/a.g.dart","bytes_base64":STANDARD.encode(b"generated")}),
            json!({"t":"build.escrever","id":11,"asset":"p|lib/other.dart","bytes_base64":STANDARD.encode(b"improper")}),
            json!({"t":"build.resultado","id":2,"saidas":[],"falhou":false}),
        ].into();
        let mut client = ClienteBuild::novo(FakeChannel {
            received,
            sent: sent.clone(),
        });
        client
            .preparar(&ScriptDeBuilders {
                aplicacoes: vec![("p:b".into(), "package:p/b.dart".into(), vec!["b".into()])],
                chave_de_cache: "digest".into(),
            })
            .unwrap();
        let mut service = FakeService {
            writes: Vec::new(),
        };
        let request = PedidoAcao {
            fase: 0,
            chave: "p:b".into(),
            fabrica: "b".into(),
            opcoes: Mapa::default(),
            raiz: true,
            entrada: AssetId::novo("p", "lib/a.dart"),
            saidas_permitidas: vec![AssetId::novo("p", "lib/a.g.dart")],
        };
        let result = client.executar(&request, &mut service).unwrap();
        assert!(!result.falhou);
        assert_eq!(service.writes[0].1.as_ref(), b"generated");
        let sent = sent.lock().unwrap();
        assert_eq!(sent[0]["t"], "ola");
        assert_eq!(sent[1]["t"], "build.carregar");
        assert_eq!(sent[2]["isRoot"], true);
        assert_eq!(sent[3]["bytes_base64"], STANDARD.encode(b"source"));
        assert_eq!(sent[4], json!({"t":"build.resposta","id":10}));
        assert_eq!(sent[5]["erro"], "SaidaNaoPermitida");
    }

    #[test]
    fn builder_extensions_and_new_round() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let channel = FakeChannel {
            received: [
                json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
                json!({"t":"build.carregado","id":1}),
                json!({"t":"build.extensoes","id":2,"extensoes":[[".b",[".c",".d"]],[".a",[".e"]]]}),
            ].into(),
            sent: sent.clone(),
        };
        let mut client = ClienteBuild::novo(channel);
        let request = PedidoExtensoes {
            chave: "p:b".into(),
            fabrica: "b".into(),
            opcoes: Mapa::default(),
            raiz: true,
        };
        assert!(
            client.extensoes(&request).is_err(),
            "extensions before loading"
        );
        client
            .preparar(&ScriptDeBuilders {
                aplicacoes: vec![],
                chave_de_cache: "x".into(),
            })
            .unwrap();
        client.nova_rodada();
        // The order of the `buildExtensions` map is preserved (the first match wins).
        assert_eq!(
            client.extensoes(&request).unwrap(),
            Some(vec![
                (".b".to_string(), vec![".c".to_string(), ".d".to_string()]),
                (".a".to_string(), vec![".e".to_string()]),
            ])
        );
        let sent = sent.lock().unwrap();
        assert_eq!(sent[2], json!({"t":"build.rodada"}));
        assert_eq!(sent[3]["t"], "build.extensoes");
        assert_eq!(sent[3]["isRoot"], true);
    }

    #[test]
    fn post_processor_inputs_and_deletions() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let channel = FakeChannel {
            received: [
                json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
                json!({"t":"build.carregado","id":1}),
                json!({"t":"build.entradas_pos","id":2,"entradas":[".draft"]}),
                json!({"t":"build.escrever","id":7,"asset":"p|lib/a.g.dart","bytes_base64":STANDARD.encode(b"r")}),
                json!({"t":"build.resultado","id":3,"saidas":[],"falhou":false,"apagados":["p|lib/a.dart"]}),
            ].into(),
            sent: sent.clone(),
        };
        let mut client = ClienteBuild::novo(channel);
        client
            .preparar(&ScriptDeBuilders {
                aplicacoes: vec![],
                chave_de_cache: "x".into(),
            })
            .unwrap();
        let ext_request = PedidoExtensoes {
            chave: "p:cleanup".into(),
            fabrica: "cleanup".into(),
            opcoes: Mapa::default(),
            raiz: true,
        };
        assert_eq!(
            client.entradas_pos(&ext_request).unwrap(),
            Some(vec![".draft".to_string()])
        );
        let request = PedidoAcao {
            fase: 1,
            chave: "p:cleanup".into(),
            fabrica: "cleanup".into(),
            opcoes: Mapa::default(),
            raiz: true,
            entrada: AssetId::novo("p", "lib/a.dart"),
            saidas_permitidas: Vec::new(),
        };
        let mut service = FakeService {
            writes: Vec::new(),
        };
        let r = client.pos_processar(&request, &mut service).unwrap();
        assert_eq!(r.apagados, vec![AssetId::novo("p", "lib/a.dart")]);
        assert_eq!(service.writes.len(), 1);
        let sent = sent.lock().unwrap();
        assert_eq!(sent[2]["t"], "build.entradas_pos");
        assert_eq!(sent[3]["t"], "build.posprocessar");
        assert_eq!(sent[3]["saidas_permitidas"], json!([]));
    }

    #[test]
    fn handshake_rejects_executors_without_build_service() {
        for response in [
            json!({"t":"ola","protocolo":"dfexec/2","servicos":["build"]}),
            json!({"t":"ola","protocolo":"dfexec/1","servicos":["macro"]}),
        ] {
            let channel = FakeChannel {
                received: [response].into(),
                sent: Arc::new(Mutex::new(Vec::new())),
            };
            let mut client = ClienteBuild::novo(channel);
            assert!(
                client
                    .preparar(&ScriptDeBuilders {
                        aplicacoes: vec![],
                        chave_de_cache: "x".into()
                    })
                    .is_err()
            );
            assert!(matches!(
                client.disponibilidade(),
                Disponibilidade::Indisponivel(_)
            ));
        }
    }

    #[test]
    fn incomplete_build_result_is_not_success() {
        let channel = FakeChannel {
            received: [
                json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
                json!({"t":"build.carregado","id":1}),
                json!({"t":"build.resultado","id":2,"falhou":false}),
            ]
            .into(),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let mut client = ClienteBuild::novo(channel);
        client
            .preparar(&ScriptDeBuilders {
                aplicacoes: vec![],
                chave_de_cache: "x".into(),
            })
            .unwrap();
        let request = PedidoAcao {
            fase: 0,
            chave: "p:b".into(),
            fabrica: "b".into(),
            opcoes: Mapa::default(),
            raiz: true,
            entrada: AssetId::novo("p", "lib/a.dart"),
            saidas_permitidas: Vec::new(),
        };
        let mut service = FakeService {
            writes: Vec::new(),
        };
        assert!(client.executar(&request, &mut service).is_err());
    }

    #[test]
    fn invalid_query_gets_error_without_ending_action() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let channel = FakeChannel {
            received: [
                json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
                json!({"t":"build.carregado","id":1}),
                json!({"t":"build.ler","id":9,"asset":"invalid"}),
                json!({"t":"build.escrever","id":10,"asset":"p|lib/a.g.dart","bytes_base64":"?"}),
                json!({"t":"build.existe","id":11,"asset":"p|lib/a.dart"}),
                json!({"t":"build.resultado","id":2,"saidas":[],"falhou":false}),
            ]
            .into(),
            sent: sent.clone(),
        };
        let mut client = ClienteBuild::novo(channel);
        client
            .preparar(&ScriptDeBuilders {
                aplicacoes: vec![],
                chave_de_cache: "x".into(),
            })
            .unwrap();
        let request = PedidoAcao {
            fase: 0,
            chave: "p:b".into(),
            fabrica: "b".into(),
            opcoes: Mapa::default(),
            raiz: true,
            entrada: AssetId::novo("p", "lib/a.dart"),
            saidas_permitidas: Vec::new(),
        };
        let mut service = FakeService {
            writes: Vec::new(),
        };
        assert!(!client.executar(&request, &mut service).unwrap().falhou);
        let sent = sent.lock().unwrap();
        assert_eq!(sent[3]["id"], 9);
        assert!(sent[3]["erro"].as_str().unwrap().contains("AssetId"));
        assert_eq!(sent[4]["id"], 10);
        assert!(sent[4]["erro"].as_str().unwrap().contains("base64"));
        assert_eq!(
            sent[5],
            json!({"t":"build.resposta","id":11,"sim":true})
        );
        assert!(service.writes.is_empty());
    }

    /// An executor that loads and then hangs on the action: once the deadline
    /// runs out, the action fails (the engine records it as `Origem::Falha`)
    /// and the process is terminated, without blocking the host.
    #[cfg(unix)]
    #[test]
    fn hung_action_exhausts_the_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("responses");
        let mut b = Vec::new();
        for m in [
            json!({"t":"ola","protocolo":"dfexec/1","servicos":["build"]}),
            json!({"t":"build.carregado","id":1}),
        ] {
            dartforge_dfexec::escrever_quadro(&mut b, &m).unwrap();
        }
        std::fs::write(&script, b).unwrap();
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg("cat \"$0\"; exec sleep 60").arg(&script);
        let channel = CanalDeProcesso::iniciar(c)
            .unwrap()
            .com_prazo(Some(std::time::Duration::from_millis(500)));
        let mut client = ClienteBuild::novo(channel);
        client
            .preparar(&ScriptDeBuilders { aplicacoes: vec![], chave_de_cache: "x".into() })
            .unwrap();
        let request = PedidoAcao {
            fase: 0,
            chave: "p:b".into(),
            fabrica: "b".into(),
            opcoes: Mapa::default(),
            raiz: true,
            entrada: AssetId::novo("p", "lib/a.dart"),
            saidas_permitidas: vec![AssetId::novo("p", "lib/a.g.dart")],
        };
        let mut service = FakeService { writes: Vec::new() };
        let t0 = std::time::Instant::now();
        let e = client.executar(&request, &mut service).unwrap_err();
        assert!(e.0.contains("não respondeu"), "{}", e.0);
        assert!(t0.elapsed() < std::time::Duration::from_secs(20));
    }
}
