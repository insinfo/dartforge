//! Short keys for named parameters (`docs/JS-PRODUCAO-TAMANHO.md` §8.2).
//!
//! In the DDC contract a named argument is the key of an object
//! (`f(1, {limite: 2})`), read in the function's prologue (`opts && "limite"
//! in opts ? opts.limite : …`), and the name is also in the rti recipe of the
//! function's type (`~(core|int{limite:core|int})`), which the dynamic call
//! check (`_checkAndCall`) compares with the object's keys. So the name cannot
//! go through `oxc`'s property renaming: the recipe is a *string*.
//!
//! Here the emitter replaces the name with a short key (`$a`, `$bK`) from a
//! **global, per-name** map, applied at the same time in the call, in the
//! prologue and in the recipe (`Ctx::named_key`). Since it is a function of the
//! name alone, caller and callee agree without knowing about each other,
//! dynamic calls included.
//!
//! These keep the source name:
//! * the named parameters of `external` functions and of interop classes
//!   (they become JavaScript object keys);
//! * the words of the live `JS()` *templates*;
//! * **everything**, if the program has its own live `noSuchMethod` (the
//!   `Invocation` of a dynamic call would show the keys).
//!
//! `Function.apply` builds the keys from the name's `Symbol`: with it live,
//! the map also goes to the runtime (`dart.nomeadoJS`).
//!
//! What visibly changes: the `toString` of a function type with named
//! parameters shows the short key.

use dartforge_elements::model::{ClassKind, Element, FunctionElementId};
use dartforge_types::table::{Type, TypeId};
use std::collections::{BTreeSet, HashMap, HashSet};

/// The `n`-th key suffix: one letter out of 54, then the following ones out
/// of 64 (the numbering of recipe ids, `dartforge_emit_js::id_number`).
fn suffix(mut n: usize) -> String {
    let (a, b) = (dartforge_emit_js::ID_FIRST, dartforge_emit_js::ID_NEXT);
    let mut s = vec![a[n % a.len()]];
    n /= a.len();
    while n > 0 {
        n -= 1;
        s.push(b[n % b.len()]);
        n /= b.len();
    }
    String::from_utf8(s).unwrap_or_default()
}

/// The name → short key map, or `None` when the program needs the source
/// names at run time.
pub fn compute(a: &dartforge_emit_js::Analysis<'_>, world: &dartforge_world::World) -> Option<NamedArgs> {
    let p = a.program;
    let i = a.interner;
    let mut table = false;
    // `Function.apply` live.
    if let (Some(core), Some(fsym), Some(asym)) = (p.core, i.lookup("Function"), i.lookup("apply"))
        && let Some(Element::Class(fc)) = p.library(core).declared.get(&fsym).and_then(|b| b.getter)
            && p.class(fc).static_members.get(&asym).is_some_and(|f| world.is_live_function(*f)) {
                // `Function.apply` translates the names at run time
                // (`dart.nomeadoJS`, emitted with the table).
                table = true;
            }
    // The program's own `noSuchMethod`, live outside the SDK.
    for (k, f) in p.functions.iter().enumerate() {
        if f.class.is_some() && !p.library(f.library).is_sdk && i.resolve(f.name) == "noSuchMethod" && world.is_live_function(FunctionElementId(k as u32)) {
            if std::env::var("DARTFORGE_JSPROD_TRACE").is_ok() {
                eprintln!("[jsprod] named args: noSuchMethod live in {}; source names", p.library(f.library).uri);
            }
            return None;
        }
    }
    let ctx = a.ctx();
    let mut all: BTreeSet<String> = BTreeSet::new();
    // How many functions declare each name: the most common ones get the
    // one-letter keys.
    let mut frequency: HashMap<String, usize> = HashMap::new();
    let mut fixed: HashSet<String> = world.js_names.clone();
    for (k, f) in p.functions.iter().enumerate() {
        let Some(ft) = a.outline.functions.get(k) else { continue };
        let interop = f.external || f.class.is_some_and(|c| ctx.is_js_class(c) || p.class(c).kind == ClassKind::ExtensionType);
        for par in ft.parameters.iter() {
            if par.kind != dartforge_frontend::ast::ParameterKind::Named {
                continue;
            }
            let Some(n) = par.name else { continue };
            let n = i.resolve(n).to_string();
            if interop {
                fixed.insert(n.clone());
            }
            *frequency.entry(n.clone()).or_default() += 1;
            all.insert(n);
        }
    }
    // Names that only appear in written function types (no declaration).
    for t in 0..a.table.len() {
        if let Type::Function { named, .. } = a.table.get(TypeId(t as u32)) {
            for (s, _, _) in named.iter() {
                all.insert(i.resolve(*s).to_string());
            }
        }
    }
    let mut map: HashMap<String, String> = HashMap::new();
    let mut k = 0usize;
    let existing: HashSet<String> = all.iter().cloned().collect();
    let mut order: Vec<String> = all.into_iter().collect();
    order.sort_by(|x, y| frequency.get(y).unwrap_or(&0).cmp(frequency.get(x).unwrap_or(&0)).then_with(|| x.cmp(y)));
    for n in order {
        if fixed.contains(&n) {
            continue;
        }
        // The short key cannot coincide with a source name that stays.
        let short = loop {
            let c = format!("${}", suffix(k));
            k += 1;
            if !fixed.contains(&c) && !existing.contains(&c) {
                break c;
            }
        };
        if short.len() < n.len() {
            map.insert(n, short);
        }
    }
    Some(NamedArgs { map, table })
}

/// The map, and whether the runtime needs it at run time.
#[derive(Clone, Default, Debug)]
pub struct NamedArgs {
    pub map: HashMap<String, String>,
    pub table: bool,
}

impl NamedArgs {
    /// The name → key JSON, for `dart.nomeadoJS` (deterministic).
    pub fn json(&self) -> String {
        let mut v: Vec<(&String, &String)> = self.map.iter().collect();
        v.sort();
        let items: Vec<String> = v.iter().map(|(k, c)| format!("{}:{}", json_str(k), json_str(c))).collect();
        format!("{{{}}}", items.join(","))
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}
