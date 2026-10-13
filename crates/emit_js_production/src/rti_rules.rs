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

use std::collections::HashSet;

const CALLS: &[&str] = &["_Universe.addRules(dart.typeUniverse, JSON.parse(", "_Universe.addOrUpdateRules(dart.typeUniverse, JSON.parse("];

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
    !k.contains('|') && k.contains('.')
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
    let Some((cited, found, lists)) = citations(js) else { return js.to_string() };
    let variables = if std::env::var("DARTFORGE_JSPROD_TYPE_VARIABLES").is_ok_and(|v| v == "0") { None } else { Some(cited_type_variables(js, &found)) };
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

/// The cited names (see [`prune`]), the JSON of the rules and the
/// `addRtiResources` lists; `None` if something does not have the expected
/// shape.
fn citations(js: &str) -> Option<(HashSet<String>, Found, Vec<(usize, usize)>)> {
    // The spans that do not count as citations: the JSON of the rules and
    // the `addRtiResources` lists (start and end of each `[...]` list).
    let mut lists: Vec<(usize, usize)> = Vec::new();
    for (p, _) in js.match_indices(RESOURCES) {
        let rest = &js[p..];
        let Some(a) = rest.find(", [") else { continue };
        let Some(f) = rest[a..].find("]);") else { continue };
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
    let mut outside: Vec<(usize, usize)> = found.iter().map(|a| (a.0, a.1)).chain(lists.iter().copied()).collect();
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
fn rewrite(js: &str, cited: &HashSet<String>, variables: Option<&HashSet<String>>, mut found: Found, lists: Vec<(usize, usize)>) -> String {
    for (_, _, v) in found.iter_mut() {
        if let Some(o) = v.as_object_mut() {
            for rule in o.values_mut() {
                let Some(r) = rule.as_object_mut() else { continue };
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
        let json = serde_json::to_string(&v).unwrap_or_default();
        edits.push((start, end, dartforge_emit_js::js::string_literal(&json)));
    }
    for (start, end) in lists {
        let items: Vec<&str> = js[start + 1..end - 1].split(", ").collect();
        let kept: Vec<&str> = items
            .iter()
            .enumerate()
            .filter(|(i, x)| *i == 0 || x.strip_prefix('"').and_then(|y| y.strip_suffix('"')).is_none_or(|y| cited.contains(y)))
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
                Some(target) if target != k && body.len() > target.len() + MIN_FORWARD_GAIN => *rule = serde_json::Value::String(target.clone()),
                Some(_) => {}
                None if count.get(k) == Some(&1) => {
                    first.insert(body, k.clone());
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(json: &str) -> String {
        format!("dart_rti._Universe.addRules(dart.typeUniverse, JSON.parse({}));\n", dartforge_emit_js::js::string_literal(json))
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
