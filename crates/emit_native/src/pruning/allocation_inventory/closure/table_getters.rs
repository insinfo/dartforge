//! Allocation-only witnesses for generated immutable method-table getters.
//! Returning a method address still exposes the registry; it is never purity.
use std::collections::{HashMap, HashSet};

fn symbol(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_$.-".contains(&byte))
}

// Quoted LLVM names may spell the same symbol with hex escapes. Count those
// definitions too; a differently printed alias must not evade uniqueness.
fn defined_name(line: &str) -> Option<String> {
    canonical_symbol(crate::pruning::cited(line).next()?)
}

/// Normalize an LLVM symbol already stripped of its optional quotes.
pub(in crate::pruning) fn canonical_symbol(raw: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        if raw.as_bytes()[index] == b'\\' {
            let value = raw.get(index + 1..index + 3)?;
            bytes.push(u8::from_str_radix(value, 16).ok()?);
            index += 3;
        } else {
            bytes.push(raw.as_bytes()[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

fn table(line: &str, name: &str) -> Option<u32> {
    let value = line.strip_prefix(&format!("@{name}$d = private unnamed_addr constant {{ "))?;
    let (types, values) = value.split_once(" } { ")?;
    let values: Vec<_> = values.strip_suffix(" }")?.split(", ").collect();
    let types: Vec<_> = types.split(", ").collect();
    if values.len() < 2 || values.len() != types.len() || types[..2] != ["i64", "i64"] {
        return None;
    }
    let cid = values[0].strip_prefix("i64 ")?.parse::<u32>().ok()?;
    let count = values[1].strip_prefix("i64 ")?.parse::<usize>().ok()?;
    if cid > i32::MAX as u32 || count.checked_mul(2)?.checked_add(2)? != values.len() {
        return None;
    }
    let mut previous = None;
    for (types, values) in types[2..]
        .as_chunks::<2>()
        .0
        .iter()
        .zip(values[2..].as_chunks::<2>().0)
    {
        let hash = values[0].strip_prefix("i64 ")?.parse::<i64>().ok()?;
        if *types != ["i64", "ptr"]
            || !symbol(values[1].strip_prefix("ptr @")?)
            || previous.is_some_and(|previous| previous >= hash)
        {
            return None;
        }
        previous = Some(hash);
    }
    Some(cid)
}

/// Accept only the emitter's exact constant-return body and complete table.
/// Unique public definitions and private data prevent cross-module replacement.
pub(in crate::pruning) fn observe(modules: &[(&str, &str)]) -> HashMap<String, u32> {
    // Production SDK tables are emitted later by the linker assembler. Avoid
    // building a source index when this snapshot has no getter definitions.
    if !modules
        .iter()
        .any(|(_, ir)| ir.contains("define ptr @df.mt."))
    {
        return HashMap::new();
    }
    let mut counts = HashMap::new();
    let mut globals = HashSet::new();
    for (_, ir) in modules {
        for line in ir.lines() {
            if line.starts_with("define ") {
                if let Some(name) = defined_name(line) {
                    *counts.entry(name).or_insert(0usize) += 1;
                }
            } else if line.starts_with('@')
                && let Some(name) = defined_name(line)
            {
                globals.insert(name);
            }
        }
    }
    let mut accepted = HashMap::new();
    for (_, ir) in modules {
        let lines: Vec<_> = ir.lines().collect();
        let mut data = HashMap::<String, Vec<&str>>::new();
        for line in &lines {
            if line.starts_with('@')
                && let Some(name) = defined_name(line)
            {
                data.entry(name).or_default().push(line);
            }
        }
        for body in lines.windows(4) {
            let Some(name) = body[0].strip_prefix("define ptr @").and_then(|value| {
                value
                    .strip_suffix("() nounwind {")
                    .or_else(|| value.strip_suffix("() {"))
            }) else {
                continue;
            };
            if !name.starts_with("df.mt.")
                || !symbol(name)
                || counts.get(name) != Some(&1)
                || globals.contains(name)
                || body[1] != "b0:"
                || body[2] != format!("  ret ptr @{name}$d")
                || body[3] != "}"
            {
                continue;
            }
            let data_name = format!("{name}$d");
            if counts.contains_key(&data_name) {
                continue;
            }
            if let Some(lines) = data.get(data_name.as_str())
                && let [line] = lines.as_slice()
                && let Some(cid) = table(line, name)
            {
                accepted.insert(name.to_string(), cid);
            }
        }
    }
    accepted
}

/// The callback may only read its certified table. Registration still exposes
/// method addresses; the certificate says nothing about their later invocation.
pub(super) fn allocation(call: &super::Call, getters: &HashMap<String, u32>) -> Option<u32> {
    if getters.is_empty() {
        return None;
    }
    let parameters = match call.symbol.as_str() {
        "dartforge_object_new_t" => vec!["i64", "i64", "ptr"],
        "dartforge_object_new_mt" => vec!["i64", "i64", "i64", "ptr"],
        _ => return None,
    };
    if call.abi.result != "i64"
        || call.abi.parameters != parameters
        || call.arguments.len() != parameters.len()
    {
        return None;
    }
    let cid = call.arguments[0]
        .strip_prefix("i64 ")?
        .parse::<u32>()
        .ok()?;
    let fields = call.arguments[1]
        .strip_prefix("i64 ")?
        .parse::<i64>()
        .ok()?;
    if fields < 0 {
        return None;
    }
    if parameters.len() == 4 {
        call.arguments[2]
            .strip_prefix("i64 ")?
            .parse::<u32>()
            .ok()?;
    }
    let getter = call.arguments.last()?.strip_prefix("ptr @")?;
    (getters.get(getter) == Some(&cid)).then_some(cid)
}
