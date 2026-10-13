//! Reload proportional to the edit (J04): the new generation compiles only the
//! functions that changed.
//!
//! The emitter writes the whole program on every reload, and parsing and linking
//! the whole IR is the bulk of its cost in a large project (200 classes: 12 MB of
//! IR, 0.55 s of parsing and 2.0 s of linking per edit of one body). But every
//! call between program functions goes through the stable entry
//! (`version_definitions` replaces the uses with the trampoline), so a function
//! whose code is the same as the published implementation's may stay in it: the
//! new generation **declares** it, and its callers land in the trampoline, which
//! points to the live implementation.
//!
//! "The same code" is textual and conservative. A function's fingerprint is its
//! text plus the text of everything it reaches that belongs to the generation
//! itself: local functions (`internal`/`private`, without a trampoline) and module
//! globals, transitively. A reached program function enters only by name (the
//! call goes to the trampoline, which publishes the current version). The rest of
//! the context that the text does not carry is stable by construction: class ids
//! (J03), indices of the globals area (it only grows), field offsets (in the text)
//! and the runtime.
//!
//! Not kept (compiled again):
//! * a function that reaches a **mutable** module global (the cache of a call
//!   site by selector): the old generation stores addresses of old
//!   implementations in it;
//! * a function that reaches a program function whose signature in the new IR is
//!   not that of the published entry (J03: the name becomes another entry);
//! * the generation's registration functions (area, tables, RTI), which
//!   publication runs through the new generation;
//! * one that has no published implementation with the same fingerprint.
//!
//! The output IR has the kept ones as `declare`, without the local functions and
//! globals that only they reached; anything outside the shape the emitter writes
//! makes the IR go through whole, without a delta.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// What is known about the published implementation of a stable entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Published {
    /// Fingerprint of the code (text and local closure).
    pub fingerprint: u64,
    /// The signature as the IR writes it, without parameter names.
    pub fn_signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    Function { local: bool },
    Global { mutable: bool },
    Comdat,
}

#[derive(Debug)]
struct Item<'a> {
    name: String,
    tipo: Type,
    text: &'a str,
}

/// The module read by its top-level lines: function definitions, globals and
/// comdats; the rest (header, comments, `declare`) passes as it is.
struct IrModule<'a> {
    /// Chunks in text order: `Ok(item)` or `Err(text that passes through)`.
    fragments: Vec<Result<usize, &'a str>>,
    items: Vec<Item<'a>>,
    by_name: HashMap<String, usize>,
}

/// The name after the `@` (or the comdat `$`) at the start of `s`: quoted, or up
/// to the first character that is not part of an LLVM identifier.
fn name_at(s: &str) -> Option<(String, usize)> {
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some((rest[..end].to_string(), end + 2));
    }
    let end = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '-')))
        .map_or(s.len(), |(i, _)| i);
    (end > 0).then(|| (s[..end].to_string(), end))
}

/// The `@name` references of a text.
fn references(text: &str) -> impl Iterator<Item = String> + '_ {
    text.match_indices('@').filter_map(|(i, _)| name_at(&text[i + 1..]).map(|(n, _)| n))
}

fn do_read(ir: &str) -> Option<IrModule<'_>> {
    let mut m = IrModule { fragments: Vec::new(), items: Vec::new(), by_name: HashMap::new() };
    let mut pos = 0;
    let mut start_pass = 0;
    while pos < ir.len() {
        let line_end = ir[pos..].find('\n').map_or(ir.len(), |i| pos + i + 1);
        let line = &ir[pos..line_end];
        let item = if let Some(rest) = line.strip_prefix("define ") {
            // Up to the `}` line that closes the body.
            let end = ir[pos..].find("\n}\n").map(|i| pos + i + 3)?;
            let at = rest.find('@')?;
            let (name, _) = name_at(&rest[at + 1..])?;
            let before = &rest[..at];
            let local = before.split_whitespace().any(|p| p == "internal" || p == "private");
            Some((name, Type::Function { local }, end))
        } else if let Some(rest) = line.strip_prefix('@') {
            let (name, n) = name_at(rest)?;
            let after = rest[n..].strip_prefix(" = ")?;
            // One global per line; the `declare` of external data is `external global`.
            if after.starts_with("external ") {
                None
            } else {
                let mutable = after.split_whitespace().find(|p| *p == "constant" || *p == "global") == Some("global");
                Some((name, Type::Global { mutable }, line_end))
            }
        } else if let Some(rest) = line.strip_prefix('$') {
            let (name, n) = name_at(rest)?;
            rest[n..].starts_with(" = comdat ").then_some((name, Type::Comdat, line_end))
        } else {
            None
        };
        match item {
            Some((name, tipo, end)) => {
                if start_pass < pos {
                    m.fragments.push(Err(&ir[start_pass..pos]));
                }
                let key = match tipo {
                    Type::Comdat => format!("${name}"),
                    _ => name.clone(),
                };
                if m.by_name.insert(key, m.items.len()).is_some() {
                    return None;
                }
                m.fragments.push(Ok(m.items.len()));
                m.items.push(Item { name, tipo, text: &ir[pos..end] });
                pos = end;
                start_pass = pos;
            }
            None => pos = line_end,
        }
    }
    if start_pass < ir.len() {
        m.fragments.push(Err(&ir[start_pass..]));
    }
    Some(m)
}

/// The signature of a definition: the `define` line without linkage, without
/// parameter names, without `comdat` and without the `{`. `declare <this>` is its
/// declaration.
fn fn_signature(text: &str) -> Option<String> {
    let line = text.lines().next()?.strip_prefix("define ")?;
    let line = line.trim_end().strip_suffix('{')?.trim_end();
    let opens = line.find('(')?;
    // The `)` that closes the parameter list.
    let mut level = 0i32;
    let mut closes = None;
    for (i, c) in line[opens..].char_indices() {
        match c {
            '(' | '{' | '[' | '<' => level += 1,
            ')' | '}' | ']' | '>' => {
                level -= 1;
                if level == 0 {
                    closes = Some(opens + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let closes = closes?;
    let head: Vec<&str> = line[..opens]
        .split_whitespace()
        .filter(|p| !matches!(*p, "linkonce_odr" | "internal" | "private" | "dso_local" | "weak_odr" | "linkonce"))
        .collect();
    // `comdat` and `comdat($name)`: the declaration does not belong to a comdat.
    let after = line[closes + 1..]
        .split_whitespace()
        .filter(|p| *p != "comdat" && !p.starts_with("comdat("))
        .collect::<Vec<_>>()
        .join(" ");
    // Parameters without each one's name (`%x`), at the outer level.
    let mut params = Vec::new();
    let mut current = String::new();
    let mut level = 0i32;
    for c in line[opens + 1..closes].chars() {
        match c {
            '(' | '{' | '[' | '<' => level += 1,
            ')' | '}' | ']' | '>' => level -= 1,
            ',' if level == 0 => {
                params.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        params.push(current);
    }
    let params: Vec<String> = params
        .iter()
        .map(|p| p.split_whitespace().filter(|t| !t.starts_with('%')).collect::<Vec<_>>().join(" "))
        .collect();
    let mut s = format!("{}({})", head.join(" "), params.join(", "));
    if !after.is_empty() {
        s.push(' ');
        s.push_str(&after);
    }
    Some(s)
}

/// The infrastructure of each generation's globals area: the descriptor
/// (`@df.area`) and the module id (`@df.area_id`), read by the local
/// `df.obter_area`. They neither enter the fingerprint nor prevent keeping the
/// function: the area layout only grows between generations, and the runtime
/// accepts the descriptor of a previous generation as a prefix of the current
/// layout, returning the same area (`dartforge_area_de_globais`).
const INFRASTRUCTURE: [&str; 2] = ["df.area", "df.area_id"];

/// The local closure of `i`: the generation's items it reaches (local functions
/// and globals, transitively), the program functions reached (by name) and
/// whether some mutable global is on the way.
fn closure_set(m: &IrModule, i: usize) -> (BTreeSet<usize>, BTreeSet<String>, bool) {
    let mut locals = BTreeSet::new();
    let mut program = BTreeSet::new();
    let mut mutable = false;
    let mut stack = vec![i];
    let mut seen_set = HashSet::from([i]);
    while let Some(k) = stack.pop() {
        for r in references(m.items[k].text) {
            if INFRASTRUCTURE.contains(&r.as_str()) {
                continue;
            }
            let Some(&j) = m.by_name.get(&r) else { continue };
            match m.items[j].tipo {
                Type::Function { local: false } => {
                    if j != i {
                        program.insert(r);
                    }
                }
                Type::Global { mutable: true } => mutable = true,
                Type::Comdat => {}
                _ => {
                    if seen_set.insert(j) {
                        locals.insert(j);
                        stack.push(j);
                    }
                }
            }
        }
    }
    (locals, program, mutable)
}

fn fingerprint(m: &IrModule, i: usize, locals: &BTreeSet<usize>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    m.items[i].text.hash(&mut h);
    let mut names: Vec<(&str, &str)> = locals.iter().map(|&j| (m.items[j].name.as_str(), m.items[j].text)).collect();
    names.sort();
    names.hash(&mut h);
    h.finish()
}

/// The IR of the new generation with only what changed.
pub(crate) struct Delta {
    /// The IR to compile.
    pub ir: String,
    /// The functions that stay in the published implementation (declared in the IR).
    pub kept_fns: Vec<String>,
    /// What the generation publishes: the program functions defined in the output
    /// IR, with each one's fingerprint.
    pub new_items: HashMap<String, Published>,
}

/// The delta of `ir` against the implementations `live_list`. `excluded`:
/// functions that the new generation always defines (the registrations). `None`
/// when the IR is not in the expected shape — then it goes through whole.
pub(crate) fn delta(ir: &str, live_list: &HashMap<String, Published>, excluded: &[&str]) -> Option<Delta> {
    let m = do_read(ir)?;
    let mut new_items = HashMap::new();
    let mut kept_fns: HashSet<usize> = HashSet::new();
    let mut declarations: HashMap<usize, String> = HashMap::new();
    let fn_signatures: HashMap<&str, String> = m
        .items
        .iter()
        .filter(|it| it.tipo == Type::Function { local: false })
        .filter_map(|it| Some((it.name.as_str(), fn_signature(it.text)?)))
        .collect();
    for (i, item) in m.items.iter().enumerate() {
        if item.tipo != (Type::Function { local: false }) {
            continue;
        }
        let fn_signature = fn_signatures.get(item.name.as_str())?.clone();
        let (locals, program, mutable) = closure_set(&m, i);
        let published = Published { fingerprint: fingerprint(&m, i, &locals), fn_signature };
        let keep = !mutable
            && !excluded.contains(&item.name.as_str())
            && live_list.get(&item.name) == Some(&published)
            && program.iter().all(|r| match (fn_signatures.get(r.as_str()), live_list.get(r)) {
                (Some(new_area), Some(live_one)) => *new_area == live_one.fn_signature,
                // A program function the new IR does not define: the reference
                // is the same the live one already made.
                (None, _) => true,
                (Some(_), None) => false,
            });
        if keep {
            kept_fns.insert(i);
            declarations.insert(i, format!("declare {}\n", published.fn_signature));
        } else {
            new_items.insert(item.name.clone(), published);
        }
    }
    if kept_fns.is_empty() {
        return Some(Delta { ir: ir.to_string(), kept_fns: Vec::new(), new_items });
    }
    // What is still reached: from the program functions that stay defined and
    // from the globals that are not local (the local ones only exist through
    // their users).
    let mut live_set: HashSet<usize> = HashSet::new();
    let mut stack: Vec<usize> = m
        .items
        .iter()
        .enumerate()
        .filter(|(i, it)| match it.tipo {
            Type::Function { local: false } => !kept_fns.contains(i),
            Type::Global { .. } => !it.text.split_whitespace().nth(2).is_some_and(|p| p == "private" || p == "internal"),
            _ => false,
        })
        .map(|(i, _)| i)
        .collect();
    live_set.extend(stack.iter().copied());
    while let Some(k) = stack.pop() {
        for r in references(m.items[k].text) {
            let Some(&j) = m.by_name.get(&r) else { continue };
            if !kept_fns.contains(&j) && live_set.insert(j) {
                stack.push(j);
            }
        }
    }
    // Comdat with only the defined member.
    for (i, it) in m.items.iter().enumerate() {
        if it.tipo == Type::Comdat
            && m.by_name.get(&it.name).is_some_and(|&f| live_set.contains(&f))
        {
            live_set.insert(i);
        }
    }
    let mut out = String::with_capacity(ir.len() / 2);
    for t in &m.fragments {
        match t {
            Err(text) => out.push_str(text),
            Ok(i) if kept_fns.contains(i) => out.push_str(&declarations[i]),
            Ok(i) if live_set.contains(i) => out.push_str(m.items[*i].text),
            Ok(i) => {
                // A local function reached only by the kept ones: it disappears;
                // their generation keeps its own copy.
                let _ = i;
            }
        }
    }
    let mut names: Vec<String> = kept_fns.iter().map(|&i| m.items[i].name.clone()).collect();
    names.sort();
    Some(Delta { ir: out, kept_fns: names, new_items })
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    /// What the first generation of `ir` publishes.
    fn published_list(ir: &str) -> HashMap<String, Published> {
        delta(ir, &HashMap::new(), &[]).expect("known shape").new_items
    }

    const V1: &str = r#"; cabeçalho
$"g" = comdat any
@.str.0 = private unnamed_addr constant [2 x i8] c"ab"
@cache = internal global [2 x i64] zeroinitializer
declare i64 @dartforge_print(i64)

define i64 @f(i64 %v0, ptr %v1) {
b0:
  %r = call i64 @ajuda(i64 %v0)
  %s = call i64 @g(i64 %r)
  ret i64 %s
}

define internal i64 @ajuda(i64 %x) {
b0:
  %p = call i64 @dartforge_print(i64 ptrtoint (ptr @.str.0 to i64))
  ret i64 %x
}

define linkonce_odr i64 @"g"(i64 %v0) comdat {
b0:
  ret i64 %v0
}

define i64 @h(i64 %v0) {
b0:
  %c = load i64, ptr @cache
  ret i64 %c
}
"#;

    #[test]
    fn signature_without_names_or_linkage() {
        assert_eq!(fn_signature("define linkonce_odr i64 @\"g\"(i64 %v0) comdat {\n}").unwrap(), "i64 @\"g\"(i64)");
        assert_eq!(
            fn_signature("define linkonce_odr i64 @g$t(i64 %v0, ptr %v1) comdat($\"g$t\") {\n}").unwrap(),
            "i64 @g$t(i64, ptr)"
        );
        assert_eq!(
            fn_signature("define internal signext i8 @k({ i64, ptr } %a, ptr noalias %b) {\n}").unwrap(),
            "signext i8 @k({ i64, ptr }, ptr noalias)"
        );
    }

    #[test]
    fn keeps_unchanged_and_compiles_rest() {
        let live_list = published_list(V1);
        assert_eq!(live_list.len(), 3, "{live_list:?}");
        // `g` changed: only it is compiled; `f` (which calls `g` through the
        // trampoline) stays in the live implementation; `h` uses a mutable global
        // and is never kept.
        let v2 = V1.replace("  ret i64 %v0\n}\n\ndefine i64 @h", "  %y = add i64 %v0, 1\n  ret i64 %y\n}\n\ndefine i64 @h");
        let d = delta(&v2, &live_list, &[]).expect("known shape");
        assert_eq!(d.kept_fns, vec!["f".to_string()]);
        assert!(d.ir.contains("declare i64 @f(i64, ptr)\n"), "{}", d.ir);
        // The local reached only by `f` disappears; `g`'s comdat stays.
        assert!(!d.ir.contains("@ajuda("), "{}", d.ir);
        assert!(d.ir.contains("$\"g\" = comdat any"));
        assert!(d.ir.contains("%y = add i64 %v0, 1"));
        assert!(d.ir.contains("define i64 @h("));
        assert_eq!(d.new_items.keys().collect::<BTreeSet<_>>(), BTreeSet::from([&"g".to_string(), &"h".to_string()]));
    }

    #[test]
    fn changed_local_recompiles_users() {
        let live_list = published_list(V1);
        let v2 = V1.replace("c\"ab\"", "c\"xy\"");
        let d = delta(&v2, &live_list, &[]).unwrap();
        // The string changes the closure of `ajuda` and therefore that of `f`.
        assert!(!d.kept_fns.contains(&"f".to_string()), "{:?}", d.kept_fns);
        assert!(d.kept_fns.contains(&"g".to_string()));
    }

    #[test]
    fn changed_callee_signature_recompiles_caller() {
        let live_list = published_list(V1);
        let v2 = V1
            .replace("define linkonce_odr i64 @\"g\"(i64 %v0)", "define linkonce_odr i64 @\"g\"(i64 %v0, i64 %v1)")
            .replace("call i64 @g(i64 %r)", "call i64 @g(i64 %r)");
        let d = delta(&v2, &live_list, &[]).unwrap();
        assert!(d.kept_fns.is_empty(), "{:?}", d.kept_fns);
    }

    #[test]
    fn excluded_and_non_live_are_compiled() {
        let live_list = published_list(V1);
        let d = delta(V1, &live_list, &["f"]).unwrap();
        assert_eq!(d.kept_fns, vec!["g".to_string()]);
        let d = delta(V1, &HashMap::new(), &[]).unwrap();
        assert!(d.kept_fns.is_empty());
        assert_eq!(d.ir, V1);
    }
}
