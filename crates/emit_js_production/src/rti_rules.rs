//! rti rules only for the types that are tested
//! (`docs/JS-PRODUCAO-TAMANHO.md` §8.1).
//!
//! `_Universe.addRules` registers, for each class `C`, its supertypes
//! (`"lib|T": [args]`) and the substitutions of inherited type variables
//! (`"T.E": "recipe"`). The subtype check `S <: T` only looks up the `T`
//! entry of `S`'s rule (`rti.dart` `_isSubtype`, the `lookupSupertype` over
//! `_findRule(sName)`), and `T` is always a type the program wrote in a
//! recipe: the target of an `is`/`as`, of a `catch`, of a type argument, of a
//! substitution. `dart2js` makes the same cut
//! (`RuntimeTypesChecks.requiredChecks`, `js_backend/runtime_types.dart:26-45`).
//!
//! The computation: the `lib|Class` names cited anywhere in the file,
//! excluding the keys of the rules themselves (the described classes and the
//! listed supertypes), plus those cited in the **values** of the rules
//! (supertype arguments and substitutions, which become targets when
//! evaluated). A supertype entry whose name is not in that set is never
//! looked up and goes. The forwarding rules all stay.
//!
//! The substitutions of type variables (`"Iterable.E": "1"`) are looked up
//! only by `_Universe.evalTypeVariable`, with the name the recipe *parser*
//! read from a recipe of the text (`Iterable.E` inside `|j<Iterable.E>`): an
//! entry whose name no recipe of the file writes is never read and goes too.
//! The rule values never cite a type variable by name (they use the indexes
//! `"1"`, `"2"`… of `module.rs::rule_recipe`).

use std::collections::{HashMap, HashSet};

const CALLS: &[&str] = &[
    "_Universe.addRules(dart.typeUniverse, JSON.parse(",
    "_Universe.addOrUpdateRules(dart.typeUniverse, JSON.parse(",
];

/// Reads the double-quoted JS *string* that starts at `start` (on the `"`):
/// the end (after the quote) and the content without JS escapes.
fn js_string(t: &str, start: usize) -> Option<(usize, String)> {
    let b = t.as_bytes();
    if b.get(start) != Some(&b'"') {
        return None;
    }
    let mut i = start + 1;
    let mut out = String::new();
    while i < b.len() {
        match b[i] {
            b'"' => return Some((i + 1, out)),
            b'\\' => {
                let c = *b.get(i + 1)?;
                match c {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let h = t.get(i + 2..i + 6)?;
                        out.push(char::from_u32(u32::from_str_radix(h, 16).ok()?)?);
                        i += 4;
                    }
                    _ => out.push(c as char),
                }
                i += 2;
            }
            _ => {
                let ch = t[i..].chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

/// The `lib|Class` and `|Class` names of a text.
fn class_names(t: &str, out: &mut HashSet<String>) {
    let b = t.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    for (p, _) in t.match_indices('|') {
        let mut i = p;
        while i > 0 && id(b[i - 1]) {
            i -= 1;
        }
        let mut f = p + 1;
        while f < b.len() && id(b[f]) {
            f += 1;
        }
        // `|Name` (no library) is the short recipe of `Ctx::class_recipe`.
        if f > p + 1 {
            out.insert(t[i..f].to_string());
        }
    }
}

/// The `Name.Var` words of a text (dotted pairs of identifiers): a superset of
/// the type variable recipes it writes (`JSArray.E`). Plain JS member access
/// (`core.Object`) also lands here, which only keeps more.
fn dotted_names(t: &str, out: &mut HashSet<String>) {
    let b = t.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    for (p, _) in t.match_indices('.') {
        let mut i = p;
        while i > 0 && id(b[i - 1]) {
            i -= 1;
        }
        // A short class id (`|h.E`).
        if i > 0 && b[i - 1] == b'|' {
            i -= 1;
        }
        let mut f = p + 1;
        while f < b.len() && id(b[f]) {
            f += 1;
        }
        if i < p && f > p + 1 {
            out.insert(t[i..f].to_string());
        }
    }
}

/// Whether a rule key is the substitution of a type variable (`List.E`), not
/// a supertype (`|j`, `lib|C`).
fn is_type_variable_key(k: &str) -> bool {
    // A supertype is `|id` or `lib|C`; only a type variable has a period
    // (`List.E`, or `|h.E` with short ids).
    k.contains('.')
}

/// `addRtiResources(C, ["a|C", "b|I", …])` puts on `C`'s prototype the mark
/// `$is_<recipe>` of each interface in the list, which the fast `is` test
/// (`_isTestViaProperty`) reads. Like the rules, it only matters for a cited
/// target; the first item (the recipe of the class itself) always stays.
const RESOURCES: &str = "addRtiResources(";

/// Prunes the never-consulted supertype entries from the file's rti rules
/// and the corresponding interface marks. Returns the text unchanged if
/// something does not have the expected shape.
pub fn prune(js: &str) -> String {
    let Some((cited, found, lists)) = citations(js) else {
        return js.to_string();
    };
    let variables = if std::env::var("DARTFORGE_JSPROD_TYPE_VARIABLES").is_ok_and(|v| v == "0") {
        None
    } else {
        Some(cited_type_variables(js, &found))
    };
    rewrite(js, &cited, variables.as_ref(), found, lists)
}

/// The classes (`lib|C`, `|id`) whose name a message can show: those cited
/// outside the rule keys and the `addRtiResources` lists (as in [`prune`]),
/// plus the first of each list (the class itself, which is the type of its
/// instances). A class that only appears as a supertype in the rules never
/// becomes an rti object, and `_rtiToString` never shows it (§9.5 of
/// `docs/JS-PRODUCAO-TAMANHO.md`).
pub fn displayable(js: &str) -> HashSet<String> {
    let Some((mut cited, _, lists)) = citations(js) else {
        let mut all = HashSet::new();
        class_names(js, &mut all);
        return all;
    };
    for (start, end) in lists {
        if let Some(first) = js[start + 1..end - 1].split(", ").next() {
            class_names(first, &mut cited);
        }
    }
    cited
}

type Found = Vec<(usize, usize, serde_json::Value)>;

/// The type variable names that the text writes outside the rules JSON (see
/// [`dotted_names`]).
fn cited_type_variables(js: &str, found: &Found) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut pos = 0;
    for (start, end, _) in found {
        if *start >= pos {
            dotted_names(&js[pos..*start], &mut out);
            pos = *end;
        }
    }
    dotted_names(&js[pos..], &mut out);
    out
}

type Citations = (HashSet<String>, Found, Vec<(usize, usize)>);

/// The cited names (see [`prune`]), the JSON of the rules and the
/// `addRtiResources` lists; `None` if something does not have the expected
/// shape.
fn citations(js: &str) -> Option<Citations> {
    // The spans that do not count as citations: the JSON of the rules and
    // the `addRtiResources` lists (start and end of each `[...]` list).
    let mut lists: Vec<(usize, usize)> = Vec::new();
    for (p, _) in js.match_indices(RESOURCES) {
        let rest = &js[p..];
        let Some(a) = rest.find(", [") else { continue };
        let Some(f) = rest[a..].find("]);") else {
            continue;
        };
        if rest[..a + f].contains('\n') {
            continue;
        }
        lists.push((p + a + 2, p + a + f + 1));
    }
    // The calls and their JSON.
    let mut found: Vec<(usize, usize, serde_json::Value)> = Vec::new();
    for c in CALLS {
        for (p, _) in js.match_indices(c) {
            let start = p + c.len();
            let (end, content) = js_string(js, start)?;
            let v = serde_json::from_str::<serde_json::Value>(&content).ok()?;
            found.push((start, end, v));
        }
    }
    if found.is_empty() {
        return None;
    }
    found.sort_by_key(|a| a.0);
    // The names cited outside the rules and the lists, and in the values of
    // the rules.
    let mut cited: HashSet<String> = HashSet::new();
    let mut outside: Vec<(usize, usize)> = found
        .iter()
        .map(|a| (a.0, a.1))
        .chain(lists.iter().copied())
        .collect();
    outside.sort();
    let mut pos = 0;
    for (start, end) in &outside {
        if *start >= pos {
            class_names(&js[pos..*start], &mut cited);
            pos = *end;
        }
    }
    class_names(&js[pos..], &mut cited);
    for (_, _, v) in &found {
        if let Some(o) = v.as_object() {
            for rule in o.values() {
                let Some(r) = rule.as_object() else { continue };
                for x in r.values() {
                    match x {
                        serde_json::Value::String(s) => class_names(s, &mut cited),
                        serde_json::Value::Array(a) => {
                            for s in a.iter().filter_map(|s| s.as_str()) {
                                class_names(s, &mut cited);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Some((cited, found, lists))
}

/// Rewrites each JSON without the uncited supertype entries (and, with
/// `variables`, the unwritten type variable substitutions) and each mark
/// list without the uncited interfaces.
fn rewrite(
    js: &str,
    cited: &HashSet<String>,
    variables: Option<&HashSet<String>>,
    mut found: Found,
    lists: Vec<(usize, usize)>,
) -> String {
    for (_, _, v) in found.iter_mut() {
        if let Some(o) = v.as_object_mut() {
            for rule in o.values_mut() {
                let Some(r) = rule.as_object_mut() else {
                    continue;
                };
                r.retain(|k, _| match variables {
                    Some(v) if is_type_variable_key(k) => v.contains(k),
                    _ => !k.contains('|') || cited.contains(k),
                });
            }
            // Empty rule: without it `_isSubtype` gives the same `false`
            // (missing rule), and no type variable is evaluated through the
            // class.
            o.retain(|_, rule| rule.as_object().is_none_or(|r| !r.is_empty()));
        }
    }
    if std::env::var("DARTFORGE_JSPROD_RULE_FORWARDING").map_or(true, |v| v != "0") {
        forward_identical(js, &mut found);
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (start, end, v) in found {
        let json = rules_json(&v);
        edits.push((start, end, dartforge_emit_js::js::string_literal(&json)));
    }
    for (start, end) in lists {
        let items: Vec<&str> = js[start + 1..end - 1].split(", ").collect();
        let kept: Vec<&str> = items
            .iter()
            .enumerate()
            .filter(|(i, x)| {
                *i == 0
                    || x.strip_prefix('"')
                        .and_then(|y| y.strip_suffix('"'))
                        .is_none_or(|y| cited.contains(y))
            })
            .map(|(_, x)| *x)
            .collect();
        edits.push((start, end, format!("[{}]", kept.join(", "))));
    }
    edits.sort_by_key(|t| t.0);
    let mut out = String::with_capacity(js.len());
    let mut pos = 0;
    for (start, end, new) in edits {
        if start < pos {
            continue;
        }
        out.push_str(&js[pos..start]);
        out.push_str(&new);
        pos = end;
    }
    out.push_str(&js[pos..]);
    out
}

/// The JSON of a rules object, its entries ordered by their value (then by
/// key): the rules of classes alike (the views of every library, which
/// differ only in the type argument) become neighbors, and gzip finds them.
/// `addRules` assigns every key of one call before any lookup, so the order
/// is not observable. `DARTFORGE_JSPROD_RULE_ORDER=0` keeps the key order.
fn rules_json(v: &serde_json::Value) -> String {
    let by_value = std::env::var("DARTFORGE_JSPROD_RULE_ORDER").map_or(true, |x| x != "0");
    let Some(o) = v.as_object().filter(|_| by_value) else {
        return serde_json::to_string(v).unwrap_or_default();
    };
    let mut entries: Vec<(String, String)> = o
        .iter()
        .map(|(k, x)| {
            (
                serde_json::to_string(x).unwrap_or_default(),
                serde_json::to_string(k).unwrap_or_default(),
            )
        })
        .collect();
    entries.sort();
    let body: Vec<String> = entries
        .into_iter()
        .map(|(x, k)| format!("{k}:{x}"))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// A rule identical to one registered before it becomes the forwarding
/// string `"|B": "|A"`: `_Universe.findRule` and `_isSubtype` follow a string
/// rule to the named class's rule (`rti.dart`), so `B` keeps the same
/// supertypes and substitutions. In ngdart projects most view classes share
/// one rule body. The target is the first class with the body, in text order
/// (its `addRules` already ran when `B` is looked up), and it must be a key
/// registered only once. `addOrUpdateRules` does not accept the forwarding
/// form, so the keys it touches neither forward nor are forwarded to.
fn forward_identical(js: &str, found: &mut Found) {
    // A short body (`{"|ir":[]}`) costs gzip almost nothing when repeated;
    // the forward only pays off in raw bytes above this many (measured on
    // the corpus and limitless_ui: 12 keeps ~90% of the raw gain and halves
    // the small gzip loss).
    const MIN_FORWARD_GAIN: usize = 12;
    let is_update = |start: usize| js[..start].ends_with(CALLS[1]);
    let mut count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut updated: HashSet<String> = HashSet::new();
    for (start, _, v) in found.iter() {
        let Some(o) = v.as_object() else { continue };
        for k in o.keys() {
            *count.entry(k.clone()).or_default() += 1;
            if is_update(*start) {
                updated.insert(k.clone());
            }
        }
    }
    let mut first: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (start, _, v) in found.iter_mut() {
        if is_update(*start) {
            continue;
        }
        let Some(o) = v.as_object_mut() else { continue };
        for (k, rule) in o.iter_mut() {
            if updated.contains(k) || !rule.is_object() {
                continue;
            }
            let body = serde_json::to_string(rule).unwrap_or_default();
            match first.get(&body) {
                Some(target) if target != k && body.len() > target.len() + MIN_FORWARD_GAIN => {
                    *rule = serde_json::Value::String(target.clone())
                }
                Some(_) => {}
                None if count.get(k) == Some(&1) => {
                    first.insert(body, k.clone());
                }
                None => {}
            }
        }
    }
}

/// The program's `addRules` calls (from `program_start` on) become one, in
/// the place of the last: each library registered its classes' rules in its
/// own `JSON.parse`, repeating the rules of the classes they share (the
/// ngdart views and directives) and compressing apart. `addRules` is an
/// `Object.assign` into the universe's rules, so one call with the later
/// value of each key leaves the same rules once the libraries have loaded.
/// Loading a library defines classes and lazy members and checks no type
/// (the rules are first read when `main` runs). A key that an
/// `addOrUpdateRules` of the program also touches keeps the order of every
/// call: nothing is merged then. `DARTFORGE_JSPROD_MERGE_RULES=0` turns it
/// off.
pub fn merge_program_rules(js: &str, program_start: usize) -> String {
    const PREFIX: &str = "dart_rti.";
    let mut calls: Vec<(usize, usize, serde_json::Map<String, serde_json::Value>)> = Vec::new();
    let mut updated: HashSet<String> = HashSet::new();
    for (k, c) in CALLS.iter().enumerate() {
        for (p, _) in js[program_start..].match_indices(c) {
            let p = program_start + p;
            let Some((end, content)) = js_string(js, p + c.len()) else {
                return js.to_string();
            };
            let Ok(serde_json::Value::Object(o)) =
                serde_json::from_str::<serde_json::Value>(&content)
            else {
                return js.to_string();
            };
            if k == 1 {
                updated.extend(o.keys().cloned());
                continue;
            }
            // The whole statement: `dart_rti._Universe.addRules(…, JSON.parse("…"));`.
            if !js[..p].ends_with(PREFIX) || !js[end..].starts_with("));") {
                return js.to_string();
            }
            calls.push((p - PREFIX.len(), end + 3, o));
        }
    }
    if calls.len() < 2
        || calls
            .iter()
            .any(|(_, _, o)| o.keys().any(|k| updated.contains(k)))
    {
        return js.to_string();
    }
    calls.sort_by_key(|c| c.0);
    let mut merged = serde_json::Map::new();
    for (_, _, o) in &calls {
        for (k, v) in o {
            merged.insert(k.clone(), v.clone());
        }
    }
    let json = rules_json(&serde_json::Value::Object(merged));
    let last = calls.len() - 1;
    let mut out = String::with_capacity(js.len());
    let mut pos = 0;
    for (i, (start, end, _)) in calls.iter().enumerate() {
        out.push_str(&js[pos..*start]);
        if i == last {
            out.push_str(&format!(
                "{PREFIX}{}{}));",
                CALLS[0],
                dartforge_emit_js::js::string_literal(&json)
            ));
        }
        pos = *end;
    }
    out.push_str(&js[pos..]);
    out
}

/// The program's `dart.addRtiResources(C, [...])` statements (from
/// `program_start` on) become one call, in the place of the last, over a
/// flat list: `C, "|a", D, ["|b", "|c"], …` (a single recipe without its
/// list). The resources (`$ccache`, `$interfaceRecipe` and the interface
/// marks) are first read when an instance's type is computed or tested,
/// which loading a library never does; at the last statement every listed
/// class is already defined. The exception is a class that `applyMixin` or
/// `registerExtension` receives: they copy its marks at load, so its
/// statement stays. Each statement cost the call and the list:
/// 2,067 in limitless_ui. `DARTFORGE_JSPROD_BATCH_RESOURCES=0` turns it off.
///
/// The SDK's statements (before `program_start`) join the same call, except
/// those of the classes whose marks are copied at load: the arguments of
/// `applyMixin`/`registerExtension`, the classes with a `[dart.mixinOn]`,
/// and every superclass of those (`registerExtension` copies the marks of
/// the whole Dart chain of a native type). Without a readable superclass
/// for one of them the SDK stays as it is.
/// `DARTFORGE_JSPROD_BATCH_SDK_RESOURCES=0` keeps the SDK's in place.
pub fn batch_program_resources(js: &str, program_start: usize) -> String {
    const CALL: &str = "dart.addRtiResources(";
    // `applyMixin` and `registerExtension` copy the marks at load: a class
    // they receive keeps its statement where it is.
    let mut copied: HashSet<&str> = HashSet::new();
    for c in ["applyMixin(", "registerExtension("] {
        for (p, _) in js[program_start..].match_indices(c) {
            let rest = &js[program_start + p + c.len()..];
            let args = &rest[..rest.find(");").unwrap_or(0)];
            copied.extend(args.split(", ").map(str::trim));
        }
    }
    let sdk_pinned =
        if std::env::var("DARTFORGE_JSPROD_BATCH_SDK_RESOURCES").is_ok_and(|v| v == "0") {
            None
        } else {
            sdk_copied_classes(js, program_start)
        };
    let mut found: Vec<(usize, usize, &str, Vec<&str>)> = Vec::new();
    let scan_from = if sdk_pinned.is_some() {
        0
    } else {
        program_start
    };
    for (p, _) in js[scan_from..].match_indices(CALL) {
        let p = scan_from + p;
        // A whole statement on its own line: `dart.addRtiResources(C, [...]);`.
        if p > 0 && js.as_bytes()[p - 1] != b'\n' {
            continue;
        }
        let rest = &js[p + CALL.len()..];
        let Some(line_end) = rest.find('\n') else {
            continue;
        };
        let line = &rest[..line_end];
        let Some((class, list)) = line.split_once(", [") else {
            continue;
        };
        let Some(list) = list.strip_suffix("]);") else {
            continue;
        };
        // Only a class reached through its library's namespace (`L$lib.C`,
        // `core.C`): a local of the library's IIFE (a mixin application)
        // would not be in scope at the last statement.
        let Some((namespace, _)) = class.split_once('.') else {
            continue;
        };
        let in_sdk = p < program_start;
        let pinned = if in_sdk {
            sdk_pinned.as_ref().is_none_or(|s| s.contains(class))
        } else {
            !namespace.starts_with("L$") || copied.contains(class)
        };
        if pinned
            || !class
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'.')
        {
            continue;
        }
        let items: Vec<&str> = list.split(", ").collect();
        if items.is_empty()
            || !items.iter().all(|x| {
                x.len() >= 2
                    && x.starts_with('"')
                    && x.ends_with('"')
                    && !x[1..x.len() - 1].contains(['"', '\\'])
            })
        {
            continue;
        }
        found.push((p, p + CALL.len() + line_end + 1, class, items));
    }
    // The loop costs ~100 bytes and each statement saves ~6: in a small
    // program the batch loses (measured on the corpus, raw and gzip).
    const MIN_STATEMENTS: usize = 32;
    if found.len() < MIN_STATEMENTS {
        return js.to_string();
    }
    let mut flat: Vec<String> = Vec::new();
    for (_, _, class, items) in &found {
        flat.push(class.to_string());
        flat.push(if items.len() == 1 {
            items[0].to_string()
        } else {
            format!("[{}]", items.join(", "))
        });
    }
    let call = resource_batch_call(
        &flat,
        std::env::var("DARTFORGE_JSPROD_RESOURCE_IDS").as_deref() != Ok("0"),
    );
    let last = found.len() - 1;
    let mut out = String::with_capacity(js.len());
    let mut pos = 0;
    for (i, (start, end, _, _)) in found.iter().enumerate() {
        out.push_str(&js[pos..*start]);
        if i == last {
            out.push_str(&call);
        }
        pos = *end;
    }
    out.push_str(&js[pos..]);
    out
}

/// The SDK classes (before `program_start`) whose marks some runtime call
/// copies at load: the arguments of `applyMixin`/`registerExtension` in the
/// whole file, the classes with a `[dart.mixinOn]`, and the superclasses of
/// all of them, read from the `N = class C extends S {` lines. `None` when a
/// superclass of one of them is not such a line (nothing of the SDK may
/// move then).
fn sdk_copied_classes(js: &str, program_start: usize) -> Option<HashSet<String>> {
    let mut supers: HashMap<&str, &str> = HashMap::new();
    for line in js[..program_start].lines() {
        let l = line
            .strip_prefix("const ")
            .or_else(|| line.strip_prefix("let "))
            .or_else(|| line.strip_prefix("var "))
            .unwrap_or(line);
        let Some((name, rest)) = l.split_once(" = class ") else {
            continue;
        };
        if name.contains(' ') {
            continue;
        }
        let Some((_, after)) = rest.split_once(" extends ") else {
            continue;
        };
        let Some(sup) = after
            .strip_suffix(" {")
            .or_else(|| after.strip_suffix(" {};"))
        else {
            continue;
        };
        if sup.contains(' ') {
            continue;
        }
        supers.insert(name, sup);
    }
    let mut pinned: HashSet<String> = HashSet::new();
    let mut queue: Vec<&str> = Vec::new();
    for c in ["applyMixin(", "registerExtension("] {
        for (p, _) in js.match_indices(c) {
            // The runtime's own definition is not a call.
            if js[..p].ends_with("function ") {
                continue;
            }
            let rest = &js[p + c.len()..];
            let args = &rest[..rest.find(')').unwrap_or(0)];
            queue.extend(
                args.split(',')
                    .map(str::trim)
                    .filter(|a| !a.starts_with(['"', '\'']) && !a.is_empty()),
            );
        }
    }
    for (p, _) in js.match_indices("[dart.mixinOn] = ") {
        let before = &js[..p];
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        queue.push(before[start..].trim());
    }
    while let Some(c) = queue.pop() {
        if !pinned.insert(c.to_string()) || c == "core.Object" {
            continue;
        }
        match supers.get(c) {
            Some(s) => queue.push(s),
            // A program class (`L$…`) or a local has no SDK marks to keep;
            // an SDK class without a readable superclass stops the batch.
            None if c.starts_with("L$") => {}
            None => return None,
        }
    }
    Some(pinned)
}

// Transport only: reconstruct the exact canonical recipe string before the
// runtime sees it. No class, rule, ordering or runtime check is removed.
fn resource_batch_call(flat: &[String], compact: bool) -> String {
    let ordinary = format!(
        "((a) => {{ for (let i = 0; i < a.length; i += 2) dart.addRtiResources(a[i], typeof a[i + 1] == \"string\" ? [a[i + 1]] : a[i + 1]); }})([{}]);\n",
        flat.join(", ")
    );
    if !compact {
        return ordinary;
    }
    let mut encoded = flat.to_vec();
    let mut previous = None;
    for index in (1..flat.len()).step_by(2) {
        let Some(number) = resource_id_number(&flat[index]) else {
            continue;
        };
        let token = if previous.is_some_and(|p| p + 1 == number) {
            "0".to_string()
        } else {
            (number + 1).to_string()
        };
        if token.len() < flat[index].len() {
            encoded[index] = token;
            previous = Some(number);
        }
    }
    let alphabet = std::str::from_utf8(dartforge_emit_js::ID_NEXT).expect("ASCII recipe alphabet");
    let candidate = format!(
        "((a) => {{ let p = -1; const c = \"{alphabet}\"; for (let i = 0; i < a.length; i += 2) {{ let r = a[i + 1]; if (typeof r == \"number\") {{ let n = r == 0 ? p + 1 : r - 1; p = n; r = \"|\" + c[n % 54]; for (n = (n / 54) | 0; n > 0; n = (n / 64) | 0) r += c[(--n) % 64]; }} dart.addRtiResources(a[i], typeof r == \"string\" ? [r] : r); }} }})([{}]);\n",
        encoded.join(", ")
    );
    // Leave small batches alone; the decoder and a gzip dictionary can cost
    // more than the removed literals. Real application sizes are measured.
    if candidate.len().saturating_add(512) < ordinary.len() {
        candidate
    } else {
        ordinary
    }
}

fn resource_id_number(literal: &str) -> Option<usize> {
    let id = literal.strip_prefix("\"|")?.strip_suffix('"')?;
    // Bound arithmetic even for arbitrary input. Five characters cover over
    // 900 million ids and keep the JS decoder's bitwise quotients in range.
    if id.is_empty() || id.len() > 5 {
        return None;
    }
    dartforge_emit_js::id_number(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe_id(mut number: usize) -> String {
        let mut id = String::new();
        id.push(dartforge_emit_js::ID_FIRST[number % 54] as char);
        number /= 54;
        while number > 0 {
            number -= 1;
            id.push(dartforge_emit_js::ID_NEXT[number % 64] as char);
            number /= 64;
        }
        id
    }

    #[test]
    fn resource_transport_accepts_only_bounded_canonical_singletons() {
        for n in 0..(54 + 54 * 64) {
            assert_eq!(
                resource_id_number(&format!("\"|{}\"", recipe_id(n))),
                Some(n)
            );
        }
        for n in [54 * 65, 54 * 65 + 1, 1_000_000, 919_151_669] {
            assert_eq!(
                resource_id_number(&format!("\"|{}\"", recipe_id(n))),
                Some(n)
            );
        }
        for invalid in [
            "\"|\"",
            "\"|0\"",
            "\"|é\"",
            "\"|a\\n\"",
            "\"|aaaaaa\"",
            "[\"|a\"]",
            "\"named.Type\"",
        ] {
            assert_eq!(resource_id_number(invalid), None, "{invalid}");
        }
        let small = vec!["L$a.C".to_string(), "\"|aaaaa\"".to_string()];
        assert_eq!(
            resource_batch_call(&small, true),
            resource_batch_call(&small, false)
        );
    }

    #[test]
    #[ignore = "requires Node"]
    fn resource_transport_preserves_class_identity_strings_and_call_order_in_node() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut flat = Vec::new();
        let mut add = |literal: String| {
            flat.push(format!("classes[{}]", flat.len() / 2));
            flat.push(literal);
        };
        // Exhaust both short-id alphabets, consecutive and gapped sequences,
        // then repeat backwards to exercise explicit ordinals and resets.
        for n in 0..(54 + 54 * 64) {
            add(format!("\"|{}\"", recipe_id(n)));
        }
        for n in (0..300).rev() {
            add(format!("\"|{}\"", recipe_id(n * 17)));
        }
        for n in [1_000_000, 919_151_669] {
            add(format!("\"|{}\"", recipe_id(n)));
        }
        for literal in [
            "\"ordinary.Type\"",
            "[\"|a\", \"|b\"]",
            "\"|é\"",
            "\"|aaaaaa\"",
            "\"|0\"",
        ] {
            add(literal.to_string());
        }
        let ordinary = resource_batch_call(&flat, false);
        let compact = resource_batch_call(&flat, true);
        assert!(compact.len() + 512 < ordinary.len());
        assert!(compact.contains("let p = -1"));
        let source = format!(
            "const classes=Array.from({{length:{}}},(_,i)=>({{i}})); let calls=[]; const dart={{addRtiResources(c,r){{if(c!==classes[c.i])throw Error('identity');calls.push([c.i,r]);}}}}; {ordinary} const expected=JSON.stringify(calls); calls=[]; {compact} if(JSON.stringify(calls)!==expected)throw Error('resource transport mismatch'); console.log('resources-exact');",
            flat.len() / 2
        );
        let mut child = Command::new("node")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Node");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"resources-exact\n");
    }

    #[test]
    fn program_resources_batch_into_the_last_statement() {
        let mut js = "core.S = class S extends core.T {\n};\ncore.T = class T extends core.Object {\n};\ncore.U = class U extends core.Object {\n};\ndart.addRtiResources(core.T, [\"|t\"]);\ndart.addRtiResources(core.S, [\"|s\"]);\ndart.registerExtension(\"Array\", core.S);\ndart.addRtiResources(core.U, [\"|u\"]);\nEND\nL$a.A = class {};\ndart.addRtiResources(L$a.A, [\"|a\"]);\nL$a.B = class {};\ndart.addRtiResources(L$a.B, [\"|b\", \"|c\"]);\n".to_string();
        for i in 0..40 {
            js.push_str(&format!("dart.addRtiResources(L$a.C{i}, [\"|c{i}\"]);\n"));
        }
        js.push_str("main();\n");
        let out = batch_program_resources(&js, js.find("END").unwrap());
        // A native peer and its superclass keep their marks in place for
        // `registerExtension`; the other SDK class joins the batch.
        assert!(
            out.contains(
                "dart.addRtiResources(core.T, [\"|t\"]);\ndart.addRtiResources(core.S, [\"|s\"]);"
            ),
            "{out}"
        );
        assert!(
            out.contains(
                "})([core.U, \"|u\", L$a.A, \"|a\", L$a.B, [\"|b\", \"|c\"], L$a.C0, \"|c0\""
            ),
            "{out}"
        );
        assert!(out.contains("L$a.C39, \"|c39\"]);\nmain();"), "{out}");
        assert_eq!(out.matches("addRtiResources(L$").count(), 0, "{out}");
    }

    #[test]
    fn program_rules_merge_into_the_last_call() {
        let js = format!(
            "{}SDK_END\n{}f();\n{}",
            rules(r#"{"|S":{"|x":[]}}"#),
            rules(r#"{"|A":{"|x":[]},"|B":{"|y":[]}}"#),
            rules(r#"{"|B":{"|y":[]},"|C":"|A"}"#)
        );
        let start = js.find("SDK_END").unwrap();
        let out = merge_program_rules(&js, start);
        assert_eq!(out.matches("addRules").count(), 2, "{out}");
        assert!(
            out.contains(r#"{\"|C\":\"|A\",\"|A\":{\"|x\":[]},\"|B\":{\"|y\":[]}}"#),
            "{out}"
        );
        assert!(
            out.find("f();").unwrap() < out.rfind("addRules").unwrap(),
            "{out}"
        );
        assert!(out.trim_end().ends_with("\"));"), "{out}");
    }

    fn rules(json: &str) -> String {
        format!(
            "dart_rti._Universe.addRules(dart.typeUniverse, JSON.parse({}));\n",
            dartforge_emit_js::js::string_literal(json)
        )
    }

    #[test]
    fn identical_rules_forward_to_the_first() {
        let body = r#"{"|Up":["|vJ"],"|Wp":[],"|Xn":[],"EmbeddedView.T":"|vJ"}"#;
        let js = format!(
            "{}{}dart_rti._Universe.addOrUpdateRules(dart.typeUniverse, JSON.parse({}));\nf(\"|Up\", \"|Wp\", \"|Xn\", \"|vJ\", \"EmbeddedView.T\");\n",
            rules(&format!(r#"{{"|A":{body},"|U":{body}}}"#)),
            rules(&format!(r#"{{"|B":{body},"|C":{{"|Xn":[]}}}}"#)),
            dartforge_emit_js::js::string_literal(&format!(r#"{{"|U":{body}}}"#)),
        );
        let out = prune(&js);
        // `|B` forwards to `|A` (the first, registered once); `|U` is touched by
        // `addOrUpdateRules`, so it keeps its body; the short body stays.
        assert!(out.contains(r#"\"|B\":\"|A\""#), "{out}");
        assert!(!out.contains(r#"\"|U\":\"|A\""#), "{out}");
        assert!(out.contains(r#"\"|C\":{\"|Xn\":[]}"#), "{out}");
    }
}
