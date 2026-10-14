//! Artifact-bound transport for allocation-only SDK body return witnesses.
//!
//! This is a separate proof schema from declaration/reachability summaries. The
//! published producer audits immutable HIR, then binds records to every finalized
//! SDK object and pruning summary in the exact selected compilation cache.
use super::*;
use std::path::Path;

const SCHEMA: u64 = 1;
const FILE: &str = "allocation-returns.json";

#[derive(Clone, Debug)]
pub(crate) struct Signature {
    pub(crate) symbol: String,
    pub(crate) parameters: Vec<Type>,
    pub(crate) result: Type,
    body_digest: String,
}
#[derive(Clone, Debug)]
pub(crate) struct Export {
    pub(crate) symbol: String,
    pub(crate) parameters: Vec<Type>,
    pub(crate) classes: Classes,
    pub(crate) closure: Vec<Signature>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Imports {
    pub(crate) exports: HashMap<String, Export>,
    pub(crate) tables: HashMap<u32, Vec<(String, String)>>,
}

pub(crate) fn export(modules: &[&Module], proof: &Provenance<'_>) -> Vec<Export> {
    super::allocation_only_exports::candidates(modules, proof)
        .into_iter()
        .filter_map(|candidate| {
            if candidate.classes.contains(&0) || candidate.closure.iter().any(|f| shared(&f.symbol))
            {
                return None;
            }
            Some(Export {
                symbol: candidate.symbol.into(),
                parameters: candidate.parameters,
                classes: candidate.classes,
                closure: candidate
                    .closure
                    .into_iter()
                    .map(|f| Signature {
                        symbol: f.symbol.clone(),
                        parameters: f.params.iter().map(|(_, _, ty)| *ty).collect(),
                        result: f.return_ty,
                        // Function HIR has no unordered instruction collections. The
                        // digest records the exact producer snapshot, not a nominal type.
                        body_digest: blake3::hash(format!("{f:?}").as_bytes())
                            .to_hex()
                            .to_string(),
                    })
                    .collect(),
            })
        })
        .collect()
}
/// Bind every closure member to one actual final emitted strong definition.
/// The cache trusts compiler-owned artifacts, as `.poda` already does. This
/// validates producer linkage/body identity before publication, rather than
/// interpreting declaration-only names as runtime body certificates.
pub(crate) fn bind_final_ir(
    exports: Vec<Export>,
    libraries: &[crate::sdk_module::SdkLibrary],
) -> Vec<Export> {
    let mut definitions: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for library in libraries {
        let mut lines = library.ir.lines();
        while let Some(header) = lines.next() {
            if !header.starts_with("define ") {
                continue;
            }
            let Some((prefix, rest)) = header.split_once('@') else {
                continue;
            };
            let Some((symbol, _)) = rest.split_once('(') else {
                continue;
            };
            let symbol = symbol.trim_matches('"').to_string();
            let mut body = String::from(header);
            body.push('\n');
            if !header.ends_with('}') {
                for line in lines.by_ref() {
                    body.push_str(line);
                    body.push('\n');
                    if line == "}" {
                        break;
                    }
                }
            }
            definitions
                .entry(symbol)
                .or_default()
                .push((prefix.to_string(), body));
        }
    }
    exports
        .into_iter()
        .filter_map(|mut export| {
            for dependency in &mut export.closure {
                let rows = definitions.get(&dependency.symbol)?;
                if rows.len() != 1 {
                    return None;
                }
                let (prefix, body) = &rows[0];
                if [
                    "linkonce",
                    "weak",
                    "available_externally",
                    "internal",
                    "private",
                ]
                .iter()
                .any(|word| {
                    prefix
                        .split_whitespace()
                        .any(|token| token.starts_with(word))
                }) || !prefix
                    .split_whitespace()
                    .any(|token| token == dependency.result.llvm_ir())
                {
                    return None;
                }
                let header = body.lines().next()?;
                let arguments = header.split_once('(')?.1.split_once(')')?.0;
                let observed: Vec<_> = if arguments.trim().is_empty() {
                    Vec::new()
                } else {
                    arguments
                        .split(',')
                        .map(|argument| argument.trim().split_whitespace().next())
                        .collect::<Option<Vec<_>>>()?
                };
                if observed
                    != dependency
                        .parameters
                        .iter()
                        .map(|ty| ty.llvm_ir())
                        .collect::<Vec<_>>()
                {
                    return None;
                }
                dependency.body_digest = blake3::hash(body.as_bytes()).to_hex().to_string();
            }
            Some(export)
        })
        .collect()
}

fn shared(symbol: &str) -> bool {
    symbol.starts_with("dfc.") || symbol.ends_with("$tear") || symbol.ends_with("$tearm")
}
fn code(ty: Type) -> Option<&'static str> {
    Some(match ty {
        Type::Ref => "ref",
        Type::Ptr => "ptr",
        Type::Void => "void",
        Type::I1 => "i1",
        Type::I8 => "i8",
        Type::I64 => "i64",
        Type::F64 => "f64",
        _ => return None,
    })
}
fn ty(value: &serde_json::Value) -> Option<Type> {
    Some(match value.as_str()? {
        "ref" => Type::Ref,
        "ptr" => Type::Ptr,
        "void" => Type::Void,
        "i1" => Type::I1,
        "i8" => Type::I8,
        "i64" => Type::I64,
        "f64" => Type::F64,
        _ => return None,
    })
}
fn parameters(values: &[Type]) -> Option<Vec<&'static str>> {
    values.iter().map(|t| code(*t)).collect()
}
fn parse_parameters(value: &serde_json::Value) -> Option<Vec<Type>> {
    let result: Vec<Type> = value.as_array()?.iter().map(ty).collect::<Option<_>>()?;
    (!result.contains(&Type::Void)).then_some(result)
}
fn digest(path: &Path) -> Option<String> {
    Some(
        blake3::hash(&std::fs::read(path).ok()?)
            .to_hex()
            .to_string(),
    )
}
fn fields(value: &serde_json::Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|k| object.contains_key(*k))
    })
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub(crate) fn publish(
    directory: &Path,
    key: &str,
    production: bool,
    artifacts: &[String],
    exports: &[Export],
    tables: &[crate::pruning::ModuleTable],
) -> Result<String, String> {
    let artifact_records = artifacts
        .iter()
        .map(|name| {
            let hash = digest(&directory.join(name))
                .ok_or_else(|| format!("cannot hash SDK proof artifact {name}"))?;
            Ok(serde_json::json!({"name":name,"digest":hash}))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let records = exports.iter().filter_map(|export| {
        let closure = export.closure.iter().map(|f| Some(serde_json::json!({
            "symbol":f.symbol,"parameters":parameters(&f.parameters)?,"result":code(f.result)?,"body":f.body_digest
        }))).collect::<Option<Vec<_>>>()?;
        Some(serde_json::json!({"symbol":export.symbol,"parameters":parameters(&export.parameters)?,
            "classes":export.classes.iter().copied().collect::<Vec<_>>(),"closure":closure}))
    }).collect::<Vec<_>>();
    let mut cid_counts = HashMap::new();
    for (cid, _, _) in tables {
        *cid_counts.entry(*cid).or_insert(0usize) += 1;
    }
    let table_records: Vec<_> = tables
        .iter()
        .filter(|(cid, _, pairs)| {
            *cid >= dartforge_runtime::layout::FIRST_FREE_CID as u32
                && cid_counts.get(cid) == Some(&1)
                && pairs.iter().any(|(selector, symbol)| {
                    selector.starts_with("g:") && exports.iter().any(|e| e.symbol == *symbol)
                })
        })
        .map(|(cid, table, pairs)| serde_json::json!({"cid":cid,"table":table,"pairs":pairs}))
        .collect();
    let value = serde_json::json!({"schema":SCHEMA,"cache_key":key,"production":production,
        "artifacts":artifact_records,"exports":records,"tables":table_records});
    let text = format!(
        "{}\n",
        serde_json::to_string(&value).map_err(|e| e.to_string())?
    );
    std::fs::write(directory.join(FILE), &text)
        .map_err(|e| format!("cannot publish SDK allocation proof: {e}"))?;
    Ok(blake3::hash(text.as_bytes()).to_hex().to_string())
}

/// Missing, old, altered or malformed bundles conservatively expose no facts.
pub(crate) fn load(directory: &Path, key: &str, production: bool, artifacts: &[String]) -> Imports {
    load_checked(directory, key, production, artifacts).unwrap_or_default()
}
fn load_checked(
    directory: &Path,
    key: &str,
    production: bool,
    artifacts: &[String],
) -> Option<Imports> {
    if !production || std::env::var("DARTFORGE_NO_SDK_ALLOCATION_PROOFS").as_deref() == Ok("1") {
        return None;
    }
    let text = std::fs::read_to_string(directory.join(FILE)).ok()?;
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("pronto")).ok()?).ok()?;
    if !fields(&marker, &["cache_key", "allocation_proof"])
        || marker["cache_key"].as_str()? != key
        || marker["allocation_proof"].as_str()? != blake3::hash(text.as_bytes()).to_hex().as_str()
    {
        return None;
    }
    if text.len() > 16 << 20 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    // Reject duplicate JSON keys and alternative encodings by requiring the
    // exact canonical form emitted by this producer schema.
    if text != format!("{}\n", serde_json::to_string(&value).ok()?)
        || !fields(
            &value,
            &[
                "schema",
                "cache_key",
                "production",
                "artifacts",
                "exports",
                "tables",
            ],
        )
        || value["schema"].as_u64()? != SCHEMA
        || value["cache_key"].as_str()? != key
        || value["production"].as_bool()? != production
    {
        return None;
    }
    let observed = value["artifacts"].as_array()?;
    if observed.len() != artifacts.len() {
        return None;
    }
    for (record, expected) in observed.iter().zip(artifacts) {
        if !fields(record, &["name", "digest"])
            || record["name"].as_str()? != expected
            || record["digest"].as_str()? != digest(&directory.join(expected))?
        {
            return None;
        }
    }
    let mut defined = HashMap::new();
    let mut actual_tables = HashMap::new();
    let mut ambiguous_cids = HashSet::new();
    let mut class_counts = HashMap::new();
    for name in artifacts.iter().filter(|name| name.ends_with(".poda")) {
        let summary = crate::pruning::Summary::from_string(
            &std::fs::read_to_string(directory.join(name)).ok()?,
        )
        .ok()?;
        ambiguous_cids.extend(summary.ambiguous_classes.iter().copied());
        for cid in &summary.nominal_classes {
            *class_counts.entry(*cid).or_insert(0usize) += 1;
        }
        for definition in &summary.definitions {
            *defined
                .entry(summary.names[definition.name as usize].clone())
                .or_insert(0usize) += 1;
        }
        for table in &summary.tables {
            let table_name = summary.names[table.node as usize]
                .strip_suffix("$d")?
                .to_string();
            let pairs: BTreeSet<_> = table
                .pairs
                .iter()
                .map(|(selector, function)| (*selector, summary.names[*function as usize].clone()))
                .collect();
            if actual_tables
                .insert(table.cid, (table_name, pairs))
                .is_some()
            {
                ambiguous_cids.insert(table.cid);
            }
        }
    }
    let mut result = Imports::default();
    let mut signatures: HashMap<String, (Vec<Type>, Type, String)> = HashMap::new();
    for record in value["exports"].as_array()? {
        if !fields(record, &["symbol", "parameters", "classes", "closure"]) {
            return None;
        }
        let symbol = record["symbol"].as_str()?.to_string();
        if symbol.is_empty() || shared(&symbol) {
            return None;
        }
        let parameters = parse_parameters(&record["parameters"])?;
        let mut classes = Classes::new();
        for cid in record["classes"].as_array()? {
            let cid = u32::try_from(cid.as_u64()?).ok()?;
            if cid == 0
                || cid > i32::MAX as u32
                || (cid >= dartforge_runtime::layout::FIRST_FREE_CID as u32
                    && (class_counts.get(&i64::from(cid)) != Some(&1)
                        || ambiguous_cids.contains(&i64::from(cid))))
                || !classes.insert(cid)
            {
                return None;
            }
        }
        if classes.is_empty() || classes.iter().all(|c| *c == 1) {
            return None;
        }
        let mut closure = Vec::new();
        let mut seen = HashSet::new();
        for dependency in record["closure"].as_array()? {
            if !fields(dependency, &["symbol", "parameters", "result", "body"]) {
                return None;
            }
            let name = dependency["symbol"].as_str()?.to_string();
            let signature = Signature {
                symbol: name.clone(),
                parameters: parse_parameters(&dependency["parameters"])?,
                result: ty(&dependency["result"])?,
                body_digest: dependency["body"].as_str()?.to_string(),
            };
            if name.is_empty()
                || defined.get(&name) != Some(&1)
                || shared(&name)
                || !seen.insert(name.clone())
                || !valid_digest(&signature.body_digest)
            {
                return None;
            }
            let identity = (
                signature.parameters.clone(),
                signature.result,
                signature.body_digest.clone(),
            );
            if signatures
                .insert(name, identity.clone())
                .is_some_and(|old| old != identity)
            {
                return None;
            }
            closure.push(signature);
        }
        if !closure
            .iter()
            .any(|f| f.symbol == symbol && f.parameters == parameters && f.result == Type::Ref)
            || result
                .exports
                .insert(
                    symbol.clone(),
                    Export {
                        symbol,
                        parameters,
                        classes,
                        closure,
                    },
                )
                .is_some()
        {
            return None;
        }
    }
    let mut table_symbols = HashSet::new();
    for record in value["tables"].as_array()? {
        if !fields(record, &["cid", "table", "pairs"]) {
            return None;
        }
        let cid = u32::try_from(record["cid"].as_u64()?).ok()?;
        let table = record["table"].as_str()?;
        if cid < dartforge_runtime::layout::FIRST_FREE_CID as u32
            || !table_symbols.insert(table.to_string())
        {
            return None;
        }
        let mut pairs = Vec::new();
        let mut hashes = HashSet::new();
        for pair in record["pairs"].as_array()? {
            let pair = pair.as_array()?;
            if pair.len() != 2 {
                return None;
            }
            let selector = pair[0].as_str()?.to_string();
            let symbol = pair[1].as_str()?.to_string();
            if selector.is_empty()
                || symbol.is_empty()
                || !hashes.insert(crate::lower::closures::hash_name(&selector))
            {
                return None;
            }
            pairs.push((selector, symbol));
        }
        let actual = actual_tables.get(&i64::from(cid))?;
        let observed: BTreeSet<_> = pairs
            .iter()
            .map(|(selector, function)| {
                (
                    crate::lower::closures::hash_name(selector),
                    function.clone(),
                )
            })
            .collect();
        if class_counts.get(&i64::from(cid)) != Some(&1)
            || ambiguous_cids.contains(&i64::from(cid))
            || actual.0 != table
            || actual.1 != observed
        {
            return None;
        }
        if result.tables.insert(cid, pairs).is_some() {
            return None;
        }
    }
    Some(result)
}

impl Imports {
    /// Program definitions cannot replace any linked SDK dependency, even when
    /// their nominal signatures or return class sets happen to match.
    pub(crate) fn reconcile(&self, modules: &[&Module]) -> Self {
        if modules.iter().any(|m| {
            !m.arc_fixed_ancestry || m.arc_memory || m.arc_prepared || m.arc_aot_ports.is_some()
        }) {
            return Self::default();
        }
        let local: HashSet<_> = modules
            .iter()
            .flat_map(|m| &m.functions)
            .map(|f| f.symbol.as_str())
            .collect();
        if [
            "dartforge_object_new",
            "dartforge_selector",
            "dartforge_register_methods",
        ]
        .iter()
        .any(|name| local.contains(name))
        {
            return Self::default();
        }
        let exports: HashMap<_, _> = self
            .exports
            .iter()
            .filter(|(_, e)| e.closure.iter().all(|f| !local.contains(f.symbol.as_str())))
            .map(|(symbol, export)| (symbol.clone(), export.clone()))
            .collect();
        let registry_closed = registry::closed(modules);
        let tables = self
            .tables
            .iter()
            .filter(|(cid, _)| {
                modules.iter().all(|m| {
                    registry_closed
                        && !m.pruning_ambiguous_class_ids.contains(cid)
                        && !m.arc_external_ancestry.ambiguous_classes.contains(cid)
                        && !m.classes.iter().any(|class| class.id == **cid)
                        && !m.table_functions.contains_key(cid)
                        && !m.method_tables.iter().any(|(id, _, _)| id == *cid)
                })
            })
            .map(|(cid, pairs)| (*cid, pairs.clone()))
            .collect();
        Self { exports, tables }
    }
}

#[cfg(test)]
mod tests;
