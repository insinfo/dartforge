//! Client of the `build.*` service of `dfexec/1`. The channel and the framing
//! are the same ones used by macros; the real executor can serve both.
use crate::executor::{
    ActionRequest, ActionResult, Availability, BuildStepService, BuilderScript, DartExecutor,
    ExecutorError, ExtensionsRequest, Level, RuntimeExtensions,
};
use crate::graph::AssetId;
use crate::value::{Map, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use dartforge_dfexec::{Channel, PROTOCOL, ProcessChannel};
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;

pub struct BuildClient<C: Channel> {
    channel: C,
    next_id: u64,
    started: bool,
    loaded: bool,
    prepare_failure: Option<String>,
}

impl BuildClient<ProcessChannel> {
    /// Starts a compatible process. The handshake and the loading are left
    /// for the first Dart action; the engine should build this client on
    /// demand so that the process is kept warm only when needed.
    ///
    /// Each response from the process has the deadline of
    /// [`executor_timeout`]: a hung builder becomes a failure of the action
    /// (`Origin::Failure`), not a stuck `dartforge build`.
    pub fn start(command: std::process::Command) -> Result<Self, String> {
        ProcessChannel::start(command).map(|c| Self::new(c.with_deadline(executor_timeout())))
    }
}

/// How long the host waits for each message from the Dart executor (the VM
/// or the native one) before considering it hung: `DARTFORGE_BUILD_TIMEOUT` in
/// seconds (`0` disables it), 600 s by default — slack for the SDK summary and
/// the resolution of the first builder on a loaded machine; the `build.*`
/// queries in the middle of an action restart the count.
pub fn executor_timeout() -> Option<std::time::Duration> {
    let s = match std::env::var("DARTFORGE_BUILD_TIMEOUT") {
        Ok(v) => v
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|s| s.is_finite() && *s >= 0.0)
            .unwrap_or(600.0),
        Err(_) => 600.0,
    };
    (s > 0.0).then(|| std::time::Duration::from_secs_f64(s))
}

impl<C: Channel> BuildClient<C> {
    pub fn new(channel: C) -> Self {
        Self {
            channel,
            next_id: 1,
            started: false,
            loaded: false,
            prepare_failure: None,
        }
    }

    pub fn channel(&self) -> &C {
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
        service: &mut dyn BuildStepService,
    ) -> Result<JsonValue, ExecutorError> {
        loop {
            let message = self.channel.receive().map_err(ExecutorError)?;
            let kind = message.get("t").and_then(JsonValue::as_str).unwrap_or("");
            if matches!(
                kind,
                "build.read"
                    | "build.exists"
                    | "build.glob"
                    | "build.digest"
                    | "build.write"
                    | "build.log"
                    | "build.resolver"
            ) || kind.starts_with("build.resolver.")
            {
                let response = match self.serve(&message, service) {
                    Ok(response) => response,
                    Err(error) => {
                        let request =
                            message
                                .get("id")
                                .and_then(JsonValue::as_u64)
                                .ok_or_else(|| {
                                    ExecutorError(format!("build query without id: {message}"))
                                })?;
                        json!({"t":"build.response","id":request,"error":error.0})
                    }
                };
                self.channel.send(&response).map_err(ExecutorError)?;
                continue;
            }
            if message.get("id").and_then(JsonValue::as_u64) != Some(id) {
                return Err(ExecutorError(format!(
                    "out-of-order response from the executor: {message}"
                )));
            }
            if kind == "error" {
                return Err(ExecutorError(
                    message
                        .get("message")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("error without a message")
                        .into(),
                ));
            }
            if kind != expected {
                return Err(ExecutorError(format!("expected {expected}, got {message}")));
            }
            return Ok(message);
        }
    }

    fn serve(
        &self,
        m: &JsonValue,
        s: &mut dyn BuildStepService,
    ) -> Result<JsonValue, ExecutorError> {
        let kind = m.get("t").and_then(JsonValue::as_str).unwrap_or("");
        let id = m
            .get("id")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| ExecutorError("build query without id".into()))?;
        let asset = || {
            m.get("asset")
                .and_then(JsonValue::as_str)
                .and_then(AssetId::from_text)
                .ok_or_else(|| ExecutorError(format!("{kind}: invalid AssetId")))
        };
        let mut response = json!({"t":"build.response","id":id});
        match kind {
            "build.read" => {
                let asset = asset()?;
                match s.read(&asset) {
                    Some(bytes) => response["bytes_base64"] = json!(STANDARD.encode(bytes)),
                    None => response["error"] = json!("AssetNotFound"),
                }
            }
            "build.exists" => response["exists"] = json!(s.can_read(&asset()?)),
            "build.glob" => {
                let pattern = m
                    .get("pattern")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| ExecutorError("glob without a pattern".into()))?;
                response["assets"] = json!(
                    s.find_assets(pattern)
                        .into_iter()
                        .map(|a| a.text())
                        .collect::<Vec<_>>()
                );
            }
            "build.digest" => {
                response["hex"] = s
                    .digest(&asset()?)
                    .map(|d| d.iter().map(|b| format!("{b:02x}")).collect::<String>())
                    .map_or(JsonValue::Null, JsonValue::String);
            }
            "build.write" => {
                let asset = asset()?;
                let bytes = m
                    .get("bytes_base64")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| ExecutorError("write without bytes".into()))?;
                let bytes: Arc<[u8]> = STANDARD
                    .decode(bytes)
                    .map_err(|e| ExecutorError(format!("invalid base64: {e}")))?
                    .into();
                if s.write(&asset, bytes).is_err() {
                    response["error"] = json!("OutputNotAllowed");
                }
            }
            "build.log" => {
                let level = match m.get("level").and_then(JsonValue::as_str).unwrap_or("info") {
                    "fine" => Level::Fine,
                    "warning" => Level::Warning,
                    "severe" => Level::Severe,
                    _ => Level::Info,
                };
                s.log(
                    level,
                    m.get("message").and_then(JsonValue::as_str).unwrap_or(""),
                );
            }
            "build.resolver" => response["error"] = json!("unavailable"),
            t if t.starts_with("build.resolver.") => response["error"] = json!("unavailable"),
            _ => return Err(ExecutorError(format!("unknown query: {kind}"))),
        }
        Ok(response)
    }

    /// `build.run` and `build.post_process`: the same request and the
    /// same `build.result` response; the post-processor adds `deleted`.
    fn action(
        &mut self,
        kind: &str,
        p: &ActionRequest,
        service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        if !self.loaded {
            return Err(ExecutorError(format!("{kind} before build.load")));
        }
        let id = self.id();
        self.channel.send(&json!({"t":kind,"id":id,"phase":p.phase,"key":p.key,
            "factory":p.factory,"options":map_json(&p.options),"isRoot":p.root,"input":p.input.text(),
            "allowed_outputs":p.allowed_outputs.iter().map(AssetId::text).collect::<Vec<_>>()
        })).map_err(ExecutorError)?;
        let response = self.receive(id, "build.result", service)?;
        let mut outputs = Vec::new();
        let list = response
            .get("outputs")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| ExecutorError("build.result without a list of outputs".into()))?;
        for output in list {
            let asset = output
                .get("asset")
                .and_then(JsonValue::as_str)
                .and_then(AssetId::from_text)
                .ok_or_else(|| ExecutorError("result with an invalid AssetId".into()))?;
            let bytes = output
                .get("bytes_base64")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| ExecutorError("result without bytes".into()))?;
            outputs.push((
                asset,
                Arc::from(
                    STANDARD
                        .decode(bytes)
                        .map_err(|e| ExecutorError(e.to_string()))?,
                ),
            ));
        }
        let logs = response
            .get("logs")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .map(|log| {
                (
                    log.get("level")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("info")
                        .to_string(),
                    log.get("message")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect();
        let failed = response
            .get("failed")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| ExecutorError("build.result without the failed flag".into()))?;
        let mut deleted = Vec::new();
        for a in response
            .get("deleted")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
        {
            deleted.push(
                a.as_str().and_then(AssetId::from_text).ok_or_else(|| {
                    ExecutorError("result with an invalid deleted AssetId".into())
                })?,
            );
        }
        Ok(ActionResult {
            outputs,
            logs,
            failed,
            deleted_outputs: deleted,
        })
    }

    fn handshake(&mut self) -> Result<(), ExecutorError> {
        self.channel.send(&json!({"t":"hello","protocol":PROTOCOL,"services":["build"],"engine":crate::VERSION})).map_err(ExecutorError)?;
        let response = self.channel.receive().map_err(ExecutorError)?;
        let services = response.get("services").and_then(JsonValue::as_array);
        if response.get("t").and_then(JsonValue::as_str) != Some("hello")
            || response.get("protocol").and_then(JsonValue::as_str) != Some(PROTOCOL)
            || !services.is_some_and(|s| s.iter().any(|v| v.as_str() == Some("build")))
        {
            return Err(ExecutorError(format!(
                "incompatible build handshake: {response}"
            )));
        }
        self.started = true;
        Ok(())
    }
}

struct NoService;
impl BuildStepService for NoService {
    fn can_read(&mut self, _: &AssetId) -> bool {
        false
    }
    fn read(&mut self, _: &AssetId) -> Option<Arc<[u8]>> {
        None
    }
    fn find_assets(&mut self, _: &str) -> Vec<AssetId> {
        Vec::new()
    }
    fn digest(&mut self, _: &AssetId) -> Option<[u8; 32]> {
        None
    }
    fn write(
        &mut self,
        id: &AssetId,
        _: Arc<[u8]>,
    ) -> Result<(), crate::executor::DisallowedOutput> {
        Err(crate::executor::DisallowedOutput(id.clone()))
    }
    fn resolver(&mut self) -> Option<&mut dyn crate::executor::ResolverService> {
        None
    }
    fn log(&mut self, _: Level, _: &str) {}
}

impl<C: Channel> DartExecutor for BuildClient<C> {
    fn availability(&self) -> Availability {
        self.prepare_failure
            .as_ref()
            .map_or(Availability::Available, |e| {
                Availability::Unavailable(e.clone())
            })
    }

    fn prepare(&mut self, script: &BuilderScript) -> Result<(), ExecutorError> {
        if let Some(error) = &self.prepare_failure {
            return Err(ExecutorError(error.clone()));
        }
        let result: Result<(), ExecutorError> = (|| {
            if !self.started {
                self.handshake()?;
            }
            let id = self.id();
            let applications: Vec<_> = script.applications.iter().map(|(key, import, factories)|
                json!({"key":key,"import":import,"factories":factories})).collect();
            self.channel.send(&json!({"t":"build.load","id":id,"script":{"applications":applications,"cache_key":script.cache_key}})).map_err(ExecutorError)?;
            self.receive(id, "build.loaded", &mut NoService)?;
            self.loaded = true;
            Ok(())
        })();
        if let Err(e) = &result {
            self.prepare_failure = Some(e.0.clone());
        }
        result
    }

    fn new_round(&mut self) {
        if self.loaded {
            // Notification without a response: the executor resets its
            // per-build state before the next action, which comes through the
            // same ordered channel.
            if let Err(e) = self.channel.send(&json!({"t":"build.round"})) {
                self.prepare_failure = Some(e);
                self.loaded = false;
            }
        }
    }

    fn extensions(
        &mut self,
        p: &ExtensionsRequest,
    ) -> Result<Option<RuntimeExtensions>, ExecutorError> {
        if !self.loaded {
            return Err(ExecutorError("build.extensions before build.load".into()));
        }
        let id = self.id();
        self.channel
            .send(
                &json!({"t":"build.extensions","id":id,"key":p.key,"factory":p.factory,
            "options":map_json(&p.options),"isRoot":p.root}),
            )
            .map_err(ExecutorError)?;
        let response = self.receive(id, "build.extensions", &mut NoService)?;
        let invalid = || ExecutorError(format!("invalid build.extensions: {response}"));
        let pairs = response
            .get("extensions")
            .and_then(JsonValue::as_array)
            .ok_or_else(invalid)?;
        let mut v = Vec::with_capacity(pairs.len());
        for pair in pairs {
            let input = pair
                .get(0)
                .and_then(JsonValue::as_str)
                .ok_or_else(invalid)?;
            let outputs = pair
                .get(1)
                .and_then(JsonValue::as_array)
                .ok_or_else(invalid)?
                .iter()
                .map(|s| s.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(invalid)?;
            v.push((input.to_string(), outputs));
        }
        Ok(Some(v))
    }

    fn execute(
        &mut self,
        p: &ActionRequest,
        service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        self.action("build.run", p, service)
    }

    fn post_inputs(&mut self, p: &ExtensionsRequest) -> Result<Option<Vec<String>>, ExecutorError> {
        if !self.loaded {
            return Err(ExecutorError("build.post_inputs before build.load".into()));
        }
        let id = self.id();
        self.channel
            .send(
                &json!({"t":"build.post_inputs","id":id,"key":p.key,"factory":p.factory,
            "options":map_json(&p.options),"isRoot":p.root}),
            )
            .map_err(ExecutorError)?;
        let response = self.receive(id, "build.post_inputs", &mut NoService)?;
        let invalid = || ExecutorError(format!("invalid build.post_inputs: {response}"));
        let v = response
            .get("inputs")
            .and_then(JsonValue::as_array)
            .ok_or_else(invalid)?
            .iter()
            .map(|s| s.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(invalid)?;
        Ok(Some(v))
    }

    fn post_process_step(
        &mut self,
        p: &ActionRequest,
        service: &mut dyn BuildStepService,
    ) -> Result<ActionResult, ExecutorError> {
        self.action("build.post_process", p, service)
    }

    fn shutdown(&mut self) {
        if self.started {
            let _ = self.channel.send(&json!({"t":"end"}));
            let _ = self.channel.receive();
            self.started = false;
            self.loaded = false;
            self.prepare_failure = None;
        }
    }
}

fn map_json(map: &Map) -> JsonValue {
    value_json(&Value::Map(map.clone()))
}

fn value_json(value: &Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(v) => json!(v),
        Value::Int(v) => json!(v),
        Value::Real(v) => serde_json::from_str(v).unwrap_or_else(|_| json!(v)),
        Value::Text(v) => json!(v),
        Value::List(v) => JsonValue::Array(v.iter().map(value_json).collect()),
        Value::Map(v) => JsonValue::Object(
            v.0.iter()
                .filter_map(|(k, v)| k.as_text().map(|k| (k.to_string(), value_json(v))))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_dfexec::Channel;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeChannel {
        received: VecDeque<JsonValue>,
        sent: Arc<Mutex<Vec<JsonValue>>>,
    }
    impl Channel for FakeChannel {
        fn send(&mut self, m: &JsonValue) -> Result<(), String> {
            self.sent.lock().unwrap().push(m.clone());
            Ok(())
        }
        fn receive(&mut self) -> Result<JsonValue, String> {
            self.received
                .pop_front()
                .ok_or_else(|| "fake executor finished".into())
        }
    }

    struct FakeService {
        writes: Vec<(AssetId, Arc<[u8]>)>,
    }
    impl BuildStepService for FakeService {
        fn can_read(&mut self, id: &AssetId) -> bool {
            id.text() == "p|lib/a.dart"
        }
        fn read(&mut self, id: &AssetId) -> Option<Arc<[u8]>> {
            self.can_read(id).then(|| Arc::from(&b"source"[..]))
        }
        fn find_assets(&mut self, _: &str) -> Vec<AssetId> {
            vec![AssetId::new("p", "lib/a.dart")]
        }
        fn digest(&mut self, _: &AssetId) -> Option<[u8; 32]> {
            None
        }
        fn write(
            &mut self,
            id: &AssetId,
            bytes: Arc<[u8]>,
        ) -> Result<(), crate::executor::DisallowedOutput> {
            if id.text() != "p|lib/a.g.dart" {
                return Err(crate::executor::DisallowedOutput(id.clone()));
            }
            self.writes.push((id.clone(), bytes));
            Ok(())
        }
        fn resolver(&mut self) -> Option<&mut dyn crate::executor::ResolverService> {
            None
        }
        fn log(&mut self, _: Level, _: &str) {}
    }

    #[test]
    fn handshake_load_and_build_queries_on_the_same_channel() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let received = [
            json!({"t":"hello","protocol":"dfexec/1","services":["build"],"executor":"test","abi":"a"}),
            json!({"t":"build.loaded","id":1}),
            json!({"t":"build.read","id":9,"asset":"p|lib/a.dart"}),
            json!({"t":"build.write","id":10,"asset":"p|lib/a.g.dart","bytes_base64":STANDARD.encode(b"generated")}),
            json!({"t":"build.write","id":11,"asset":"p|lib/other.dart","bytes_base64":STANDARD.encode(b"improper")}),
            json!({"t":"build.result","id":2,"outputs":[],"failed":false}),
        ].into();
        let mut client = BuildClient::new(FakeChannel {
            received,
            sent: sent.clone(),
        });
        client
            .prepare(&BuilderScript {
                applications: vec![("p:b".into(), "package:p/b.dart".into(), vec!["b".into()])],
                cache_key: "digest".into(),
            })
            .unwrap();
        let mut service = FakeService { writes: Vec::new() };
        let request = ActionRequest {
            phase: 0,
            key: "p:b".into(),
            factory: "b".into(),
            options: Map::default(),
            root: true,
            input: AssetId::new("p", "lib/a.dart"),
            allowed_outputs: vec![AssetId::new("p", "lib/a.g.dart")],
        };
        let result = client.execute(&request, &mut service).unwrap();
        assert!(!result.failed);
        assert_eq!(service.writes[0].1.as_ref(), b"generated");
        let sent = sent.lock().unwrap();
        assert_eq!(sent[0]["t"], "hello");
        assert_eq!(sent[1]["t"], "build.load");
        assert_eq!(sent[2]["isRoot"], true);
        assert_eq!(sent[3]["bytes_base64"], STANDARD.encode(b"source"));
        assert_eq!(sent[4], json!({"t":"build.response","id":10}));
        assert_eq!(sent[5]["error"], "OutputNotAllowed");
    }

    #[test]
    fn builder_extensions_and_new_round() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let channel = FakeChannel {
            received: [
                json!({"t":"hello","protocol":"dfexec/1","services":["build"]}),
                json!({"t":"build.loaded","id":1}),
                json!({"t":"build.extensions","id":2,"extensions":[[".b",[".c",".d"]],[".a",[".e"]]]}),
            ].into(),
            sent: sent.clone(),
        };
        let mut client = BuildClient::new(channel);
        let request = ExtensionsRequest {
            key: "p:b".into(),
            factory: "b".into(),
            options: Map::default(),
            root: true,
        };
        assert!(
            client.extensions(&request).is_err(),
            "extensions before loading"
        );
        client
            .prepare(&BuilderScript {
                applications: vec![],
                cache_key: "x".into(),
            })
            .unwrap();
        client.new_round();
        // The order of the `buildExtensions` map is preserved (the first match wins).
        assert_eq!(
            client.extensions(&request).unwrap(),
            Some(vec![
                (".b".to_string(), vec![".c".to_string(), ".d".to_string()]),
                (".a".to_string(), vec![".e".to_string()]),
            ])
        );
        let sent = sent.lock().unwrap();
        assert_eq!(sent[2], json!({"t":"build.round"}));
        assert_eq!(sent[3]["t"], "build.extensions");
        assert_eq!(sent[3]["isRoot"], true);
    }

    #[test]
    fn post_processor_inputs_and_deletions() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let channel = FakeChannel {
            received: [
                json!({"t":"hello","protocol":"dfexec/1","services":["build"]}),
                json!({"t":"build.loaded","id":1}),
                json!({"t":"build.post_inputs","id":2,"inputs":[".draft"]}),
                json!({"t":"build.write","id":7,"asset":"p|lib/a.g.dart","bytes_base64":STANDARD.encode(b"r")}),
                json!({"t":"build.result","id":3,"outputs":[],"failed":false,"deleted":["p|lib/a.dart"]}),
            ].into(),
            sent: sent.clone(),
        };
        let mut client = BuildClient::new(channel);
        client
            .prepare(&BuilderScript {
                applications: vec![],
                cache_key: "x".into(),
            })
            .unwrap();
        let ext_request = ExtensionsRequest {
            key: "p:cleanup".into(),
            factory: "cleanup".into(),
            options: Map::default(),
            root: true,
        };
        assert_eq!(
            client.post_inputs(&ext_request).unwrap(),
            Some(vec![".draft".to_string()])
        );
        let request = ActionRequest {
            phase: 1,
            key: "p:cleanup".into(),
            factory: "cleanup".into(),
            options: Map::default(),
            root: true,
            input: AssetId::new("p", "lib/a.dart"),
            allowed_outputs: Vec::new(),
        };
        let mut service = FakeService { writes: Vec::new() };
        let r = client.post_process_step(&request, &mut service).unwrap();
        assert_eq!(r.deleted_outputs, vec![AssetId::new("p", "lib/a.dart")]);
        assert_eq!(service.writes.len(), 1);
        let sent = sent.lock().unwrap();
        assert_eq!(sent[2]["t"], "build.post_inputs");
        assert_eq!(sent[3]["t"], "build.post_process");
        assert_eq!(sent[3]["allowed_outputs"], json!([]));
    }

    #[test]
    fn handshake_rejects_executors_without_build_service() {
        for response in [
            json!({"t":"hello","protocol":"dfexec/2","services":["build"]}),
            json!({"t":"hello","protocol":"dfexec/1","services":["macro"]}),
        ] {
            let channel = FakeChannel {
                received: [response].into(),
                sent: Arc::new(Mutex::new(Vec::new())),
            };
            let mut client = BuildClient::new(channel);
            assert!(
                client
                    .prepare(&BuilderScript {
                        applications: vec![],
                        cache_key: "x".into()
                    })
                    .is_err()
            );
            assert!(matches!(
                client.availability(),
                Availability::Unavailable(_)
            ));
        }
    }

    #[test]
    fn incomplete_build_result_is_not_success() {
        let channel = FakeChannel {
            received: [
                json!({"t":"hello","protocol":"dfexec/1","services":["build"]}),
                json!({"t":"build.loaded","id":1}),
                json!({"t":"build.result","id":2,"failed":false}),
            ]
            .into(),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let mut client = BuildClient::new(channel);
        client
            .prepare(&BuilderScript {
                applications: vec![],
                cache_key: "x".into(),
            })
            .unwrap();
        let request = ActionRequest {
            phase: 0,
            key: "p:b".into(),
            factory: "b".into(),
            options: Map::default(),
            root: true,
            input: AssetId::new("p", "lib/a.dart"),
            allowed_outputs: Vec::new(),
        };
        let mut service = FakeService { writes: Vec::new() };
        assert!(client.execute(&request, &mut service).is_err());
    }

    #[test]
    fn invalid_query_gets_error_without_ending_action() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let channel = FakeChannel {
            received: [
                json!({"t":"hello","protocol":"dfexec/1","services":["build"]}),
                json!({"t":"build.loaded","id":1}),
                json!({"t":"build.read","id":9,"asset":"invalid"}),
                json!({"t":"build.write","id":10,"asset":"p|lib/a.g.dart","bytes_base64":"?"}),
                json!({"t":"build.exists","id":11,"asset":"p|lib/a.dart"}),
                json!({"t":"build.result","id":2,"outputs":[],"failed":false}),
            ]
            .into(),
            sent: sent.clone(),
        };
        let mut client = BuildClient::new(channel);
        client
            .prepare(&BuilderScript {
                applications: vec![],
                cache_key: "x".into(),
            })
            .unwrap();
        let request = ActionRequest {
            phase: 0,
            key: "p:b".into(),
            factory: "b".into(),
            options: Map::default(),
            root: true,
            input: AssetId::new("p", "lib/a.dart"),
            allowed_outputs: Vec::new(),
        };
        let mut service = FakeService { writes: Vec::new() };
        assert!(!client.execute(&request, &mut service).unwrap().failed);
        let sent = sent.lock().unwrap();
        assert_eq!(sent[3]["id"], 9);
        assert!(sent[3]["error"].as_str().unwrap().contains("AssetId"));
        assert_eq!(sent[4]["id"], 10);
        assert!(sent[4]["error"].as_str().unwrap().contains("base64"));
        assert_eq!(sent[5], json!({"t":"build.response","id":11,"exists":true}));
        assert!(service.writes.is_empty());
    }

    /// An executor that loads and then hangs on the action: once the deadline
    /// runs out, the action fails (the engine records it as `Origin::Failure`)
    /// and the process is terminated, without blocking the host.
    #[cfg(unix)]
    #[test]
    fn hung_action_exhausts_the_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("responses");
        let mut b = Vec::new();
        for m in [
            json!({"t":"hello","protocol":"dfexec/1","services":["build"]}),
            json!({"t":"build.loaded","id":1}),
        ] {
            dartforge_dfexec::write_frame(&mut b, &m).unwrap();
        }
        std::fs::write(&script, b).unwrap();
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg("cat \"$0\"; exec sleep 60").arg(&script);
        let channel = ProcessChannel::start(c)
            .unwrap()
            .with_deadline(Some(std::time::Duration::from_millis(500)));
        let mut client = BuildClient::new(channel);
        client
            .prepare(&BuilderScript {
                applications: vec![],
                cache_key: "x".into(),
            })
            .unwrap();
        let request = ActionRequest {
            phase: 0,
            key: "p:b".into(),
            factory: "b".into(),
            options: Map::default(),
            root: true,
            input: AssetId::new("p", "lib/a.dart"),
            allowed_outputs: vec![AssetId::new("p", "lib/a.g.dart")],
        };
        let mut service = FakeService { writes: Vec::new() };
        let t0 = std::time::Instant::now();
        let e = client.execute(&request, &mut service).unwrap_err();
        assert!(e.0.contains("did not respond"), "{}", e.0);
        assert!(t0.elapsed() < std::time::Duration::from_secs(20));
    }
}
