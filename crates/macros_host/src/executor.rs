//! The macro executor as seen from the host: the [`MacroExecutor`] trait and
//! its implementations.
//!
//! * [`Unavailable`] — the one the product uses today. The real executor is
//!   the **native** one (D4): the macro compiled by `emit_native` and run by
//!   our executor in a separate process. It comes in once the native backend
//!   compiles what the API uses (`async`/`Future`, collections,
//!   `dart:convert`); the plug-in point is [`DfexecExecutor`] with the native
//!   process's channel.
//! * [`DfexecExecutor`] — the client of the `macro.*` service of `dfexec/1`
//!   over any [`Channel`]: a process ([`crate::protocol::ProcessChannel`]),
//!   or, in tests, a recorded session ([`RecordedChannel`]). It is the same
//!   client for the native executor and for the materialization builder that
//!   runs the same macro on a VM (docs/MACROS-COMPATIBILIDADE.md).
use crate::assembly::{ExecutionResult, result_from_json};
use crate::protocol::{Channel, Handshake, check_hello, host_hello};
use serde_json::{Value, json};

/// The three phases (spec, "Phases").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    Types,
    Declarations,
    Definitions,
}

impl Phase {
    pub const ALL: [Phase; 3] = [Phase::Types, Phase::Declarations, Phase::Definitions];

    /// The name used in the protocol.
    pub fn name(self) -> &'static str {
        match self {
            Phase::Types => "types",
            Phase::Declarations => "declarations",
            Phase::Definitions => "definitions",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable(String),
}

/// Error of a query, as the macro sees it (`MacroException`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    /// `implementacao` (wrong use of the API), `ciclo` (introspection cycle in
    /// phase 2) or `inesperado` (unexpected).
    pub kind: &'static str,
    pub message: String,
}

impl QueryError {
    pub fn implementation(m: impl Into<String>) -> Self {
        QueryError { kind: "implementation", message: m.into() }
    }
    pub fn unexpected(m: impl Into<String>) -> Self {
        QueryError { kind: "unexpected", message: m.into() }
    }
}

/// Who answers the queries of an execution (the host, on top of
/// `elements`). Each answer is recorded by whoever serves it (digest cache,
/// docs/MACROS-PROTOCOLO.md §6).
pub trait QueryService {
    fn query(&mut self, kind: &str, args: &Value) -> Result<Value, QueryError>;
}

/// One execution: the instance, the phase, the target and the prefetched
/// model.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub instance: u64,
    pub phase: Phase,
    pub target: Value,
    pub model: Value,
}

/// The executor, from the host's point of view. One persistent process per
/// session (governing rule, item 3).
pub trait MacroExecutor: Send {
    fn availability(&self) -> Availability;
    /// Handshake; returns the macros of the bootstrap.
    fn start(&mut self) -> Result<Handshake, String>;
    /// `macro_` is `uri#Class`; `arguments` is `{positional, named}`.
    /// Returns the instance and the names of the macro interfaces it
    /// implements (`ClassDeclarationsMacro`…), which decide the phases.
    fn instantiate(&mut self, macro_: &str, constructor: &str, arguments: &Value) -> Result<(u64, Vec<String>), String>;
    fn execute(&mut self, request: &ExecutionRequest, queries: &mut dyn QueryService) -> Result<ExecutionResult, String>;
    fn shut_down(&mut self);
}

/// The product's implementation today: refuses, giving the reason.
pub struct Unavailable(pub String);

impl Default for Unavailable {
    fn default() -> Self {
        Unavailable(
            "DartForge does not run macros yet: the executor is the native backend (docs/MACROS-PROTOCOLO.md §3), which \
             does not compile what the macro API uses yet; to materialize the augmentation, use `dartforge macros \
             --materialize --dart <dart executable>`"
                .into(),
        )
    }
}

impl MacroExecutor for Unavailable {
    fn availability(&self) -> Availability {
        Availability::Unavailable(self.0.clone())
    }
    fn start(&mut self) -> Result<Handshake, String> {
        Err(self.0.clone())
    }
    fn instantiate(&mut self, _: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        Err(self.0.clone())
    }
    fn execute(&mut self, _: &ExecutionRequest, _: &mut dyn QueryService) -> Result<ExecutionResult, String> {
        Err(self.0.clone())
    }
    fn shut_down(&mut self) {}
}

/// The `macro.*` client over a channel. Requests are numbered by the host;
/// during an execution, it answers the executor's `macro.query` queries.
pub struct DfexecExecutor<C: Channel> {
    channel: C,
    next: u64,
    started: bool,
}

impl<C: Channel> DfexecExecutor<C> {
    pub fn new(channel: C) -> Self {
        DfexecExecutor { channel, next: 1, started: false }
    }

    /// The channel (so tests can read the recording).
    pub fn channel(&self) -> &C {
        &self.channel
    }

    /// Gives the channel back (to wrap it, for example in a recorder).
    pub fn into_channel(self) -> C {
        self.channel
    }

    fn id(&mut self) -> u64 {
        let n = self.next;
        self.next += 1;
        n
    }

    /// Waits for the reply `t` to request `id`, answering the queries along
    /// the way.
    fn await_reply(&mut self, id: u64, t: &str, queries: &mut dyn QueryService) -> Result<Value, String> {
        loop {
            let m = self.channel.receive()?;
            match m.get("t").and_then(Value::as_str) {
                Some("macro.query") => {
                    let qid = m.get("id").cloned().unwrap_or(Value::Null);
                    let kind = m.get("kind").and_then(Value::as_str).unwrap_or("");
                    let args = m.get("args").cloned().unwrap_or(Value::Null);
                    let reply = match queries.query(kind, &args) {
                        Ok(value) => json!({"t": "macro.response", "id": qid, "value": value}),
                        Err(e) => json!({"t": "macro.response", "id": qid, "error": {"kind": e.kind, "message": e.message}}),
                    };
                    self.channel.send(&reply)?;
                }
                Some("error") => {
                    let msg = m.get("message").and_then(Value::as_str).unwrap_or("");
                    return Err(format!("executor: {msg}"));
                }
                Some(x) if x == t && m.get("id").and_then(Value::as_u64) == Some(id) => return Ok(m),
                _ => return Err(format!("unexpected message from the executor (expected {t} {id}): {m}")),
            }
        }
    }
}

struct NoQueries;
impl QueryService for NoQueries {
    fn query(&mut self, kind: &str, _: &Value) -> Result<Value, QueryError> {
        Err(QueryError::implementation(format!("query {kind} outside an execution")))
    }
}

impl<C: Channel> MacroExecutor for DfexecExecutor<C> {
    fn availability(&self) -> Availability {
        Availability::Available
    }

    fn start(&mut self) -> Result<Handshake, String> {
        self.channel.send(&host_hello())?;
        let r = check_hello(&self.channel.receive()?)?;
        self.started = true;
        Ok(r)
    }

    fn instantiate(&mut self, macro_: &str, constructor: &str, arguments: &Value) -> Result<(u64, Vec<String>), String> {
        let id = self.id();
        self.channel.send(&json!({"t": "macro.instantiate", "id": id, "macro": macro_, "constructor": constructor, "arguments": arguments}))?;
        let r = self.await_reply(id, "macro.instance", &mut NoQueries)?;
        let instance = r.get("instance").and_then(Value::as_u64).ok_or_else(|| format!("reply without an instance: {r}"))?;
        let interfaces = r
            .get("interfaces")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        Ok((instance, interfaces))
    }

    fn execute(&mut self, p: &ExecutionRequest, queries: &mut dyn QueryService) -> Result<ExecutionResult, String> {
        let id = self.id();
        self.channel.send(&json!({"t": "macro.execute", "id": id, "instance": p.instance, "phase": p.phase.name(),
            "target": p.target, "model": p.model}))?;
        let r = self.await_reply(id, "macro.result", queries)?;
        result_from_json(r.get("result").ok_or("missing result")?)
    }

    fn shut_down(&mut self) {
        if self.started {
            let _ = self.channel.send(&json!({"t": "end"}));
            let _ = self.channel.receive();
            self.started = false;
        }
    }
}

// ------------------------------------------------------------ recording

/// A channel that records everything that goes through it (`>` left the
/// host, `<` came from the executor), to replay the session without the
/// executor.
pub struct RecordingChannel<C: Channel> {
    pub inner: C,
    pub session: Vec<(char, Value)>,
}

impl<C: Channel> Channel for RecordingChannel<C> {
    fn send(&mut self, m: &Value) -> Result<(), String> {
        self.session.push(('>', m.clone()));
        self.inner.send(m)
    }
    fn receive(&mut self) -> Result<Value, String> {
        let m = self.inner.receive()?;
        self.session.push(('<', m.clone()));
        Ok(m)
    }
}

/// The fake executor: replays a recorded session. It checks that each host
/// message is **equal** to the recorded one — the model, the requests and the
/// answers to the queries — and returns the executor's messages in order.
/// It is the protocol test without any executor.
pub struct RecordedChannel {
    session: std::collections::VecDeque<(char, Value)>,
}

impl RecordedChannel {
    pub fn new(session: Vec<(char, Value)>) -> Self {
        RecordedChannel { session: session.into() }
    }

    /// One line per message: `> {json}` or `< {json}`.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut v = Vec::new();
        for (n, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            let (dir, rest) = l.split_at(1);
            let dir = dir.chars().next().unwrap_or(' ');
            if dir != '>' && dir != '<' {
                return Err(format!("line {}: invalid direction", n + 1));
            }
            v.push((dir, serde_json::from_str(rest.trim()).map_err(|e| format!("line {}: {e}", n + 1))?));
        }
        Ok(RecordedChannel::new(v))
    }

    /// What is left unconsumed.
    pub fn remaining(&self) -> usize {
        self.session.len()
    }
}

/// The recording as text (the format of [`RecordedChannel::from_text`]).
pub fn session_to_text(session: &[(char, Value)]) -> String {
    let mut s = String::new();
    for (d, m) in session {
        s.push(*d);
        s.push(' ');
        s.push_str(&m.to_string());
        s.push('\n');
    }
    s
}

impl Channel for RecordedChannel {
    fn send(&mut self, m: &Value) -> Result<(), String> {
        match self.session.pop_front() {
            Some(('>', expected)) if &expected == m => Ok(()),
            Some(('>', expected)) => Err(format!("the host sent\n  {m}\nbut the recorded session has\n  {expected}")),
            Some(('<', r)) => Err(format!("the host sent {m} where the recorded session expects to receive {r}")),
            _ => Err(format!("the host sent {m} after the end of the recorded session")),
        }
    }
    fn receive(&mut self) -> Result<Value, String> {
        match self.session.pop_front() {
            Some(('<', m)) => Ok(m),
            Some((_, m)) => Err(format!("the host expected to receive, but the recorded session sends {m}")),
            None => Err("end of the recorded session".into()),
        }
    }
}
