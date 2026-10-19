//! Artifact-bound allocation inventory consumption for fixed-image analysis.
//!
//! Allocation closure and exposure closure are distinct. This first consumer
//! admits only source functions whose complete closure is allocation-free and
//! consists of direct, ABI-matched definitions and reviewed LLVM expect calls.
//! Raw memory, native calls, image registration and unknown records stay open.
use super::super::Summary;
use crate::hir::Type;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

const SCOPE: &str =
    "allocation closure only; import pruning and native/registry exposure remain unverified";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Abi {
    result: String,
    parameters: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleRecord {
    module: String,
    final_ir_digest: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Global {
    module: String,
    symbol: String,
    kind: String,
    initializer_digest: String,
    references: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    symbol: String,
    abi: Abi,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Function {
    module: String,
    symbol: String,
    key: String,
    body_digest: String,
    local: bool,
    unique_linkage: bool,
    complete_body: bool,
    abi: Option<Abi>,
    direct_classes: Vec<u32>,
    unknown_obligations: Vec<String>,
    exposure_obligations: Vec<String>,
    calls: Vec<Call>,
    dependencies: Vec<String>,
    transitive_classes: Option<Vec<u32>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    modules: Vec<ModuleRecord>,
    globals: Vec<Global>,
    functions: Vec<Function>,
    scope: String,
}

#[derive(Clone, Debug)]
struct Fact {
    abi: Option<Abi>,
    local: bool,
    pure: bool,
}
/// Immutable facts from a verified compiler-owned cache snapshot.
#[derive(Clone, Debug)]
pub(crate) struct Inventory {
    facts: HashMap<String, Fact>,
    bytes: HashSet<String>,
    definitions: HashSet<String>,
    summaries: Vec<Summary>,
    known_allocations: usize,
}
fn digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn primitive(ty: &str) -> bool {
    matches!(
        ty,
        "void" | "i1" | "i8" | "i16" | "i32" | "i64" | "float" | "double" | "ptr"
    )
}
impl Abi {
    fn valid(&self) -> bool {
        primitive(&self.result) && self.parameters.iter().all(|p| p != "void" && primitive(p))
    }
    fn matches(&self, args: &[Type], result: Type) -> bool {
        self.result == result.llvm_ir()
            && self.parameters.len() == args.len()
            && self
                .parameters
                .iter()
                .zip(args)
                .all(|(expected, actual)| expected == actual.llvm_ir())
    }
}
fn intrinsic(call: &Call) -> bool {
    let ty = match call.symbol.as_str() {
        "llvm.expect.i1" => "i1",
        "llvm.expect.i64" => "i64",
        _ => return false,
    };
    call.abi.result == ty && call.abi.parameters == [ty, ty]
}

impl Inventory {
    /// Missing, stale or malformed inventories expose no optimization facts.
    /// The compiler-owned cache must stay immutable for the duration of linking.
    pub(crate) fn load(
        directory: &Path,
        key: &str,
        production: bool,
        names: &[String],
        runtime: &Path,
    ) -> Option<Self> {
        if !production {
            return None;
        }
        let mut value =
            super::publication::read_verified(directory, key, production, names, runtime)?;
        let mut summaries = Vec::new();
        for artifact in value["artifacts"].as_array()? {
            let name = artifact["name"].as_str()?;
            if !name.ends_with(".poda") {
                continue;
            }
            // Parse the same bytes whose digest is checked, not a memoized summary
            // or a second unverified snapshot of a mutable cache path.
            let bytes = std::fs::read(directory.join(name)).ok()?;
            if blake3::hash(&bytes).to_hex().as_str() != artifact["digest"].as_str()? {
                return None;
            }
            summaries.push(Summary::from_string(std::str::from_utf8(&bytes).ok()?).ok()?);
        }
        let functions = value["inventory"]["functions"].as_array()?;
        for record in functions {
            for field in ["abi", "transitive_classes"] {
                record.as_object()?.get(field)?;
            }
        }
        let raw: Payload = serde_json::from_value(value["inventory"].take()).ok()?;
        Self::parse(raw, summaries)
    }

    fn parse(raw: Payload, summaries: Vec<Summary>) -> Option<Self> {
        if raw.scope != SCOPE {
            return None;
        }
        let modules: HashSet<_> = raw.modules.iter().map(|m| m.module.as_str()).collect();
        if modules.len() != raw.modules.len()
            || modules.len() != summaries.len()
            || raw
                .modules
                .iter()
                .any(|m| m.module.is_empty() || !digest(&m.final_ir_digest))
            || summaries
                .iter()
                .map(|s| s.module.as_str())
                .collect::<HashSet<_>>()
                != modules
        {
            return None;
        }
        let declared: HashMap<_, HashMap<_, usize>> = summaries
            .iter()
            .map(|s| {
                let mut names = HashMap::new();
                for definition in &s.definitions {
                    *names
                        .entry(s.names.get(definition.name as usize)?.as_str())
                        .or_insert(0) += 1;
                }
                Some((s.module.as_str(), names))
            })
            .collect::<Option<_>>()?;
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, f) in raw.functions.iter().enumerate() {
            let key = if f.local {
                format!("{}#{}", f.module, f.symbol)
            } else {
                f.symbol.clone()
            };
            if !modules.contains(f.module.as_str())
                || f.symbol.is_empty()
                || key != f.key
                || !digest(&f.body_digest)
                || f.abi.as_ref().is_some_and(|a| !a.valid())
                || !ordered(&f.direct_classes)
                || f.direct_classes.iter().any(|cid| *cid > i32::MAX as u32)
                || !ordered(&f.unknown_obligations)
                || !ordered(&f.exposure_obligations)
                || f.transitive_classes
                    .as_ref()
                    .is_some_and(|cs| !ordered(cs) || cs.iter().any(|cid| *cid > i32::MAX as u32))
                || f.calls
                    .iter()
                    .any(|c| c.symbol.is_empty() || !c.abi.valid())
                || f.dependencies.iter().collect::<HashSet<_>>().len() != f.dependencies.len()
            {
                return None;
            }
            by_key.entry(f.key.clone()).or_default().push(index);
        }
        let mut edges = vec![Vec::new(); raw.functions.len()];
        let mut pure_calls = vec![true; raw.functions.len()];
        for (index, f) in raw.functions.iter().enumerate() {
            for call in &f.calls {
                let local = format!("{}#{}", f.module, call.symbol);
                let targets = by_key
                    .get(local.as_str())
                    .or_else(|| by_key.get(call.symbol.as_str()));
                match targets {
                    Some(targets)
                        if targets.len() == 1
                            && raw.functions[targets[0]].abi.as_ref() == Some(&call.abi) =>
                    {
                        edges[index].push(targets[0])
                    }
                    None if intrinsic(call) => {}
                    _ => pure_calls[index] = false,
                }
            }
            edges[index].sort_unstable();
            edges[index].dedup();
            let expected: BTreeSet<_> = edges[index]
                .iter()
                .map(|&i| raw.functions[i].key.as_str())
                .collect();
            if f.dependencies
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != expected
            {
                return None;
            }
        }
        let mut allocations: Vec<Option<BTreeSet<u32>>> = vec![None; edges.len()];
        let mut pure = vec![false; edges.len()];
        let mut byte_keys = HashSet::new();
        let mut globals = HashSet::new();
        let mut global_counts = HashMap::new();
        for g in &raw.globals {
            if !modules.contains(g.module.as_str())
                || g.symbol.is_empty()
                || !digest(&g.initializer_digest)
                || !matches!(g.kind.as_str(), "native_bytes" | "unclassified_initializer")
            {
                return None;
            }
            let local = format!("{}#{}", g.module, g.symbol);
            let key = if declared[g.module.as_str()].contains_key(local.as_str()) {
                local
            } else {
                g.symbol.clone()
            };
            globals.insert(key.clone());
            *global_counts.entry(key.clone()).or_insert(0usize) += 1;
            // Public data can interpose with a program helper. Admit only local
            // native byte constants; managed images remain a separate obligation.
            if g.kind == "native_bytes"
                && g.references.is_empty()
                && key.contains('#')
                && declared[g.module.as_str()].get(key.as_str()) == Some(&1)
                && !by_key.contains_key(key.as_str())
            {
                byte_keys.insert(key);
            }
        }
        // Declarations and interposable globals may repeat across SDK modules.
        // They remain opaque; only a uniquely defined local byte constant is safe.
        byte_keys.retain(|key| global_counts.get(key) == Some(&1));
        for component in crate::optimize::scc::components(&edges) {
            let members: HashSet<_> = component.iter().copied().collect();
            let mut known = true;
            let mut is_pure = true;
            let mut classes = BTreeSet::new();
            for &index in &component {
                let f = &raw.functions[index];
                known &= f.complete_body
                    && f.unique_linkage
                    && f.abi.is_some()
                    && f.unknown_obligations.is_empty()
                    && by_key[f.key.as_str()].len() == 1;
                classes.extend(f.direct_classes.iter().copied());
                let source_prefix = format!("df.{}.", crate::context::escape_str(&f.module));
                is_pure &= f.exposure_obligations.is_empty()
                    && pure_calls[index]
                    && (f.local || f.symbol.starts_with(&source_prefix))
                    && declared[f.module.as_str()].get(f.key.as_str()) == Some(&1)
                    && !globals.contains(&f.key);
                for &target in &edges[index] {
                    if members.contains(&target) {
                        continue;
                    }
                    match &allocations[target] {
                        Some(cs) => classes.extend(cs),
                        None => known = false,
                    }
                    is_pure &= pure[target];
                }
            }
            for &index in &component {
                if known {
                    allocations[index] = Some(classes.clone());
                }
                pure[index] = known && is_pure && classes.is_empty();
                if raw.functions[index]
                    .transitive_classes
                    .as_ref()
                    .map(|v| v.iter().copied().collect::<BTreeSet<_>>())
                    != allocations[index]
                {
                    return None;
                }
            }
        }
        let known_allocations = allocations.iter().filter(|c| c.is_some()).count();
        let mut facts = HashMap::new();
        for (index, f) in raw.functions.into_iter().enumerate() {
            let unique = by_key[f.key.as_str()].len() == 1;
            if unique {
                facts.insert(
                    f.key,
                    Fact {
                        abi: f.abi,
                        local: f.local,
                        pure: pure[index],
                    },
                );
            }
        }
        let definitions = declared
            .values()
            .flat_map(|names| names.keys().map(|s| (*s).to_string()))
            .collect();
        Some(Self {
            facts,
            bytes: byte_keys,
            definitions,
            summaries,
            known_allocations,
        })
    }

    /// Reconcile with the actual graph inputs; never trust a stale path-only memo.
    pub(crate) fn reconcile(
        &self,
        sdk: &[&Summary],
        local: &HashSet<String>,
        class_ids: &HashSet<u32>,
    ) -> bool {
        if sdk.len() != self.summaries.len()
            || sdk
                .iter()
                .map(|s| s.module.as_str())
                .collect::<HashSet<_>>()
                .len()
                != sdk.len()
            || sdk
                .iter()
                .any(|s| !self.summaries.iter().any(|expected| expected == *s))
        {
            return false;
        }
        let runtime: HashSet<_> = crate::llvm::externs::EXTERNS
            .iter()
            .map(|e| e.name())
            .collect();
        if self
            .definitions
            .iter()
            .filter(|name| !name.contains('#'))
            .any(|name| {
                local.contains(name) || runtime.contains(name.as_str()) || name.starts_with("llvm.")
            })
        {
            return false;
        }
        sdk.iter().all(|s| {
            s.tables
                .iter()
                .all(|table| u32::try_from(table.cid).is_ok_and(|cid| !class_ids.contains(&cid)))
                && s.nominal_classes
                    .iter()
                    .chain(&s.ambiguous_classes)
                    .all(|cid| u32::try_from(*cid).is_ok_and(|cid| !class_ids.contains(&cid)))
        })
    }
    pub(crate) fn known_node(&self, key: &str) -> bool {
        self.bytes.contains(key)
            || self.facts.get(key).is_some_and(|f| f.pure)
            || matches!(key, "llvm.expect.i1" | "llvm.expect.i64")
                && !self.definitions.contains(key)
    }
    pub(crate) fn call(&self, symbol: &str, args: &[Type], result: Type) -> bool {
        self.facts.get(symbol).is_some_and(|f| {
            !f.local && f.pure && f.abi.as_ref().is_some_and(|abi| abi.matches(args, result))
        })
    }
    pub(crate) fn contains_definition(&self, key: &str) -> bool {
        self.definitions.contains(key)
    }
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        (
            self.facts.len(),
            self.known_allocations,
            self.facts.values().filter(|f| f.pure).count(),
        )
    }
}

#[cfg(test)]
mod tests;
