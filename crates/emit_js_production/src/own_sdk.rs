//! Production profile with the SDK compiled through our own pipeline
//! (`docs/JS-PRODUCAO-SDK-PROPRIO.md`).
//!
//! The final file is the *bootstrap* (`dart`, namespaces, `dartx`), the SDK
//! module emitted by the SDK mode of `crates/emit_js` and the program's
//! modules, as in the old path — but without a single byte of `dart_sdk.js`.
//!
//! There is a single closed world, with the SDK inside it (§4 of the
//! specification): what the emitter writes without it being in the AST
//! (`dart.fn`, `JSArray.of`, `dart.str`…) is found by the text verifier and
//! comes back as a root, until the fixed point.

use crate::{Options, Production, WorldReport, bundle, filter, minify, verify};
use dartforge_elements::model::{Element, LibraryId};
use dartforge_emit_js::EmittedWithSdk;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// `registerExtension` tags that are **JavaScript built-in** types: the value
/// (`"olá"`, `42`, `[]`, a function) becomes a Dart instance through the tag,
/// without any name in the program. They are roots, like the impacts of
/// `js_backend/backend_impact.dart` in dart2js.
const BUILTIN_TAGS: &[&str] = &[
    "Object",
    "String",
    "Number",
    "Boolean",
    "Array",
    "Function",
    "Symbol",
    "BigInt",
    "Error",
    "TypeError",
    "RangeError",
];

/// Initial roots: `main`, `core.Object` and the classes of the built-in tags.
/// The rest comes from the fixed point with the text.
fn roots(
    a: &dartforge_emit_js::Analysis<'_>,
    e: &dartforge_world::WorldInput<'_>,
) -> dartforge_world::Roots {
    let mut r = dartforge_world::Roots::default();
    if let (Some(lib), Some(sym)) = (a.program.entry, a.interner.lookup("main"))
        && let Some(Element::Function(f)) = a
            .program
            .library(lib)
            .exported
            .get(&sym)
            .and_then(|b| b.getter)
    {
        r.functions.push(f);
    }
    // With the `Object` members as selectors (`dartforge_world`'s
    // `object_selectors`), `core.Object` is instantiated by `Object()` or by
    // the foreign gate, not as a root.
    let object_root = std::env::var("DARTFORGE_JSPROD_OBJECT_SELECTORS").is_ok_and(|v| v == "0");
    if object_root
        && let (Some(core), Some(sym)) = (a.program.core, a.interner.lookup("Object"))
        && let Some(Element::Class(c)) = a
            .program
            .library(core)
            .declared
            .get(&sym)
            .and_then(|b| b.getter)
    {
        r.instantiated_classes.push(c);
    }
    for (c, tags) in dartforge_world::native_tags(e) {
        if tags.iter().any(|t| BUILTIN_TAGS.contains(&t.as_str())) {
            r.instantiated_classes.push(c);
        }
    }
    r
}

/// JS namespace → library, for the verifier: the program's (`L$…`) and the
/// SDK's (`core`, `dart`, `dart_rti`…).
fn libraries(
    a: &dartforge_emit_js::Analysis<'_>,
    modules: &[(String, String)],
) -> HashMap<String, LibraryId> {
    let mut libs = verify::libraries_from_text(modules, a.program);
    for (i, l) in a.program.libraries.iter().enumerate() {
        let Some(n) = l.uri.strip_prefix("dart:") else {
            continue;
        };
        let var = match n {
            "_runtime" => "dart",
            "_rti" => "dart_rti",
            x => x,
        };
        libs.insert(var.to_string(), LibraryId(i as u32));
    }
    libs
}

/// What the production profile removes from the program's text before
/// checking and assembling (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §6):
/// `dart.trackLibraries(…)` (registration for the debugger) and
/// `dart._checkModuleNullSafetyMode(…)` (a check between separately compiled
/// modules) — in a single file, neither has a consumer. Removed before
/// verification, the runtime functions they call leave the world too.
fn strip_unused(e: &mut EmittedWithSdk) {
    for (_, t) in e.emitted.modules.iter_mut() {
        let mut out = String::with_capacity(t.len());
        let mut skip = false;
        for line in t.split_inclusive('\n') {
            if skip {
                if line.starts_with('}') && line.trim_end().ends_with(");") {
                    skip = false;
                }
                continue;
            }
            if line.starts_with("dart._checkModuleNullSafetyMode(") {
                continue;
            }
            if line.starts_with("dart.trackLibraries(") {
                skip = !line.trim_end().ends_with(");");
                continue;
            }
            out.push_str(line);
        }
        *t = out;
    }
}

/// Without `dart:html` and its family, the world holds the runtime's DOM
/// `polyfill` (`dartforge_world::DOM_LIBRARIES`): its call in the
/// initializer of `dart.global` leaves the text too, so the verifier does
/// not bring it back.
fn strip_polyfill(
    a: &dartforge_emit_js::Analysis<'_>,
    world: &dartforge_world::World,
    e: &mut EmittedWithSdk,
) {
    let input = dartforge_world::WorldInput {
        program: a.program,
        interner: a.interner,
        table: a.table,
        outline: a.outline,
        bodies: a.bodies,
    };
    let dead = (0..a.program.functions.len())
        .map(|i| dartforge_elements::model::FunctionElementId(i as u32))
        .any(|f| dartforge_world::is_dom_polyfill(input, f) && !world.is_live_function(f));
    if !dead {
        return;
    }
    for t in [&mut e.sdk, &mut e.bootstrap] {
        if let Some(k) = t.find("dart.polyfill(globalState);") {
            t.replace_range(k..k + "dart.polyfill(globalState);".len(), "");
        }
    }
}

/// Member names that the text passes to dynamic dispatch or to a *tearoff*
/// (`dsend`/`dgsend`/`dload`/`dput`/`bind` and the `Repl` variants): the
/// literal strings and the private symbols (`var x = dart.privateName(L,
/// "_n")` used as an argument).
fn dynamic_names(texts: &[(String, String)]) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for (_, t) in texts {
        let (r, w) = crate::sdk::signature_selectors_by_kind(t);
        out.extend(r);
        out.extend(w);
        let mut privates: HashMap<&str, &str> = HashMap::new();
        for line in t.lines() {
            let Some(r) = line.strip_prefix("var ") else {
                continue;
            };
            let Some((var, rest)) = r.split_once(" = dart.privateName(") else {
                continue;
            };
            let Some(q) = rest.find('"') else { continue };
            let rest = &rest[q + 1..];
            let Some(end) = rest.find('"') else { continue };
            privates.insert(var, &rest[..end]);
        }
        let b = t.as_bytes();
        for (p, _) in t.match_indices('(') {
            let mut i = p;
            while i > 0
                && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'$')
            {
                i -= 1;
            }
            let position = match &t[i..p] {
                "dsend" | "dload" | "dput" | "bind" | "dsendRepl" | "dloadRepl" | "dputRepl" => 1,
                "dgsend" | "dgsendRepl" => 2,
                _ => continue,
            };
            // The argument at the requested position, up to the comma at
            // depth zero.
            let mut j = p + 1;
            let mut arg = 0;
            let mut depth = 0i32;
            let mut start = j;
            while j < b.len() {
                match b[j] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' if depth == 0 => break,
                    b')' | b']' | b'}' => depth -= 1,
                    b',' if depth == 0 => {
                        if arg == position {
                            break;
                        }
                        arg += 1;
                        start = j + 1;
                    }
                    _ => {}
                }
                j += 1;
            }
            if arg == position {
                let a = t[start..j].trim();
                if let Some(n) = privates.get(a) {
                    out.insert(n.to_string());
                } else if a.len() >= 2
                    && (a.starts_with('"') && a.ends_with('"')
                        || a.starts_with('\'') && a.ends_with('\''))
                {
                    // Operators (`dsend(a, "+", [b])`), which extraction by
                    // identifier does not accept, and the program's private
                    // names (`"_n$1c"` → `_n`).
                    let s = &a[1..a.len() - 1];
                    out.insert(
                        dartforge_emit_js::dart_name_of_private(s)
                            .unwrap_or(s)
                            .to_string(),
                    );
                }
            }
        }
    }
    out
}

/// `DARTFORGE_JSPROD_WHY=name`: each function (of the program or of the SDK)
/// with that name, whether it is live, its owner class and that class's
/// level, and whether the selector is live for it.
fn diagnose(a: &dartforge_emit_js::Analysis<'_>, world: &dartforge_world::World, target: &str) {
    let p = a.program;
    for (i, f) in p.functions.iter().enumerate() {
        if a.interner.resolve(f.name) != target {
            continue;
        }
        let fid = dartforge_elements::model::FunctionElementId(i as u32);
        let owner = f
            .class
            .map(|c| {
                format!(
                    "{} [{:?}] selector={}",
                    a.interner.resolve(p.class(c).name),
                    world.class_level(c),
                    world.selector_live_for(p, target, c)
                )
            })
            .unwrap_or_default();
        eprintln!(
            "[jsprod] {}::{target} live={} {owner} cause={:?}",
            p.library(f.library).uri,
            world.is_live_function(fid),
            world.function_cause(fid)
        );
        eprintln!(
            "[jsprod]   chain: {}",
            cause_chain(a, world, world.function_cause(fid))
        );
    }
    for (i, c) in p.classes.iter().enumerate() {
        if a.interner.resolve(c.name) == target {
            let cid = dartforge_elements::model::ClassId(i as u32);
            eprintln!(
                "[jsprod] class {}::{target} [{:?}] chain: {}",
                p.library(c.library).uri,
                world.class_level(cid),
                cause_chain(a, world, world.class_cause(cid))
            );
        }
    }
}

/// The chain of liveness causes, from an element up to a root (diagnostics).
fn cause_chain(
    a: &dartforge_emit_js::Analysis<'_>,
    world: &dartforge_world::World,
    mut cause: Option<dartforge_world::Cause>,
) -> String {
    use dartforge_world::Cause;
    let p = a.program;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    while let Some(c) = cause {
        if !seen.insert(format!("{c:?}")) || out.len() > 40 {
            out.push("…".to_string());
            break;
        }
        let (label, next) = match c {
            Cause::Root => ("root".to_string(), None),
            Cause::Function(f) => {
                let e = p.function(f);
                let owner = e
                    .class
                    .map(|c| format!("{}.", a.interner.resolve(p.class(c).name)))
                    .unwrap_or_default();
                (
                    format!("{owner}{}", a.interner.resolve(e.name)),
                    world.function_cause(f),
                )
            }
            Cause::Variable(v) => (
                format!("var {}", a.interner.resolve(p.variable(v).name)),
                world.variable_cause(v),
            ),
            Cause::Class(k) => (
                format!("class {}", a.interner.resolve(p.class(k).name)),
                world.class_cause(k),
            ),
        };
        out.push(label);
        cause = next;
    }
    out.join(" <- ")
}

/// One phase of the fixed point: world → filtered emission → text
/// verification. Returns the final emission, or `None` if it did not
/// converge.
fn fixed_point(
    a: &dartforge_emit_js::Analysis<'_>,
    input: dartforge_world::WorldInput<'_>,
    op: Options,
    signatures: Option<&HashSet<String>>,
    report: &mut WorldReport,
) -> Result<Option<EmittedWithSdk>, String> {
    let mut roots = roots(a, &input);
    // No records at run time until the world or the text shows one
    // (`DARTFORGE_JSPROD_RECORDS_GATE=0` assumes they may exist).
    let mut opm = dartforge_world::Options {
        include_sdk: true,
        without_asserts: !op.asserts,
        without_records: std::env::var("DARTFORGE_JSPROD_RECORDS_GATE").map_or(true, |v| v != "0"),
    };
    let constants = dartforge_world::Constants::new(input, !op.asserts);
    constants.set_without_records(opm.without_records);
    let identity_hash = a.program.core.and_then(|core| {
        match a
            .program
            .library(core)
            .declared
            .get(&a.interner.lookup("identityHashCode")?)?
            .getter?
        {
            Element::Function(f) => Some(f),
            _ => None,
        }
    });
    let max_rounds: usize = std::env::var("DARTFORGE_JSPROD_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
        .max(1);
    let trace = std::env::var("DARTFORGE_JSPROD_TRACE").is_ok();
    // The engine stays alive between rounds: each one only adds roots.
    let mut computation = dartforge_world::Computation::new(input, opm);
    for round in 1..=max_rounds {
        let t = Instant::now();
        computation.seed(&roots);
        let mut world = computation.world();
        if opm.without_records && world.records_seen() {
            // The assumption failed: the record branches the world skipped
            // are needed, so it starts over with them.
            opm.without_records = false;
            constants.set_without_records(false);
            computation = dartforge_world::Computation::new(input, opm);
            computation.seed(&roots);
            world = computation.world();
        }
        report.world_time += t.elapsed();
        // The emitter folds the same `p == null` the world folded.
        constants.set_null_params(world.null_params());
        if trace {
            let names: Vec<&str> = world
                .null_params()
                .iter()
                .map(|k| a.interner.resolve(k.name))
                .collect();
            eprintln!(
                "[jsprod] round {round}: {} parameter(s) always null: {}",
                names.len(),
                names.join(", ")
            );
        }
        // `DARTFORGE_JSPROD_NAMED=0` keeps the source names.
        let named_args = if std::env::var("DARTFORGE_JSPROD_NAMED").map_or(true, |v| v != "0") {
            crate::named_args::compute(a, &world)
        } else {
            None
        };
        let ad = filter::Adapter {
            world: &world,
            program: a.program,
            stub: op.stub,
            signatures,
            constants: Some(&constants),
            named_args: named_args.as_ref(),
            omit_checks: op.omit_checks,
            identity_hash,
        };
        let t = Instant::now();
        let mut e = a.emit_with_sdk(Some(&ad))?;
        // The namespace → library map comes from `trackLibraries`, which
        // `strip_unused` removes: read before.
        let libs = libraries(a, &e.emitted.modules);
        strip_unused(&mut e);
        strip_polyfill(a, &world, &mut e);
        report.emit_time += t.elapsed();
        let t = Instant::now();
        let mut texts = e.emitted.modules.clone();
        texts.push((
            "dart_sdk.js".to_string(),
            format!("{}{}", e.bootstrap, e.sdk),
        ));
        let missing = verify::check(&texts, &libs, a.program, a.interner, &world);
        report.verify_time += t.elapsed();
        report.rounds += 1;
        // A record recipe the world did not see (a signature, a rule): the
        // same restart, and another round.
        if opm.without_records && texts.iter().any(|(_, t)| cites_record_recipe(t)) {
            opm.without_records = false;
            constants.set_without_records(false);
            computation = dartforge_world::Computation::new(input, opm);
            continue;
        }
        report.no_element = missing.no_element.clone();
        // `DARTFORGE_JSPROD_DUMP=file`: the round's text (diagnostics).
        if let Ok(p) = std::env::var("DARTFORGE_JSPROD_DUMP") {
            let _ = std::fs::write(
                p,
                texts
                    .iter()
                    .map(|(_, t)| t.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if trace {
            eprintln!(
                "[jsprod] round {round}: {} missing",
                missing.description.len()
            );
            for d in missing.description.iter().take(40) {
                eprintln!("[jsprod]   {d}");
            }
        }
        if missing.is_empty() {
            report.stats = world.stats.clone();
            report.js_names = world.js_names.clone();
            report.named_args = named_args.clone();
            if let Ok(target) = std::env::var("DARTFORGE_JSPROD_WHY") {
                diagnose(a, &world, &target);
            }
            return Ok(Some(e));
        }
        report.fixes.extend(missing.description.iter().cloned());
        roots.functions.extend(missing.functions);
        roots.variables.extend(missing.variables);
        roots.type_classes.extend(missing.classes);
        roots
            .instantiated_classes
            .extend(missing.instantiated_classes);
        roots.tearoffs.extend(missing.tearoffs);
        roots.selectors.extend(missing.selectors);
    }
    Ok(None)
}

/// Whether `text` holds a string literal that looks like an rti recipe with
/// a record type (`"+(|a,|b)"`, `"|F<+n(|a)>"`): `+`, then the partial
/// shape, then `(`, in a literal without spaces. A false positive only keeps
/// the record branches of `dart:_rti`.
fn cites_record_recipe(text: &str) -> bool {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < b.len() && b[j] != b'"' && b[j] != b'\n' {
            j += if b[j] == b'\\' { 2 } else { 1 };
        }
        let lit = &b[start..j.min(b.len())];
        if !lit.contains(&b' ') {
            for (k, &c) in lit.iter().enumerate() {
                if c != b'+' {
                    continue;
                }
                let mut m = k + 1;
                while m < lit.len()
                    && (lit[m].is_ascii_alphanumeric()
                        || lit[m] == b','
                        || lit[m] == b'_'
                        || lit[m] == b'$')
                {
                    m += 1;
                }
                if m < lit.len() && lit[m] == b'(' {
                    return true;
                }
            }
        }
        i = j + 1;
    }
    false
}

/// Closed world (with the SDK) → filtered emission → text verification,
/// until the fixed point; in two phases (§6.1 of the specification): the
/// first, with every signature, finds the names of dynamic dispatch; the
/// second starts again from the roots emitting only the signatures of those
/// names — whatever they kept alive only as a type goes too. If the second
/// finds new names, they go in and it starts again (the set only grows: it
/// terminates).
///
/// # Errors
///
/// Those of the emission.
pub fn emit(
    a: &dartforge_emit_js::Analysis<'_>,
    op: Options,
) -> Result<(EmittedWithSdk, Option<WorldReport>), String> {
    if !op.prune_user {
        let mut e = a.emit_with_sdk(None)?;
        strip_unused(&mut e);
        return Ok((e, None));
    }
    let input = dartforge_world::WorldInput {
        program: a.program,
        interner: a.interner,
        table: a.table,
        outline: a.outline,
        bodies: a.bodies,
    };
    let mut report = WorldReport::default();
    let Some(first) = fixed_point(a, input, op, None, &mut report)? else {
        report.pruning_disabled = true;
        report
            .fixes
            .push("fixed point not reached; pruning disabled".to_string());
        let mut e = a.emit_with_sdk(None)?;
        strip_unused(&mut e);
        return Ok((e, Some(report)));
    };
    if std::env::var("DARTFORGE_JSPROD_SIGNATURES").is_ok_and(|v| v == "all") {
        return Ok((first, Some(report)));
    }
    let texts = |e: &EmittedWithSdk| {
        let mut v = e.emitted.modules.clone();
        v.push(("dart_sdk.js".to_string(), e.sdk.clone()));
        v
    };
    let mut names = dynamic_names(&texts(&first));
    let trace = std::env::var("DARTFORGE_JSPROD_TRACE").is_ok();
    for phase in 0..8 {
        let Some(e) = fixed_point(a, input, op, Some(&names), &mut report)? else {
            if trace {
                eprintln!("[jsprod] signature phase {phase}: the fixed point did not converge");
            }
            break;
        };
        let new = dynamic_names(&texts(&e));
        if new.is_subset(&names) {
            return Ok((e, Some(report)));
        }
        if trace {
            let mut extra: Vec<&String> = new.difference(&names).collect();
            extra.sort();
            eprintln!(
                "[jsprod] signature phase {phase}: {} new name(s): {:?}",
                extra.len(),
                extra.iter().take(20).collect::<Vec<_>>()
            );
        }
        names.extend(new);
    }
    if trace {
        eprintln!("[jsprod] signature phases did not close: every signature stays");
    }
    // The second phase did not close: the first one (with every signature)
    // stands.
    Ok((first, Some(report)))
}

/// The prefix of every closed rti recipe the emitter writes.
const RTI_PREFIX: &str = "dart_rti._Universe.eval(dart_rti._theUniverse(), ";

/// Replaces `dart_rti._Universe.eval(dart_rti._theUniverse(), "R", true)` with
/// `t$R("R")` (§6.3 of the specification): the same call, through a function
/// declared once. `t$` is a prefix reserved by the emitter (a Dart identifier
/// starting with it is renamed), so `t$R` collides with nothing.
pub fn alias_recipes(js: &str) -> String {
    let mut out = String::with_capacity(js.len());
    let mut rest = js;
    while let Some(p) = rest.find(RTI_PREFIX) {
        let after = &rest[p + RTI_PREFIX.len()..];
        let b = after.as_bytes();
        // A double-quoted JS string, then `, true)`.
        let end = if b.first() == Some(&b'"') {
            let mut i = 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            (i < b.len() && after[i + 1..].starts_with(", true)")).then_some(i + 1)
        } else {
            None
        };
        match end {
            Some(f) => {
                out.push_str(&rest[..p]);
                out.push_str("t$R(");
                out.push_str(&after[..f]);
                out.push(')');
                rest = &after[f + ", true)".len()..];
            }
            None => {
                out.push_str(&rest[..p + RTI_PREFIX.len()]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Short recipe id → class name (`dartforge_emit_js::recipe_ids`), for
/// the table that `_rtiToString` looks up (§9.5 of
/// `docs/JS-PRODUCAO-TAMANHO.md`).
pub fn recipe_names(a: &dartforge_emit_js::Analysis<'_>) -> HashMap<String, String> {
    dartforge_emit_js::recipe_ids(a.program, a.interner)
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            (
                id,
                a.interner
                    .resolve(
                        a.program
                            .class(dartforge_elements::model::ClassId(i as u32))
                            .name,
                    )
                    .to_string(),
            )
        })
        .collect()
}

/// What DDC's `unmangleGlobalNameIfPreservedAnyways` (`js_names.dart`)
/// returns: `null`. In SDK mode with short ids, it reads the `t$N` table.
const UNMANGLE: &str = "unmangleGlobalNameIfPreservedAnyways = function unmangleGlobalNameIfPreservedAnyways(name) {\nreturn null;";

/// Puts into the file the `t$N` table of the names of the classes the text
/// can show and hooks `unmangleGlobalNameIfPreservedAnyways` to it. The
/// table is a single *string*, with the names separated by spaces in id
/// order (`dartforge_emit_js::id_number`), empty for those that do not
/// appear: one byte per class instead of the quotes and key of an object.
/// `t$I` converts the id into the number, like `id_number`. Without the
/// function in the text (nobody shows types), nothing changes.
fn name_table(js: &str, rti: &HashMap<String, String>) -> String {
    let Some(p) = js.find(UNMANGLE) else {
        return js.to_string();
    };
    let displayable = crate::rti_rules::displayable(js);
    let mut names: Vec<&str> = Vec::new();
    for k in &displayable {
        let Some(id) = k.strip_prefix('|') else {
            continue;
        };
        let (Some(name), Some(n)) = (rti.get(id), dartforge_emit_js::id_number(id)) else {
            continue;
        };
        if names.len() <= n {
            names.resize(n + 1, "");
        }
        names[n] = name.as_str();
    }
    let list = names.join(" ");
    let front = front_coded(&names);
    // The decoder slices UTF-16 units: only for ASCII names (Dart identifiers
    // are). In a small table the gain does not pay the decoder and gzip
    // loses a little: only from 1 KB saved on.
    let table = if front.len() + FRONT_DECODER.len() + 1024 < list.len()
        && names.iter().all(|n| n.is_ascii())
    {
        FRONT_DECODER.replace("#", &dartforge_emit_js::js::string_literal(&front))
    } else {
        format!(
            "{}.split(\" \")",
            dartforge_emit_js::js::string_literal(&list)
        )
    };
    let first = std::str::from_utf8(dartforge_emit_js::ID_FIRST).unwrap_or_default();
    let others = std::str::from_utf8(dartforge_emit_js::ID_NEXT).unwrap_or_default();
    let mut out = String::with_capacity(js.len() + list.len() + 400);
    out.push_str(&format!(
        "var t$N = {table};\nvar t$I = function(id) {{ let a = \"{first}\", b = \"{others}\", n = a.indexOf(id[1]), p = {}; for (let i = 2; i < id.length; i++) {{ n += (b.indexOf(id[i]) + 1) * p; p *= {}; }} return n; }};\n",
        first.len(),
        others.len(),
    ));
    out.push_str(&js[..p]);
    out.push_str(&UNMANGLE.replace(
        "return null;",
        "return name[0] === \"|\" && t$N[t$I(name)] || null;",
    ));
    out.push_str(&js[p + UNMANGLE.len()..]);
    out
}

/// The digits of the shared prefix length in [`front_coded`].
const FRONT_DIGITS: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Decodes the [`front_coded`] table (`#` is the *string*) into the array of
/// names, once, at load.
const FRONT_DECODER: &str = "(function (s) { let a = \"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\", r = s.split(\" \"), p = \"\"; for (let i = 0; i < r.length; i++) if (r[i]) p = r[i] = p.slice(0, a.indexOf(r[i][0])) + r[i].slice(1); return r; })(#)";

/// The names table with front coding: each non-empty name is written as one
/// digit of [`FRONT_DIGITS`] (how many leading characters it shares with the
/// previous non-empty name) and the rest. Ids follow program order, so the
/// views of one ngdart component (`_ViewLiAccordionComponent0`,
/// `…Component1`) sit side by side: in limitless_ui the table goes from 50 to
/// 28 KB.
fn front_coded(names: &[&str]) -> String {
    let digits = FRONT_DIGITS.as_bytes();
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    let mut prev: &str = "";
    for &n in names {
        if n.is_empty() {
            out.push(String::new());
            continue;
        }
        let k = prev
            .bytes()
            .zip(n.bytes())
            .take_while(|(a, b)| a == b)
            .count()
            .min(digits.len() - 1);
        let k = (0..=k).rev().find(|&i| n.is_char_boundary(i)).unwrap_or(0);
        out.push(format!("{}{}", digits[k] as char, &n[k..]));
        prev = n;
    }
    out.join(" ")
}

/// Removes the `var x = dart.privateName(lib, "_n");` lines whose variable
/// nothing else cites. The SDK prelude declares one per private name of the
/// library, also for the statics that are plain JS names; besides the bytes,
/// the *string* of an unused one kept a renamable static name
/// (`_getCanonicalRecipe`) reserved, since every *string* of the runtime
/// reserves (`minify::IdentifierStrings`). `DARTFORGE_JSPROD_PRIVATE_NAMES=0`
/// keeps them.
fn drop_unused_private_names(js: &str) -> String {
    if std::env::var("DARTFORGE_JSPROD_PRIVATE_NAMES").is_ok_and(|v| v == "0") {
        return js.to_string();
    }
    let mut count: HashMap<&str, usize> = HashMap::new();
    let b = js.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut i = 0;
    while i < b.len() {
        if id(b[i]) {
            let s = i;
            while i < b.len() && id(b[i]) {
                i += 1;
            }
            *count.entry(&js[s..i]).or_default() += 1;
        } else {
            i += 1;
        }
    }
    let mut out = String::with_capacity(js.len());
    for line in js.split_inclusive('\n') {
        let unused = line
            .strip_prefix("var ")
            .and_then(|r| r.split_once(" = dart.privateName("))
            .is_some_and(|(name, rest)| {
                // `lib, "_n");` and nothing else on the line.
                let simple = rest
                    .trim_end()
                    .strip_suffix(");")
                    .and_then(|a| a.split_once(", \""))
                    .is_some_and(|(lib, n)| {
                        lib.bytes().all(id)
                            && n.strip_suffix('"').is_some_and(|n| n.bytes().all(id))
                    });
                simple && count.get(name) == Some(&1)
            });
        if !unused {
            out.push_str(line);
        }
    }
    out
}

/// The aliases of a module prelude that mean the same in every module:
/// `var $x = dartx.x;`, `var $x = dartx["+"];` and `var n = dart.privateName(L,
/// "n");` of an SDK library (declared by the *bootstrap*). The generated
/// name is a function of the value, but two modules may still give one name
/// two values (`bundle.rs` explains why each module is an IIFE).
fn prelude_alias(line: &str) -> Option<(&str, &str)> {
    let (name, value) = line
        .strip_prefix("var ")?
        .strip_suffix(';')?
        .split_once(" = ")?;
    let ident = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$')
    };
    if !ident(name) {
        return None;
    }
    let ok = if let Some(x) = value.strip_prefix("dartx.") {
        ident(x)
    } else if let Some(x) = value.strip_prefix("dartx[\"") {
        x.strip_suffix("\"]")
            .is_some_and(|k| !k.contains(['"', '\\']))
    } else if let Some(args) = value
        .strip_prefix("dart.privateName(")
        .and_then(|r| r.strip_suffix(')'))
    {
        args.split_once(", \"").is_some_and(|(lib, n)| {
            ident(lib)
                && !lib.starts_with("L$")
                && n.strip_suffix('"')
                    .is_some_and(|n| !n.contains(['"', '\\']))
        })
    } else {
        false
    };
    ok.then_some((name, value))
}

/// Hoists the prelude aliases ([`prelude_alias`]) of the SDK body and of
/// each module out of their IIFEs into one declaration, placed after the
/// *bootstrap* (which creates `dartx`, `dart.privateName` and the SDK
/// namespaces). Every library then spells `dartx.add` with the same
/// variable, the 474 copies of `var _is = …` of a large program become one,
/// and gzip sees the same name in every library. An alias whose name has two
/// values in two modules stays local everywhere. Returns the declaration
/// (empty when nothing is hoisted); `DARTFORGE_JSPROD_HOIST_PRELUDES=0`
/// leaves the preludes alone.
fn hoist_preludes(bodies: &mut [&mut String]) -> String {
    use std::collections::BTreeMap;
    // name → value, or `None` when two modules disagree.
    let mut values: BTreeMap<String, Option<String>> = BTreeMap::new();
    for body in bodies.iter() {
        for line in body.lines() {
            if !line.starts_with("var ") {
                break;
            }
            if let Some((name, value)) = prelude_alias(line) {
                match values.get(name) {
                    None => {
                        values.insert(name.to_string(), Some(value.to_string()));
                    }
                    Some(Some(v)) if v != value => {
                        values.insert(name.to_string(), None);
                    }
                    _ => {}
                }
            }
        }
    }
    let hoist = |line: &str| {
        prelude_alias(line)
            .is_some_and(|(n, v)| values.get(n).is_some_and(|x| x.as_deref() == Some(v)))
    };
    for body in bodies.iter_mut() {
        let mut out = String::with_capacity(body.len());
        let mut prelude = true;
        for line in body.split_inclusive('\n') {
            if prelude && !line.starts_with("var ") {
                prelude = false;
            }
            if prelude && hoist(line.trim_end()) {
                continue;
            }
            out.push_str(line);
        }
        **body = out;
    }
    let mut decl = String::new();
    for (name, value) in &values {
        if let Some(v) = value {
            decl.push_str(&format!("var {name} = {v};\n"));
        }
    }
    decl
}

/// Assembles the single file.
pub fn assemble(
    e: &EmittedWithSdk,
    op: Options,
    names: Option<&crate::properties::Names>,
    rti: Option<&HashMap<String, String>>,
) -> Production {
    let modules: Vec<bundle::Module> = e
        .emitted
        .modules
        .iter()
        .filter(|(p, _)| p != "preambulo.js")
        .map(|(p, t)| bundle::split(p, t))
        .collect();
    let (mut modules, cycles) = bundle::order(modules);
    let mut sdk_body = e.sdk.clone();
    let hoisted = if std::env::var("DARTFORGE_JSPROD_HOIST_PRELUDES").map_or(true, |v| v != "0") {
        let mut bodies: Vec<&mut String> = vec![&mut sdk_body];
        bodies.extend(modules.iter_mut().map(|m| &mut m.body));
        hoist_preludes(&mut bodies)
    } else {
        String::new()
    };
    let entry =
        bundle::entry_from_mjs(&e.emitted.entry, &modules).unwrap_or_else(|| "L$main".to_string());
    // The SDK module in an IIFE, like the program's: the `var`s of its
    // prelude (`dartx` aliases, private names) are locals; what is shared
    // (`dart`, the namespaces, `dartx`) is in the *bootstrap*.
    // The SDK end marker separates, for property renaming, the runtime's
    // *strings* from the program's (`minify::SDK_END_MARKER`); it only exists
    // when that renaming is going to run, and compression deletes it.
    let names_ok = std::env::var("DARTFORGE_JSPROD_NAMES").map_or(true, |v| v != "0");
    let props = names_ok && std::env::var("DARTFORGE_JSPROD_PROPS").map_or(true, |v| v != "0");
    let marker = if op.minify && props && names.is_some() {
        format!("\"{}\";\n", minify::SDK_END_MARKER)
    } else {
        String::new()
    };
    let sdk = format!(
        "{}const t$R = r => {RTI_PREFIX}r, true);\n{hoisted}(function () {{\n{}}})();\n{marker}",
        e.bootstrap, sdk_body
    );
    let user = modules
        .iter()
        .map(|m| m.body.len() + m.namespaces.iter().map(|n| n.len() + 1).sum::<usize>())
        .sum();
    let preamble = if op.stub { bundle::STUB_PREAMBLE } else { "" };
    let js = bundle::assemble(&sdk, &modules, &entry, preamble);
    // rti rules only for the supertypes that some recipe cites
    // (`rti_rules.rs`); `DARTFORGE_JSPROD_RULES=0` turns it off.
    let js = if std::env::var("DARTFORGE_JSPROD_RULES").map_or(true, |v| v != "0") {
        crate::rti_rules::prune(&js)
    } else {
        js
    };
    // The program's rules in one call (`rti_rules::merge_program_rules`):
    // the program starts at the `NodeList` bridge that `bundle::assemble`
    // writes right after the SDK.
    let js = match js.find("if (typeof NodeList !== 'undefined'") {
        Some(start) if std::env::var("DARTFORGE_JSPROD_MERGE_RULES").map_or(true, |v| v != "0") => {
            crate::rti_rules::merge_program_rules(&js, start)
        }
        _ => js,
    };
    // After pruning the rules: the table is not a citation.
    let js = match rti {
        Some(r) if std::env::var("DARTFORGE_JSPROD_SHORT_RECIPE").map_or(true, |v| v != "0") => {
            name_table(&js, r)
        }
        _ => js,
    };
    let js = alias_recipes(&js);
    let js = drop_unused_private_names(&js);
    // After the name table, which reads the resource lists.
    let js = match js.find("if (typeof NodeList !== 'undefined'") {
        Some(start)
            if std::env::var("DARTFORGE_JSPROD_BATCH_RESOURCES").map_or(true, |v| v != "0") =>
        {
            crate::rti_rules::batch_program_resources(&js, start)
        }
        _ => js,
    };
    let before_compaction = js.len();
    // Identifiers by scope and whitespace (oxc); `DARTFORGE_JSPROD_NAMES=0`
    // stays with whitespace compaction only, and `DARTFORGE_JSPROD_PROPS=0`
    // turns off property renaming (`names_ok` and `props`, above).
    let js = if op.minify && props && names.is_some() {
        match minify::minify_with_properties(
            &js,
            names.unwrap_or(&crate::properties::Names::default()),
        ) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("warning: {e}; the file goes out only compacted");
                minify::compact(&js)
            }
        }
    } else if op.minify && names_ok {
        match minify::minify_names(&js) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("warning: {e}; the file goes out only compacted");
                minify::compact(&js)
            }
        }
    } else if op.minify {
        minify::compact(&js)
    } else {
        js
    };
    Production {
        js,
        modules: modules.len(),
        cycles,
        sdk_before: sdk.len(),
        sdk_after: sdk.len(),
        sdk_units: 0,
        sdk_live: 0,
        user,
        before_compaction,
        world: None,
        index: None,
        prune_time: Duration::ZERO,
    }
}

#[cfg(test)]
mod front_coding_tests {
    use super::*;

    #[test]
    fn finds_record_recipes_only() {
        assert!(cites_record_recipe(r#"t$R("+(|a,|b)")"#));
        assert!(cites_record_recipe(r#"x = "|F<+n(|a)>";"#));
        assert!(!cites_record_recipe(r#"dsend(a, "+", [b])"#));
        assert!(!cites_record_recipe(r#"new RegExp("^Symbol\((_.+)\)$")"#));
        assert!(!cites_record_recipe(r#""a + b(c)""#));
    }

    /// The JS decoder of `FRONT_DECODER`, in Rust.
    fn decode(s: &str) -> Vec<String> {
        let mut prev = String::new();
        s.split(' ')
            .map(|x| {
                if x.is_empty() {
                    return String::new();
                }
                let k = FRONT_DIGITS.find(&x[..1]).unwrap();
                prev = format!("{}{}", &prev[..k], &x[1..]);
                prev.clone()
            })
            .collect()
    }

    #[test]
    fn front_coding_round_trips() {
        let names = [
            "int",
            "",
            "_ViewLiAccordionComponent0",
            "_ViewLiAccordionComponent1",
            "",
            "ViewLiAccordionComponentHost0",
            "int",
        ];
        let enc = front_coded(&names);
        assert_eq!(
            decode(&enc),
            names.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
        assert!(enc.contains(" p1 "), "{enc}");
    }
}
