//! Pruning of the method tables at the link of the production AOT
//! (docs/NATIVO-PODA-DE-TABELAS.md).
//!
//! In the production profile the source SDK does not define the method tables of
//! its classes: each library writes, next to the cached bitcode, a
//! **summary** ([`summarize`]) with what each function and each global cites, the
//! selectors each function calls and the content of the tables. At the link of the
//! program, [`assemble`] reads the IR of the program with the same rules, computes a
//! fixpoint per selector (Bacon's RTA: an entry of the table of a
//! live class only comes in if its selector is called by some live function) and
//! returns the IR of the program with every table defined, with only the live
//! pairs. LTO and `/OPT:REF` remove the rest.
//!
//! The JIT, reload and the development DLL do not go through here: there the
//! tables stay whole, in the SDK modules (§3.8 of the specification).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub(crate) mod receiver_provenance;
pub(crate) mod arc_ports;

/// First line of a summary; the number changes with the format.
const HEADER: &str = "dartforge-poda\t6";

/// Selectors that live in every live class, without a call in the IR (§3.6):
/// `c:call` the runtime looks up by text (`dartforge_closure_entry`, the
/// callable class and `Function.apply`); the others are the protocol of
/// `Object` that the runtime reaches through SDK helpers — cheap, because
/// almost every live class already has them live.
pub const ROOT_SELECTORS: &[&str] = &["c:call", "c:toString", "c:==", "g:hashCode", "c:noSuchMethod", "g:runtimeType"];

/// Runtime functions that look up in the table by a name that only exists
/// at run time (§3.7). Cited by a live function, every live table entry
/// stays. Empty: no current mechanism does that (the native
/// `dart:mirrors` does not invoke by name); the next one goes here.
pub const CALL_BY_NAME: &[&str] = &[];

/// A method table as the lowering builds it (`Module::method_tables`):
/// the cid, the function that returns the table and the `(selector, entry)` pairs.
pub type ModuleTable = (u32, String, Vec<(String, String)>);

/// The hash of a selector (64-bit FNV-1a, `lower::closures::hash_name`).
fn selector_hash(text: &str) -> i64 {
    crate::lower::closures::hash_name(text)
}

/// The `(hash, selector, entry)` pairs of a method table, sorted
/// by hash and without a repeated hash: the order of the runtime's binary search
/// (`dartforge_register_table`).
///
/// # Panics
/// Two different selectors with the same hash in the same class.
pub fn table_pairs(cid: u32, methods: &[(String, String)]) -> Vec<(i64, String, String)> {
    let mut pairs: Vec<(i64, String, String)> =
        methods.iter().map(|(s, f)| (selector_hash(s), s.clone(), f.clone())).collect();
    pairs.sort();
    for par in pairs.windows(2) {
        assert!(
            par[0].0 != par[1].0 || par[0].1 == par[1].1,
            "repeated selector hash in class {cid}: {} and {}",
            par[0].1,
            par[1].1
        );
    }
    pairs.dedup_by(|a, b| a.0 == b.0);
    pairs
}

/// The line of a method table: `prefix` (`@X$d = private unnamed_addr
/// constant `), the types and `{ cid, n, (hash, entry)… }`. `pairs` already comes in
/// table order, with the entries written as in the IR (`@f`).
///
/// ```
/// use dartforge_emit_native::pruning::table_line;
/// let l = table_line("@t$d = private unnamed_addr constant ", 7, &[(5, "@f".to_string())]);
/// assert_eq!(l, "@t$d = private unnamed_addr constant { i64, i64, i64, ptr } { i64 7, i64 1, i64 5, ptr @f }\n");
/// ```
pub fn table_line(prefix: &str, cid: i64, pairs: &[(i64, String)]) -> String {
    let mut types = String::from("i64, i64");
    let mut items = format!("i64 {cid}, i64 {}", pairs.len());
    for (h, f) in pairs {
        types.push_str(", i64, ptr");
        write!(items, ", i64 {h}, ptr {f}").unwrap();
    }
    format!("{prefix}{{ {types} }} {{ {items} }}\n")
}

/// A character of an unquoted LLVM identifier.
fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'$' | b'.' | b'_')
}

/// A name written with `@`: without quotes if it allows.
fn llvm_name(name: &str) -> String {
    let simple = !name.is_empty()
        && !name.as_bytes()[0].is_ascii_digit()
        && name.bytes().all(name_char);
    if simple { format!("@{name}") } else { format!("@\"{name}\"") }
}

/// The cited symbols (`@x` and `@"x"`) in `text`, without the `@` and without the quotes.
/// An `@` inside a string constant gives a name that nothing defines — it is
/// conservative, it never removes anything.
fn cited(text: &str) -> impl Iterator<Item = &str> {
    let b = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if b[i] != b'@' {
                i += 1;
                continue;
            }
            i += 1;
            if i < b.len() && b[i] == b'"' {
                let ini = i + 1;
                let end = text[ini..].find('"').map_or(b.len(), |k| ini + k);
                i = end + 1;
                return Some(&text[ini..end]);
            }
            let ini = i;
            while i < b.len() && name_char(b[i]) {
                i += 1;
            }
            if i > ini {
                return Some(&text[ini..i]);
            }
        }
        None
    })
}

/// The name defined by a `define … @f(` line, and whether it is local
/// (`internal`/`private`).
fn define_name(line: &str) -> Option<(&str, bool)> {
    let rest = line.strip_prefix("define ")?;
    let at_sign = rest.find('@')?;
    let local = rest[..at_sign].split_whitespace().any(|p| p == "internal" || p == "private");
    let name = cited(&rest[at_sign..]).next()?;
    Some((name, local))
}

/// The name of a global line `@g = …`, whether it is local, and the rest after
/// the `=`.
fn global_name(line: &str) -> Option<(&str, bool, &str)> {
    if !line.starts_with('@') {
        return None;
    }
    let name = cited(line).next()?;
    let equal = line.find(" = ")?;
    let rest = &line[equal + 3..];
    let local = rest.starts_with("private ") || rest.starts_with("internal ");
    Some((name, local, rest))
}

/// Decodes the `c"…"` of a byte constant (`\XX`).
fn constant_text(line: &str) -> Option<String> {
    let ini = line.find("c\"")? + 2;
    let end = ini + line[ini..].find('"')?;
    let s = &line[ini..end];
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && let Some(h) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            v.push(h);
            i += 3;
            continue;
        }
        v.push(b[i]);
        i += 1;
    }
    Some(String::from_utf8_lossy(&v).into_owned())
}

/// The hash and the index of the name of a call `@df.selector(ptr …, i64 …, i64
/// <hash>, ptr @df.seln.<k>, i64 …)`; `None` if the hash is not constant.
fn selector_call(line: &str) -> Option<(i64, &str)> {
    let i = line.find("@df.selector(")? + "@df.selector(".len();
    let mut args = line[i..].split(", ");
    args.next()?;
    args.next()?;
    let h: i64 = args.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let name = args.next()?.strip_prefix("ptr @")?;
    Some((h, name.trim_end_matches(')')))
}

/// The hash and the name (`@df.seln.<k>`) of the descriptor `@df.seld.<k> = …
/// { i64 <hash>, ptr @df.seln.<k>, i64 <len> }` (the compact dispatch,
/// `llvm/selectors.rs`).
fn selector_descriptor(rest: &str) -> Option<(i64, &str)> {
    let values = rest[rest.find("} {")? + 3..].trim_start();
    let mut it = values.split(", ");
    let h: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let name = it.next()?.trim().strip_prefix("ptr @")?;
    Some((h, name))
}

/// The descriptor of the call `@df.selector_d(ptr …, i64 …, ptr @df.seld.<k>)`.
fn compact_selector_call(line: &str) -> Option<&str> {
    let i = line.find("@df.selector_d(")? + "@df.selector_d(".len();
    let mut args = line[i..].split(", ");
    args.next()?;
    args.next()?;
    let d = args.next()?.strip_prefix("ptr @")?;
    Some(d.trim_end_matches(')'))
}

/// A method table read from a line `@X$d = … constant { … } { i64
/// cid, i64 n, i64 h, ptr @f, … }`: `(cid, [(hash, entry)])`.
fn line_table(rest: &str) -> Option<(i64, Vec<(i64, String)>)> {
    let values = &rest[rest.find("} {")? + 3..];
    let values = values.trim_end().strip_suffix('}')?.trim();
    let mut it = values.split(", ");
    let cid: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let n: usize = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
    let mut pairs = Vec::with_capacity(n);
    for _ in 0..n {
        let h: i64 = it.next()?.strip_prefix("i64 ")?.trim().parse().ok()?;
        let f = it.next()?.strip_prefix("ptr ")?.trim();
        pairs.push((h, cited(f).next()?.to_string()));
    }
    Some((cid, pairs))
}

/// A definition of the summary: the name (index), what it cites and the selectors
/// it calls.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Definition {
    pub name: u32,
    pub refs: Vec<u32>,
    pub calls: Vec<i64>,
}

/// A method table of the summary: the table node (`X$d`, index), the cid and the
/// `(hash, entry)` pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SummarizedTable {
    pub node: u32,
    pub cid: i64,
    pub pairs: Vec<(i64, u32)>,
}

/// The summary of a module (an SDK library or the program): §3.3.
///
/// Local names (`private`/`internal`) carry the `<module>#` prefix,
/// because they repeat in every module (`@df.seln.0`, `@df.class`).
/// A declaration access observed in one definition's original HIR.
/// Layout premises are metadata, never a proof of physical receivers or stored values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredFieldAccess {
    pub definition: u32,
    pub value_id: u32,
    pub write: bool,
    pub declaration: crate::hir::FieldDeclaration,
}

impl DeclaredFieldAccess {
    fn payload(&self) -> String {
        let field = &self.declaration;
        serde_json::json!({"uri": field.library_uri, "owner": field.owner, "field": field.name,
            "final": field.is_final, "late": field.is_late, "slot": field.layout_index,
            "storage": match field.storage_type {
                crate::hir::Type::Ref => "ref", crate::hir::Type::I64 => "i64",
                crate::hir::Type::I8 => "i8", crate::hir::Type::I1 => "i1",
                crate::hir::Type::F64 => "f64", _ => "unsupported",
            }}).to_string()
    }

    fn parse(definition: u32, value_id: u32, write: bool, payload: &str) -> Result<Self, String> {
        let bad = || "malformed pruning field declaration".to_string();
        let value: serde_json::Value = serde_json::from_str(payload).map_err(|_| bad())?;
        let object = value.as_object().filter(|object| object.len() == 7).ok_or_else(bad)?;
        let string = |key| object.get(key).and_then(|value| value.as_str())
            .filter(|value| !value.is_empty()).ok_or_else(bad);
        let storage_type = match string("storage")? {
            "ref" => crate::hir::Type::Ref, "i64" => crate::hir::Type::I64,
            "i8" => crate::hir::Type::I8, "i1" => crate::hir::Type::I1,
            "f64" => crate::hir::Type::F64, _ => return Err(bad()),
        };
        Ok(Self { definition, value_id, write, declaration: crate::hir::FieldDeclaration {
            library_uri: string("uri")?.to_string(), owner: string("owner")?.to_string(),
            name: string("field")?.to_string(),
            is_final: object.get("final").and_then(|value| value.as_bool()).ok_or_else(bad)?,
            is_late: object.get("late").and_then(|value| value.as_bool()).ok_or_else(bad)?,
            layout_index: object.get("slot").and_then(|value| value.as_u64())
                .and_then(|index| usize::try_from(index).ok()).ok_or_else(bad)?,
            storage_type,
        } })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub module: String,
    pub names: Vec<String>,
    pub definitions: Vec<Definition>,
    pub tables: Vec<SummarizedTable>,
    /// The texts of the selectors (for the report).
    pub selectors: Vec<(i64, String)>,
    /// The tables whose function belongs to this summary, but that the module does not define
    /// (the production SDK): the program defines them (`assemble`). Indices of the node
    /// of the function.
    pub externals: Vec<u32>,
    /// The RTI recipes with a function type (`F<…>`) and the node that has them (the
    /// function, or the constant of the text in the IR): where the native signatures
    /// that the program may ask of the runtime come from (§3.14).
    pub recipes: Vec<(u32, String)>,
    /// Calls on a declared SDK `this`: (calling definition, selector, receiver cone).
    /// Every other call remains in `Definition::calls` and is unrestricted.
    pub receiver_calls: Vec<(u32, i64, i64)>,
    /// Allocation-proved physical identities (caller, selector, one possible CID).
    /// Multiple entries for one call denote a union, never a subtype cone.
    pub exact_receiver_calls: Vec<(u32, i64, i64)>,
    /// Persistent declaration observations; absence does not imply a closed writer census.
    pub declared_field_accesses: Vec<DeclaredFieldAccess>,
    /// Physical slot declaration premises, independently of stored-value certificates.
    pub declared_field_layouts: HashMap<u32, Vec<Option<crate::hir::FieldDeclaration>>>,
    /// Nominal identities with complete emitted parent edges. Other identities
    /// cannot be excluded from a receiver cone.
    pub nominal_classes: Vec<i64>,
    /// CID aliases without a unique nominal declaration remain open.
    pub ambiguous_classes: Vec<i64>,
    /// Runtime subtype edges, including interfaces and mixins.
    pub parent_edges: Vec<(i64, i64)>,
    pub(crate) arc_ports: Vec<arc_ports::Inventory>,
    index: HashMap<String, u32>,
}

impl Summary {
    fn new(module: &str) -> Summary {
        Summary { module: module.to_string(), ..Default::default() }
    }

    fn id(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), i);
        i
    }

    /// The text of the summary (the format of §3.3).
    pub fn to_text(&self) -> String {
        let mut s = format!("{HEADER}\t{}\n", self.module);
        for n in &self.names {
            writeln!(s, "N\t{n}").unwrap();
        }
        let join = |v: &mut dyn Iterator<Item = String>| v.collect::<Vec<_>>().join(" ");
        for d in &self.definitions {
            writeln!(
                s,
                "D\t{}\t{}\t{}",
                d.name,
                join(&mut d.refs.iter().map(u32::to_string)),
                join(&mut d.calls.iter().map(i64::to_string))
            )
            .unwrap();
        }
        for t in &self.tables {
            writeln!(s, "T\t{}\t{}\t{}", t.node, t.cid, join(&mut t.pairs.iter().map(|(h, f)| format!("{h}:{f}")))).unwrap();
        }
        for e in &self.externals {
            writeln!(s, "X\t{e}").unwrap();
        }
        for (h, t) in &self.selectors {
            writeln!(s, "S\t{h}\t{t}").unwrap();
        }
        for (node, t) in &self.recipes {
            writeln!(s, "R\t{node}\t{t}").unwrap();
        }
        for (node, selector, cone) in &self.receiver_calls {
            writeln!(s, "C\t{node}\t{selector}\t{cone}").unwrap();
        }
        for (node, selector, cid) in &self.exact_receiver_calls {
            writeln!(s, "E\t{node}\t{selector}\t{cid}").unwrap();
        }
        for field in &self.declared_field_accesses {
            writeln!(s, "F\t{}\t{}:{}\t{}", field.definition, field.value_id,
                if field.write { "w" } else { "r" }, field.payload()).unwrap();
        }
        let mut layouts: Vec<_> = self.declared_field_layouts.iter().collect();
        layouts.sort_by_key(|(cid, _)| **cid);
        for (cid, slots) in layouts {
            let payload: Vec<_> = slots.iter().map(|field| field.as_ref().map_or(serde_json::Value::Null, |field|
                serde_json::from_str(&DeclaredFieldAccess { definition: 0, value_id: 0, write: false,
                    declaration: field.clone() }.payload()).expect("field payload is JSON"))).collect();
            writeln!(s, "L\t{cid}\t{}", serde_json::Value::Array(payload)).unwrap();
        }
        for cid in &self.nominal_classes {
            writeln!(s, "K\t{cid}").unwrap();
        }
        for cid in &self.ambiguous_classes {
            writeln!(s, "A\t{cid}").unwrap();
        }
        for (child, parent) in &self.parent_edges {
            writeln!(s, "P\t{child}\t{parent}").unwrap();
        }
        for inventory in &self.arc_ports {
            writeln!(s, "V\t{}", inventory.registration).unwrap();
            for port in &inventory.ports {
                writeln!(s, "B\t{}\t{}\t{}:{}:{}", inventory.registration, port.target, port.kind, port.contract, port.declaration).unwrap();
            }
        }
        s
    }

    /// Reads the text of [`Summary::to_text`].
    ///
    /// # Errors
    /// A header of another version or a malformed line.
    pub fn from_string(text: &str) -> Result<Summary, String> {
        let mut lines = text.lines();
        let first = lines.next().unwrap_or_default();
        let module = first
            .strip_prefix(HEADER)
            .and_then(|r| r.strip_prefix('\t'))
            .ok_or_else(|| format!("pruning summary with an unknown header: {first:?}"))?;
        let mut r = Summary::new(module);
        let bad = |l: &str| format!("malformed pruning summary: {l:?}");
        let numbers = |c: Option<&str>| -> Result<Vec<u32>, String> {
            c.unwrap_or_default().split(' ').filter(|x| !x.is_empty()).map(|x| x.parse().map_err(|_| bad(x))).collect()
        };
        for l in lines {
            let mut c = l.splitn(4, '\t');
            match c.next() {
                Some("N") => {
                    let n = c.next().ok_or_else(|| bad(l))?;
                    r.index.insert(n.to_string(), r.names.len() as u32);
                    r.names.push(n.to_string());
                }
                Some("D") => {
                    let name = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let refs = numbers(c.next())?;
                    let calls = c
                        .next()
                        .unwrap_or_default()
                        .split(' ')
                        .filter(|x| !x.is_empty())
                        .map(|x| x.parse().map_err(|_| bad(l)))
                        .collect::<Result<_, _>>()?;
                    r.definitions.push(Definition { name, refs, calls });
                }
                Some("T") => {
                    let node = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let cid = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let mut pairs = Vec::new();
                    for p in c.next().unwrap_or_default().split(' ').filter(|x| !x.is_empty()) {
                        let (h, f) = p.split_once(':').ok_or_else(|| bad(l))?;
                        pairs.push((h.parse().map_err(|_| bad(l))?, f.parse().map_err(|_| bad(l))?));
                    }
                    r.tables.push(SummarizedTable { node, cid, pairs });
                }
                Some("X") => r.externals.push(c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?),
                Some("S") => {
                    let h = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.selectors.push((h, c.next().unwrap_or_default().to_string()));
                }
                Some("R") => {
                    let node = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.recipes.push((node, c.next().unwrap_or_default().to_string()));
                }
                Some("C") => {
                    let node = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let selector = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let cone = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.receiver_calls.push((node, selector, cone));
                }
                Some("E") => {
                    let node = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let selector = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let cid = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.exact_receiver_calls.push((node, selector, cid));
                }
                Some("F") => {
                    let definition = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let (value_id, operation) = c.next().and_then(|value| value.split_once(':')).ok_or_else(|| bad(l))?;
                    let value_id = value_id.parse().map_err(|_| bad(l))?;
                    let write = match operation { "w" => true, "r" => false, _ => return Err(bad(l)) };
                    let payload = c.next().ok_or_else(|| bad(l))?;
                    if c.next().is_some() { return Err(bad(l)); }
                    r.declared_field_accesses.push(DeclaredFieldAccess::parse(definition, value_id, write, payload)?);
                }
                Some("L") => {
                    let cid: u32 = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let payload: serde_json::Value = serde_json::from_str(c.next().ok_or_else(|| bad(l))?)
                        .map_err(|_| bad(l))?;
                    if c.next().is_some() || cid < dartforge_runtime::layout::FIRST_FREE_CID as u32
                        || r.declared_field_layouts.contains_key(&cid) { return Err(bad(l)); }
                    let array = payload.as_array().ok_or_else(|| bad(l))?;
                    let slots = array.iter().enumerate().map(|(index, field)| {
                        if field.is_null() { return Ok(None); }
                        let field = DeclaredFieldAccess::parse(0, 0, false, &field.to_string())?.declaration;
                        if field.layout_index != index { return Err(bad(l)); }
                        Ok(Some(field))
                    }).collect::<Result<Vec<_>, String>>()?;
                    r.declared_field_layouts.insert(cid, slots);
                }
                Some("K") => r.nominal_classes.push(c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?),
                Some("A") => r.ambiguous_classes.push(c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?),
                Some("P") => {
                    let child = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let parent = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.parent_edges.push((child, parent));
                }
                Some("V") => {
                    let registration = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    r.arc_ports.push(arc_ports::Inventory { registration, ports: vec![] });
                }
                Some("B") => {
                    let registration: u32 = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let target = c.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let mut data = c.next().ok_or_else(|| bad(l))?.splitn(3, ':');
                    let kind = data.next().ok_or_else(|| bad(l))?.to_string();
                    let contract = data.next().and_then(|x| x.parse().ok()).ok_or_else(|| bad(l))?;
                    let declaration = data.next().ok_or_else(|| bad(l))?.to_string();
                    r.arc_ports.iter_mut().find(|i| i.registration == registration).ok_or_else(|| bad(l))?
                        .ports.push(arc_ports::Port { target, kind, contract, declaration });
                }
                _ if l.is_empty() => {}
                _ => return Err(bad(l)),
            }
        }
        let n = r.names.len() as u32;
        let outside = r.definitions.iter().any(|d| d.name >= n || d.refs.iter().any(|&x| x >= n))
            || r.tables.iter().any(|t| t.node >= n || t.pairs.iter().any(|&(_, f)| f >= n))
            || r.externals.iter().any(|&e| e >= n);
        let outside = outside || r.receiver_calls.iter().chain(&r.exact_receiver_calls).any(|&(node, _, _)| node >= n)
            || r.declared_field_accesses.iter().any(|field| field.definition >= n);
        if outside {
            return Err("pruning summary with an index outside the names table".to_string());
        }
        if r.nominal_classes.iter().chain(&r.ambiguous_classes).any(|&cid| u32::try_from(cid).is_err())
            || r.parent_edges.iter().any(|&(child, parent)| u32::try_from(child).is_err() || u32::try_from(parent).is_err())
            || r.receiver_calls.iter().chain(&r.exact_receiver_calls).any(|&(_, _, cone)| u32::try_from(cone).is_err())
        {
            return Err("pruning summary with an invalid nominal class identity".to_string());
        }
        let mut accesses = HashSet::new();
        if r.declared_field_accesses.iter().any(|field|
            !r.definitions.iter().any(|definition| definition.name == field.definition)
                || !accesses.insert((field.definition, field.value_id))) {
            return Err("pruning field access without a unique definition/SSA identity".to_string());
        }
        if r.declared_field_layouts.keys().any(|&cid| r.ambiguous_classes.contains(&i64::from(cid))) {
            return Err("pruning declared layout uses an ambiguous physical CID".to_string());
        }
        arc_ports::validate(&r)?;
        Ok(r)
    }
}

/// The summary of a module from its IR (§3.3). `module` is the prefix
/// of the local names; `externals` are the tables that the module cites, but does not
/// define (the production SDK, with `with_tables_at_link`): `(cid,
/// table function, methods)`, like `Module::method_tables`.
///
/// ```
/// use dartforge_emit_native::pruning::summarize;
/// let ir = "define i64 @f(i64 %x) {\n  %a = call i64 @g(i64 %x)\n  ret i64 %a\n}\n";
/// let r = summarize(ir, "m", &[]);
/// assert_eq!(r.names, vec!["f".to_string(), "g".to_string()]);
/// assert_eq!(r.definitions[0].refs, vec![1]);
/// ```
pub fn summarize(ir: &str, module: &str, externals: &[ModuleTable]) -> Summary {
    // Step 1: the local names and the texts of the selectors.
    let mut locals: HashSet<&str> = HashSet::new();
    let mut seln_strings: HashMap<&str, String> = HashMap::new();
    let mut descriptors: HashMap<&str, (i64, &str)> = HashMap::new();
    let mut recipes: Vec<(&str, String)> = Vec::new();
    let mut nominal_classes = Vec::new();
    let mut ambiguous_classes = Vec::new();
    let mut parent_edges = Vec::new();
    for l in ir.lines() {
        if let Some(cid) = l.strip_prefix("; df.cone-class ").and_then(|v| v.parse::<u32>().ok()).map(i64::from) {
            nominal_classes.push(cid);
        }
        if let Some(cid) = l.strip_prefix("; df.cone-ambiguous ").and_then(|v| v.parse::<u32>().ok()).map(i64::from) {
            ambiguous_classes.push(cid);
        }
        if let Some(edge) = l.strip_prefix("; df.cone-parent ") {
            let mut values = edge.split_whitespace();
            if let (Some(child), Some(parent), None) = (values.next(), values.next(), values.next())
                && let (Ok(child), Ok(parent)) = (child.parse::<u32>(), parent.parse::<u32>())
            {
                parent_edges.push((i64::from(child), i64::from(parent)));
            }
        }
        if let Some((n, true)) = define_name(l) {
            locals.insert(n);
        } else if let Some((n, local, rest)) = global_name(l) {
            if local {
                locals.insert(n);
            }
            if n.starts_with("df.seln.")
                && let Some(t) = constant_text(l)
            {
                seln_strings.insert(n, t);
            } else if n.starts_with("df.seld.")
                && let Some(d) = selector_descriptor(rest)
            {
                descriptors.insert(n, d);
            } else if (rest.contains("F\\3C") || rest.contains("F<"))
                && let Some(t) = constant_text(l)
                && t.contains("F<")
            {
                // `F<` in the IR: `F\3C` in the private strings
                // (`emit_string_constants`), `F<` in the static objects.
                recipes.push((n, t.trim_end_matches('\0').to_string()));
            }
        }
    }
    let mut r = Summary::new(module);
    nominal_classes.sort_unstable();
    nominal_classes.dedup();
    parent_edges.sort_unstable();
    parent_edges.dedup();
    r.nominal_classes = nominal_classes;
    ambiguous_classes.sort_unstable();
    ambiguous_classes.dedup();
    r.ambiguous_classes = ambiguous_classes;
    r.parent_edges = parent_edges;
    let key = |n: &str| if locals.contains(n) { format!("{module}#{n}") } else { n.to_string() };
    let mut seen_selectors: HashSet<i64> = HashSet::new();
    // Step 2: the definitions.
    let mut current: Option<Definition> = None;
    let mut receiver_cone: Option<i64> = None;
    let mut receiver_exact: Option<Vec<i64>> = None;
    for l in ir.lines() {
        if let Some(d) = current.as_mut() {
            if let Some(cone) = l.trim().strip_prefix("; df.selector-cone ") {
                receiver_cone = cone.parse::<u32>().ok().map(i64::from);
                receiver_exact = None;
                continue;
            }
            if let Some(classes) = l.trim().strip_prefix("; df.selector-exact ") {
                receiver_exact = classes.split(',').map(|cid| cid.parse::<u32>().ok().map(i64::from)).collect();
                receiver_cone = None;
                continue;
            }
            if l == "}" {
                receiver_cone = None;
                receiver_exact = None;
                r.definitions.push(current.take().expect("current definition"));
                continue;
            }
            let call = if l.contains("@df.selector(") {
                selector_call(l)
            } else if l.contains("@df.selector_d(") {
                compact_selector_call(l).and_then(|d| descriptors.get(d).copied())
            } else {
                None
            };
            let cone = receiver_cone.take();
            let exact = receiver_exact.take();
            if let Some((h, seln)) = call {
                if let Some(classes) = &exact {
                    r.exact_receiver_calls.extend(classes.iter().map(|&cid| (d.name, h, cid)));
                } else if let Some(cone) = cone {
                    r.receiver_calls.push((d.name, h, cone));
                } else {
                    d.calls.push(h);
                }
                if let Some(t) = seln_strings.get(seln) {
                    if seen_selectors.insert(h) {
                        r.selectors.push((h, t.clone()));
                    }
                    // The fallback of the typed selector (`dartforge_selector`).
                    if let Some(without_t) = t.strip_prefix('t') {
                        let h2 = selector_hash(without_t);
                        if let Some(classes) = &exact {
                            r.exact_receiver_calls.extend(classes.iter().map(|&cid| (d.name, h2, cid)));
                        } else if let Some(cone) = cone {
                            r.receiver_calls.push((d.name, h2, cone));
                        } else {
                            d.calls.push(h2);
                        }
                        if seen_selectors.insert(h2) {
                            r.selectors.push((h2, without_t.to_string()));
                        }
                    }
                }
            }
            if l.contains('@') {
                let refs: Vec<String> = cited(l).map(key).collect();
                for x in refs {
                    let i = r.id(&x);
                    d.refs.push(i);
                }
            }
            continue;
        }
        if let Some((n, _)) = define_name(l) {
            let k = key(n);
            let name = r.id(&k);
            let after = &l[l.find('(').unwrap_or(l.len())..];
            let refs: Vec<String> = cited(after).map(key).collect();
            let refs = refs.iter().map(|x| r.id(x)).collect();
            if l.trim_end().ends_with('}') {
                // A function on a single line.
                r.definitions.push(Definition { name, refs, calls: Vec::new() });
            } else {
                current = Some(Definition { name, refs, calls: Vec::new() });
            }
            continue;
        }
        if let Some((n, _, rest)) = global_name(l) {
            let k = key(n);
            let node = r.id(&k);
            if n.starts_with("df.mt.")
                && n.ends_with("$d")
                && let Some((cid, pairs)) = line_table(rest)
            {
                let pairs = pairs.iter().map(|(h, f)| (*h, r.id(&key(f)))).collect();
                r.tables.push(SummarizedTable { node, cid, pairs });
                continue;
            }
            let refs: Vec<String> = cited(rest).map(key).collect();
            let refs = refs.iter().map(|x| r.id(x)).collect();
            r.definitions.push(Definition { name: node, refs, calls: Vec::new() });
        }
    }
    for (cid, sym_name, methods) in externals {
        let function = r.id(sym_name);
        let node = r.id(&format!("{sym_name}$d"));
        let pairs: Vec<(i64, u32)> = table_pairs(*cid, methods)
            .into_iter()
            .map(|(h, s, f)| {
                if seen_selectors.insert(h) {
                    r.selectors.push((h, s));
                }
                (h, r.id(&f))
            })
            .collect();
        r.definitions.push(Definition { name: function, refs: vec![node], calls: Vec::new() });
        r.tables.push(SummarizedTable { node, cid: i64::from(*cid), pairs });
        r.externals.push(function);
    }
    for (n, t) in recipes {
        let node = r.id(&key(n));
        r.recipes.push((node, t));
    }
    arc_ports::read_ir(&mut r, ir);
    r
}

/// The end of the RTI recipe term that starts at `i` (letter, digits, `<…>`,
/// `?`; `lower/rti.rs`, `write_type`).
fn term_end(b: &[u8], mut i: usize) -> Option<usize> {
    if !b.get(i)?.is_ascii_uppercase() {
        return None;
    }
    i += 1;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if b.get(i) == Some(&b'<') {
        let mut n = 0usize;
        loop {
            match b.get(i)? {
                b'<' => n += 1,
                b'>' => {
                    n -= 1;
                    if n == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
    if b.get(i) == Some(&b'?') {
        i += 1;
    }
    Some(i)
}

/// The comma-separated terms starting at `i`, up to `end` (which stays at the
/// returned position).
fn terms_until(b: &[u8], mut i: usize, end: u8) -> Option<(Vec<(usize, usize)>, usize)> {
    let mut v = Vec::new();
    if b.get(i) == Some(&end) {
        return Some((v, i));
    }
    loop {
        let f = term_end(b, i)?;
        v.push((i, f));
        match *b.get(f)? {
            b',' => i = f + 1,
            c if c == end => return Some((v, f)),
            _ => return None,
        }
    }
}

/// The decimal number at `i` and the position after it.
fn number_in(b: &[u8], mut i: usize) -> Option<(i64, usize)> {
    let ini = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    std::str::from_utf8(&b[ini..i]).ok()?.parse().ok().map(|n| (n, i))
}

/// The letter (or `S<rti>.`) of a native type `C<id>` in the FFI key.
fn ffi_letter(b: &[u8], i: usize, letters: &HashMap<i64, char>, composites: &HashSet<i64>, s: &mut String) -> Option<i64> {
    if b.get(i) != Some(&b'C') {
        return None;
    }
    let (id, _) = number_in(b, i + 1)?;
    if composites.contains(&id) {
        s.push_str(&format!("S{id}."));
    } else {
        s.push(*letters.get(&id)?);
    }
    Some(id)
}

/// The FFI keys (`lower/ffi.rs::signature_key`, the same as the
/// runtime's, `crates/runtime/src/ffi.rs::signature_key`) of the
/// native signatures written in the RTI recipe `text`: each term
/// `F<0;R;n;P1,…,Pn;>` without optional or named parameters whose return and
/// parameters are native type classes (`letters`, the table the runtime
/// receives; `S<rti>.` for structs and unions by value, `composites`;
/// `VarArgs`, the letter `*`, opens the variadic types). Dart requires the
/// constant native type in `asFunction`, `lookupFunction`, `fromFunction` and
/// `NativeCallable` (the front end's FFI transformer rejects a
/// generic one), so every signature the program asks of the runtime is
/// written in a recipe of the call site (§3.14).
pub fn recipe_ffi_keys(text: &str, letters: &HashMap<i64, char>, composites: &HashSet<i64>, exit: &mut HashSet<String>) {
    let b = text.as_bytes();
    // The part of the key of the term `b[i..f]`.
    let part = |i: usize, f: usize, s: &mut String| -> Option<()> {
        let before = s.len();
        let id = ffi_letter(b, i, letters, composites, s)?;
        if !s[before..].ends_with('*') || composites.contains(&id) {
            return Some(());
        }
        // `VarArgs<(T1, T2…)>` or `VarArgs<T>`: the types after the mark.
        let (_, j) = number_in(b, i + 1)?;
        if j >= f || b[j] != b'<' {
            return Some(());
        }
        let types: Vec<(usize, usize)> = if b.get(j + 1) == Some(&b'R') && b.get(j + 2) == Some(&b'<') {
            let (v, end) = terms_until(b, j + 3, b';')?;
            if b.get(end + 1) != Some(&b'>') {
                return None;
            }
            v
        } else {
            vec![(j + 1, term_end(b, j + 1)?)]
        };
        for (a, _) in types {
            let before = s.len();
            ffi_letter(b, a, letters, composites, s)?;
            if s[before..].ends_with('*') {
                return None;
            }
        }
        Some(())
    };
    let mut i = 0;
    while let Some(d) = text[i..].find("F<") {
        let ini = i + d;
        i = ini + 2;
        let key = (|| -> Option<String> {
            let (generics, j) = number_in(b, ini + 2)?;
            if generics != 0 || b.get(j) != Some(&b';') {
                return None;
            }
            let r_end = term_end(b, j + 1)?;
            if b.get(r_end) != Some(&b';') {
                return None;
            }
            let (n, k) = number_in(b, r_end + 1)?;
            if b.get(k) != Some(&b';') {
                return None;
            }
            let (params, end) = terms_until(b, k + 1, b';')?;
            // No optional nor named parameters.
            if params.len() as i64 != n || b.get(end + 1) != Some(&b'>') {
                return None;
            }
            let mut s = String::new();
            part(j + 1, r_end, &mut s)?;
            s.push('_');
            for (a, z) in params {
                part(a, z, &mut s)?;
            }
            Some(s)
        })();
        if let Some(c) = key {
            exit.insert(c);
        }
    }
}


/// Why a node is live (the first time it came in).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Root,
    /// Cited by the node.
    Ref(u32),
    /// A table pair (node) through the selector.
    Table(u32, i64),
}

/// The result of the fixpoint over several summaries (§3.5).
pub struct Reach {
    pub names: Vec<String>,
    /// The cause of each live node.
    pub causes: HashMap<u32, Cause>,
    /// The called selectors and the first live function that called each one
    /// (`None` for the roots).
    pub callees: HashMap<i64, Option<u32>>,
    /// The accepted caller for each live table/selector pair. A scoped call
    /// cannot explain retention of a class outside its receiver cone.
    pub pair_callers: HashMap<(u32, i64), Option<u32>>,
    /// Some function of [`CALL_BY_NAME`] stayed live.
    pub all_pairs: bool,
    pub strings: HashMap<i64, String>,
}

impl Reach {
    pub fn live(&self, node: u32) -> bool {
        self.causes.contains_key(&node)
    }

    pub fn live_pair(&self, h: i64) -> bool {
        self.all_pairs || self.callees.contains_key(&h)
    }

    /// Whether a live table actually accepts this selector's receiver.
    /// A selector called for another nominal class does not keep this pair.
    pub fn live_table_pair(&self, table: u32, selector: i64) -> bool {
        self.pair_callers.contains_key(&(table, selector))
    }
}

/// The nodes, the edges and the tables of all the summaries, with the names
/// unified.
struct Graph {
    names: Vec<String>,
    index: HashMap<String, u32>,
    refs: Vec<Vec<u32>>,
    calls: Vec<Vec<i64>>,
    receiver_calls: Vec<Vec<(i64, i64)>>,
    exact_receiver_calls: Vec<Vec<(i64, i64)>>,
    nominal_classes: HashSet<i64>,
    ambiguous_classes: HashSet<i64>,
    parent_edges: HashMap<i64, Vec<i64>>,
    /// Table node → (cid, pairs).
    tables: HashMap<u32, (i64, Vec<(i64, u32)>)>,
    strings: HashMap<i64, String>,
    /// The nodes some module defines (the others belong to the runtime or are external).
    defined: HashSet<u32>,
}

impl Graph {
    fn id(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), i);
        self.refs.push(Vec::new());
        self.calls.push(Vec::new());
        self.receiver_calls.push(Vec::new());
        self.exact_receiver_calls.push(Vec::new());
        i
    }

    fn of(summaries: &[&Summary]) -> Graph {
        let mut g = Graph {
            names: Vec::new(),
            index: HashMap::new(),
            refs: Vec::new(),
            calls: Vec::new(),
            receiver_calls: Vec::new(),
            exact_receiver_calls: Vec::new(),
            nominal_classes: HashSet::new(),
            ambiguous_classes: HashSet::new(),
            parent_edges: HashMap::new(),
            tables: HashMap::new(),
            strings: HashMap::new(),
            defined: HashSet::new(),
        };
        for r in summaries {
            let map: Vec<u32> = r.names.iter().map(|n| g.id(n)).collect();
            for d in &r.definitions {
                let n = map[d.name as usize] as usize;
                g.defined.insert(n as u32);
                g.refs[n].extend(d.refs.iter().map(|&x| map[x as usize]));
                g.calls[n].extend_from_slice(&d.calls);
            }
            g.nominal_classes.extend(r.nominal_classes.iter().copied());
            g.ambiguous_classes.extend(r.ambiguous_classes.iter().copied());
            for &(child, parent) in &r.parent_edges {
                g.parent_edges.entry(child).or_default().push(parent);
            }
            for &(node, selector, cone) in &r.receiver_calls {
                g.receiver_calls[map[node as usize] as usize].push((selector, cone));
            }
            for &(node, selector, cid) in &r.exact_receiver_calls {
                g.exact_receiver_calls[map[node as usize] as usize].push((selector, cid));
            }
            for t in &r.tables {
                let pairs = t.pairs.iter().map(|&(h, f)| (h, map[f as usize])).collect();
                g.tables.insert(map[t.node as usize], (t.cid, pairs));
                g.defined.insert(map[t.node as usize]);
            }
            for (h, s) in &r.selectors {
                g.strings.entry(*h).or_insert_with(|| s.clone());
            }
        }
        g
    }

    /// A name root (§3.6), if some module defines it: the entry of the
    /// process, what the runtime may cite by symbol and the LLVM
    /// globals. (A runtime function cited by Dart code is not a root: the
    /// report says who cites it.)
    fn root(name: &str) -> bool {
        name == "main" || name.starts_with("dartforge_") || name.starts_with("llvm.")
    }

    /// Missing ancestry is an open world: it cannot exclude a runtime identity.
    fn receiver_matches(&self, cid: i64, cone: i64) -> bool {
        let mut pending = vec![cid];
        let mut visited = HashSet::new();
        let mut incomplete = false;
        while let Some(class) = pending.pop() {
            if class == cone {
                return true;
            }
            if !visited.insert(class) {
                continue;
            }
            if !self.nominal_classes.contains(&class) || self.ambiguous_classes.contains(&class) {
                incomplete = true;
                continue;
            }
            if let Some(parents) = self.parent_edges.get(&class) {
                pending.extend(parents.iter().copied());
            }
        }
        incomplete
    }

    /// The fixpoint (§3.5). `prune = false`: every pair of a live table lives.
    fn reachability(self, prune: bool) -> Reach {
        let mut causes: HashMap<u32, Cause> = HashMap::new();
        let mut callees: HashMap<i64, Option<u32>> = HashMap::new();
        let mut pending_set: HashMap<i64, Vec<(u32, u32)>> = HashMap::new();
        let mut queue: Vec<(u32, Cause)> = Vec::new();
        let mut all = !prune;
        let mut all_caller = None;
        let mut unrestricted = HashSet::new();
        let mut unrestricted_callers = HashMap::new();
        let mut cones: HashMap<i64, HashSet<i64>> = HashMap::new();
        let mut cone_callers = HashMap::new();
        let mut exact_classes: HashMap<i64, HashSet<i64>> = HashMap::new();
        let mut exact_callers = HashMap::new();
        let mut pair_callers = HashMap::new();
        let mut receiver_cache = HashMap::new();
        let mut matches = |cid, cone| *receiver_cache.entry((cid, cone))
            .or_insert_with(|| self.receiver_matches(cid, cone));
        for (i, n) in self.names.iter().enumerate() {
            if Self::root(n) && self.defined.contains(&(i as u32)) {
                queue.push((i as u32, Cause::Root));
            }
        }
        for s in ROOT_SELECTORS {
            let hash = selector_hash(s);
            callees.insert(hash, None);
            unrestricted.insert(hash);
            unrestricted_callers.insert(hash, None);
        }
        let by_name: HashSet<u32> = CALL_BY_NAME.iter().filter_map(|n| self.index.get(*n).copied()).collect();
        while let Some((node, cause)) = queue.pop() {
            if causes.contains_key(&node) {
                continue;
            }
            causes.insert(node, cause);
            if by_name.contains(&node) && !all {
                all = true;
                all_caller = Some(node);
                for (h, v) in pending_set.drain() {
                    for (t, f) in v {
                        pair_callers.entry((t, h)).or_insert(all_caller);
                        queue.push((f, Cause::Table(t, h)));
                    }
                }
            }
            for &h in &self.calls[node as usize] {
                callees.entry(h).or_insert(Some(node));
                if unrestricted.insert(h) {
                    unrestricted_callers.insert(h, Some(node));
                    if let Some(v) = pending_set.remove(&h) {
                        for (t, f) in v {
                            pair_callers.entry((t, h)).or_insert(Some(node));
                            queue.push((f, Cause::Table(t, h)));
                        }
                    }
                }
            }
            for &(hash, cone) in &self.receiver_calls[node as usize] {
                callees.entry(hash).or_insert(Some(node));
                cone_callers.entry((hash, cone)).or_insert(node);
                if cones.entry(hash).or_default().insert(cone)
                    && let Some(waiting) = pending_set.get_mut(&hash)
                {
                    waiting.retain(|&(table, function)| {
                        let cid = self.tables[&table].0;
                        if matches(cid, cone) {
                            pair_callers.entry((table, hash)).or_insert(Some(node));
                            queue.push((function, Cause::Table(table, hash)));
                            false
                        } else {
                            true
                        }
                    });
                }
            }
            for &(hash, cid) in &self.exact_receiver_calls[node as usize] {
                callees.entry(hash).or_insert(Some(node));
                exact_callers.entry((hash, cid)).or_insert(node);
                if exact_classes.entry(hash).or_default().insert(cid)
                    && let Some(waiting) = pending_set.get_mut(&hash)
                {
                    waiting.retain(|&(table, function)| {
                        if self.tables[&table].0 == cid {
                            pair_callers.entry((table, hash)).or_insert(Some(node));
                            queue.push((function, Cause::Table(table, hash)));
                            false
                        } else { true }
                    });
                }
            }
            if let Some((_, pairs)) = self.tables.get(&node) {
                for &(h, f) in pairs {
                    let caller = if all {
                        Some(all_caller)
                    } else if unrestricted.contains(&h) {
                        Some(unrestricted_callers[&h])
                    } else {
                        let nominal = cones.get(&h).and_then(|values| values.iter()
                            .filter(|&&cone| matches(self.tables[&node].0, cone))
                            .map(|&cone| cone_callers[&(h, cone)]).min());
                        let exact = exact_classes.get(&h).filter(|classes| classes.contains(&self.tables[&node].0))
                            .map(|_| exact_callers[&(h, self.tables[&node].0)]);
                        nominal.into_iter().chain(exact).min().map(Some)
                    };
                    if let Some(caller) = caller {
                        pair_callers.entry((node, h)).or_insert(caller);
                        queue.push((f, Cause::Table(node, h)));
                    } else {
                        pending_set.entry(h).or_default().push((node, f));
                    }
                }
            }
            queue.extend(self.refs[node as usize].iter().map(|&r| (r, Cause::Ref(node))));
        }
        Reach { names: self.names, causes, callees, pair_callers, all_pairs: all, strings: self.strings }
    }
}

/// The fixpoint over the summaries (§3.5), for those who only want the reach.
pub fn reach(summaries: &[&Summary], prune: bool) -> Reach {
    Graph::of(summaries).reachability(prune)
}

/// Pruning numbers (`--timings`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Statistics {
    pub tables: usize,
    pub live_tables: usize,
    pub pairs: usize,
    pub live_pairs: usize,
    pub nodes: usize,
    pub live_nodes: usize,
    /// Program definitions removed before generation (§3.12).
    pub pruned_functions: usize,
    pub pruned_globals: usize,
    pub pruned_bytes: usize,
    pub time: std::time::Duration,
}

/// The IR of the program with the tables assembled.
pub struct Assembly {
    pub ir: String,
    pub statistics: Statistics,
    pub reachability: Reach,
}

/// Assembles the method tables in the IR of the program (§3.8): those of the program
/// rewritten with only the live pairs, and those of the SDK summaries defined,
/// pruned, with the function that returns each one. `prune = false`: all the
/// pairs (`DARTFORGE_NO_TABLE_PRUNING`).
///
/// # Errors
/// A table line of the program that cannot be read (format outside that of
/// [`table_line`]).
pub fn assemble(ir: &str, summaries: &[&Summary], prune: bool) -> Result<Assembly, String> {
    let t0 = std::time::Instant::now();
    const PROGRAM: &str = "programa";
    let program = summarize(ir, PROGRAM, &[]);
    let mut all: Vec<&Summary> = summaries.to_vec();
    all.push(&program);
    let reachability = reach(&all, prune);
    let index: HashMap<&str, u32> = reachability.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
    let mut st = Statistics { nodes: reachability.names.len(), live_nodes: reachability.causes.len(), ..Default::default() };
    // The live pairs of a table, written as in the IR.
    let live_pairs = |node: Option<u32>, pairs: &[(i64, String)], st: &mut Statistics| -> Vec<(i64, String)> {
        st.tables += 1;
        st.pairs += pairs.len();
        if !node.is_some_and(|n| reachability.live(n)) {
            return Vec::new();
        }
        st.live_tables += 1;
        let table = node.expect("live table has a graph identity");
        let v: Vec<(i64, String)> = pairs.iter()
            .filter(|(h, _)| reachability.live_table_pair(table, *h)).cloned().collect();
        st.live_pairs += v.len();
        v
    };
    // The functions of the tables the program defines.
    let externals: HashSet<&str> =
        summaries.iter().flat_map(|r| r.externals.iter().map(move |&e| r.names[e as usize].as_str())).collect();
    let arc_registrations: HashSet<&str> = all.iter().flat_map(|r| r.arc_ports.iter()
        .map(|inventory| r.names[inventory.registration as usize].as_str())).collect();
    let mut declared_set: HashSet<String> = HashSet::new();
    let mut exit = String::with_capacity(ir.len() + ir.len() / 4);
    // With pruning, the program definitions the fixpoint did not reach
    // leave right here (§3.12): LTO and `/OPT:REF` would remove them later, but only
    // after optimizing and generating them — in new_sali/backend, most
    // of the IR.
    let dead = |n: &str, local: bool| -> bool {
        if !prune {
            return false;
        }
        let key = if local { format!("{PROGRAM}#{n}") } else { n.to_string() };
        index.get(key.as_str()).is_some_and(|&node| !reachability.live(node))
    };
    let mut skipping = false;
    for l in ir.split_inclusive('\n') {
        let without_end = l.trim_end_matches(['\n', '\r']);
        if skipping {
            st.pruned_bytes += l.len();
            if without_end == "}" {
                skipping = false;
            }
            continue;
        }
        if let Some(d) = without_end.strip_prefix("declare ") {
            if let Some(n) = d.find('@').and_then(|i| cited(&d[i..]).next()) {
                if externals.contains(n) || arc_registrations.contains(n) {
                    continue;
                }
                declared_set.insert(n.to_string());
            }
        } else if let Some((n, local)) = define_name(without_end) {
            if dead(n, local) {
                st.pruned_functions += 1;
                st.pruned_bytes += l.len();
                skipping = !without_end.trim_end().ends_with('}');
                continue;
            }
            declared_set.insert(n.to_string());
        } else if let Some((n, local, rest)) = global_name(without_end) {
            let table = n.starts_with("df.mt.") && n.ends_with("$d");
            if !table && dead(n, local) {
                st.pruned_globals += 1;
                st.pruned_bytes += l.len();
                continue;
            }
            declared_set.insert(n.to_string());
            if table {
                let (cid, pairs) = line_table(rest).ok_or_else(|| format!("unreadable method table in the program: {n}"))?;
                let pairs: Vec<(i64, String)> = pairs.into_iter().map(|(h, f)| (h, llvm_name(&f))).collect();
                let key = if local { format!("{PROGRAM}#{n}") } else { n.to_string() };
                let live_set = live_pairs(index.get(key.as_str()).copied(), &pairs, &mut st);
                let prefix = &without_end[..without_end.len() - rest.len()];
                let constant = rest.find("constant ").map_or(0, |i| i + "constant ".len());
                exit.push_str(&table_line(&format!("{prefix}{}", &rest[..constant]), cid, &live_set));
                continue;
            }
        }
        exit.push_str(l);
    }
    // The tables of the summaries.
    exit.push_str("\n; SDK method tables assembled at link time (docs/NATIVO-PODA-DE-TABELAS.md)\n");
    let mut declare: Vec<String> = Vec::new();
    for r in summaries {
        let table_of: HashMap<u32, &SummarizedTable> = r.tables.iter().map(|t| (t.node, t)).collect();
        for &e in &r.externals {
            let function = &r.names[e as usize];
            let Some(t) = r.index.get(&format!("{function}$d")).and_then(|node| table_of.get(node)) else {
                return Err(format!("the summary of {} cites table {function} without its content", r.module));
            };
            let node = &r.names[t.node as usize];
            let pairs: Vec<(i64, String)> = t.pairs.iter().map(|&(h, f)| (h, llvm_name(&r.names[f as usize]))).collect();
            let live_set = live_pairs(index.get(node.as_str()).copied(), &pairs, &mut st);
            for (_, f) in &live_set {
                let name = f.trim_start_matches('@').trim_matches('"');
                if declared_set.insert(name.to_string()) {
                    declare.push(format!("declare i64 {f}(i64, ptr, ptr)\n"));
                }
            }
            let data = llvm_name(node);
            exit.push_str(&table_line(&format!("{data} = private unnamed_addr constant "), t.cid, &live_set));
            writeln!(exit, "define ptr {}() {{\nb0:\n  ret ptr {data}\n}}", llvm_name(function)).unwrap();
        }
    }
    for d in declare {
        exit.push_str(&d);
    }
    arc_ports::assemble(&mut exit, &all, &reachability, prune, &mut declared_set)?;
    st.time = t0.elapsed();
    Ok(Assembly { ir: exit, statistics: st, reachability })
}

/// The "why did this stay" report (§3.10): one line per live node.
pub fn report(reachability: &Reach) -> String {
    let text = |h: i64| reachability.strings.get(&h).cloned().unwrap_or_else(|| h.to_string());
    let name = |i: u32| reachability.names[i as usize].as_str();
    let mut lines: Vec<String> = reachability
        .causes
        .iter()
        .map(|(&node, cause)| match cause {
            Cause::Root => format!("{}\troot\t-\t-\t-", name(node)),
            Cause::Ref(p) => format!("{}\tref\t{}\t-\t-", name(node), name(*p)),
            Cause::Table(t, h) => {
                let who = reachability.pair_callers.get(&(*t, *h)).copied().flatten().map_or("root", name);
                format!("{}\ttable\t{}\t{}\t{}", name(node), name(*t), text(*h), who)
            }
        })
        .collect();
    lines.sort();
    let mut s = String::from("symbol\tcause\tparent\tselector\tcaller\n");
    for l in lines {
        s.push_str(&l);
        s.push('\n');
    }
    s
}

/// The summaries of the production SDK in `files`, read once per process.
///
/// # Errors
/// A missing file (a production SDK without a summary) or an unreadable one.
pub fn read_summaries(files: &[PathBuf]) -> Result<Vec<std::sync::Arc<Summary>>, String> {
    use std::sync::{Arc, Mutex, OnceLock};
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, Arc<Summary>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    files
        .iter()
        .map(|a| {
            if let Some(r) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(a) {
                return Ok(r.clone());
            }
            let text = std::fs::read_to_string(a).map_err(|e| {
                format!("the pruning summary {} cannot be read ({e}); delete the cached production SDK to recompile it", a.display())
            })?;
            let r = Arc::new(Summary::from_string(&text).map_err(|e| format!("{}: {e}", a.display()))?);
            memo.lock().unwrap_or_else(|e| e.into_inner()).insert(a.clone(), r.clone());
            Ok(r)
        })
        .collect()
}

/// The assembly of the production link: reads the summaries, assembles, writes the
/// report if `DARTFORGE_WHY` asks and shows the numbers with `timings`.
///
/// # Errors
/// Those of [`read_summaries`] and [`assemble`], or a report that cannot be written.
pub fn assemble_production(ir: &str, files: &[PathBuf], timings: bool) -> Result<String, String> {
    let summaries = read_summaries(files)?;
    let refs: Vec<&Summary> = summaries.iter().map(|r| r.as_ref()).collect();
    let prune = !std::env::var("DARTFORGE_NO_TABLE_PRUNING").is_ok_and(|v| v == "1");
    let m = assemble(ir, &refs, prune)?;
    if let Some(file) = std::env::var_os("DARTFORGE_WHY") {
        let file = Path::new(&file);
        std::fs::write(file, report(&m.reachability)).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    if timings {
        let e = m.statistics;
        eprintln!(
            "  Pruning:   {} of {} tables live, {} of {} pairs, {} of {} symbols; outside the IR: {} functions, {} globals, {} MB ({:?}){}",
            e.live_tables,
            e.tables,
            e.live_pairs,
            e.pairs,
            e.live_nodes,
            e.nodes,
            e.pruned_functions,
            e.pruned_globals,
            e.pruned_bytes / 1_000_000,
            e.time,
            if prune { "" } else { " [no pruning]" }
        );
    }
    Ok(m.ir)
}

/// The summary of the HIR module of the program, before emission (§3.13): the same
/// graph that [`summarize`] would read from the IR the emission would write, with the
/// references the emitter adds by itself put as roots of the
/// entry (`dartforge_entry`): `main`, the registrations of the libraries, the
/// method tables of the program (the registration cites all of them), the `toString`
/// and the vtables of the classes, the FFI trampolines and callbacks, the
/// helpers and the source roots.
pub fn hir_summary(m: &crate::hir::Module) -> Summary {
    hir_summary_with(m, None)
}

/// [`hir_summary`] with the FFI trampolines and callbacks as roots only
/// for the keys of `live_ffis` (`None`: all of them, §3.14).
fn hir_summary_with(m: &crate::hir::Module, live_ffis: Option<&HashSet<String>>) -> Summary {
    use crate::hir::{Constant, Instruction, Operand};
    let registry_closed = receiver_provenance::registry::closed(&[m]);
    let provenance = if receiver_provenance::enabled() && registry_closed { receiver_provenance::analyze(&[m]) } else { receiver_provenance::Provenance::default() };
    let mut r = Summary::new("programa");
    r.declared_field_layouts = m.declared_field_layouts.iter()
        .filter(|(cid, _)| !m.pruning_ambiguous_class_ids.contains(cid))
        .map(|(cid, fields)| (*cid, fields.clone())).collect();
    r.nominal_classes = m.pruning_class_ids.iter().map(|&cid| i64::from(cid)).collect();
    r.nominal_classes.sort_unstable();
    r.ambiguous_classes = m.pruning_ambiguous_class_ids.iter().map(|&cid| i64::from(cid)).collect();
    r.ambiguous_classes.sort_unstable();
    r.parent_edges = m.subtyping_edges.iter().map(|&(child, parent)| (i64::from(child), i64::from(parent))).collect();
    r.parent_edges.sort_unstable();
    r.parent_edges.dedup();
    let mut seen_selectors: HashSet<i64> = HashSet::new();
    let mut selector = |r: &mut Summary, d: &mut Definition, text: &str, cone: Option<i64>, exact: Option<&receiver_provenance::Classes>| {
        let h = selector_hash(text);
        if let Some(classes) = exact {
            r.exact_receiver_calls.extend(classes.iter().map(|&cid| (d.name, h, i64::from(cid))));
        } else if let Some(cone) = cone {
            r.receiver_calls.push((d.name, h, cone));
        } else {
            d.calls.push(h);
        }
        if seen_selectors.insert(h) {
            r.selectors.push((h, text.to_string()));
        }
        // The fallback of the typed selector (`dartforge_selector`), as in the IR.
        if let Some(without_t) = text.strip_prefix('t') {
            let h2 = selector_hash(without_t);
            if let Some(classes) = exact {
                r.exact_receiver_calls.extend(classes.iter().map(|&cid| (d.name, h2, i64::from(cid))));
            } else if let Some(cone) = cone {
                r.receiver_calls.push((d.name, h2, cone));
            } else {
                d.calls.push(h2);
            }
            if seen_selectors.insert(h2) {
                r.selectors.push((h2, without_t.to_string()));
            }
        }
    };
    for f in &m.functions {
        let mut d = Definition { name: r.id(&f.symbol), ..Default::default() };
        let mut names: Vec<&str> = Vec::new();
        // The `Constant::Function`s of the operands (the visitor does not borrow the
        // operand beyond the call).
        let mut functions: Vec<String> = Vec::new();
        let mut recipes: Vec<String> = Vec::new();
        for b in &f.blocks {
            for (value_id, inst, _) in &b.instructions {
                if let Instruction::DeclaredFieldCall { declaration, name, .. } = inst {
                    r.declared_field_accesses.push(DeclaredFieldAccess {
                        definition: d.name, value_id: value_id.0,
                        write: name == "dartforge_object_set", declaration: declaration.clone(),
                    });
                }
                match inst {
                    // The RTI recipes with a function type (§3.14).
                    Instruction::Const(Constant::String(t)) if t.contains("F<") => recipes.push(t.clone()),
                    Instruction::CallStatic { symbol, .. } => names.push(symbol),
                    Instruction::AllocClosure { code_symbol, .. } | Instruction::TearOff { code_symbol } => names.push(code_symbol),
                    Instruction::AllocClosureTyped { code_symbol, typed, .. } => {
                        names.push(code_symbol);
                        names.push(typed);
                    }
                    Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } => {
                        names.push(name);
                        // The allocation cites the method table of the class
                        // (`dartforge_object_new_t`, `llvm/mod.rs`).
                        if name.starts_with("dartforge_object_new")
                            && let Some((Operand::Constant(Constant::Int(cid)), _)) = args.first()
                            && let Some(t) = m.table_functions.get(&(*cid as u32))
                        {
                            names.push(t);
                        }
                    }
                    Instruction::CallSelector { selector: s, recv, .. } | Instruction::CallSelectorForward { selector: s, recv, .. } => {
                        let cone = m.pruning_this_receivers.get(&f.symbol).filter(|_| registry_closed)
                            .filter(|(parameter, _)| f.params.iter().any(|(id, _, ty)| id == parameter && *ty == crate::hir::Type::Ref))
                            .filter(|(parameter, _)| recv == &Operand::Val(*parameter))
                            .map(|(_, cid)| i64::from(*cid));
                        let exact = if let Operand::Val(value) = recv { provenance.values_for(f).and_then(|values| values.get(value)) } else { None };
                        selector(&mut r, &mut d, s, cone, exact);
                    }
                    Instruction::FunctionTable(v) => names.extend(v.iter().map(String::as_str)),
                    _ => {}
                }
                crate::optimize::operands::operands(inst, &mut |o| match o {
                    Operand::Constant(Constant::Function(s)) => functions.push(s.clone()),
                    Operand::Constant(Constant::String(t)) if t.contains("F<") => recipes.push(t.clone()),
                    _ => {}
                });
            }
            crate::optimize::operands::terminator_operands(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    functions.push(s.clone());
                }
            });
        }
        d.refs = names.into_iter().chain(functions.iter().map(String::as_str)).map(|n| r.id(n)).collect();
        r.recipes.extend(recipes.into_iter().map(|t| (d.name, t)));
        r.definitions.push(d);
    }
    // The method tables of the program (`df.mt.X` returns `df.mt.X$d`).
    for (cid, sym_name, methods) in &m.method_tables {
        let function = r.id(sym_name);
        let node = r.id(&format!("{sym_name}$d"));
        let pairs: Vec<(i64, u32)> = table_pairs(*cid, methods)
            .into_iter()
            .map(|(h, s, f)| {
                if seen_selectors.insert(h) {
                    r.selectors.push((h, s));
                }
                (h, r.id(&f))
            })
            .collect();
        r.definitions.push(Definition { name: function, refs: vec![node], calls: Vec::new() });
        r.tables.push(SummarizedTable { node, cid: i64::from(*cid), pairs });
    }
    // The entry: what the emitter cites without an HIR instruction.
    let mut roots: Vec<String> = Vec::new();
    roots.extend(m.entry_symbol.iter().cloned());
    roots.extend(m.sdk_records.iter().cloned());
    roots.extend(m.record.iter().cloned());
    roots.extend(m.start_rti.iter().cloned());
    roots.extend(m.call_dart.iter().cloned());
    roots.extend(m.method_tables.iter().map(|(_, s, _)| s.clone()));
    roots.extend(m.table_functions.values().cloned());
    for c in &m.classes {
        roots.extend(c.to_string_symbol.iter().cloned());
        roots.extend(c.vtable.iter().map(|(_, s)| s.clone()));
    }
    let live_ffi = |k: &str| live_ffis.is_none_or(|v| v.contains(k));
    roots.extend(m.ffi_trampolines.iter().filter(|(k, _)| live_ffi(k)).map(|(_, s)| s.clone()));
    roots.extend(m.ffi_callbacks.iter().filter(|c| live_ffi(&c.key)).map(|c| c.body.clone()));
    roots.extend(m.helpers.iter().map(|(_, s)| s.clone()));
    roots.extend(m.source_roots.iter().cloned());
    roots.extend(m.globals.iter().map(|(_, _, s)| s.clone()));
    let mut entry = Definition { name: r.id("dartforge_entry"), ..Default::default() };
    entry.refs = roots.iter().map(|n| r.id(n)).collect();
    r.definitions.push(entry);
    r
}

/// HIR pruning numbers (`--timings`).
#[derive(Debug, Clone, Copy, Default)]
pub struct HirPruning {
    pub functions: usize,
    pub live_functions: usize,
    pub pairs: usize,
    pub live_pairs: usize,
    /// The FFI signatures that stayed, and whether their pruning was worth it (§3.14).
    pub ffi: (usize, bool),
}

/// The pruning of the HIR of the program before optimization and emission (§3.13): the
/// fixpoint of [`assemble`] over the HIR summary ([`hir_summary`]) and the
/// summaries of the SDK libraries, and the module without the functions it does not
/// reach and without the selector table pairs nobody calls. It is the
/// decision the link assembly would take later (the same graph), without
/// optimizing nor emitting what is going to leave.
pub fn prune_hir(m: &mut crate::hir::Module, sdk: &[&Summary]) -> HirPruning {
    // The FFI trampolines and callbacks (one pair per native signature of the
    // whole type table) only stay through the signatures written in the
    // live recipes (§3.14). A live trampoline may reach new
    // recipes: the fixpoint repeats until the keys stop growing.
    let letters: HashMap<i64, char> = m.ffi_types.iter().copied().collect();
    let composites: HashSet<i64> = m.ffi_composites.iter().map(|c| c.rti).collect();
    let prune_ffi = !m.ffi_trampolines.is_empty() && !std::env::var("DARTFORGE_NO_FFI_PRUNING").is_ok_and(|v| v == "1");
    let mut live_ffis: HashSet<String> = HashSet::new();
    let (program, reachability) = loop {
        let program = hir_summary_with(m, prune_ffi.then_some(&live_ffis));
        let mut all: Vec<&Summary> = sdk.to_vec();
        all.push(&program);
        let reachability = reach(&all, true);
        if !prune_ffi {
            break (program, reachability);
        }
        let index: HashMap<&str, u32> = reachability.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
        let before = live_ffis.len();
        for r in &all {
            for (node, t) in &r.recipes {
                if index.get(r.names[*node as usize].as_str()).is_some_and(|&i| reachability.live(i)) {
                    recipe_ffi_keys(t, &letters, &composites, &mut live_ffis);
                }
            }
        }
        if live_ffis.len() == before {
            break (program, reachability);
        }
    };
    drop(program);
    if prune_ffi {
        m.ffi_trampolines.retain(|(k, _)| live_ffis.contains(k));
        m.ffi_callbacks.retain(|c| live_ffis.contains(&c.key));
    }
    let index: HashMap<&str, u32> = reachability.names.iter().enumerate().map(|(i, n)| (n.as_str(), i as u32)).collect();
    let live = |n: &str| index.get(n).is_none_or(|&i| reachability.live(i));
    let mut e = HirPruning { functions: m.functions.len(), ffi: (live_ffis.len(), prune_ffi), ..Default::default() };
    let mut removed_set: HashSet<String> = HashSet::new();
    m.functions.retain(|f| {
        let stays = live(&f.symbol) || f.symbol.starts_with("dartforge_");
        if !stays {
            removed_set.insert(f.symbol.clone());
        }
        stays
    });
    e.live_functions = m.functions.len();
    m.dart_scalar_parameters.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.dart_rti_parameters.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.dart_vector_parameters.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.arc_uniform_returns.retain(|sym_name| !removed_set.contains(sym_name));
    m.dart_ref_returns.retain(|sym_name| !removed_set.contains(sym_name));
    m.arc_direct_local_functions.retain(|sym_name| !removed_set.contains(sym_name));
    m.arc_sites.retain(|sym_name, _| !removed_set.contains(sym_name));
    m.arc_shared_functions.retain(|sym_name| !removed_set.contains(sym_name));
    // The table pair whose selector nobody calls goes away, and so does that of the function that left
    // (the entry of a live pair is always reached).
    for (_, table, pairs) in &mut m.method_tables {
        e.pairs += pairs.len();
        let node = index.get(format!("{table}$d").as_str()).copied();
        pairs.retain(|(s, f)| node.is_none_or(|n| reachability.live_table_pair(n, selector_hash(s)))
            && !removed_set.contains(f));
        e.live_pairs += pairs.len();
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> i64 {
        selector_hash(s)
    }

    /// A toy SDK: class `C` has `used` and `dead`; the table is
    /// external, and function `allocates` cites the function of the table.
    fn sdk() -> Summary {
        let ir = String::from(
            "define i64 @C.usado$c(i64 %t, ptr %a, ptr %d) {\n  ret i64 0\n}\n\
             define i64 @C.morto$c(i64 %t, ptr %a, ptr %d) {\n  %x = call i64 @C.morto(i64 %t)\n  ret i64 %x\n}\n\
             define i64 @C.morto(i64 %t) {\n  ret i64 1\n}\n\
             define internal i64 @df.class(i64 %h) {\n  ret i64 0\n}\n\
             define i64 @aloca() {\n  %v = call i64 @dartforge_object_new_t(i64 200, i64 0, ptr @df.mt.C)\n  ret i64 %v\n}\n\
             declare ptr @df.mt.C()\n"
        );
        let methods = vec![("c:usado".to_string(), "C.usado$c".to_string()), ("c:morto".to_string(), "C.morto$c".to_string())];
        summarize(&ir, "dart:x", &[(200, "df.mt.C".to_string(), methods)])
    }

    fn program(call: &str) -> String {
        format!(
            "@df.seln.0 = private unnamed_addr constant [{} x i8] c\"{call}\"\n\
             define internal i64 @df.class(i64 %h) {{\n  ret i64 0\n}}\n\
             define i32 @main() {{\n  %o = call i64 @aloca()\n  %f = call ptr @df.selector(ptr %ic, i64 %o, i64 {}, ptr @df.seln.0, i64 {})\n  ret i32 0\n}}\n\
             @df.mt.P$d = private unnamed_addr constant {{ i64, i64, i64, ptr }} {{ i64 300, i64 1, i64 {}, ptr @P.nunca$c }}\n\
             define ptr @df.mt.P() {{\nb0:\n  ret ptr @df.mt.P$d\n}}\n\
             define i64 @P.nunca$c(i64 %t, ptr %a, ptr %d) {{\n  ret i64 0\n}}\n\
             declare ptr @df.mt.C()\ndeclare i64 @aloca()\n",
            call.len(),
            h(call),
            call.len(),
            h("c:nunca")
        )
    }

    #[test]
    fn cited_reads_plain_and_quoted_names() {
        let v: Vec<&str> = cited("  call void @f(ptr @\"df.s.1\", ptr @g.h$c, i64 %x) ; @").collect();
        assert_eq!(v, vec!["f", "df.s.1", "g.h$c"]);
        assert_eq!(llvm_name("df.dart$3acore.x"), "@df.dart$3acore.x");
        assert_eq!(llvm_name("a b"), "@\"a b\"");
    }

    #[test]
    fn summary_round_trip() {
        let r = sdk();
        let again = Summary::from_string(&r.to_text()).unwrap();
        assert_eq!(again.names, r.names);
        assert_eq!(again.definitions, r.definitions);
        assert_eq!(again.tables, r.tables);
        assert_eq!(again.externals, r.externals);
        assert_eq!(again.selectors, r.selectors);
        assert!(Summary::from_string("outra coisa\n").is_err());
    }

    #[test]
    fn locals_carry_module_prefix() {
        let r = sdk();
        assert!(r.names.contains(&"dart:x#df.class".to_string()));
        assert!(!r.names.contains(&"df.class".to_string()));
    }

    #[test]
    fn typed_selector_also_calls_untyped() {
        let ir = program("tc:usado");
        let r = summarize(&ir, "p", &[]);
        let main = r.definitions.iter().find(|d| r.names[d.name as usize] == "main").unwrap();
        assert!(main.calls.contains(&h("tc:usado")));
        assert!(main.calls.contains(&h("c:usado")));
    }

    #[test]
    fn only_the_called_selector_pair_stays() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], true).unwrap();
        let ir = &m.ir;
        assert!(ir.contains("ptr @C.usado$c"), "{ir}");
        assert!(!ir.contains("ptr @C.morto$c"), "{ir}");
        assert!(ir.contains("declare i64 @C.usado$c(i64, ptr, ptr)"));
        assert!(ir.contains("define ptr @df.mt.C()"));
        assert!(!ir.contains("declare ptr @df.mt.C()"), "the declaration of the table function goes away: {ir}");
        // Nobody cites `df.mt.P`: the program table comes out empty.
        assert!(ir.contains("@df.mt.P$d = private unnamed_addr constant { i64, i64 } { i64 300, i64 0 }"), "{ir}");
        assert!(!m.reachability.live(m.reachability.names.iter().position(|n| n == "C.morto").unwrap() as u32));
        let e = m.statistics;
        assert_eq!((e.tables, e.live_tables, e.pairs, e.live_pairs), (2, 1, 3, 1));
    }

    #[test]
    fn compact_call_reads_selector_from_descriptor() {
        let ir = format!(
            "@df.seln.0 = private unnamed_addr constant [7 x i8] c\"tc:usado\"\n\
             @df.seld.0 = private unnamed_addr constant {{ i64, ptr, i64 }} {{ i64 {}, ptr @df.seln.0, i64 8 }}\n\
             define i32 @main() {{\n  %f = call ptr @df.selector_d(ptr %ic, i64 %o, ptr @df.seld.0)\n  ret i32 0\n}}\n",
            h("tc:usado")
        );
        let r = summarize(&ir, "programa", &[]);
        let main = r.definitions.iter().find(|d| r.names[d.name as usize] == "main").unwrap();
        assert!(main.calls.contains(&h("tc:usado")), "{:?}", main.calls);
        // The fallback of the typed selector.
        assert!(main.calls.contains(&h("c:usado")), "{:?}", main.calls);
        // The descriptor is cited (it stays live with its caller).
        assert!(main.refs.iter().any(|&i| r.names[i as usize] == "programa#df.seld.0"));
    }

    /// An HIR module: `main` calls `f` and the selector `c:used`; nobody cites `g`;
    /// class 300 has the pairs `c:used` and `c:dead`.
    fn hir_module() -> crate::hir::Module {
        use crate::hir::*;
        let function = |name: &str, instructions: Vec<(ValueId, Instruction, Type)>| Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: Vec::new(),
            return_ty: Type::Void,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: instructions, terminator: Terminator::Return(None) }],
        };
        let mut m = Module::new();
        m.functions.push(function(
            "dart_main",
            vec![
                (ValueId(0), Instruction::CallStatic { symbol: "f".into(), args: Vec::new(), ret_ty: Type::Void }, Type::Void),
                (
                    ValueId(1),
                    Instruction::CallSelector {
                        selector: "c:usado".into(),
                        recv: Operand::Constant(Constant::Null),
                        args: Vec::new(),
                        names: Vec::new(),
                        tuple_types: Operand::Constant(Constant::Int(0)),
                    },
                    Type::Ref,
                ),
            ],
        ));
        m.functions.push(function("f", Vec::new()));
        m.functions.push(function("g", Vec::new()));
        m.functions.push(function("P.usado$c", Vec::new()));
        m.functions.push(function("P.morto$c", Vec::new()));
        m.entry_symbol = Some("dart_main".into());
        m.sdk_records = vec!["df.register.core".into()];
        m.method_tables = vec![(
            300,
            "df.mt.P".into(),
            vec![("c:usado".into(), "P.usado$c".into()), ("c:morto".into(), "P.morto$c".into())],
        )];
        m
    }

    #[test]
    fn hir_pruning_removes_unreachable() {
        let mut m = hir_module();
        m.dart_scalar_parameters.insert("f".into(), HashSet::new());
        m.dart_scalar_parameters.insert("g".into(), HashSet::new());
        m.dart_rti_parameters.insert("f".into(), HashSet::new());
        m.dart_rti_parameters.insert("g".into(), HashSet::new());
        m.dart_vector_parameters.insert("f".into(), Default::default());
        m.dart_vector_parameters.insert("g".into(), Default::default());
        m.arc_uniform_returns.extend(["f".into(), "g".into()]);
        m.arc_shared_functions.extend(["f".into(), "g".into()]);
        let e = prune_hir(&mut m, &[]);
        assert!(m.dart_scalar_parameters.contains_key("f"));
        assert!(!m.dart_scalar_parameters.contains_key("g"));
        assert!(m.dart_rti_parameters.contains_key("f"));
        assert!(!m.dart_rti_parameters.contains_key("g"));
        assert!(m.dart_vector_parameters.contains_key("f"));
        assert!(!m.dart_vector_parameters.contains_key("g"));
        assert!(m.arc_uniform_returns.contains("f"));
        assert!(!m.arc_uniform_returns.contains("g"));
        assert!(m.arc_shared_functions.contains("f"));
        assert!(!m.arc_shared_functions.contains("g"));
        let names: Vec<&str> = m.functions.iter().map(|f| f.symbol.as_str()).collect();
        assert_eq!(names, vec!["dart_main", "f", "P.usado$c"], "{e:?}");
        assert_eq!(m.method_tables[0].2, vec![("c:usado".to_string(), "P.usado$c".to_string())]);
        assert_eq!((e.functions, e.live_functions, e.pairs, e.live_pairs), (5, 3, 2, 1));
        let f = m.functions[1].clone();
        let mut dead_sdk = f.clone();
        dead_sdk.symbol = "df.dart$3acore.dead".into();
        m.functions = vec![f, dead_sdk];
        m.dart_scalar_parameters.insert("df.dart$3acore.dead".into(), HashSet::new());
        m.dart_rti_parameters.insert("df.dart$3acore.dead".into(), HashSet::new());
        m.dart_vector_parameters.insert("df.dart$3acore.dead".into(), Default::default());
        m.arc_uniform_returns.insert("df.dart$3acore.dead".into());
        m.arc_shared_functions.insert("df.dart$3acore.dead".into());
        crate::source::prune(&mut m);
        assert!(m.dart_scalar_parameters.contains_key("f"));
        assert!(!m.dart_scalar_parameters.contains_key("df.dart$3acore.dead"));
        assert!(!m.dart_rti_parameters.contains_key("df.dart$3acore.dead"));
        assert!(m.dart_vector_parameters.contains_key("f"));
        assert!(!m.dart_vector_parameters.contains_key("df.dart$3acore.dead"));
        assert!(m.arc_uniform_returns.contains("f"));
        assert!(!m.arc_uniform_returns.contains("df.dart$3acore.dead"));
        assert!(!m.arc_shared_functions.contains("df.dart$3acore.dead"));
    }

    #[test]
    fn hir_pruning_sees_selector_called_by_sdk() {
        // The SDK calls `c:dead` in a function its registration reaches.
        let ir = "define void @df.register.core() {\n  call void @x()\n  ret void\n}\n\
                  define void @x() {\n  %f = call ptr @df.selector(ptr %ic, i64 %o, i64 HASH, ptr @df.seln.0, i64 7)\n  ret void\n}\n\
                  @df.seln.0 = private unnamed_addr constant [7 x i8] c\"c:morto\"\n"
            .replace("HASH", &h("c:morto").to_string());
        let sdk = summarize(&ir, "dart:core", &[]);
        let mut m = hir_module();
        prune_hir(&mut m, &[&sdk]);
        assert!(m.functions.iter().any(|f| f.symbol == "P.morto$c"));
        assert_eq!(m.method_tables[0].2.len(), 2);
        assert!(!m.functions.iter().any(|f| f.symbol == "g"));
    }

    #[test]
    fn dead_program_definitions_leave_the_ir() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], true).unwrap();
        let ir = &m.ir;
        // `df.mt.P` (nobody cites it) and its pair go away; `main` and the uncited local
        // `df.class` too: only what the fixpoint reaches stays.
        assert!(!ir.contains("define ptr @df.mt.P()"), "{ir}");
        assert!(!ir.contains("define i64 @P.nunca$c"), "{ir}");
        assert!(!ir.contains("define internal i64 @df.class"), "{ir}");
        assert!(ir.contains("define i32 @main()"), "{ir}");
        assert!(ir.contains("@df.seln.0 = private"), "the text of the selector that main cites stays: {ir}");
        let e = m.statistics;
        assert_eq!(e.pruned_functions, 3, "{ir}");
        // Without pruning, nothing goes away.
        let m = assemble(&program("c:usado"), &[&s], false).unwrap();
        assert!(m.ir.contains("define i64 @P.nunca$c"));
        assert_eq!(m.statistics.pruned_functions, 0);
    }

    #[test]
    fn without_pruning_all_live_table_pairs() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], false).unwrap();
        assert!(m.ir.contains("ptr @C.morto$c"));
    }

    #[test]
    fn root_selector_keeps_the_pair() {
        let methods = vec![("c:call".to_string(), "C.call$c".to_string())];
        let ir = "define i64 @aloca() {\n  %v = call i64 @dartforge_object_new_t(i64 200, i64 0, ptr @df.mt.C)\n  ret i64 %v\n}\n";
        let s = summarize(ir, "dart:x", &[(200, "df.mt.C".to_string(), methods)]);
        let m = assemble(&program("c:outro"), &[&s], true).unwrap();
        assert!(m.ir.contains("ptr @C.call$c"));
    }

    #[test]
    fn report_tells_the_cause() {
        let s = sdk();
        let m = assemble(&program("c:usado"), &[&s], true).unwrap();
        let r = report(&m.reachability);
        assert!(r.contains("C.usado$c\ttable\tdf.mt.C$d\tc:usado\tmain\n"), "{r}");
        assert!(r.contains("main\troot\t"), "{r}");
        assert!(r.contains("aloca\tref\tmain\t"), "{r}");
    }

    #[test]
    fn program_table_rewritten_with_only_live_pairs() {
        let mut pairs = [(h("c:vivo"), "@P.vivo$c"), (h("c:morto"), "@P.morto$c")];
        pairs.sort();
        let ir = format!(
            "@df.seln.0 = private unnamed_addr constant [7 x i8] c\"c:vivo\"\n\
             define i32 @main() {{\n  %p = call ptr @df.mt.P()\n  %f = call ptr @df.selector(ptr %ic, i64 0, i64 {}, ptr @df.seln.0, i64 6)\n  ret i32 0\n}}\n\
             @df.mt.P$d = private unnamed_addr constant {{ i64, i64, i64, ptr, i64, ptr }} {{ i64 300, i64 2, i64 {}, ptr {}, i64 {}, ptr {} }}\n\
             define ptr @df.mt.P() {{\nb0:\n  ret ptr @df.mt.P$d\n}}\n",
            h("c:vivo"),
            pairs[0].0,
            pairs[0].1,
            pairs[1].0,
            pairs[1].1,
        );
        let m = assemble(&ir, &[], true).unwrap();
        assert!(m.ir.contains("ptr @P.vivo$c"));
        assert!(!m.ir.contains("@P.morto$c"));
        assert!(m.ir.contains("{ i64, i64, i64, ptr } { i64 300, i64 1,"));
    }
}

#[cfg(test)]
mod ffi_tests {
    use super::*;

    fn keys(text: &str) -> Vec<String> {
        let letters: HashMap<i64, char> = [(703, 'i'), (719, 'p'), (737, 'v'), (736, '*'), (698, 'd')].into_iter().collect();
        let composites: HashSet<i64> = [900].into_iter().collect();
        let mut s = HashSet::new();
        recipe_ffi_keys(text, &letters, &composites, &mut s);
        let mut v: Vec<String> = s.into_iter().collect();
        v.sort();
        v
    }

    #[test]
    fn signature_key_in_recipe() {
        assert_eq!(keys("F<0;C703;2;C703,C719<C900>;>"), vec!["i_ip"]);
        // Inside another recipe (the tuple of `asFunction`, `NativeFunction<F>`).
        assert_eq!(keys("L<C597<F<0;C737;0;;>>,F<0;D;0;;>>"), vec!["v_"]);
        // Struct by value and variadics.
        assert_eq!(keys("F<0;C900;1;C719?;>"), vec!["S900._p"]);
        assert_eq!(keys("F<0;C703;2;C719,C736<R<C703,C698;>>;>"), vec!["i_p*id"]);
        assert_eq!(keys("F<0;C703;2;C719,C736<C698>;>"), vec!["i_p*d"]);
        // They are not native signatures: a Dart type, a generic one, optional, named.
        assert!(keys("F<0;D;1;C703;>").is_empty());
        assert!(keys("F<1;C703;1;B0;>").is_empty());
        assert!(keys("F<0;C703;0;C703;>").is_empty());
        assert!(keys("F<0;C703;0;;x:C703>").is_empty());
    }

    #[test]
    fn ir_recipes_go_to_summary() {
        let ir = "@\"df.s.1\" = linkonce_odr constant { i64, [24 x i8] } { i64 1, [24 x i8] c\"F<0;C703;1;C719;>\\00\\00\\00\\00\\00\\00\\00\" }\n\
                  @.str.2 = private unnamed_addr constant [9 x i8] c\"F\\3C0\\3BV\\3B0\\3B\\3B\\3E\"\n\
                  define i64 @f() {\n  %a = call i64 @g(ptr @\"df.s.1\", ptr @.str.2)\n  ret i64 %a\n}\n";
        let r = summarize(ir, "m", &[]);
        let strings: Vec<&str> = r.recipes.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(strings, vec!["F<0;C703;1;C719;>", "F<0;V;0;;>"]);
        assert_eq!(r.names[r.recipes[0].0 as usize], "df.s.1");
        assert_eq!(r.names[r.recipes[1].0 as usize], "m#.str.2");
        let again = Summary::from_string(&r.to_text()).unwrap();
        assert_eq!(again.recipes, r.recipes);
    }
}

#[cfg(test)]
mod receiver_cone_tests {
    use super::*;

    #[test]
    fn assembled_table_does_not_reference_an_excluded_adapter() {
        let hash = selector_hash("c:unary-");
        let mut sdk = Summary::new("sdk");
        let caller = sdk.id("sdk.unaryCaller");
        sdk.definitions.push(Definition { name: caller, ..Default::default() });
        sdk.receiver_calls.push((caller, hash, 10));
        sdk.nominal_classes = vec![0, 10];
        sdk.parent_edges.push((10, 0));
        let table = table_line("@df.mt.Unrelated$d = private unnamed_addr constant ",
            20, &[(hash, "@Unrelated.unary$c".into())]);
        let ir = format!("; df.cone-class 0\n; df.cone-class 20\n; df.cone-parent 20 0\n\
            {table}\
            define ptr @df.mt.Unrelated() {{\n ret ptr @df.mt.Unrelated$d\n}}\n\
            define i64 @Unrelated.unary$c(i64 %this, ptr %args, ptr %desc) {{\n ret i64 %this\n}}\n\
            define i32 @main() {{\n %t = call ptr @df.mt.Unrelated()\n\
              call void @sdk.unaryCaller()\n ret i32 0\n}}\n");
        let assembled = assemble(&ir, &[&sdk], true).unwrap();
        assert!(!assembled.ir.contains("define i64 @Unrelated.unary$c"));
        assert!(!assembled.ir.contains("ptr @Unrelated.unary$c"),
            "A selector called for another class cannot resurrect a pruned table entry");
        assert!(assembled.ir.contains("{ i64 20, i64 0 }"));

        // An ordinary unknown receiver must retain the body and table entry.
        sdk.definitions[0].calls.push(hash);
        let dynamic = assemble(&ir, &[&sdk], true).unwrap();
        assert!(dynamic.ir.contains("define i64 @Unrelated.unary$c"));
        assert!(dynamic.ir.contains("ptr @Unrelated.unary$c"));
    }

    fn fixture(cone: i64) -> Summary {
        let mut summary = Summary::new("receiver-cones");
        let main = summary.id("main");
        let selector = selector_hash("g:length");
        let mut refs = Vec::new();
        for cid in [10, 11, 12, 13, 20, 777] {
            let node = summary.id(&format!("table.{cid}"));
            let function = summary.id(&format!("length.{cid}"));
            summary.tables.push(SummarizedTable { node, cid, pairs: vec![(selector, function)] });
            summary.definitions.push(Definition { name: function, ..Default::default() });
            refs.push(node);
        }
        summary.definitions.push(Definition { name: main, refs, calls: Vec::new() });
        summary.receiver_calls.push((main, selector, cone));
        summary.nominal_classes = vec![0, 10, 11, 12, 13, 20, 30];
        // 12 uses a mixin; 13 implements the receiver interface. Both must stay.
        summary.parent_edges = vec![(10, 0), (11, 10), (12, 10), (12, 30), (13, 10), (20, 0), (30, 0)];
        summary
    }

    fn reached(result: &Reach, name: &str) -> bool {
        result.names.iter().position(|n| n == name).is_some_and(|i| result.live(i as u32))
    }

    #[test]
    fn unrelated_nominal_receiver_is_removed_but_subclasses_and_interfaces_stay() {
        let summary = fixture(10);
        let result = reach(&[&summary], true);
        for cid in [10, 11, 12, 13, 777] {
            assert!(reached(&result, &format!("length.{cid}")), "missing {cid}");
        }
        assert!(!reached(&result, "length.20"));
        // A mixin's own `this` also accepts its application class.
        let summary = fixture(30);
        let result = reach(&[&summary], true);
        assert!(reached(&result, "length.12"));
        assert!(!reached(&result, "length.20"));
    }

    #[test]
    fn exact_classes_do_not_expand_to_subclasses_or_unknown_identities() {
        let mut summary = fixture(10);
        let (caller, selector, _) = summary.receiver_calls.pop().unwrap();
        summary.exact_receiver_calls = vec![(caller, selector, 10), (caller, selector, 11)];
        assert_eq!(Summary::from_string(&summary.to_text()).unwrap(), summary);
        let result = reach(&[&summary], true);
        assert!(reached(&result, "length.10"));
        assert!(reached(&result, "length.11"));
        for cid in [12, 13, 20, 777] { assert!(!reached(&result, &format!("length.{cid}"))); }
        // Unknown calls still dominate precise facts and preserve every table.
        summary.definitions.iter_mut().find(|d| d.name == caller).unwrap().calls.push(selector);
        let result = reach(&[&summary], true);
        for cid in [10, 11, 12, 13, 20, 777] { assert!(reached(&result, &format!("length.{cid}"))); }
        for entry in ["E\t999999\t1\t10\n", "E\t0\t1\t-1\n", "E\t0\t1\t4294967296\n"] {
            assert!(Summary::from_string(&format!("{}{entry}", summary.to_text())).is_err());
        }
    }

    #[test]
    fn actual_hir_and_llvm_use_the_same_allocation_receiver_classes() {
        use crate::hir::*;
        let function = Function { symbol: "main".into(), name: "main".into(), params: vec![], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                (ValueId(0), Instruction::AllocList { elements: vec![] }, Type::Ref),
                (ValueId(1), Instruction::CallSelector { selector: "g:length".into(), recv: Operand::Val(ValueId(0)), args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Int(0)) }, Type::Ref),
            ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }], debug_info: None };
        let module = Module { functions: vec![function], ..Module::default() };
        let hir = hir_summary(&module);
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(ir.contains("; df.selector-exact 10"));
        let llvm = summarize(&ir, "programa", &[]);
        let triples = |summary: &Summary| summary.exact_receiver_calls.iter().map(|&(node, selector, cid)| (summary.names[node as usize].clone(), selector, cid)).collect::<Vec<_>>();
        assert_eq!(triples(&hir), triples(&llvm));
        assert_eq!(triples(&hir), vec![("main".into(), selector_hash("g:length"), 10)]);
    }

    #[test]
    fn malformed_or_detached_exact_ir_annotations_remain_unrestricted() {
        let selector = selector_hash("g:length");
        for annotation in ["; df.selector-exact invalid", "; df.selector-exact 10\n  %other = add i64 1, 2"] {
            let ir = format!("define i64 @main() {{\n  {annotation}\n  %f = call ptr @df.selector(ptr %cache, i64 %this, i64 {selector}, ptr @df.seln.0, i64 8)\n  ret i64 0\n}}\n");
            let summary = summarize(&ir, "sdk", &[]);
            assert!(summary.exact_receiver_calls.is_empty());
            assert!(summary.definitions[0].calls.contains(&selector));
        }
    }

    #[test]
    fn unknown_receiver_call_overrides_restricted_calls_without_order_dependence() {
        let mut summary = fixture(10);
        let later = summary.id("unknown-caller");
        summary.definitions.push(Definition { name: later, refs: Vec::new(), calls: vec![selector_hash("g:length")] });
        summary.definitions.iter_mut().find(|d| summary.names[d.name as usize] == "main").unwrap().refs.push(later);
        for reverse in [false, true] {
            if reverse { summary.definitions.reverse(); }
            let result = reach(&[&summary], true);
            assert!(reached(&result, "length.20"));
            let evidence = report(&result);
            assert!(evidence.lines().find(|line| line.starts_with("length.20\t"))
                .unwrap().ends_with("\tunknown-caller"));
        }
    }

    #[test]
    fn incomplete_ancestry_and_legacy_summaries_are_conservative() {
        let mut summary = fixture(10);
        summary.parent_edges.push((20, 888));
        assert!(reached(&reach(&[&summary], true), "length.20"));
        summary.parent_edges.retain(|&(child, _)| child != 20);
        summary.parent_edges.push((20, 0));
        summary.ambiguous_classes.push(0);
        assert!(reached(&reach(&[&summary], true), "length.20"));
        summary.nominal_classes.clear();
        assert!(reached(&reach(&[&summary], true), "length.20"));
    }

    #[test]
    fn receiver_metadata_round_trips_and_rejects_invalid_definition_indices() {
        let summary = fixture(10);
        assert_eq!(Summary::from_string(&summary.to_text()).unwrap(), summary);
        let invalid = format!("{}C\t999999\t1\t10\n", summary.to_text());
        assert!(Summary::from_string(&invalid).is_err());
    }

    #[test]
    fn ir_comment_scopes_only_the_immediately_following_dispatch() {
        let ir = "; df.cone-class 10\n; df.cone-parent 10 0\n\
                  define i64 @main(i64 %this) {\n\
                    ; df.selector-cone 10\n\
                    %f = call ptr @df.selector(ptr %cache, i64 %this, i64 HASH, ptr @df.seln.0, i64 8)\n\
                    ; df.selector-cone 20\n\
                    %unrelated = add i64 1, 2\n\
                    %g = call ptr @df.selector(ptr %cache, i64 %other, i64 HASH, ptr @df.seln.0, i64 8)\n\
                    ret i64 0\n}\n\
                  @df.seln.0 = private unnamed_addr constant [8 x i8] c\"g:length\"\n"
            .replace("HASH", &selector_hash("g:length").to_string());
        let summary = summarize(&ir, "sdk", &[]);
        assert_eq!(summary.receiver_calls.len(), 1);
        assert_eq!(summary.receiver_calls[0].2, 10);
        assert!(summary.definitions[0].calls.contains(&selector_hash("g:length")));
        assert_eq!(summary.nominal_classes, vec![10]);
        assert_eq!(summary.parent_edges, vec![(10, 0)]);
    }

    #[test]
    fn actual_sdk_getter_adapter_keeps_its_formal_receiver_cone() {
        // Check the actual SDK summary, including both uniform getter adapters.
        // Whole programs have other unknown g:length sites, so exclusion here
        // concerns this proven call only, not every call in the SDK graph.
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let sdk = crate::sdk_module::compiled_sdk_in_profile(crate::sdk_tests(),
                &crate::driver::NativeDriverOptions::default().clang,
                crate::sdk_module::SdkProfile::Production).unwrap();
            let summaries = read_summaries(&sdk.summaries).unwrap();
            let core = summaries.iter().find(|s| s.module == "dart:core").unwrap();
            let hash = selector_hash("g:length");
            let mut expected_cone = None;
            for suffix in ["", "$g", "$c"] {
                let symbol = format!("df.dart$3acore.StringBuffer.isEmpty{suffix}");
                let node = core.names.iter().position(|name| name == &symbol).unwrap() as u32;
                let definition = core.definitions.iter().find(|d| d.name == node).unwrap();
                assert!(!definition.calls.contains(&hash), "unrestricted getter: {symbol}");
                let call = core.receiver_calls.iter().find(|&&(n, h, _)| n == node && h == hash)
                    .expect("SDK getter must preserve its actual formal receiver");
                if let Some(cone) = expected_cone { assert_eq!(call.2, cone); }
                expected_cone = Some(call.2);
                assert!(core.nominal_classes.contains(&call.2));

                // Project the real call into an otherwise controlled graph:
                // its class/subclasses stay, an unrelated closed class does not,
                // and an unknown runtime identity remains conservative.
                let mut controlled = fixture(call.2);
                for cid in &mut controlled.nominal_classes { if *cid == 10 { *cid = call.2; } }
                for (child, parent) in &mut controlled.parent_edges {
                    if *child == 10 { *child = call.2; }
                    if *parent == 10 { *parent = call.2; }
                }
                controlled.tables.iter_mut().find(|t| t.cid == 10).unwrap().cid = call.2;
                let result = reach(&[&controlled], true);
                assert!(reached(&result, "length.10"));
                assert!(reached(&result, "length.11"));
                assert!(reached(&result, "length.777"));
                assert!(!reached(&result, "length.20"));
            }
        }).unwrap().join().unwrap();
    }
}

#[cfg(test)]
mod declared_field_summary_tests {
    use super::*;
    use crate::hir::*;

    fn module() -> Module {
        let declaration = FieldDeclaration { library_uri: "dart:convert".into(), owner: "Base64Codec".into(),
            name: "_encoder".into(), is_final: true, is_late: false, layout_index: 0, storage_type: Type::Ref };
        let field = Instruction::CallRuntime { name: "dartforge_object_get".into(),
            args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                (Operand::Constant(Constant::Int(0)), Type::I64)], ret_ty: Type::Ref }
            .with_field_declaration(Some(declaration.clone()));
        Module { functions: vec![Function { symbol: "getter".into(), name: "getter".into(),
            params: vec![(ValueId(0), "this".into(), Type::Ref)], return_ty: Type::Ref, debug_info: None,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![(ValueId(1), field, Type::Ref)],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }],
        }], declared_field_layouts: HashMap::from([(128, vec![Some(declaration)])]),
            classes: vec![ClassDef { id: 128, name: "Base64Codec".into(), field_count: 1,
                vtable: vec![], to_string_symbol: None }], ..Module::default() }
    }

    #[test]
    fn hir_field_observation_survives_disk_summary_roundtrip() {
        let module = module();
        let before_ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        let summary = hir_summary(&module);
        assert_eq!(summary.declared_field_accesses.len(), 1);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("convert.poda");
        std::fs::write(&path, summary.to_text()).unwrap();
        let parsed = Summary::from_string(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(summary, parsed);
        let access = &parsed.declared_field_accesses[0];
        assert_eq!(parsed.names[access.definition as usize], "getter");
        assert_eq!(access.value_id, 1);
        assert_eq!(access.declaration.identity(), ("dart:convert", "Base64Codec", "_encoder"));
        assert_eq!(access.declaration.storage_type, Type::Ref);
        assert_eq!(before_ir, crate::llvm::LlvmEmitter::new(&module).emit_all());
    }

    #[test]
    fn persistent_field_facts_refuse_corruption_and_old_schema() {
        let summary = hir_summary(&module()).to_text();
        assert!(Summary::from_string(&summary.replace("dartforge-poda\t6", "dartforge-poda\t5")).is_err());
        assert!(Summary::from_string(&summary.replace("\"storage\":\"ref\"", "\"storage\":\"ptr\"")).is_err());
        let record = summary.lines().find(|line| line.starts_with("F\t")).unwrap();
        assert!(Summary::from_string(&format!("{summary}{record}\n")).is_err());
        let layout = summary.lines().find(|line| line.starts_with("L\t")).unwrap();
        assert!(Summary::from_string(&format!("{summary}{layout}\n")).is_err());
        assert!(Summary::from_string(&summary.replace("L\t128\t", "L\t0\t")).is_err());
        assert!(Summary::from_string(&summary.replace("\"slot\":0", "\"slot\":1")).is_err());
        let mut ambiguous = hir_summary(&module());
        ambiguous.ambiguous_classes.push(128);
        assert!(Summary::from_string(&ambiguous.to_text()).is_err());
    }
    #[test]
    fn actual_sdk_base64_declarations_survive_summary_roundtrip() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            use dartforge_types::table::{CoreTypes, TypeTable};
            let directory = tempfile::tempdir().unwrap();
            let entry = directory.path().join("main.dart");
            std::fs::write(&entry, "import 'dart:convert'; void main() { print(base64.encode([65])); }").unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = dartforge_intern::Interner::new();
            let (program, diagnostics) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diagnostics.is_empty());
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let libraries: Vec<_> = program.libraries.iter().enumerate()
                .filter(|(_, library)| matches!(library.uri.as_str(), "dart:core" | "dart:convert"))
                .map(|(index, _)| dartforge_elements::model::LibraryId(index as u32)).collect();
            let (bodies, _) = dartforge_types::infer_library_bodies(&program, &interner, &mut table,
                &core, &mut outline, &libraries);
            let convert = libraries.iter().copied().find(|id| program.library(*id).uri == "dart:convert").unwrap();
            let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
                .with_source_sdk().only_the_library(convert);
            let module = crate::lower::lower_program(&context);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            let summary = hir_summary(&module);
            let fields: Vec<_> = summary.declared_field_accesses.iter()
                .filter(|field| field.declaration.identity() == ("dart:convert", "Base64Codec", "_encoder"))
                .collect();
            assert!(fields.iter().any(|field| field.write));
            assert!(fields.iter().any(|field| !field.write));
            assert!(fields.iter().all(|field| field.declaration.is_final
                && !field.declaration.is_late && field.declaration.layout_index == 0
                && field.declaration.storage_type == Type::Ref));
            let path = directory.path().join("convert.poda");
            std::fs::write(&path, summary.to_text()).unwrap();
            let parsed = Summary::from_string(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(summary.declared_field_accesses, parsed.declared_field_accesses);
            assert_eq!(summary.declared_field_layouts, parsed.declared_field_layouts);
            let class = module.classes.iter().find(|class| class.name == "Base64Codec").unwrap();
            let layout = parsed.declared_field_layouts.get(&class.id).unwrap();
            assert_eq!(layout.len(), class.field_count);
            assert_eq!(layout[0].as_ref().unwrap().identity(), ("dart:convert", "Base64Codec", "_encoder"));
        }).unwrap().join().unwrap();
    }

}
