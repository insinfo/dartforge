//! The expansion cache (docs/MACROS-PROTOCOLO.md §6): reuses the result of a
//! macro application in a phase without talking to the executor, when
//! everything the previous execution **observed** is still the same.
//!
//! For each `(application, phase)` the cache keeps a record:
//!
//! * the fingerprint of the `macro.executar` **request** — the identity of
//!   the macro implementation (the source of its library's import closure),
//!   the constructor and the arguments, the phase, the target and the model
//!   with the prefetched members —, plus the executor identity and the host
//!   version;
//! * each `macro.consulta`, **in order**, with the fingerprint of the answer
//!   — including the negative ones ("does not exist" errors) and the
//!   collections in the order the executor received them;
//! * the result.
//!
//! On a new application the host builds the request again; if the
//! fingerprint matches, it replays the recorded queries against the current
//! program (the *revalidation*, like the build engine's,
//! docs/BUILD-MOTOR.md §4). All equal, the recorded result holds — the macro
//! is a deterministic function of what it received (the spec forbids I/O
//! outside `Resource`, and instance state between phases is not
//! guaranteed). Any difference, the macro runs again and the record is
//! replaced. The first query that differs stops the revalidation: the
//! following ones depended on the old answer.
//!
//! **The ids.** The request and the answers carry the ids of the [`Table`]
//! and the `StaticType` keys ([`Statics`]). The cache keeps both and uses
//! them in every compilation it serves: an id never changes meaning, and a
//! new identifier (an added field) gets a new id without shifting the
//! others. That is why an edit an application does not observe changes
//! neither its request nor its answers. A compilation without a cache starts
//! from an empty table, with the usual ids (those of the recorded CFE
//! session).
//!
//! **Where it lives.** In memory, with whoever compiles several times in the
//! same process (`dartforge dev`, the LSP, the tests): the build engine does
//! not write records to disk either (docs/BUILD-MOTOR.md §4, D-B1), and
//! governing rule 6 of macros forbids bookkeeping on disk.
use crate::aplicacoes::{Application, Target};
use crate::assembly::ExecutionResult;
use crate::executor::{Phase, QueryError, QueryService};
use crate::modelo::{Key, Table};
use crate::protocol::HOST_VERSION;
use crate::queries::Statics;
use dartforge_elements::model::{LibraryId, Program};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Fingerprint (blake3).
pub type Digest = [u8; 32];

/// The **stable** identity of an application: what it is, not where it is
/// in the AST. It survives edits to other declarations and reloads; two
/// equal annotations on the same target are told apart by `ordinal`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApplicationIdentity {
    pub library: String,
    pub target: Target,
    /// `uri#Class` of the macro.
    pub macro_: String,
    pub constructor: String,
    /// The arguments in the protocol format, as JSON.
    pub arguments: String,
    /// How many identical applications come before this one on the same
    /// target.
    pub ordinal: usize,
}

impl ApplicationIdentity {
    /// The identities of `apps`, in the same order.
    pub fn of_all(apps: &[Application]) -> Vec<ApplicationIdentity> {
        let mut out: Vec<ApplicationIdentity> = Vec::with_capacity(apps.len());
        for a in apps {
            let mut id = ApplicationIdentity {
                library: a.library.clone(),
                target: a.target.clone(),
                macro_: a.macro_.clone(),
                constructor: a.constructor.clone(),
                arguments: a.arguments.to_string(),
                ordinal: 0,
            };
            id.ordinal = out.iter().filter(|x| **x == id).count();
            out.push(id);
        }
        out
    }
}

impl std::fmt::Display for ApplicationIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let target = match &self.target {
            Target::Library(_) => "library".to_string(),
            Target::Declaration(
                Key::Method { owner, name, .. } | Key::Field { owner, name, .. } | Key::Constructor { owner, name, .. },
            ) => format!("{owner}.{name}"),
            Target::Declaration(c) => c.name().to_string(),
        };
        let class = self.macro_.rsplit('#').next().unwrap_or("");
        let constructor = if self.constructor.is_empty() { String::new() } else { format!(".{}", self.constructor) };
        write!(f, "{} {target} @{class}{constructor}", self.library)?;
        if self.ordinal > 0 {
            write!(f, " #{}", self.ordinal)?;
        }
        Ok(())
    }
}

/// A macro instance: the implementation, the constructor and the arguments.
/// The implementation enters through the fingerprint of its source (not its
/// name): editing the macro invalidates its instances.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InstanceKey {
    pub implementation: Digest,
    pub macro_: String,
    pub constructor: String,
    pub arguments: String,
}

/// A query made by the execution and the fingerprint of its answer.
#[derive(Debug, Clone)]
pub(crate) struct RecordedQuery {
    kind: String,
    args: Value,
    response: Digest,
}

/// What an execution read and produced.
#[derive(Debug, Clone)]
pub(crate) struct Record {
    request: Digest,
    queries: Vec<RecordedQuery>,
    result: ExecutionResult,
}

/// The expansion cache of an executor. One value per executor
/// configuration: the identity given to [`MacroCache::new`] goes into every
/// fingerprint.
///
/// ```
/// use dartforge_macros_host::cache::MacroCache;
/// let cache = MacroCache::new("vm dart-3.6.2");
/// assert!(cache.is_empty());
/// ```
#[derive(Debug)]
pub struct MacroCache {
    /// Without a cache (single compilation): nothing is recorded or
    /// revalidated.
    pub(crate) enabled: bool,
    pub(crate) executor_identity: String,
    pub(crate) table: Table,
    pub(crate) statics: Statics,
    /// The macro interfaces of each instance (the phases it runs in): with
    /// them the host decides the phases without instantiating in the
    /// executor.
    pub(crate) interfaces: HashMap<InstanceKey, Vec<String>>,
    pub(crate) records: Records,
}

impl MacroCache {
    /// An empty cache for the executor identified by `executor_identity`
    /// (the executable and the version of the macro API, for example).
    pub fn new(executor_identity: impl Into<String>) -> Self {
        MacroCache {
            enabled: true,
            executor_identity: executor_identity.into(),
            table: Table::default(),
            statics: Statics::default(),
            interfaces: HashMap::new(),
            records: HashMap::new(),
        }
    }

    /// The state of a compilation without a cache: a new table and nothing
    /// recorded.
    pub(crate) fn disabled() -> Self {
        MacroCache { enabled: false, ..MacroCache::new("") }
    }

    /// How many `(application, phase)` expansions are recorded.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

}

/// The records per `(application, phase)`.
pub(crate) type Records = HashMap<(ApplicationIdentity, Phase), Record>;

/// The recorded result of `key`, if it still holds for `request` and the
/// recorded queries answer the same in `queries` (the current program).
pub(crate) fn revalidate(
    records: &Records,
    key: &(ApplicationIdentity, Phase),
    request: &Digest,
    queries: &mut dyn QueryService,
) -> Revalidation {
    let Some(r) = records.get(key) else { return Revalidation::Missing };
    if r.request != *request {
        return Revalidation::RequestChanged;
    }
    for (i, c) in r.queries.iter().enumerate() {
        if response_digest(&queries.query(&c.kind, &c.args)) != c.response {
            return Revalidation::QueryChanged { replayed: i + 1 };
        }
    }
    Revalidation::Valid { result: Box::new(r.result.clone()), replayed: r.queries.len() }
}

/// The outcome of a revalidation.
pub(crate) enum Revalidation {
    Missing,
    RequestChanged,
    /// Query number `replayed` answered differently.
    QueryChanged { replayed: usize },
    Valid { result: Box<ExecutionResult>, replayed: usize },
}

/// Records a new [`Record`] (after executing) and says whether the result is
/// the same as the previous record's — "same output": the following
/// applications do not lose the cache because of this one, since the
/// program they see does not change.
pub(crate) fn record(
    records: &mut Records,
    key: (ApplicationIdentity, Phase),
    request: Digest,
    queries: Vec<RecordedQuery>,
    result: &ExecutionResult,
) -> bool {
    let same = records.get(&key).is_some_and(|r| r.result == *result);
    records.insert(key, Record { request, queries, result: result.clone() });
    same
}

/// Forgets what this compilation did not use (removed applications): the
/// cache does not grow with the edit history.
pub(crate) fn prune(
    records: &mut Records,
    interfaces: &mut HashMap<InstanceKey, Vec<String>>,
    used: &HashSet<(ApplicationIdentity, Phase)>,
    instances: &HashSet<InstanceKey>,
) {
    records.retain(|k, _| used.contains(k));
    interfaces.retain(|k, _| instances.contains(k));
}

/// Records, in order, the queries that go through to `inner` (only with the
/// cache enabled: without it, there is nothing to revalidate later).
pub(crate) struct Recorder<'s> {
    inner: &'s mut dyn QueryService,
    enabled: bool,
    pub(crate) queries: Vec<RecordedQuery>,
}

impl<'s> Recorder<'s> {
    pub fn new(inner: &'s mut dyn QueryService, enabled: bool) -> Self {
        Recorder { inner, enabled, queries: Vec::new() }
    }
}

impl QueryService for Recorder<'_> {
    fn query(&mut self, kind: &str, args: &Value) -> Result<Value, QueryError> {
        let r = self.inner.query(kind, args);
        if self.enabled {
            self.queries.push(RecordedQuery { kind: kind.to_string(), args: args.clone(), response: response_digest(&r) });
        }
        r
    }
}

fn hash_field(h: &mut blake3::Hasher, s: &str) {
    h.update(&(s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}

/// The fingerprint of an answer, including an error one (a negative answer
/// is a dependency too).
fn response_digest(r: &Result<Value, QueryError>) -> Digest {
    let mut h = blake3::Hasher::new();
    match r {
        Ok(v) => {
            hash_field(&mut h, "ok");
            hash_field(&mut h, &v.to_string());
        }
        Err(e) => {
            hash_field(&mut h, "erro");
            hash_field(&mut h, e.kind);
            hash_field(&mut h, &e.message);
        }
    }
    *h.finalize().as_bytes()
}

/// The fingerprint of the `macro.executar` request, without the instance
/// number (which belongs to the executor process, not to the macro).
pub(crate) fn request_digest(executor: &str, instance: &InstanceKey, phase: Phase, target: &Value, model: &Value) -> Digest {
    let mut h = blake3::Hasher::new();
    hash_field(&mut h, HOST_VERSION);
    hash_field(&mut h, executor);
    h.update(&instance.implementation);
    hash_field(&mut h, &instance.macro_);
    hash_field(&mut h, &instance.constructor);
    hash_field(&mut h, &instance.arguments);
    hash_field(&mut h, phase.name());
    hash_field(&mut h, &target.to_string());
    hash_field(&mut h, &model.to_string());
    *h.finalize().as_bytes()
}

/// The identity of the implementation of the macro declared in library
/// `uri`: the source of every unit of its import and export closure (SDK
/// units enter by URI; the SDK version is part of the executor identity).
pub(crate) fn implementation_identity(program: &Program, uri: &str) -> Digest {
    let mut h = blake3::Hasher::new();
    hash_field(&mut h, HOST_VERSION);
    hash_field(&mut h, uri);
    let Some(start) = program.libraries.iter().position(|l| l.uri == uri) else {
        return *h.finalize().as_bytes();
    };
    let mut seen: HashSet<LibraryId> = HashSet::new();
    let mut stack = vec![LibraryId(start as u32)];
    while let Some(l) = stack.pop() {
        if !seen.insert(l) {
            continue;
        }
        let lib = program.library(l);
        stack.extend(lib.imports.iter().map(|i| i.library));
        stack.extend(lib.exports.iter().map(|e| e.library));
    }
    let mut libs: Vec<LibraryId> = seen.into_iter().collect();
    libs.sort_by(|a, b| program.library(*a).uri.cmp(&program.library(*b).uri));
    for l in libs {
        let lib = program.library(l);
        hash_field(&mut h, &lib.uri);
        if lib.is_sdk {
            continue;
        }
        for &u in &lib.units {
            let unit = program.unit(u);
            hash_field(&mut h, &unit.uri);
            hash_field(&mut h, &unit.source);
        }
    }
    *h.finalize().as_bytes()
}
