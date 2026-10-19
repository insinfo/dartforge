//! Closed world over `dart_sdk.js`.
//!
//! `dart2js` computes reachability over Dart elements because it compiles the
//! SDK together with the program. We link against a `dart_sdk.js` **already
//! compiled by DDC** — the decision of `docs/EMISSAO-DDC.md`, which gave us
//! exact semantics for free — so our reachability has to be computed inside a
//! ready-made JS artifact. It works, because the file is not arbitrary JS: it
//! is compiler output, with a small and regular grammar
//! (`docs/JS-PRODUCAO.md` §1.3 has the full table of shapes).
//!
//! What this module does, in order:
//!
//! 1. splits the file into top-level declarations (`scan::declarations`);
//! 2. classifies each one: which symbol it **defines**, or which symbol it
//!    **hangs on** (a `dart.setLibraryUri(C, …)` is not a root, it is
//!    metadata of `C`), splitting up the three giant declarations that, whole,
//!    reference the entire program;
//! 3. extracts the references — including those no JS analyzer sees: the
//!    class names inside **rti recipes**, because
//!    `dart_rti._Universe.eval("core|List<core|int>")` resolves the type by
//!    name at run time;
//! 4. runs the fixed point (`reach`) and re-emits the live units in file
//!    order.

use std::collections::HashSet;

use crate::bundle::Module;
use crate::reach::{self, Symbols, Unit};
use crate::scan;

/// The 38 libraries that `dart_sdk.js` declares with
/// `var X = Object.create(dart.library)`. An identifier is only read as
/// `library.member` when the prefix is in this list — otherwise `this.foo`
/// and `opts.bar` would become symbols.
const LIBS: &[&str] = &[
    "dart", "_ddc_only", "_async_status_codes", "_js_shared_embedded_names", "_recipe_syntax",
    "dart_rti", "_debugger", "_foreign_helper", "_interceptors", "_internal", "_isolate_helper",
    "_js_annotations", "_js_helper", "_js_names", "_js_primitives", "_js_types", "_metadata",
    "_native_typed_data", "async", "collection", "convert", "developer", "io", "isolate", "js",
    "js_interop", "js_interop_unsafe", "js_util", "math", "typed_data", "indexed_db", "html$",
    "html_common", "svg$", "web_audio", "web_gl", "core", "_http", "dartx",
];

/// In rti recipes the `dart:html` and `dart:svg` libraries appear without
/// the `$` that JS uses in the variable (`"html|Element"` but
/// `html$.Element`).
fn recipe_ns(p: &str) -> &str {
    match p {
        "html" => "html$",
        "svg" => "svg$",
        other => other,
    }
}

/// `dart.registerExtension(tag, C)` tags that are **JavaScript built-in**
/// types. For these, the link between the value and the Dart class is made at
/// run time by the object's type, not by any name that appears in the
/// program: `"olá"` is a JS `String` and only becomes `core.String` because
/// `registerExtension("String", _interceptors.JSString)` ran. So the target
/// class is a **root**, without depending on a textual citation.
///
/// It is the equivalent of the "impacts" that dart2js declares in
/// `js_backend/backend_impact.dart:93` (jsStringClass, jsIntClass,
/// jsArrayClass…) — the same idea: the runtime has entry points that
/// reachability does not see.
///
/// The typed-data tags (`ArrayBuffer`, `DataView`) are left out on purpose:
/// one only gets to them through `dart:typed_data`/`dart:html`, which the
/// program cites, and including them would cost the whole
/// `_native_typed_data`.
const BUILTIN_TAGS: &[&str] = &[
    "Object", "String", "Number", "Boolean", "Array", "Function", "Symbol", "BigInt",
    "Error", "TypeError", "RangeError",
];

/// Libraries whose classes are **always** roots. Just one:
/// `dart:_interceptors`. The interceptors are the floor of DDC's type system
/// — `null`, numbers, functions and unknown JS objects reach them through
/// type dispatch at run time, and several do not even have a
/// `registerExtension` (`JSNull` is installed directly by the runtime).
const ROOT_LIBS: &[&str] = &["_interceptors"];

fn is_lib(name: &str) -> bool {
    LIBS.contains(&name)
}

/// Symbol tables that DDC emits as `S`, `S$`, `S$0`, `S$1` etc. Each entry
/// can be pruned separately; the prefix alone is not enough to keep a private
/// key used by a live getter.
fn is_symbol_table(name: &str) -> bool {
    name == "S" || name == "S$" || name.strip_prefix("S$").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn ident_end(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$') {
        j += 1;
    }
    j
}

fn is_ident_start(b: &[u8], i: usize) -> bool {
    b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] == b'$'
}

/// Extracts from `t` every referenced symbol.
///
/// Five shapes, and the third is the one that only exists in Dart code for
/// the web: `lib.Name`, `lib['A|b']`, the class name inside an **rti recipe**
/// (`"core|List<core|int>"`), the constant index (`C[42]`, `CT.C42`) and the
/// plain identifier (which matches the file's top-level `var`s, such as the
/// mixin applications `EventTarget_ListBase$36`).
fn references(t: &str, out: &mut Vec<String>, user_module: bool) {
    let b = t.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if !is_ident_start(b, i) {
            i += 1;
            continue;
        }
        // Not the start of an identifier if the previous byte is also an
        // identifier byte.
        if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$') {
            i = ident_end(b, i);
            continue;
        }
        let end = ident_end(b, i);
        let name = &t[i..end];
        let mut advance = end;
        if end < b.len() && b[end] == b'.' && end + 1 < b.len() && is_ident_start(b, end + 1) {
            let end2 = ident_end(b, end + 1);
            let member = &t[end + 1..end2];
            if is_lib(name) || is_symbol_table(name) {
                out.push(format!("{name}.{member}"));
            } else if user_module && matches!(name, "html" | "svg") {
                // DDC exports `html$ as html` and `svg$ as svg`. The user's
                // emission cites the alias; the SDK world uses the local name.
                out.push(format!("{}.{member}", recipe_ns(name)));
            } else {
                out.push(name.to_string());
            }
            // `CT.C42` is a constant.
            if name == "CT" && member.len() > 1 && member.starts_with('C') && member[1..].bytes().all(|c| c.is_ascii_digit()) {
                out.push(format!("C#{}", &member[1..]));
            }
            advance = end2;
        } else if end < b.len() && b[end] == b'[' && (is_lib(name) || (user_module && matches!(name, "html" | "svg"))) {
            // `lib['A|b']` — an extension member, whose name has `|`.
            let mut j = end + 1;
            if j < b.len() && (b[j] == b'\'' || b[j] == b'"') {
                let quote = b[j];
                let start = j + 1;
                j = start;
                while j < b.len() && b[j] != quote {
                    j += 1;
                }
                out.push(format!("{}.{}", recipe_ns(name), &t[start..j]));
                advance = j + 1;
            } else {
                out.push(name.to_string());
            }
        } else if name == "C" && end < b.len() && b[end] == b'[' {
            let mut j = end + 1;
            let start = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > start && j < b.len() && b[j] == b']' {
                out.push(format!("C#{}", &t[start..j]));
                advance = j + 1;
            } else {
                out.push(name.to_string());
            }
        } else {
            out.push(name.to_string());
        }
        i = advance;
    }
    // rti recipes: `lib|Name` anywhere in the text (they live inside strings,
    // which is why the identifier scan does not catch them).
    let b = t.as_bytes();
    for (i, _) in t.match_indices('|') {
        if i == 0 || i + 1 >= b.len() || !is_ident_start(b, i + 1) {
            continue;
        }
        let mut start = i;
        while start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_' || b[start - 1] == b'$') {
            start -= 1;
        }
        if start == i || !is_ident_start(b, start) {
            continue;
        }
        let end = ident_end(b, i + 1);
        let ns = recipe_ns(&t[start..i]);
        if is_lib(ns) {
            out.push(format!("{ns}.{}", &t[i + 1..end]));
        }
    }
}

/// Extracts from `t` the **selectors** it uses: `x.name`, `x[$name]`,
/// `x[_name]` and every string that looks like an identifier — the latter is
/// the conservative side, because `dart.dsend(o, "name")` passes the selector
/// as a string and there is no way to tell it apart from a data string
/// without analyzing the program.
fn selectors(t: &str, out: &mut Vec<String>) {
    let b = t.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'.' if i + 1 < b.len() && is_ident_start(b, i + 1) => {
                let end = ident_end(b, i + 1);
                let name = &t[i + 1..end];
                out.push(format!("sel:{name}"));
                // `S.$head` and `S$2.$console` cite the Dart selectors `head`
                // and `console`. The literal JS name is kept too.
                if name.starts_with('$') && name.len() > 1 {
                    out.push(format!("sel:{}", &name[1..]));
                }
                i = end;
            }
            b'[' if i + 1 < b.len() => {
                let mut j = i + 1;
                if b[j] == b'$' {
                    j += 1;
                }
                if j < b.len() && is_ident_start(b, j) {
                    let end = ident_end(b, j);
                    if end < b.len() && b[end] == b']' {
                        out.push(format!("sel:{}", &t[j..end]));
                    }
                }
                i += 1;
            }
            b'"' | b'\'' => {
                let quote = b[i];
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != quote {
                    if b[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                let s = &t[start..j.min(t.len())];
                // `#` and `|` are included because the names DDC generates
                // use them: `C["_#new#tearOff"]`,
                // `async["FutureRecord2|get#wait"]`. Without them the
                // constructor tearoff would be pruned with someone still
                // citing it.
                if s.len() < 64 && is_selector_name(s) {
                    out.push(format!("sel:{s}"));
                }
                i = j + 1;
            }
            _ => i += 1,
        }
    }
}

// ------------------------------------------------------------------ classification

/// A slice of the file, with its liveness condition.
struct Slice {
    start: usize,
    end: usize,
    /// Slices of the same group are entries of an object/array literal and
    /// need commas between the live ones. `None` is a top-level declaration.
    group: Option<u32>,
    /// The comma already comes in the text (and so it is not put back).
    always: bool,
    triggers: Vec<String>,
    requirements: Vec<String>,
}

fn first_ident(t: &str) -> Option<(&str, usize)> {
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= b.len() || !is_ident_start(b, i) {
        return None;
    }
    let end = ident_end(b, i);
    Some((&t[i..end], end))
}

/// `lib.Name` at the start of `t`, if there is one.
fn qualified_target(t: &str) -> Option<String> {
    let (a, i) = first_ident(t)?;
    let b = t.as_bytes();
    if i >= b.len() || b[i] != b'.' {
        return None;
    }
    let (m, _) = first_ident(&t[i + 1..])?;
    is_lib(a).then(|| format!("{a}.{m}"))
}

/// Like [`qualified_target`], but only when `lib.Name` is really
/// **declared**: what comes after must be `=` or `[`.
///
/// The distinction is not cosmetic. Without it, `dart.applyMixin(V, M);`
/// would be read as a declaration of `dart.applyMixin` — and then the
/// statement would only be live if the function `dart.applyMixin` were live,
/// which in turn is only referenced by the call sites: a loop that erases
/// itself. That is how the 93 mixin applications disappeared from the first
/// pruned bundle, and with them the getters that `defineExtensionAccessors`
/// later looked for on the prototype.
fn qualified_declaration(t: &str) -> Option<String> {
    let (a, i) = first_ident(t)?;
    let b = t.as_bytes();
    if i >= b.len() || b[i] != b'.' {
        return None;
    }
    if !is_lib(a) {
        return None;
    }
    let (m, j) = first_ident(&t[i + 1..])?;
    let mut k = i + 1 + j;
    while k < b.len() && (b[k] == b' ' || b[k] == b'\t') {
        k += 1;
    }
    // `lib.Name = …`, `lib.Name[x] = …` — but never `lib.Name === …`.
    let decl = match b.get(k) {
        Some(b'=') => b.get(k + 1) != Some(&b'='),
        Some(b'[') => true,
        _ => false,
    };
    decl.then(|| format!("{a}.{m}"))
}

/// Name of the member in a class body entry (`get x() {…}`, `[_priv](…)`,
/// `static f(…)`, `'A|b'(…)`).
fn member_name(t: &str) -> Option<String> {
    let t = t.trim_start();
    let t = t.strip_prefix("static").map(str::trim_start).unwrap_or(t);
    let t = t.strip_prefix("get ").or_else(|| t.strip_prefix("set ")).map(str::trim_start).unwrap_or(t);
    let t = t.strip_prefix('*').map(str::trim_start).unwrap_or(t);
    let b = t.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'[' {
        // `[_priv](…)` or `[$name]` or `['A|b']`
        let mut j = 1;
        if j < b.len() && (b[j] == b'\'' || b[j] == b'"') {
            let quote = b[j];
            let start = j + 1;
            j = start;
            while j < b.len() && b[j] != quote {
                j += 1;
            }
            return Some(t[start..j].to_string());
        }
        if j < b.len() && b[j] == b'$' {
            j += 1;
        }
        if j < b.len() && is_ident_start(b, j) {
            let end = ident_end(b, j);
            // In DDC `get [S.$head]()` and `get [S$2.$console]()` use a symbol
            // alias. The selector is the alias's field, not `S`.
            let prefix = &t[j..end];
            if is_symbol_table(prefix)
                && b.get(end) == Some(&b'.') && end + 1 < b.len()
            {
                let start = end + 1 + usize::from(b[end + 1] == b'$');
                if start < b.len() && is_ident_start(b, start) {
                    let name_end = ident_end(b, start);
                    if b.get(name_end) == Some(&b']') {
                        return Some(t[start..name_end].to_string());
                    }
                }
            }
            return Some(t[j..end].to_string());
        }
        return None;
    }
    if b[0] == b'\'' || b[0] == b'"' {
        let quote = b[0];
        let mut j = 1;
        while j < b.len() && b[j] != quote {
            j += 1;
        }
        return Some(t[1..j].to_string());
    }
    if !is_ident_start(b, 0) {
        return None;
    }
    let end = ident_end(b, 0);
    // It is only a member if what comes after is `(` — otherwise it is an
    // object field.
    let rest = t[end..].trim_start();
    (rest.starts_with('(')).then(|| t[..end].to_string())
}

/// A member name that can be matched against a selector extracted from the
/// text.
///
/// An identifier (with `_`, `$`) plus `#` and `|`, which the names DDC
/// generates use (`_#new#tearOff`, `FutureRecord2|get#wait`). Whatever does
/// not match this shape — the operators — **cannot be pruned by selector**:
/// the member is declared `['+'](other) {…}`, but the caller writes
/// `p[$plus](x)`, the alias of the symbol `dartx['+']`, whose JS name has no
/// textual relation to `+`. Matching the two would require knowing the
/// `dartx` table; until we do, pruning the operator gives
/// `p.+ is not a function`.
fn is_selector_name(n: &str) -> bool {
    !n.is_empty()
        && is_ident_start(n.as_bytes(), 0)
        && n.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'#' | b'|'))
}

/// Splits `X = class … { … }` into a shell + one member per entry.
fn split_class(src: &str, start: usize, end: usize, sym: &str, slices: &mut Vec<Slice>, group: &mut u32) -> bool {
    let t = &src[start..end];
    let Some(eq) = t.find("= class ") else { return false };
    let Some(rel) = t[eq..].find('{') else { return false };
    let open = start + eq + rel;
    let Some(close) = scan::closing_brace(src, open) else { return false };
    let body = &src[open + 1..close];
    let members = scan::class_entries(body);
    if members.len() < 3 {
        return false;
    }
    // Shell: `X = class N extends S {` … `};` plus whatever comes after.
    slices.push(Slice { start, end: open + 1, group: None, always: false, triggers: vec![sym.to_string()], requirements: vec![] });
    for (a, b) in members {
        let txt = &body[a..b];
        let mut req = vec![sym.to_string()];
        // Without a name matchable against a selector, the member lives with
        // the class: that is the conservative side, and it is what keeps the
        // operators standing.
        if let Some(n) = member_name(txt)
            && is_selector_name(&n) {
                req.push(format!("sel:{n}"));
            }
        slices.push(Slice { start: open + 1 + a, end: open + 1 + b, group: None, always: false, triggers: vec![], requirements: req });
    }
    let _ = group;
    slices.push(Slice { start: close, end, group: None, always: false, triggers: vec![sym.to_string()], requirements: vec![] });
    true
}

/// Splits `f(TARGET, { … })` or `f(TARGET, () => ({ … }))` per entry, with
/// each entry conditioned on `[target, sel:key]`.
fn split_object(
    src: &str,
    range: std::ops::Range<usize>,
    open: usize,
    target: &str,
    by_selector: bool,
    slices: &mut Vec<Slice>,
    group: &mut u32,
) -> bool {
    let (start, end) = (range.start, range.end);
    let Some(close) = scan::closing_brace(src, open) else { return false };
    let body = &src[open + 1..close];
    let entries = scan::object_entries(body);
    if entries.len() < 2 {
        return false;
    }
    let g = *group;
    *group += 1;
    slices.push(Slice { start, end: open + 1, group: None, always: false, triggers: vec![target.to_string()], requirements: vec![] });
    for (a, b) in entries {
        let txt = body[a..b].trim_end().trim_end_matches(',');
        let key = entry_key(txt);
        let (trig, req) = match (&key, by_selector) {
            (Some(k), true) => (vec![], vec![target.to_string(), format!("sel:{k}")]),
            (Some(k), false) => (vec![format!("{target}.{k}")], vec![]),
            (None, _) => (vec![target.to_string()], vec![]),
        };
        let shift = body[a..b].len() - body[a..b].trim_end().len();
        let cut = b - shift - if body[a..b].trim_end().ends_with(',') { 1 } else { 0 };
        slices.push(Slice { start: open + 1 + a, end: open + 1 + cut, group: Some(g), always: false, triggers: trig, requirements: req });
    }
    slices.push(Slice { start: close, end, group: None, always: false, triggers: vec![target.to_string()], requirements: vec![] });
    true
}

/// The key of an object entry: `name:`, `get name()`, `'a|b':`, `[Sym]:`
/// (the last one is neither minifiable nor matchable, so it returns `None`).
fn entry_key(t: &str) -> Option<String> {
    let t = t.trim_start();
    // Position comment that DDC emits before the `defineLazy` entries.
    let t = if let Some(r) = t.strip_prefix("/*") { r.split_once("*/").map(|(_, x)| x.trim_start()).unwrap_or(r) } else { t };
    let t = t.strip_prefix("get ").or_else(|| t.strip_prefix("set ")).map(str::trim_start).unwrap_or(t);
    let b = t.as_bytes();
    if b.is_empty() {
        return None;
    }
    if b[0] == b'\'' || b[0] == b'"' {
        let quote = b[0];
        let mut j = 1;
        while j < b.len() && b[j] != quote {
            if b[j] == b'\\' {
                j += 1;
            }
            j += 1;
        }
        return Some(t[1..j.min(t.len())].to_string());
    }
    if !is_ident_start(b, 0) {
        return None;
    }
    let end = ident_end(b, 0);
    Some(t[..end].to_string())
}

/// Classifies the whole file into slices with conditions, and also returns
/// the roots the file itself imposes (see [`BUILTIN_TAGS`] and
/// [`ROOT_LIBS`]).
fn classify(src: &str, per_member: bool) -> (Vec<Slice>, Vec<String>) {
    let mut slices = Vec::new();
    let mut roots = Vec::new();
    let mut group = 0u32;
    for (start, end) in scan::declarations(src) {
        let t = &src[start..end];
        let tt = t.trim_start();
        let simple = |g: Vec<String>| Slice { start, end, group: None, always: false, triggers: g, requirements: vec![] };
        let always = || Slice { start, end, group: None, always: true, triggers: vec![], requirements: vec![] };

        // `dart.defineLazy(TARGET, { get x() {…}, … })` — lazy statics.
        // The 549 constants of `CT` are the largest declaration of the file
        // (124 KB).
        if let Some(r) = tt.strip_prefix("dart.defineLazy(") {
            // The target can be a library (`core`) **or a class**
            // (`html$.Event`, for its lazy statics). Reading only the first
            // identifier tied the group's head to the `html$` namespace,
            // which is always live, and the head cites the class: the whole
            // of `dart:html` went into a program that only does `print`.
            if let Some(target) = qualified_target(r).or_else(|| first_ident(r).map(|(a, _)| a.to_string())) {
                let target = target.as_str();
                let is_ct = target == "CT";
                let is_class = target.contains('.');
                if let Some(rel) = t.find('{') {
                    let open = start + rel;
                    let Some(close) = scan::closing_brace(src, open) else { slices.push(always()); continue };
                    let body = &src[open + 1..close];
                    let entries = scan::object_entries(body);
                    if entries.len() >= 2 {
                        let g = group;
                        group += 1;
                        let target_s = target.to_string();
                        slices.push(Slice { start, end: open + 1, group: None, always: false, triggers: vec![target_s.clone()], requirements: vec![] });
                        for (a, b) in entries {
                            let txt = body[a..b].trim_end();
                            let key = entry_key(txt);
                            // A **library** static is cited qualified
                            // (`core.x`), so it becomes a trigger. A
                            // **class** static is cited with three parts
                            // (`html$.Event.MOUSEDOWN`), which reference
                            // extraction does not model: it lives with the
                            // class, and only the selector prunes it.
                            let (trig, req) = match (&key, is_ct, is_class) {
                                (Some(k), true, _) => (vec![format!("C#{}", k.trim_start_matches('C'))], vec![]),
                                (Some(k), false, true) if per_member && is_selector_name(k) => (vec![], vec![target_s.clone(), format!("sel:{k}")]),
                                (_, false, true) => (vec![target_s.clone()], vec![]),
                                (Some(k), false, false) => (vec![format!("{target_s}.{k}")], vec![]),
                                (None, _, _) => (vec![target_s.clone()], vec![]),
                            };
                            let cut = a + txt.len() - usize::from(txt.ends_with(','));
                            slices.push(Slice { start: open + 1 + a, end: open + 1 + cut, group: Some(g), always: false, triggers: trig, requirements: req });
                        }
                        slices.push(Slice { start: close, end, group: None, always: false, triggers: vec![target_s.clone()], requirements: vec![] });
                        continue;
                    }
                }
            }
        }
        // `dart.copyProperties(io, { … })` — library top-level members.
        if let Some(r) = tt.strip_prefix("dart.copyProperties(")
            && let Some((target, _)) = first_ident(r)
                && is_lib(target)
                    && let Some(rel) = t.find('{') {
                        let target = target.to_string();
                        if split_object(src, start..end, start + rel, &target, false, &mut slices, &mut group) {
                            continue;
                        }
                    }
        // `dart_rti._Universe.addRules(u, JSON.parse('{"core|List":{…},…}'))`
        // — 315 KB of subtyping rules, one key per class.
        if tt.starts_with("dart_rti._Universe.add") && t.contains("JSON.parse(") {
            let jp = t.find("JSON.parse(").unwrap();
            if let Some(rel) = t[jp..].find('{') {
                let open = start + jp + rel;
                if let Some(close) = scan::closing_brace(src, open) {
                    let body = &src[open + 1..close];
                    let entries = scan::object_entries(body);
                    if entries.len() >= 2 {
                        let g = group;
                        group += 1;
                        slices.push(Slice { start, end: open + 1, group: None, always: true, triggers: vec![], requirements: vec![] });
                        for (a, b) in entries {
                            let txt = body[a..b].trim_end();
                            // The key comes escaped inside the JS string:
                            // `\"core|List\"`.
                            let sym = txt
                                .trim_start()
                                .trim_start_matches(['\\', '"', '\''])
                                .split_once('|')
                                .and_then(|(ns, rest)| {
                                    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
                                    let ns = recipe_ns(ns);
                                    is_lib(ns).then(|| format!("{ns}.{}", &rest[..end]))
                                });
                            let cut = a + txt.len() - usize::from(txt.ends_with(','));
                            match sym {
                                Some(s) => slices.push(Slice { start: open + 1 + a, end: open + 1 + cut, group: Some(g), always: false, triggers: vec![s], requirements: vec![] }),
                                None => slices.push(Slice { start: open + 1 + a, end: open + 1 + cut, group: Some(g), always: true, triggers: vec![], requirements: vec![] }),
                            }
                        }
                        slices.push(Slice { start: close, end, group: None, always: true, triggers: vec![], requirements: vec![] });
                        continue;
                    }
                }
            }
        }
        // `var S = { $delete: dartx.delete = Symbol("dartx.delete"), … }`
        // The entry defines two names: `S.$delete` (used in the bodies) and
        // `dartx.delete` (used by our modules) — either one lights it up.
        if (tt.starts_with("var S = {") || tt.starts_with("var S$")) && t.contains("Symbol(")
            && let Some(rel) = t.find('{') {
                let open = start + rel;
                if let Some(close) = scan::closing_brace(src, open) {
                    let body = &src[open + 1..close];
                    let entries = scan::object_entries(body);
                    if entries.len() >= 2 {
                        let var = tt.trim_start_matches("var ").split(' ').next().unwrap_or("S").to_string();
                        let g = group;
                        group += 1;
                        slices.push(Slice { start, end: open + 1, group: None, always: true, triggers: vec![], requirements: vec![] });
                        for (a, b) in entries {
                            let txt = body[a..b].trim_end();
                            let mut trig = Vec::new();
                            if let Some(k) = entry_key(txt) {
                                trig.push(format!("{var}.{k}"));
                            }
                            // `dartx.name = Symbol(…)` or
                            // `dart.privateName(lib, "_name")`
                            if let Some(p) = txt.find("dartx.") {
                                let rest = &txt[p + 6..];
                                let f = ident_end(rest.as_bytes(), 0);
                                if f > 0 {
                                    trig.push(format!("dartx.{}", &rest[..f]));
                                }
                            }
                            let cut = a + txt.len() - usize::from(txt.ends_with(','));
                            let empty = trig.is_empty();
                            slices.push(Slice { start: open + 1 + a, end: open + 1 + cut, group: Some(g), always: empty, triggers: trig, requirements: vec![] });
                        }
                        slices.push(Slice { start: close, end, group: None, always: true, triggers: vec![], requirements: vec![] });
                        continue;
                    }
                }
            }
        // `dart.defineExtensionMethods(C, [a, b])` and its accessor sibling:
        // they install on `C.prototype[dartx.a]` what is on `C.prototype.a`.
        // If member `a` was pruned and the name stayed in the list, the
        // runtime does `defineProperty(proto, dartx.a, undefined)` and throws
        // "Property description must be an object" — that was the first
        // defect that per-member granularity produced. Each name becomes a
        // unit with the **same** condition as the member: `[the class, the
        // selector]`.
        if (tt.starts_with("dart.defineExtensionMethods(") || tt.starts_with("dart.defineExtensionAccessors("))
            && let Some(p) = tt.find('(')
                && let Some(target) = qualified_target(&tt[p + 1..]) {
                    if let Some(rel) = t.find('[') {
                        let open = start + rel;
                        if let Some(close) = scan::closing_bracket(src, open) {
                            let body = &src[open + 1..close];
                            let entries = scan::object_entries(body);
                            if !entries.is_empty() && per_member {
                                let g = group;
                                group += 1;
                                slices.push(Slice { start, end: open + 1, group: None, always: false, triggers: vec![target.clone()], requirements: vec![] });
                                for (a, b) in entries {
                                    let txt = body[a..b].trim_end();
                                    let name = txt.trim().trim_end_matches(',').trim().trim_matches(['\x27', '"']);
                                    let cut = a + txt.len() - usize::from(txt.ends_with(','));
                                    let req = if is_selector_name(name) { vec![target.clone(), format!("sel:{name}")] } else { vec![target.clone()] };
                                    slices.push(Slice { start: open + 1 + a, end: open + 1 + cut, group: Some(g), always: false, triggers: vec![], requirements: req });
                                }
                                slices.push(Slice { start: close, end, group: None, always: false, triggers: vec![target.clone()], requirements: vec![] });
                                continue;
                            }
                        }
                    }
                    slices.push(simple(vec![target]));
                    continue;
                }
        // `dart.registerExtension("XMLHttpRequest", html$.HttpRequest);`
        // — the target is the **second** argument.
        if let Some(r) = tt.strip_prefix("dart.registerExtension(")
            && let Some((tag, rest)) = r.split_once(',')
                && let Some(v) = qualified_target(rest) {
                    let tag = tag.trim().trim_matches(['"', '\x27']);
                    if BUILTIN_TAGS.contains(&tag) {
                        roots.push(v.clone());
                    }
                    slices.push(simple(vec![v]));
                    continue;
                }
        // `lib.Name = …` / `lib.Name[…] = …`
        if let Some(v) = qualified_declaration(tt) {
            if per_member && tt.contains("= class ") && split_class(src, start, end, &v, &mut slices, &mut group) {
                continue;
            }
            slices.push(simple(vec![v]));
            continue;
        }
        // `lib['A|b'] = …` — an extension member, whose Dart name has `|` and
        // `#` and therefore only exists as a string. The string is read
        // whole: a fixed-size window truncated
        // `collection['NullableIterableExtensions|get#nonNulls']` and the
        // declaration was tied to a symbol nobody cites.
        if let Some((a, i)) = first_ident(tt)
            && is_lib(a) && tt.as_bytes().get(i) == Some(&b'[') {
                let b = tt.as_bytes();
                if matches!(b.get(i + 1), Some(b'\'') | Some(b'"')) {
                    let quote = b[i + 1];
                    let start = i + 2;
                    let mut j = start;
                    while j < b.len() && b[j] != quote {
                        j += 1;
                    }
                    if j < b.len() {
                        slices.push(simple(vec![format!("{a}.{}", &tt[start..j])]));
                        continue;
                    }
                }
            }
        // `(lib.Name.ctor = function …).prototype = …` / `(Var[dart.mixinNew] = …)`
        if let Some(r) = tt.strip_prefix('(') {
            if let Some(v) = qualified_target(r) {
                slices.push(simple(vec![v]));
                continue;
            }
            if let Some((a, i)) = first_ident(r)
                && matches!(r.as_bytes().get(i), Some(b'.') | Some(b'[')) {
                    slices.push(simple(vec![a.to_string()]));
                    continue;
                }
        }
        // `dart.anything(lib.Name, …)` / `dart.applyMixin(Var, …)`
        if (tt.starts_with("dart.") || tt.starts_with("dart_rti."))
            && let Some(p) = tt.find('(') {
                let arg = &tt[p + 1..];
                if let Some(v) = qualified_target(arg) {
                    slices.push(simple(vec![v]));
                    continue;
                }
                if let Some((a, i)) = first_ident(arg)
                    && !is_lib(a) && matches!(arg.as_bytes().get(i), Some(b',') | Some(b')')) {
                        slices.push(simple(vec![a.to_string()]));
                        continue;
                    }
            }
        // `var X = …`
        for pref in ["var ", "let ", "const "] {
            if let Some(r) = tt.strip_prefix(pref)
                && let Some((a, _)) = first_ident(r) {
                    slices.push(simple(vec![a.to_string()]));
                    break;
                }
        }
        if slices.last().map(|f| f.start) != Some(start) {
            slices.push(always());
        }
    }
    // The classes of the root libraries: scans what has already been
    // classified.
    for f in &slices {
        for g in &f.triggers {
            if let Some((lib, _)) = g.split_once('.')
                && ROOT_LIBS.contains(&lib) {
                    roots.push(g.clone());
                }
        }
    }
    roots.sort();
    roots.dedup();
    (slices, roots)
}

/// Symbols and selectors that our modules use — the roots of reachability.
pub fn user_roots(modules: &[Module]) -> Vec<String> {
    let mut v = Vec::new();
    for m in modules {
        references(&m.body, &mut v, true);
        selectors(&m.body, &mut v);
        for ns in &m.namespaces {
            references(ns, &mut v, true);
        }
    }
    // The bundle calls `main` at the end, and `dart_sdk.js` needs its own
    // initialization entry points even with nobody citing them.
    for r in ["sel:main", "dart.trackLibraries", "dart._checkModuleNullSafetyMode", "dart.global", "dart.typeUniverse", "dart_rti._theUniverse", "dart_rti._Universe", "core.Object", "core.Error", "_interceptors.Interceptor", "_js_helper.Primitives"] {
        v.push(r.to_string());
    }
    // Only **qualified** symbols become roots. A plain identifier from our
    // module (`main`, `_is`, a local variable) cannot mean a top-level `var`
    // of `dart_sdk.js`: those are internal to the DDC file and were never
    // exported — what crosses the boundary are the 38 library namespaces,
    // and we cite those qualified. Without this filter reachability exploded
    // (4.8 MB against 1.5 MB), because common names such as `name` or
    // `length` matched the SDK's symbol aliases and lit up the world.
    v.retain(|s| s.contains('.') || s.starts_with("C#") || s.starts_with("sel:"));
    v.sort();
    v.dedup();
    v
}

/// Names that `dart_sdk.js` calls **by string** on arbitrary objects:
/// `dart.dsend(o, "toJson", …)`, `dgsend(o, targs, "m", …)`, `dload(o, "x")`,
/// `dput(o, "x", v)`, `bind(o, "m")`. It is the DDC contract — a typed call
/// becomes a direct property, an untyped one carries the name as data
/// (`pkg/dev_compiler/lib/src/kernel/compiler.dart:6102-6126`) — and it is
/// rule (iii) of the SDK→user boundary (`docs/JS-PRODUCAO.md` §1.7): a user
/// member with one of these names can be called by the runtime without the
/// program citing it. The whole file, not just what survives pruning: it is
/// conservative and creates no circular dependency between the two worlds.
pub fn dynamic_selectors(src: &str) -> Vec<String> {
    let (reads, writes) = dynamic_selectors_by_kind(src);
    let mut all = reads;
    all.extend(writes);
    all.sort();
    all.dedup();
    all
}

/// Like [`dynamic_selectors`], separated by kind: `dput`/`dputRepl` write
/// (`foo`, which the verifier matches with `foo_=`), the rest (`dsend`,
/// `dload`, `bind`…) reads or calls.
pub fn dynamic_selectors_by_kind(src: &str) -> (Vec<String>, Vec<String>) {
    let b = src.as_bytes();
    let mut reads: Vec<String> = Vec::new();
    let mut writes: Vec<String> = Vec::new();
    // A single pass: each `(` looks at the identifier that precedes it (7 MB
    // in a few milliseconds; nine `match_indices` cost tens).
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    for (p, _) in b.iter().enumerate().filter(|(_, c)| **c == b'(') {
        let mut i = p;
        while i > 0 && is_id(b[i - 1]) {
            i -= 1;
        }
        let (arg_pos, write) = match &src[i..p] {
            "dput" | "dputRepl" => (1usize, true),
            "dsend" | "dload" | "bind" | "dsendRepl" | "dloadRepl" => (1usize, false),
            "dgsend" | "dgsendRepl" => (2, false),
            _ => continue,
        };
        {
            let mut j = p + 1;
            let mut arg = 0usize;
            let mut depth = 0i32;
            // Advances to the start of argument `arg_pos` (comma at depth
            // zero).
            while j < b.len() && arg < arg_pos {
                match b[j] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    b'"' | b'\'' => {
                        let quote = b[j];
                        j += 1;
                        while j < b.len() && b[j] != quote {
                            if b[j] == b'\\' {
                                j += 1;
                            }
                            j += 1;
                        }
                    }
                    b',' if depth == 0 => arg += 1,
                    _ => {}
                }
                j += 1;
            }
            if arg != arg_pos {
                continue;
            }
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                let quote = b[j];
                let start = j + 1;
                let mut k = start;
                while k < b.len() && b[k] != quote && b[k] != b'\n' {
                    k += 1;
                }
                let s = &src[start..k];
                // A production-profile private name (`"_n$1c"`): the
                // selector is `_n`.
                let s = dartforge_emit_js::dart_name_of_private(s).unwrap_or(s);
                if !s.is_empty() && s.len() < 64 && is_selector_name(s) {
                    if write {
                        writes.push(s.to_string());
                    } else {
                        reads.push(s.to_string());
                    }
                }
            }
        }
    }
    reads.sort();
    reads.dedup();
    writes.sort();
    writes.dedup();
    (reads, writes)
}

/// Index of `dart_sdk.js`: the classification into slices, the reachability
/// units (conditions, references and selectors already interned), the roots
/// the file itself imposes, the exported aliases and the selectors the
/// runtime calls by string. It depends **only** on the text and on
/// `per_member` — not on the program —, so it is built once and stored
/// (`crate::cache`, `docs/JS-PRODUCAO.md` §1.6); the fixed point and the
/// per-program re-emission run on top of it ([`prune_with_index`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkIndex {
    /// `(start, end, group)` of each slice, in file order; the group says
    /// which ones need a comma between the live ones.
    slices: Vec<(usize, usize, Option<u32>)>,
    units: Vec<Unit>,
    /// Symbol names, in `Sym` order.
    symbols: Vec<String>,
    runtime_roots: Vec<reach::Sym>,
    aliases: Vec<(String, String)>,
    /// Names the runtime calls by string, by kind
    /// ([`dynamic_selectors_by_kind`]).
    pub read_selectors: Vec<String>,
    pub write_selectors: Vec<String>,
}

impl SdkIndex {
    /// Units of the file (the report's total).
    pub fn total_units(&self) -> usize {
        self.units.len()
    }

    /// The names called by string, without kind ([`dynamic_selectors`]).
    pub fn dynamic_selectors(&self) -> Vec<String> {
        let mut all = self.read_selectors.clone();
        all.extend(self.write_selectors.iter().cloned());
        all.sort();
        all.dedup();
        all
    }
}

/// Builds the [`SdkIndex`] of `src` (the expensive part of pruning: scanning,
/// classification and reference extraction of the ~45 thousand units).
pub fn index(src: &str, per_member: bool) -> SdkIndex {
    let aliases = crate::bundle::exported_aliases(src);
    let (slices, runtime_roots) = classify(src, per_member);
    let mut symbols = Symbols::default();
    let mut units: Vec<Unit> = Vec::with_capacity(slices.len());
    let mut buf_refs: Vec<String> = Vec::new();
    let mut buf_sels: Vec<String> = Vec::new();
    for f in &slices {
        let t = &src[f.start..f.end];
        buf_refs.clear();
        references(t, &mut buf_refs, false);
        buf_sels.clear();
        if per_member {
            selectors(t, &mut buf_sels);
        }
        let mut u = Unit {
            always: f.always,
            triggers: f.triggers.iter().map(|s| symbols.intern(s)).collect(),
            requirements: f.requirements.iter().map(|s| symbols.intern(s)).collect(),
            refs: Vec::with_capacity(buf_refs.len()),
            selectors: Vec::with_capacity(buf_sels.len()),
        };
        let mut seen: HashSet<&str> = HashSet::new();
        for r in &buf_refs {
            if seen.insert(r.as_str()) {
                u.refs.push(symbols.intern(r));
            }
        }
        seen.clear();
        for r in &buf_sels {
            if seen.insert(r.as_str()) {
                u.selectors.push(symbols.intern(r));
            }
        }
        units.push(u);
    }
    let runtime_roots = runtime_roots
        .iter()
        // Local variables exported under another name must exist even when
        // the program only cites the library's alias.
        .chain(aliases.iter().map(|(original, _)| original))
        .map(|r| symbols.intern(r))
        .collect();
    let (read_selectors, write_selectors) = dynamic_selectors_by_kind(src);
    SdkIndex {
        slices: slices.iter().map(|f| (f.start, f.end, f.group)).collect(),
        units,
        symbols: symbols.names(),
        runtime_roots,
        aliases,
        read_selectors,
        write_selectors,
    }
}

/// Prunes `dart_sdk.js`: returns the pruned text, the total number of units
/// and the live ones.
pub fn prune(src: &str, roots: &[String], per_member: bool) -> (String, usize, usize) {
    prune_with_index(src, &index(src, per_member), roots)
}

/// Like [`prune`], over an already built index of `src` (the same text: the
/// slices are ranges of it).
pub fn prune_with_index(src: &str, index: &SdkIndex, roots: &[String]) -> (String, usize, usize) {
    let mut symbols = Symbols::from_names(index.symbols.clone());
    let units = &index.units;
    let slices = &index.slices;
    let aliases = &index.aliases;
    let roots: Vec<u32> = roots.iter().map(|r| symbols.intern(r)).chain(index.runtime_roots.iter().copied()).collect();
    let live = reach::solve(units, &roots, symbols.total());
    let live_count = live.iter().filter(|v| **v).count();
    // The key of a slice in diagnostics: the first trigger, otherwise the
    // first requirement.
    let key_of = |i: usize| -> &str {
        let u = &units[i];
        u.triggers.first().or(u.requirements.first()).map(|&s| index.symbols[s as usize].as_str()).unwrap_or("(always)")
    };

    // `DARTFORGE_JSPROD_DEBUG=1` tells **who** is taking up the file. It is
    // how one finds the symbol that pulls in the world (that is how it was
    // discovered that `dart.applyMixin` was being read as a declaration, and
    // not as a call).
    if std::env::var("DARTFORGE_JSPROD_DEBUG").is_ok_and(|v| v != "0") {
        let mut by_symbol: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, &(start, end, _)) in slices.iter().enumerate() {
            if !live[i] {
                continue;
            }
            *by_symbol.entry(key_of(i)).or_default() += end - start;
        }
        let mut v: Vec<(&str, usize)> = by_symbol.into_iter().collect();
        // A tie in size is broken by the symbol (the `HashMap` has no order).
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        eprintln!("[jsprod] {} roots, {} symbols, {live_count}/{} live units", roots.len(), symbols.total(), slices.len());
        for (s, n) in v.iter().take(25) {
            eprintln!("[jsprod] {n:>8}  {s}");
        }
    }
    // `DARTFORGE_JSPROD_TRIGGER=html$` lists the live units tied to a
    // symbol.
    if let Ok(target) = std::env::var("DARTFORGE_JSPROD_TRIGGER") {
        for target in target.split(',') {
            let Some(d) = symbols.lookup(target) else { continue };
            for (i, &(start, end, _)) in slices.iter().enumerate() {
                if live[i] && (units[i].triggers.contains(&d) || units[i].requirements.contains(&d)) {
                    let t = &src[start..end];
                    eprintln!("[jsprod] [{target}] {:>7}B {:?}", end - start, &t[..t.len().min(100)]);
                }
            }
        }
    }
    // `DARTFORGE_JSPROD_WHO=html$.Element` lists the live units that cite
    // the symbol — with the text, which is what makes it possible to fix the
    // classification.
    if let Ok(target) = std::env::var("DARTFORGE_JSPROD_WHO") {
        for target in target.split(',') {
            let Some(d) = symbols.lookup(target) else {
                eprintln!("[jsprod] {target}: no such symbol");
                continue;
            };
            let mut n = 0;
            for (i, u) in units.iter().enumerate() {
                if live[i] && u.refs.contains(&d) && n < 6 {
                    let t = &src[slices[i].0..slices[i].1];
                    let key = key_of(i);
                    eprintln!("[jsprod] {target} cited by [{key}] {:?}", &t[..t.len().min(110)]);
                    n += 1;
                }
            }
        }
    }
    // `DARTFORGE_JSPROD_PATH=core._BigIntImpl` answers who pulled it in.
    if let Ok(target) = std::env::var("DARTFORGE_JSPROD_PATH") {
        for target in target.split(',') {
            match symbols.lookup(target).and_then(|d| reach::path(units, &roots, symbols.total(), d)) {
                Some(c) => eprintln!("[jsprod] {target} <- {}", c.iter().rev().map(|&s| symbols.name(s)).collect::<Vec<_>>().join(" <- ")),
                None => eprintln!("[jsprod] {target}: not reached"),
            }
        }
    }

    // Re-emits in file order. The entries of a group (object or JSON) need
    // commas between the live ones — and in JSON a trailing comma is a
    // syntax error, not tolerance.
    let mut out = String::with_capacity(src.len() / 2);
    let mut open_group: Option<u32> = None;
    for (i, &(start, end, group)) in slices.iter().enumerate() {
        if !live[i] {
            continue;
        }
        match group {
            Some(g) => {
                if open_group == Some(g) {
                    out.push(',');
                }
                open_group = Some(g);
            }
            None => open_group = None,
        }
        let t = &src[start..end];
        if t.starts_with("export {") {
            continue;
        }
        out.push_str(t);
    }
    for (original, alias) in aliases {
        out.push_str(&format!("var {alias} = {original};\n"));
    }
    (out, slices.len(), live_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rti_recipe_reference() {
        let mut v = Vec::new();
        references("eval(u, \"core|List<core|int>\", true)", &mut v, false);
        assert!(v.contains(&"core.List".to_string()), "{v:?}");
        assert!(v.contains(&"core.int".to_string()), "{v:?}");
    }

    #[test]
    fn html_reference_with_dollar() {
        let mut v = Vec::new();
        references("\"html|Element\"", &mut v, false);
        assert!(v.contains(&"html$.Element".to_string()), "{v:?}");
    }

    #[test]
    fn sdk_alias_in_user_module_points_to_the_local_name() {
        let mut v = Vec::new();
        references("html.Element; svg['SvgElement'];", &mut v, true);
        assert!(v.contains(&"html$.Element".to_string()), "{v:?}");
        assert!(v.contains(&"svg$.SvgElement".to_string()), "{v:?}");
        let src = "var html$ = Object.create(dart.library);\nexport { html$ as html };\nhtml$.Element = class Element {};\n";
        let (out, _, _) = prune(src, &["html$.Element".into()], false);
        assert!(out.contains("var html$ ="), "{out}");
        assert!(out.contains("var html = html$;"), "{out}");
        assert!(out.contains("html$.Element ="), "{out}");
    }

    #[test]
    fn constant_and_bar_member_references() {
        let mut v = Vec::new();
        references("C[42] + CT.C7 + async['FutureRecord2|get#wait']", &mut v, false);
        assert!(v.contains(&"C#42".to_string()), "{v:?}");
        assert!(v.contains(&"C#7".to_string()), "{v:?}");
        assert!(v.contains(&"async.FutureRecord2|get#wait".to_string()), "{v:?}");
    }

    #[test]
    fn symbol_table_private_entry_reference() {
        let mut refs = Vec::new();
        references("this[S$1._head$1] + this[S.$head] + this[S$._private]", &mut refs, false);
        assert!(refs.contains(&"S$1._head$1".to_string()), "{refs:?}");
        assert!(refs.contains(&"S.$head".to_string()), "{refs:?}");
        assert!(refs.contains(&"S$._private".to_string()), "{refs:?}");

        let src = concat!(
            "var html$ = Object.create(dart.library);\n",
            "var S$1 = {\n",
            "  $other: dartx.other = Symbol('dartx.other'),\n",
            "  _head$1: dart.privateName('html', '_head'),\n",
            "  $unused: dartx.unused = Symbol('dartx.unused')\n",
            "};\n",
            "html$.Document = class Document {\n",
            "  get [S$1._head$1]() { return this.head; }\n",
            "};\n",
        );
        let (out, _, _) = prune(src, &["html$.Document".into()], true);
        assert!(out.contains("_head$1: dart.privateName"), "{out}");
        assert!(!out.contains("$unused: dartx.unused"), "{out}");
    }

    #[test]
    fn does_not_mistake_field_for_library() {
        let mut v = Vec::new();
        references("this.core = opts.io;", &mut v, false);
        assert!(!v.contains(&"core.x".to_string()));
        assert!(v.contains(&"this".to_string()));
    }

    #[test]
    fn class_members() {
        assert_eq!(member_name("  toString() {\n  }").as_deref(), Some("toString"));
        assert_eq!(member_name("  get length() { return 1; }").as_deref(), Some("length"));
        assert_eq!(member_name("  static f(a) {}").as_deref(), Some("f"));
        assert_eq!(member_name("  [_priv](a) {}").as_deref(), Some("_priv"));
        assert_eq!(member_name("  [$add](a) {}").as_deref(), Some("add"));
        assert_eq!(member_name("  ['A|b'](a) {}").as_deref(), Some("A|b"));
        assert_eq!(member_name("  get [S.$head]() { return this.head; }").as_deref(), Some("head"));
        assert_eq!(member_name("  get [S$2.$console]() { return x; }").as_deref(), Some("console"));
        let mut refs = Vec::new();
        selectors("document[S.$head]; window[S$2.$console];", &mut refs);
        assert!(refs.contains(&"sel:head".to_string()), "{refs:?}");
        assert!(refs.contains(&"sel:console".to_string()), "{refs:?}");
    }

    /// Regression of the defect that made `dart.applyMixin(V, M);` be read as
    /// a declaration of `dart.applyMixin` — a symbol that only its own call
    /// sites reference, so the whole set erased itself.
    #[test]
    fn call_is_not_declaration() {
        assert_eq!(qualified_declaration("core.Object = class {};"), Some("core.Object".into()));
        assert_eq!(qualified_declaration("core.Object[dart.x] = 1;"), Some("core.Object".into()));
        assert_eq!(qualified_declaration("dart.applyMixin(V, M);"), None);
        assert_eq!(qualified_declaration("dart.setLibraryUri(core.Uri, I[1]);"), None);
        assert_eq!(qualified_declaration("core.a === core.b;"), None);
    }

    /// Regression of the defect that put the whole of `dart:html` into a
    /// program that only does `print`: the target of `defineLazy` can be a
    /// **class**, and reading only the first identifier tied the group's head
    /// to the `html$` namespace, which is always live and whose head cites
    /// the class.
    #[test]
    fn class_define_lazy_does_not_tie_to_namespace() {
        let src = concat!(
            "var html$ = Object.create(dart.library);\n",
            "html$.Event = class Event {};\n",
            "dart.defineLazy(html$.Event, {\n",
            "  get A() { return 1; },\n",
            "  get B() { return 2; }\n",
            "});\n",
        );
        // `html$` alone cannot light up `html$.Event`.
        let (out, _, _) = prune(src, &["html$".into()], false);
        assert!(!out.contains("html$.Event = class"), "{out}");
        let (out, _, _) = prune(src, &["html$.Event".into()], false);
        assert!(out.contains("html$.Event = class"), "{out}");
    }

    /// The `defineExtensionMethods` list must lose the name together with
    /// the member: the runtime copies `C.prototype[name]` to
    /// `C.prototype[dartx.name]` and throws if the descriptor does not exist.
    #[test]
    fn extension_list_follows_the_member() {
        let src = concat!(
            "var core = Object.create(dart.library);\n",
            "core.C = class C {\n  viva() { return 1; }\n  morta() { return 2; }\n  outra() { return 3; }\n};\n",
            "dart.defineExtensionMethods(core.C, ['viva', 'morta', 'outra']);\n",
        );
        let (out, _, _) = prune(src, &["core.C".into(), "sel:viva".into()], true);
        assert!(out.contains("'viva'"), "{out}");
        assert!(!out.contains("'morta'"), "{out}");
        // The list stays syntactically valid (commas put back).
        assert!(out.contains("dart.defineExtensionMethods(core.C, ["), "{out}");
    }

    #[test]
    fn pruning_keeps_what_lives_and_drops_the_rest() {
        let src = concat!(
            "var core = Object.create(dart.library);\n",
            "core.Usada = class Usada { f() { return 1; } };\n",
            "core.Morta = class Morta { g() { return 2; } };\n",
        );
        let (out, total, live) = prune(src, &["core.Usada".into(), "sel:f".into()], false);
        assert!(out.contains("core.Usada"), "{out}");
        assert!(!out.contains("core.Morta"), "{out}");
        assert!(live < total);
    }
}
