//! Verifier of the emitted text against the closed world
//! (`docs/JS-PRODUCAO.md` §1.7, §5): the pruned bundle is checked **by the
//! text**, which is the truth of what the emitter wrote.
//!
//! 1. **No pending reference**: every `L$x.Name`, static `L$x.Class.member`,
//!    `L$x['k']` and every cited rti recipe `"x|Name"` must be defined in the
//!    text.
//! 2. **No forgotten selector**: every name the text uses as a member
//!    (`.name`, `[$name]`, `dart.dsend(o, "name")`) and that is a pruned
//!    member of an instantiated class.
//!
//! What is missing comes back as [`Missing`], already translated into
//! elements. The caller adds it to the roots and recomputes the world: the
//! final world is a fixed point of the world **and** of the text. Something
//! missing here is a gap in the analysis (recorded in the report), not in
//! the program — and it never silently turns into broken code.

use crate::scan;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, LibraryId, Program, VariableId};
use dartforge_intern::Interner;
use dartforge_world::{Mundo, ClassLevel};
use std::collections::{HashMap, HashSet};

#[derive(Default, Debug)]
pub struct Missing {
    pub functions: Vec<FunctionElementId>,
    pub variables: Vec<VariableId>,
    pub classes: Vec<ClassId>,
    /// Classes without a declared constructor whose `new` the text calls.
    pub instantiated_classes: Vec<ClassId>,
    pub tearoffs: Vec<FunctionElementId>,
    pub selectors: Vec<String>,
    /// Pending references that correspond to no element at all.
    pub no_element: Vec<String>,
    /// Readable text of what was missing, for the report.
    pub description: Vec<String>,
}

impl Missing {
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty() && self.variables.is_empty() && self.classes.is_empty() && self.instantiated_classes.is_empty() && self.tearoffs.is_empty() && self.selectors.is_empty()
    }
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

fn ident_end(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && is_ident(b[j]) {
        j += 1;
    }
    j
}

/// Key of an object/class body entry: `name(`, `get name()`,
/// `static name(`, `['k'](`, `"k":`.
fn entry_key(t: &str) -> Option<(bool, String)> {
    let mut t = t.trim_start();
    // Position comment before the entry.
    while let Some(r) = t.strip_prefix("/*") {
        t = r.split_once("*/").map(|(_, x)| x.trim_start()).unwrap_or("");
    }
    let (is_static, t) = match t.strip_prefix("static ") {
        Some(r) => (true, r.trim_start()),
        None => (false, t),
    };
    let t = t.strip_prefix("get ").or_else(|| t.strip_prefix("set ")).map(str::trim_start).unwrap_or(t);
    let t = t.strip_prefix('*').map(str::trim_start).unwrap_or(t);
    let b = t.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'[' && b.len() > 1 && (b[1] == b'"' || b[1] == b'\'') {
        let quote = b[1];
        let end = t[2..].find(quote as char)? + 2;
        return Some((is_static, t[2..end].to_string()));
    }
    if b[0] == b'"' || b[0] == b'\'' {
        let end = t[1..].find(b[0] as char)? + 1;
        return Some((is_static, t[1..end].to_string()));
    }
    let f = ident_end(b, 0);
    (f > 0).then(|| (is_static, t[..f].to_string()))
}

/// `L$x` at the start of `t` (after spaces): the identifier and where it
/// ends.
fn namespace<'t>(t: &'t str, idents: &HashSet<&str>) -> Option<(&'t str, usize)> {
    let f = ident_end(t.as_bytes(), 0);
    (f > 0 && idents.contains(&t[..f])).then(|| (&t[..f], f))
}

struct Definitions {
    names: HashSet<String>,
    classes: HashSet<String>,
}

fn definitions(modules: &[(String, String)], idents: &HashSet<&str>) -> Definitions {
    let mut d = Definitions { names: HashSet::new(), classes: HashSet::new() };
    for (_, text) in modules {
        // Every top-level statement the emitter writes starts at column 0:
        // reading by line start is robust to JS that the character scan does
        // not understand (regexes and *template literals* of the runtime's
        // `JS()`). The end of each span comes from brace matching.
        let mut starts: Vec<usize> = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        for start in starts {
            if start >= text.len() || matches!(text.as_bytes()[start], b' ' | b'\t' | b'\n' | b'\r' | b'}') {
                continue;
            }
            let t = &text[start..];
            // `dart.defineLazy(ns, {…})` and `dart.copyProperties(ns.C, {…})`.
            let mut handled = false;
            for pre in ["dart.copyProperties(", "dart.defineLazy("] {
                let Some(r) = t.strip_prefix(pre) else { continue };
                handled = true;
                let Some(comma) = r.find(',') else { continue };
                let target = r[..comma].trim();
                // The target is a namespace (`core`) or a class (`core.C`, its
                // lazy statics).
                let ns = target.split('.').next().unwrap_or(target);
                if !idents.contains(ns) || target.contains('(') {
                    continue;
                }
                let Some(rel) = r.find('{') else { continue };
                let open = t.len() - r.len() + rel;
                let Some(close) = scan::closing_brace(t, open) else { continue };
                let body = &t[open + 1..close];
                for (a, z) in scan::object_entries(body) {
                    if let Some((_, k)) = entry_key(&body[a..z]) {
                        d.names.insert(format!("{target}.{k}"));
                    }
                }
            }
            if handled {
                continue;
            }
            // `(L$x.C.n = function …).prototype = …` and `(L$x.C[dart.mixinNew] = …)`.
            // Only a declaration counts (`=` after the name): `dart.f(…)` is a
            // call.
            let assigns = |s: &str| s.starts_with('=') && !s.starts_with("==");
            let t2 = t.strip_prefix('(').unwrap_or(t);
            if let Some((ns, f)) = namespace(t2, idents) {
                let b = t2.as_bytes();
                if b.get(f) == Some(&b'.') {
                    let g = ident_end(b, f + 1);
                    let name = format!("{ns}.{}", &t2[f + 1..g]);
                    let rest = t2[g..].trim_start();
                    if let Some(r) = rest.strip_prefix('.') {
                        let h = ident_end(r.as_bytes(), 0);
                        if assigns(r[h..].trim_start()) {
                            d.names.insert(format!("{name}.{}", &r[..h]));
                            d.names.insert(name);
                        }
                    } else if rest.starts_with("= class ") {
                        d.classes.insert(name.clone());
                        // Static members in the class body.
                        if let Some(rel) = t2.find('{') {
                            let open = rel;
                            if let Some(close) = scan::closing_brace(t2, open) {
                                let body = &t2[open + 1..close];
                                for (a, z) in scan::class_entries(body) {
                                    if let Some((true, k)) = entry_key(&body[a..z]) {
                                        d.names.insert(format!("{name}.{k}"));
                                    }
                                }
                            }
                        }
                        d.names.insert(name);
                    } else if assigns(rest) || rest.starts_with('[') {
                        d.names.insert(name);
                    }
                } else if b.get(f) == Some(&b'[') {
                    if let Some((_, k)) = entry_key(&t2[f..]) {
                        d.names.insert(format!("{ns}.{k}"));
                    }
                }
            }
        }
    }
    d
}

/// References to namespaces (`L$x.Name`, `core.Name`, `dart.f`) and
/// `ident|Name` recipes in the text. The content of *strings* and comments
/// is not a reference (except the recipes, which live inside *strings*).
fn references(text: &str, idents: &HashSet<&str>, recipes: &HashMap<&str, &str>, ids: &HashMap<String, String>, classes: &HashSet<String>, out: &mut HashSet<String>) {
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' | b'`' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {}
            _ => {
                i += 1;
                continue;
            }
        }
        let s = i;
        let f = ident_end(b, s);
        i = f;
        if s > 0 && (is_ident(b[s - 1]) || b[s - 1] == b'.') {
            continue;
        }
        let ns = &text[s..f];
        if !idents.contains(ns) {
            continue;
        }
        let name = match b.get(f) {
            Some(b'.') => {
                let g = ident_end(b, f + 1);
                if g == f + 1 {
                    continue;
                }
                i = g;
                text[f + 1..g].to_string()
            }
            Some(b'[') => {
                if let Some((_, k)) = entry_key(&text[f..]) {
                    out.insert(format!("{ns}.{k}"));
                }
                continue;
            }
            _ => continue,
        };
        if name == "$constCache" || name == "$C" {
            continue;
        }
        let q = format!("{ns}.{name}");
        // Static class member: `L$x.C.m` or `L$x.C["_#new#tearOff"]`.
        if classes.contains(&q) {
            match b.get(i) {
                Some(b'.') => {
                    let h = ident_end(b, i + 1);
                    let m = &text[i + 1..h];
                    if !m.is_empty() && m != "prototype" {
                        out.insert(format!("{q}.{m}"));
                    }
                }
                Some(b'[') => {
                    if let Some((_, k)) = entry_key(&text[i..]) {
                        out.insert(format!("{q}.{k}"));
                    }
                }
                _ => {}
            }
        }
        out.insert(q);
    }
    // rti recipes: `ident|Name` (inside strings).
    for (p, _) in text.match_indices('|') {
        if p == 0 || p + 1 >= b.len() || !(b[p + 1].is_ascii_alphabetic() || b[p + 1] == b'_' || b[p + 1] == b'$') {
            continue;
        }
        let mut start = p;
        while start > 0 && is_ident(b[start - 1]) {
            start -= 1;
        }
        let end = ident_end(b, p + 1);
        // `|id`: the class's short id in SDK mode (`Ctx::class_recipe`).
        if start == p {
            if let Some(r) = ids.get(&text[p + 1..end]) {
                out.insert(r.clone());
            }
            continue;
        }
        let Some(var) = recipes.get(&text[start..p]) else { continue };
        out.insert(format!("{var}.{}", &text[p + 1..end]));
    }
}

/// Kind of a member use in the text: read/call (`.w`, `.w()`), write
/// (`.w = v`) or both (`.w++`, `.w += v`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
    Both,
}

/// Classifies what comes after `.name` (or `[$name]`, starting after the
/// `]`): a plain `=` is a write; `++`/`--` and compound assignments (`+=`,
/// `<<=`, `??=`…) are read+write; everything else — including `==`/`===`
/// and `=>` — is a read.
fn access_after(b: &[u8], f: usize) -> Access {
    let mut i = f;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    if b.get(i) == Some(&b'+') && b.get(i + 1) == Some(&b'+') {
        return Access::Both;
    }
    if b.get(i) == Some(&b'-') && b.get(i + 1) == Some(&b'-') {
        return Access::Both;
    }
    let mut j = i;
    if b.get(j) == Some(&b'?') && b.get(j + 1) == Some(&b'?') {
        j += 2;
    } else {
        while matches!(b.get(j).copied(), Some(b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'<' | b'>')) {
            j += 1;
        }
    }
    if b.get(j) != Some(&b'=') {
        return Access::Read;
    }
    if j == i {
        if b.get(j + 1) == Some(&b'=') || b.get(j + 1) == Some(&b'>') {
            return Access::Read;
        }
        return Access::Write;
    }
    Access::Both
}

/// Names used as members in the text, by kind: `.name`, `[$name]` and the
/// strings of `dsend`/`dload`/`dput`/`bind` (`dput` is a write, the rest are
/// reads).
fn text_selectors(text: &str, reads: &mut HashSet<String>, writes: &mut HashSet<String>) {
    let b = text.as_bytes();
    let mut i = 0usize;
    let record = |name: &str, e: Access, reads: &mut HashSet<String>, writes: &mut HashSet<String>| {
        match e {
            Access::Read => {
                reads.insert(name.to_string());
            }
            Access::Write => {
                writes.insert(name.to_string());
            }
            Access::Both => {
                reads.insert(name.to_string());
                writes.insert(name.to_string());
            }
        }
    };
    // Private symbol variables of the prelude (`var _eval =
    // dart.privateName(dart_rti, "_eval");`): `x[_eval]` uses the member
    // `_eval`.
    let mut privates: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let Some(r) = line.strip_prefix("var ") else { continue };
        let Some((var, rest)) = r.split_once(" = dart.privateName(") else { continue };
        let Some(q) = rest.find('"') else { continue };
        let rest = &rest[q + 1..];
        let Some(end) = rest.find('"') else { continue };
        privates.insert(var, &rest[..end]);
    }
    while i < b.len() {
        match b[i] {
            // String content is not a member access (`dart.privateName(L,
            // "C.field")`); the names passed as data to `dsend`/`dload` come
            // from `dynamic_selectors`, below.
            b'"' | b'\'' | b'`' => {
                let quote = b[i];
                i += 1;
                while i < b.len() && b[i] != quote {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'.' if i + 1 < b.len() && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_' || b[i + 1] == b'$') => {
                let f = ident_end(b, i + 1);
                // `opts.name` is the read of a named parameter in the emitted
                // function's prologue, not a member access.
                if !(i >= 4 && &b[i - 4..i] == b"opts" && (i == 4 || !is_ident(b[i - 5]))) {
                    let e = access_after(b, f);
                    record(&text[i + 1..f], e, reads, writes);
                }
                i = f;
            }
            // The program's private name in the production profile:
            // `x["_n$1c"]` (`dartforge_emit_js::production_private_name`).
            b'[' if i + 2 < b.len() && b[i + 1] == b'"' && b[i + 2] == b'_' => {
                let start = i + 2;
                let mut f = start;
                while f < b.len() && b[f] != b'"' && b[f] != b'\n' {
                    f += 1;
                }
                if b.get(f + 1) == Some(&b']') {
                    if let Some(n) = dartforge_emit_js::dart_name_of_private(&text[start..f]) {
                        let e = access_after(b, f + 2);
                        record(n, e, reads, writes);
                    }
                }
                i = f + 1;
            }
            b'[' if i + 2 < b.len() && (b[i + 1] == b'$' || b[i + 1] == b'_' || b[i + 1].is_ascii_alphabetic()) => {
                let f = ident_end(b, i + 1);
                if b.get(f) == Some(&b']') {
                    let var = &text[i + 1..f];
                    let e = access_after(b, f + 1);
                    if let Some(n) = privates.get(var) {
                        record(n, e, reads, writes);
                    } else if let Some(n) = var.strip_prefix('$') {
                        record(n, e, reads, writes);
                    }
                }
                i = f.max(i + 1);
            }
            _ => i += 1,
        }
    }
    let (dyn_reads, dyn_writes) = crate::sdk::dynamic_selectors_by_kind(text);
    reads.extend(dyn_reads);
    writes.extend(dyn_writes);
}

/// JS identifier → library, read from the `dart.trackLibraries("…", {"uri":
/// L$ident, …})` that each module emits — without building a `Ctx` just for
/// this.
pub fn libraries_from_text(modules: &[(String, String)], program: &Program) -> HashMap<String, LibraryId> {
    let by_uri: HashMap<&str, LibraryId> = program.libraries.iter().enumerate().map(|(i, l)| (l.uri.as_str(), LibraryId(i as u32))).collect();
    let mut out = HashMap::new();
    for (_, t) in modules {
        let Some(p) = t.find("dart.trackLibraries(") else { continue };
        let Some(rel) = t[p..].find('{') else { continue };
        let open = p + rel;
        let Some(close) = scan::closing_brace(t, open) else { continue };
        for line in t[open + 1..close].split(",\n") {
            let line = line.trim();
            let Some((uri, var)) = line.rsplit_once(": ") else { continue };
            let uri = uri.trim().trim_matches('"');
            let var = var.trim();
            if !var.starts_with("L$") {
                continue;
            }
            if let Some(&l) = by_uri.get(uri) {
                out.insert(var.to_string(), l);
            }
        }
    }
    out
}

/// Checks the emitted text against the world and returns what is missing.
pub fn check(modules: &[(String, String)], libs: &HashMap<String, LibraryId>, program: &Program, interner: &Interner, world: &Mundo) -> Missing {
    let idents: HashSet<&str> = libs.keys().map(String::as_str).collect();
    let defs = definitions(modules, &idents);
    // rti recipe (`main|D`, `_rti|Rti`) → namespace variable.
    let mut by_recipe: HashMap<String, &str> = HashMap::new();
    for (var, l) in libs {
        let lib = program.library(*l);
        let ident = match lib.uri.strip_prefix("dart:") {
            Some(n) => n.to_string(),
            None => var.strip_prefix("L$").unwrap_or(var).to_string(),
        };
        by_recipe.insert(ident, var.as_str());
        // The production profile writes the program's library with the
        // short label (`dartforge_emit_js::recipe_tag`).
        if !lib.uri.starts_with("dart:") {
            by_recipe.insert(dartforge_emit_js::recipe_tag(*l), var.as_str());
        }
    }
    let recipes: HashMap<&str, &str> = by_recipe.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    // Short recipe id (`|id`) → `var.Class`.
    let lib_var: HashMap<LibraryId, &str> = libs.iter().map(|(v, l)| (*l, v.as_str())).collect();
    let ids: HashMap<String, String> = dartforge_emit_js::recipe_ids(program, interner)
        .into_iter()
        .enumerate()
        .filter_map(|(i, id)| {
            let c = program.class(ClassId(i as u32));
            lib_var.get(&c.library).map(|v| (id, format!("{v}.{}", interner.resolve(c.name))))
        })
        .collect();
    let mut refs: HashSet<String> = HashSet::new();
    for (_, t) in modules {
        references(t, &idents, &recipes, &ids, &defs.classes, &mut refs);
    }
    let mut missing = Missing::default();
    let mut pending: Vec<&String> = refs.iter().filter(|r| !defs.names.contains(*r)).collect();
    pending.sort();
    for r in pending {
        if !resolve_reference(r, libs, program, interner, &mut missing) {
            missing.no_element.push(r.clone());
        }
    }
    // Text selectors that are pruned members of instantiated classes, by
    // kind: a read fixes a getter/method, a write fixes a setter. A read
    // never revives a setter — that is the precision the world works out.
    // The fix is per (type, selector): if the name only lives restricted to
    // the cones of other classes, the pruning here is the design of the
    // restriction by receiver — and stub mode (`--verificar-stub`, exit 97)
    // reports it if it is wrong at run time. Only what the world says is
    // live everywhere (a gap: it should be live and is not) or what nobody
    // registered (a use the analysis did not see) comes back as an
    // unrestricted root.
    let mut sel_r: HashSet<String> = HashSet::new();
    let mut sel_w: HashSet<String> = HashSet::new();
    for (_, t) in modules {
        text_selectors(t, &mut sel_r, &mut sel_w);
    }
    let mut new: HashSet<String> = HashSet::new();
    for (i, c) in program.classes.iter().enumerate() {
        if (program.library(c.library).is_sdk && !world.include_sdk) || world.classe(ClassId(i as u32)) != ClassLevel::Instantiated {
            continue;
        }
        for (&key, &f) in c.instance_members.iter() {
            let func = program.function(f);
            if world.funcao(f) || func.abstract_ {
                continue;
            }
            // The key carries the kind (`foo_=` is a write); `==` does not
            // end in `_=`, so the cut is safe.
            let k = interner.resolve(key);
            let (base, write) = match k.strip_suffix("_=") {
                Some(b) => (b, true),
                None => (k, false),
            };
            let js = dartforge_emit_js::body::js_member_name(base);
            let used = if write {
                sel_w.contains(base) || sel_w.contains(js.as_str())
            } else {
                sel_r.contains(base) || sel_r.contains(js.as_str())
            };
            if !used {
                continue;
            }
            let name = if write { format!("{base}_=") } else { base.to_string() };
            // Fix per (type, selector): the member should be live in the
            // class's cone, or nobody registered the name (a use the analysis
            // did not see) — a gap, it comes back as an unrestricted root. A
            // name only restricted to the cones of other classes is pruning
            // by design, not a fix.
            if world.selector_live_for(program, &name, ClassId(i as u32)) || !world.has_restriction(&name) {
                new.insert(name);
            }
        }
    }
    let mut new: Vec<String> = new.into_iter().collect();
    new.sort();
    for n in &new {
        missing.description.push(format!("selector `{n}` used in the text and pruned in an instantiated class"));
    }
    missing.selectors = new;
    missing
}

/// `L$x.Name` / `L$x.C.m` → element.
fn resolve_reference(r: &str, libs: &HashMap<String, LibraryId>, program: &Program, interner: &Interner, f: &mut Missing) -> bool {
    let Some((ident, names)) = r.split_once('.') else { return false };
    let Some(&lib) = libs.get(ident) else { return false };
    let (name, member) = match names.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (names, None),
    };
    // The SDK's `@JSExportName`: `dart.throw` is `throw_`, `dart.notNull` is
    // `_notNull`.
    let declared = |n: &str| interner.lookup(n).and_then(|s| program.library(lib).declared.get(&s));
    let Some(b) = declared(name).or_else(|| declared(&format!("{name}_"))).or_else(|| declared(&format!("_{name}"))) else { return false };
    let mut ok = false;
    for el in [b.getter, b.setter].into_iter().flatten() {
        // A non-interop extension type: the emitter never writes the class
        // (only the recipe, in the subtypes' rti rules).
        if let Element::Class(c) = el {
            if program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
                ok = true;
                continue;
            }
        }
        match (el, member) {
            (Element::Class(c), None) => {
                f.classes.push(c);
                f.description.push(format!("class {r} cited in the text and pruned"));
                ok = true;
            }
            (Element::Class(c), Some(m)) => {
                let class = program.class(c);
                let (tearoff, m) = match m.strip_prefix("_#").and_then(|x| x.strip_suffix("#tearOff")) {
                    Some(x) => (true, x),
                    None => (false, m),
                };
                let ctor_name = if m == "new" { "" } else { m };
                let mut found = None;
                for cand in [m, m.strip_suffix('_').unwrap_or(m)] {
                    if let Some(s) = interner.lookup(cand) {
                        if let Some(&x) = class.static_members.get(&s) {
                            found = Some(x);
                            break;
                        }
                    }
                }
                if found.is_none() {
                    for cand in [ctor_name, ctor_name.strip_suffix('_').unwrap_or(ctor_name)] {
                        if let Some(s) = interner.lookup(cand) {
                            if let Some(&x) = class.constructors.get(&s) {
                                found = Some(x);
                                break;
                            }
                        }
                    }
                }
                if found.is_none() && ctor_name.is_empty() && class.constructors.is_empty() {
                    f.instantiated_classes.push(c);
                    f.description.push(format!("{r} (class without a declared constructor) cited in the text and pruned"));
                    ok = true;
                }
                if let Some(x) = found {
                    if tearoff {
                        f.tearoffs.push(x);
                    } else {
                        f.functions.push(x);
                    }
                    f.description.push(format!("{r} cited in the text and pruned"));
                    ok = true;
                }
            }
            (Element::Function(x), _) => {
                f.functions.push(x);
                f.description.push(format!("function {r} cited in the text and pruned"));
                ok = true;
            }
            (Element::Variable(v), _) => {
                f.variables.push(v);
                f.description.push(format!("variable {r} cited in the text and pruned"));
                ok = true;
            }
            _ => {}
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_class_and_lazy_definitions() {
        let m = vec![(
            "a.js".to_string(),
            concat!(
                "L$a.C = class C extends core.Object {\n  static f() { return 1; }\n  static ['_#new#tearOff'](...args) { return new L$a.C.new(...args); }\n  g() {}\n};\n",
                "(L$a.C.new = function() {\n}).prototype = L$a.C.prototype;\n",
                "dart.defineLazy(L$a, {\n  get x() {\n    return 1;\n  },\n  get y() {\n    return 2;\n  }\n});\n",
                "L$a.h = function h() { return L$a.C.f() + L$a.x + L$a.z; };\n",
            )
            .to_string(),
        )];
        let idents: HashSet<&str> = ["L$a"].into_iter().collect();
        let d = definitions(&m, &idents);
        for n in ["L$a.C", "L$a.C.f", "L$a.C._#new#tearOff", "L$a.C.new", "L$a.x", "L$a.y", "L$a.h"] {
            assert!(d.names.contains(n), "{n}: {:?}", d.names);
        }
        assert!(!d.names.contains("L$a.C.g"), "an instance member is not a static definition");
        let recipes: HashMap<&str, &str> = [("a", "L$a")].into_iter().collect();
        let mut r = HashSet::new();
        references(&m[0].1, &idents, &recipes, &HashMap::new(), &d.classes, &mut r);
        let pend: Vec<&String> = r.iter().filter(|x| !d.names.contains(*x)).collect();
        assert_eq!(pend, vec!["L$a.z"]);
    }

    #[test]
    fn rti_recipe_is_a_reference() {
        let idents: HashSet<&str> = ["L$main"].into_iter().collect();
        let recipes: HashMap<&str, &str> = [("main", "L$main")].into_iter().collect();
        let mut r = HashSet::new();
        references("x[_is](dart_rti._Universe.eval(u, \"main|D<core|int>\"))", &idents, &recipes, &HashMap::new(), &HashSet::new(), &mut r);
        assert!(r.contains("L$main.D"), "{r:?}");
        assert!(!r.iter().any(|x| x.contains("core")));
    }

    #[test]
    fn selectors_from_text() {
        let mut r = HashSet::new();
        let mut w = HashSet::new();
        text_selectors("a.foo(); b[$bar]; dart.dsend(o, \"baz\", []); q.qux = 1; r.cor += 2; s.ok == 3;", &mut r, &mut w);
        assert!(r.contains("foo") && r.contains("bar") && r.contains("baz"), "{r:?} {w:?}");
        assert!(!r.contains("qux") && w.contains("qux"), "{r:?} {w:?}");
        assert!(r.contains("cor") && w.contains("cor"), "{r:?} {w:?}");
        assert!(r.contains("ok") && !w.contains("ok"), "{r:?} {w:?}");
    }
}
