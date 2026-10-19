//! Final-LLVM allocation obligations and conservative SCC closure.
//!
//! These facts are separate from the legacy direct leaf records in `.poda`.
//! Allocation closure is not an escape, registry, reflection or native-exposure
//! certificate, and publishing this inventory does not enable import pruning.
use std::collections::{BTreeSet, HashMap};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Abi { result: String, parameters: Vec<String> }
#[derive(Clone, Debug)]
struct Call { symbol: String, abi: Abi, arguments: Vec<String> }
#[derive(Clone, Debug)]
struct Definition {
    module: String, symbol: String, key: String, body_digest: String,
    abi: Option<Abi>, local: bool, unique_linkage: bool, complete: bool,
    direct: BTreeSet<u32>, calls: Vec<Call>, unknown: BTreeSet<String>,
    exposures: BTreeSet<String>,
}
#[derive(Debug)]
pub(crate) struct Bundle {
    definitions: Vec<Definition>,
    modules: Vec<Value>,
    globals: Vec<Value>,
    closures: Vec<Option<BTreeSet<u32>>>,
    dependencies: Vec<Vec<usize>>,
}

fn primitive(token: &str) -> bool {
    matches!(token, "void" | "i1" | "i8" | "i16" | "i32" | "i64" | "float" | "double" | "ptr")
}
/// Skip a quoted LLVM identifier/string, accepting only LLVM hex escapes.
fn quoted_end(text: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < text.len() {
        match text[index] {
            b'"' => return Some(index + 1),
            b'\\' => {
                if !text
                    .get(index + 1..index + 3)?
                    .iter()
                    .all(u8::is_ascii_hexdigit)
                {
                    return None;
                }
                index += 3;
            }
            _ => index += 1,
        }
    }
    None
}

/// Split an outer parameter list without inspecting delimiters inside quotes.
/// Mismatched aggregates, empty arguments and malformed escapes stay unproved.
fn arguments(text: &str) -> Option<(Vec<String>, usize)> {
    let bytes = text.as_bytes();
    let mut stack = Vec::new();
    let mut output = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = quoted_end(bytes, index)?;
                continue;
            }
            b'(' => stack.push(b')'),
            b'[' => stack.push(b']'),
            b'{' => stack.push(b'}'),
            b'<' => stack.push(b'>'),
            b')' if stack.is_empty() => {
                let value = text[start..index].trim();
                if value.is_empty() && !output.is_empty() {
                    return None;
                }
                if !value.is_empty() {
                    output.push(value.to_string());
                }
                return Some((output, index));
            }
            byte @ (b')' | b']' | b'}' | b'>') => {
                if stack.pop() != Some(byte) {
                    return None;
                }
            }
            b',' if stack.is_empty() => {
                let value = text[start..index].trim();
                if value.is_empty() {
                    return None;
                }
                output.push(value.to_string());
                start = index + 1;
            }
            b'\\' => return None,
            _ => {}
        }
        index += 1;
    }
    None
}

fn uncomment(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = quoted_end(bytes, index)?,
            b';' => return Some(&text[..index]),
            _ => index += 1,
        }
    }
    Some(text)
}

/// Locate body delimiters without interpreting quoted identifiers or attributes
/// as LLVM structure. The caller removes comments before scanning.
fn structural_braces(text: &str) -> Option<Vec<(usize, u8)>> {
    let bytes = text.as_bytes();
    let mut braces = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = quoted_end(bytes, index)?,
            brace @ (b'{' | b'}') => {
                braces.push((index, brace));
                index += 1;
            }
            _ => index += 1,
        }
    }
    Some(braces)
}

fn signature(text: &str) -> Option<(String, Abi, Vec<String>)> {
    let (prefix, after) = text.split_once('@')?;
    let result = prefix.split_whitespace().last()?;
    if !primitive(result) { return None }
    // Calling conventions, return attributes and constant-expression callees
    // need their own ABI rules; this first inventory does not guess them.
    let prefix_words: Vec<_> = prefix.split_whitespace().collect();
    if prefix_words[..prefix_words.len().checked_sub(1)?].iter().any(|word| !matches!(*word,
        "define" | "declare" | "call" | "internal" | "private" | "dso_local" | "dso_preemptable"
        | "linkonce_odr" | "linkonce" | "weak" | "weak_odr" | "available_externally" | "hidden" | "protected")) { return None }
    let (symbol, tail) = after.split_once('(')?;
    if symbol.is_empty() || !symbol.bytes().all(|b| b.is_ascii_alphanumeric() || b"_$.-".contains(&b)) { return None }
    let (values, end) = arguments(tail)?;
    if prefix.starts_with("call ") && !tail[end + 1..].trim().is_empty() { return None }
    let parameters = values.iter().map(|arg| {
        let mut words = arg.split_whitespace();
        let ty = words.next()?;
        if let Some(value) = words.next() {
            let operand = value.starts_with('%') || value.starts_with('@')
                || value.as_bytes().first().is_some_and(|byte| byte.is_ascii_digit() || *byte == b'-')
                || matches!(value,"null"|"undef"|"poison"|"true"|"false"|"zeroinitializer"|"getelementptr"|"ptrtoint"|"inttoptr"|"bitcast"|"add"|"sub"|"and"|"or"|"xor"|"shl"|"lshr"|"ashr"|"select"|"icmp"|"blockaddress");
            if !operand { return None }
        }
        (primitive(ty) && ty != "void").then(|| ty.to_string())
    }).collect::<Option<Vec<_>>>()?;
    Some((symbol.into(), Abi { result: result.into(), parameters }, values))
}
/// Check observational operands against their published ABI, without granting
/// allocation or registry capabilities to the call.
pub(super) fn arguments_match(result: &str, parameters: &[String], values: &[String]) -> bool {
    let text = format!("call {result} @df.inventory.arguments({})", values.join(", "));
    signature(&text).is_some_and(|(_, abi, actual)|
        abi.result == result && abi.parameters == parameters && actual == values)
}

fn global_references(text: &str) -> Vec<String> {
    super::super::cited(text).map(str::to_owned).collect()
}
/// Decode the exact immutable byte array, never a name or comment hint.
/// Unsupported initializer attributes remain an open obligation.
pub(super) fn native_byte_content(line: &str) -> Option<Vec<u8>> {
    let line = uncomment(line)?;
    let (_, value) = line.split_once(" = ")?;
    let mut value = value.trim();
    loop {
        let (word, tail) = value.split_once(' ')?;
        if matches!(
            word,
            "private" | "internal" | "unnamed_addr" | "local_unnamed_addr" | "dso_local"
        ) {
            value = tail.trim_start();
        } else {
            break;
        }
    }
    let array = value.strip_prefix("constant [")?;
    let (length, literal) = array.split_once(" x i8] ")?;
    let length = length.parse::<usize>().ok()?;
    let quoted = literal.strip_prefix('c')?;
    if !quoted.starts_with('"') {
        return None;
    }
    let end = quoted_end(quoted.as_bytes(), 0)?;
    let tail = quoted[end..].trim();
    if !tail.is_empty() {
        let alignment = tail.strip_prefix(", align ")?.parse::<u64>().ok()?;
        if !alignment.is_power_of_two() {
            return None;
        }
    }
    let encoded = &quoted.as_bytes()[1..end - 1];
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < encoded.len() {
        if encoded[index] == b'\\' {
            let hex = std::str::from_utf8(encoded.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(encoded[index]);
            index += 1;
        }
    }
    (decoded.len() == length).then_some(decoded)
}
fn native_bytes(line: &str) -> bool {
    native_byte_content(line).is_some()
}

fn analyze_body(definition: &mut Definition, body: &[&str], byte_globals: &BTreeSet<String>) {
    for raw in body {
        let Some(line) = uncomment(raw) else {
            definition.unknown.insert("malformed_quoted_operand".into());
            continue;
        };
        let line = line.trim();
        if line.is_empty() || line.ends_with(':') { continue }
        let operation = line.split_once(" = ").map_or(line, |(_, value)| value.trim());
        let operation = operation.strip_prefix("tail ").or_else(|| operation.strip_prefix("musttail ")).or_else(|| operation.strip_prefix("notail ")).unwrap_or(operation);
        let opcode = operation.split_whitespace().next().unwrap_or_default();
        if operation.split_whitespace().skip(1).any(|token| matches!(token,"call"|"invoke"|"callbr")) {
            definition.unknown.insert("multiple_instructions_or_embedded_call".into());
        }
        if opcode == "call" {
            if let Some((symbol, abi, arguments)) = signature(operation) {
                for argument in &arguments {
                    for name in global_references(argument) {
                        if !byte_globals.contains(&name) {
                            definition.unknown.insert(format!("global_or_function_address_argument:{name}"));
                            definition.exposures.insert("global_or_function_address".into());
                        }
                    }
                }
                definition.calls.push(Call {symbol, abi, arguments});
            } else {
                definition.unknown.insert("indirect_or_unsupported_call_abi".into());
            }
            continue;
        }
        if !matches!(opcode,
            "ret" | "br" | "unreachable" | "add" | "sub" | "mul" | "udiv" | "sdiv" | "urem" | "srem"
            | "shl" | "lshr" | "ashr" | "and" | "or" | "xor" | "fadd" | "fsub" | "fmul" | "fdiv" | "frem" | "fneg"
            | "icmp" | "fcmp" | "select" | "phi" | "zext" | "sext" | "trunc" | "sitofp" | "uitofp" | "fptosi" | "fptoui"
            | "fpext" | "fptrunc" | "alloca" | "load" | "store" | "getelementptr" | "bitcast" | "ptrtoint" | "inttoptr" | "freeze") {
            definition.unknown.insert(format!("unsupported_opcode:{opcode}"));
        }
        if matches!(opcode, "load" | "store" | "getelementptr" | "ptrtoint" | "inttoptr" | "bitcast") {
            // Generated allocation fast paths update the heap through raw
            // pointers without calling an allocator. Until these operations
            // have a complete memory-region proof, they can create objects.
            definition.unknown.insert("unresolved_raw_memory_allocation".into());
            definition.exposures.insert("raw_memory_or_pointer_operation".into());
        }
        for symbol in global_references(line) {
            if !byte_globals.contains(&symbol) {
                definition.unknown.insert(format!("global_image_or_function_address:{symbol}"));
                definition.exposures.insert("global_or_function_address".into());
            }
        }
    }
}

/// Exhaustively scans each definition's emitted body. Unknown syntax is a
/// recorded obligation, never an implicit empty allocation set.
pub(crate) fn audit(modules: &[(&str, &str)]) -> Bundle {
    let mut definitions = Vec::new(); let mut globals = Vec::new();
    let mut module_counts = HashMap::new();
    for (module, _) in modules { *module_counts.entry(*module).or_insert(0usize) += 1; }
    for &(module, ir) in modules {
        let byte_globals: BTreeSet<_> = ir.lines().filter(|line| line.starts_with('@') && native_bytes(line))
            .filter_map(|line| global_references(line).into_iter().next()).collect();
        for line in ir.lines().filter(|line| line.starts_with('@') && line.contains(" = ")) {
            let name = global_references(line).into_iter().next().unwrap_or_default();
            let bytes = native_bytes(line);
            globals.push(json!({"module":module,"symbol":name,"kind":if bytes {"native_bytes"} else if super::images::static_string(line).is_some() {"static_string"} else {"unclassified_initializer"},
                "initializer": line,
                "initializer_digest":blake3::hash(line.as_bytes()).to_hex().to_string(),"references":if bytes {vec![]} else {global_references(line).into_iter().skip(1).collect::<Vec<_>>()}}));
        }
        let mut lines = ir.lines();
        while let Some(header) = lines.next() {
            if !header.starts_with("define ") { continue }
            let Some(symbol) = global_references(header).into_iter().next() else { continue };
            let prefix = header.split_once('@').map_or(header, |(prefix, _)| prefix);
            let local = prefix.split_whitespace().any(|part| matches!(part,"private"|"internal"));
            let unique_linkage = !prefix.split_whitespace().any(|part| part.starts_with("weak") || part.starts_with("linkonce") || part == "available_externally");
            let abi = signature(header).map(|(_, abi, _)| abi);
            let mut body = Vec::new(); let mut text = format!("{header}\n"); let mut complete = false;
            let structural_header = uncomment(header).unwrap_or_default();
            let braces = structural_braces(structural_header).unwrap_or_default();
            if let Some(&(start, b'{')) = braces.first()
                && let Some(&(end, b'}')) = braces.last()
                && start < end
            {
                body.push(&structural_header[start + 1..end]);
                complete = true;
            }
            if !complete {
                for line in lines.by_ref() {
                    text.push_str(line); text.push('\n');
                    if uncomment(line).is_some_and(|line| line.trim() == "}") { complete = true; break }
                    body.push(line);
                }
            }
            let key = if local { format!("{module}#{symbol}") } else { symbol.clone() };
            let mut definition = Definition { module:module.into(), symbol, key, body_digest:blake3::hash(text.as_bytes()).to_hex().to_string(),
                abi, local, unique_linkage, complete, direct:BTreeSet::new(), calls:Vec::new(), unknown:BTreeSet::new(), exposures:BTreeSet::new() };
            if module_counts[module] != 1 { definition.unknown.insert("duplicate_module_identity".into()); }
            if !complete { definition.unknown.insert("incomplete_definition".into()); }
            if definition.abi.is_none() { definition.unknown.insert("unsupported_definition_abi".into()); }
            if !unique_linkage { definition.unknown.insert("interposable_linkage".into()); }
            analyze_body(&mut definition, &body, &byte_globals);
            definitions.push(definition);
        }
    }
    let mut bundle = close(definitions, globals);
    bundle.modules = modules.iter().map(|(module, ir)| json!({"module":module,"final_ir_digest":blake3::hash(ir.as_bytes()).to_hex().to_string()})).collect();
    bundle
}

fn close(mut definitions: Vec<Definition>, globals: Vec<Value>) -> Bundle {
    let mut names: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, definition) in definitions.iter().enumerate() { names.entry(definition.key.clone()).or_default().push(index); }
    let canonical: HashMap<_, _> = crate::llvm::externs::EXTERNS.iter().filter_map(|external| signature(external.decl).map(|(symbol, abi, _)| (symbol, abi))).collect();
    let mut dependencies = vec![Vec::new(); definitions.len()];
    for index in 0..definitions.len() {
        if names[&definitions[index].key].len() != 1 { definitions[index].unknown.insert("ambiguous_definition".into()); }
        let calls = definitions[index].calls.clone();
        for call in calls {
            let local_key = format!("{}#{}", definitions[index].module, call.symbol);
            let target = names.get(&local_key).or_else(|| names.get(&call.symbol));
            if let Some(targets) = target {
                if targets.len() == 1 && definitions[targets[0]].abi.as_ref() == Some(&call.abi) {
                    dependencies[index].push(targets[0]);
                } else { definitions[index].unknown.insert(format!("unresolved_definition_or_abi:{}",call.symbol)); }
                continue;
            }
            // GC allocation-neutral does not mean registry/image-neutral. These
            // APIs can introduce managed image CIDs or replace helper bindings.
            if call.symbol.starts_with("dartforge_register_") || call.symbol == "dartforge_set_migration" {
                definitions[index].unknown.insert(format!("image_registry_or_helper_binding:{}",call.symbol));
                definitions[index].exposures.insert("image_registry_or_helper_binding".into());
                continue;
            }
            // The object allocator's finite footprint is an explicit ABI rule,
            // not an inference from the result type or a function name prefix.
            if call.symbol == "dartforge_object_new" && canonical.get(&call.symbol) == Some(&call.abi) {
                let literal = |arg: &String| arg.strip_prefix("i64 ").and_then(|value| value.parse::<i64>().ok());
                let values: Option<Vec<_>> = call.arguments.iter().map(literal).collect();
                if let Some(values) = values.filter(|values| values.len() == 2 && (0..=i64::from(i32::MAX)).contains(&values[0]) && values[1] >= 0) {
                    definitions[index].direct.insert(values[0] as u32);
                } else { definitions[index].unknown.insert("nonconstant_allocation_cid_or_size".into()); }
                continue;
            }
            let intrinsic = match call.symbol.as_str() { "llvm.expect.i1"=>Some("i1"), "llvm.expect.i64"=>Some("i64"), _=>None };
            if intrinsic.is_some_and(|ty| call.abi.result == ty && call.abi.parameters == vec![ty.to_string(),ty.to_string()]) { continue }
            let effects = crate::llvm::externs::effects_of(&call.symbol);
            if canonical.get(&call.symbol) == Some(&call.abi) && !effects.allocates && !effects.calls_dart { continue }
            definitions[index].unknown.insert(format!("runtime_native_selector_or_helper_obligation:{}",call.symbol));
            definitions[index].exposures.insert("unresolved_external_boundary".into());
        }
        dependencies[index].sort_unstable(); dependencies[index].dedup();
    }
    let mut closures: Vec<Option<BTreeSet<u32>>> = vec![None; definitions.len()];
    for component in crate::optimize::scc::components(&dependencies) {
        let members: BTreeSet<_> = component.iter().copied().collect();
        let mut known = true; let mut classes = BTreeSet::new();
        for &index in &component {
            known &= definitions[index].unknown.is_empty();
            classes.extend(definitions[index].direct.iter().copied());
            for &target in &dependencies[index] {
                if members.contains(&target) { continue }
                if let Some(callee) = &closures[target] { classes.extend(callee.iter().copied()); }
                else { known = false; }
            }
        }
        if known { for index in component { closures[index] = Some(classes.clone()); } }
    }
    Bundle {definitions, modules:Vec::new(), globals, closures, dependencies}
}

impl Bundle {
    pub(crate) fn to_json(&self) -> Value {
        let functions: Vec<_> = self.definitions.iter().enumerate().map(|(index, definition)| json!({
            "module":definition.module,"symbol":definition.symbol,"key":definition.key,
            "body_digest":definition.body_digest,"local":definition.local,"unique_linkage":definition.unique_linkage,"complete_body":definition.complete,
            "abi":definition.abi.as_ref().map(|abi| json!({"result":abi.result,"parameters":abi.parameters})),
            "direct_classes":definition.direct,"unknown_obligations":definition.unknown,"exposure_obligations":definition.exposures,
            "calls":definition.calls.iter().map(|call| json!({"symbol":call.symbol,"abi":{"result":call.abi.result,"parameters":call.abi.parameters},"arguments":call.arguments})).collect::<Vec<_>>(),
            "dependencies":self.dependencies[index].iter().map(|&target| self.definitions[target].key.clone()).collect::<Vec<_>>(),
            "transitive_classes":self.closures[index],
        })).collect();
        json!({"modules":self.modules,"functions":functions,"globals":self.globals,"scope":"allocation closure only; import pruning and native/registry exposure remain unverified"})
    }
}

#[cfg(test)]
mod tests;
