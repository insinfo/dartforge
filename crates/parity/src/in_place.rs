//! The resident's reload in place (docs/LSP.md, "Décima quarta rodada").
//!
//! A load over the kept program cuts it at the first library whose text
//! changed and loads again everything after the cut. A library whose API
//! changed recently and that others depend on is laid out after them
//! (`Program::layout_hot`), so the next change of it cuts at it alone; the
//! kept libraries before the cut that depend on it (its dependents) then
//! hold the ids of its elements. The reload keeps them when:
//!
//! - every reloaded library a kept one points to (a *stable* library) has
//!   one unit and gets its elements back at the same ids: named declarations
//!   across the suffix reserve their old slots before new declarations take
//!   any remaining slots. Ambiguous names or surviving IDs outside the new
//!   suffix refuse the repair;
//! - no signature of a kept dependent names what changed (a declaration
//!   added, removed or with another signature; a class whose members changed
//!   counts only for the classes that extend, implement or mix it in);
//! - nothing that changed is used without its name (an operator, `call`…).
//!
//! [`plan`] selects the kept prefix before the load, and [`prepare`] checks
//! the new declarations before resolving scopes or supertypes; the caller cuts
//! before every dependent otherwise ([`Plan::fallback`]). After the check,
//! the dependents' imports and exports point to the stable libraries' new
//! ids. Their exporters and importers join the new libraries' namespace
//! construction before any fresh supertype can see a stale kept binding.

use dartforge_elements::frozen::Checkpoint;
use dartforge_elements::incremental::{
    ApiDigest, api_digest, signature_words, text_words, word_hash,
};
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_elements::outline::OutlineRepair;
use dartforge_frontend::ast::{self, DeclKind};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

mod slots;

/// What a reload in place checks and repairs, planned before the load.
pub(crate) struct Plan {
    /// Where to cut when the reload cannot keep the dependents: before every
    /// kept library that depends on what is reloaded.
    pub fallback: Checkpoint,
    /// The reloaded libraries kept ones point to.
    stable: Vec<Stable>,
    /// Kept libraries that export a stable library, directly or through one
    /// another, and kept libraries that import one of them or a stable one:
    /// whose scopes are built again when a stable library's names changed.
    exporters: Vec<LibraryId>,
    importers: Vec<LibraryId>,
    /// The imports (`false`) and exports (`true`) of kept libraries that
    /// point to a reloaded library, by index, with its URI: detached before
    /// the load ([`Plan::detach`]), pointed to its new id after it.
    pointers: Vec<(LibraryId, bool, usize, String)>,
    /// Every kept library that depends on a reloaded one, directly or not:
    /// none of their signatures may name what changed.
    dependents: Vec<LibraryId>,
}

/// A reloaded library kept libraries point to, as it was.
struct Stable {
    uri: String,
    /// Its file (the load does not reach it through the kept libraries).
    path: Option<std::path::PathBuf>,
    id: LibraryId,
    slots: slots::Elements,
    /// The API of its old text, when its text changed.
    old_api: Option<(Arc<ApiDigest>, HashMap<String, u64>)>,
}

/// The signature words of the kept units (what [`prepare`] compares the
/// changed names with), computed once per unit: a unit keeps its id, and its
/// signatures, until a load cuts before it (a swap only changes bodies).
#[derive(Default)]
pub(crate) struct Words {
    by_unit: HashMap<UnitId, Arc<UnitWords>>,
    /// The reloads in place that kept the dependents (tests and measurement).
    pub(crate) settled: usize,
}

/// The words of a unit's signatures, and of its class-like declarations'
/// headers (their supertypes).
struct UnitWords {
    signatures: Vec<u64>,
    headers: Vec<u64>,
}

impl Words {
    /// Forgets the units from `units` on (a load replaced them).
    pub(crate) fn truncate(&mut self, units: usize) {
        self.by_unit.retain(|u, _| (u.0 as usize) < units);
    }

    fn of(&mut self, program: &Program, u: UnitId) -> Option<Arc<UnitWords>> {
        if let Some(w) = self.by_unit.get(&u) {
            return Some(Arc::clone(w));
        }
        let unit = program.units.get(u.0 as usize)?;
        let signatures = signature_words(&unit.source, &unit.ast);
        let mut headers: Vec<u64> = class_headers(&unit.source, &unit.ast)
            .into_iter()
            .flat_map(|(_, text)| text_words(&text))
            .collect();
        headers.sort_unstable();
        headers.dedup();
        let w = Arc::new(UnitWords {
            signatures,
            headers,
        });
        self.by_unit.insert(u, Arc::clone(&w));
        Some(w)
    }
}

/// The text through each class-like declaration's last header node, by name.
/// AST spans keep metadata maps, record types and primary parameter lists
/// inside the header without mistaking their braces for the class body.
fn class_headers(text: &str, a: &ast::Ast) -> Vec<(String, String)> {
    let types_end =
        |types: &[ast::TypeId]| types.iter().map(|t| a.ty(*t).span.end).max().unwrap_or(0);
    let primary_end = |id: Option<ast::MemberId>| {
        id.and_then(|id| match &a.member(id).kind {
            ast::MemberKind::Constructor(c) => Some(
                c.parameters
                    .iter()
                    .map(|p| p.span.end)
                    .max()
                    .unwrap_or(0)
                    .max(c.name.map_or(c.class_name.span.end, |n| n.span.end)),
            ),
            _ => None,
        })
        .unwrap_or(0)
    };
    let mut out = Vec::new();
    for decl in &a.decls {
        let (name, parameters, end) = match &decl.kind {
            DeclKind::Class(c) => (
                &c.name,
                c.type_params.as_ref(),
                types_end(c.extends.as_slice())
                    .max(types_end(&c.with))
                    .max(types_end(&c.implements))
                    .max(primary_end(c.primary_constructor)),
            ),
            DeclKind::Mixin(m) => (
                &m.name,
                m.type_params.as_ref(),
                types_end(&m.on).max(types_end(&m.implements)),
            ),
            DeclKind::Enum(e) => (
                &e.name,
                e.type_params.as_ref(),
                types_end(&e.with)
                    .max(types_end(&e.implements))
                    .max(primary_end(e.primary_constructor)),
            ),
            DeclKind::ExtensionType(x) => (
                &x.name,
                x.type_params.as_ref(),
                x.representation_span.end.max(types_end(&x.implements)),
            ),
            DeclKind::Extension(x) => match &x.name {
                Some(name) => (name, x.type_params.as_ref(), a.ty(x.on).span.end),
                None => continue,
            },
            _ => continue,
        };
        let end = end
            .max(name.span.end)
            .max(parameters.iter().map(|p| p.span.end).max().unwrap_or(0));
        let Some(header) = text.get(decl.span.start..end) else {
            continue;
        };
        let name = text.get(name.span.start..name.span.end).unwrap_or("");
        out.push((name.to_string(), header.to_string()));
    }
    out
}

/// The libraries each library imports or exports (`edges`), and, the other
/// way, the ones that import it and the ones that export it.
fn targets(program: &Program, l: usize) -> impl Iterator<Item = (usize, bool)> + '_ {
    let lib = &program.libraries[l];
    lib.imports
        .iter()
        .map(|i| (i.library.0 as usize, false))
        .chain(lib.exports.iter().map(|e| (e.library.0 as usize, true)))
}

/// Plans a reload in place over `program` cut at library `start` (after the
/// frozen prefix), whose files `changed` (keys) changed. `Ok(None)`: no kept
/// library points past the cut, the cut needs nothing more. `Err(start)`:
/// the reload cannot keep the dependents; cut at `start` instead.
pub(crate) fn plan(
    program: &Program,
    from: &Checkpoint,
    frozen: Checkpoint,
    changed: &HashSet<std::path::PathBuf>,
    key: &dyn Fn(&std::path::Path) -> std::path::PathBuf,
) -> Result<Option<Plan>, usize> {
    let start = from.libraries;
    let n = program.libraries.len();
    let kept = frozen.libraries..start.min(n);
    // Kept libraries pointing past the cut.
    let mut pointers = Vec::new();
    let mut stable_ids: Vec<usize> = Vec::new();
    for l in kept.clone() {
        let lib = &program.libraries[l];
        let edges = lib
            .imports
            .iter()
            .enumerate()
            .map(|(i, x)| (false, i, x.library.0 as usize))
            .chain(
                lib.exports
                    .iter()
                    .enumerate()
                    .map(|(i, x)| (true, i, x.library.0 as usize)),
            );
        for (export, i, t) in edges {
            if t >= start {
                pointers.push((
                    LibraryId(l as u32),
                    export,
                    i,
                    program.libraries[t].uri.clone(),
                ));
                if !stable_ids.contains(&t) {
                    stable_ids.push(t);
                }
            }
        }
    }
    if pointers.is_empty() {
        return Ok(None);
    }
    // Kept signatures can reach elements through reexports or a reloaded
    // declaration's supertypes. Reserve the suffix dependencies as well.
    let mut seen: HashSet<usize> = stable_ids.iter().copied().collect();
    let mut next = 0;
    while next < stable_ids.len() {
        let library = stable_ids[next];
        for (target, _) in targets(program, library) {
            if target >= start && seen.insert(target) {
                stable_ids.push(target);
            }
        }
        next += 1;
    }
    // Everything kept that depends on what is reloaded, transitively.
    let mut by_target: HashMap<usize, Vec<(usize, bool)>> = HashMap::new();
    for l in kept.clone() {
        for (t, export) in targets(program, l) {
            by_target.entry(t).or_default().push((l, export));
        }
    }
    let mut dependent = vec![false; n];
    let mut stack: Vec<usize> = (start..n).collect();
    while let Some(t) = stack.pop() {
        for &(l, _) in by_target.get(&t).into_iter().flatten() {
            if !std::mem::replace(&mut dependent[l], true) {
                stack.push(l);
            }
        }
    }
    let dependents: Vec<LibraryId> = kept
        .clone()
        .filter(|&l| dependent[l])
        .map(|l| LibraryId(l as u32))
        .collect();
    let fallback = dependents
        .iter()
        .map(|l| program.group_start(*l))
        .min()
        .unwrap_or(start)
        .max(frozen.libraries);
    // The scopes to build again: the exporters of a stable library (through
    // other exporters too), and the importers of either.
    let mut exporter = vec![false; n];
    let mut stack: Vec<usize> = stable_ids.clone();
    while let Some(t) = stack.pop() {
        for &(l, export) in by_target.get(&t).into_iter().flatten() {
            if export && !std::mem::replace(&mut exporter[l], true) {
                stack.push(l);
            }
        }
    }
    let entry = program.entry.map(|e| e.0 as usize);
    let mut importers = Vec::new();
    for l in kept.clone() {
        if Some(l) == entry || exporter[l] {
            continue;
        }
        if targets(program, l)
            .any(|(t, export)| !export && (t >= start && stable_ids.contains(&t) || exporter[t]))
        {
            importers.push(LibraryId(l as u32));
        }
    }
    let exporters: Vec<LibraryId> = kept
        .clone()
        .filter(|&l| exporter[l] && Some(l) != entry)
        .map(|l| LibraryId(l as u32))
        .collect();
    let mut stable = Vec::new();
    for &s in &stable_ids {
        let lib = &program.libraries[s];
        if lib.is_sdk || lib.units.len() != 1 {
            return Err(fallback);
        }
        let slots = slots::Elements::capture(program, LibraryId(s as u32), from);
        let unit = &program.units[lib.units[0].0 as usize];
        let old_api = match &unit.path {
            Some(p) if changed.contains(&key(p)) => {
                let Some(api) = api_digest(&unit.source, &unit.ast, &unit.unit) else {
                    return Err(fallback);
                };
                let headers = class_headers(&unit.source, &unit.ast)
                    .into_iter()
                    .map(|(name, text)| (name, word_hash(&text)))
                    .collect();
                Some((Arc::new(api), headers))
            }
            _ => None,
        };
        stable.push(Stable {
            uri: lib.uri.clone(),
            path: unit.path.clone(),
            id: LibraryId(s as u32),
            slots,
            old_api,
        });
    }
    let fallback = program
        .checkpoint_at(fallback)
        .ok_or(frozen.libraries)
        .map_err(|_| frozen.libraries)?;
    Ok(Some(Plan {
        fallback,
        stable,
        exporters,
        importers,
        pointers,
        dependents,
    }))
}

impl Plan {
    /// Reserves the surviving slots of every reachable suffix library before
    /// placing new declarations. A refusal leaves the fresh program unchanged;
    /// the caller then reloads before the kept dependents.
    pub(crate) fn reorder(&self, program: &mut Program, from: &Checkpoint) -> bool {
        let mut slots = slots::Slots::new(program, from);
        for stable in &self.stable {
            let Some((id, _)) = program
                .libraries
                .iter()
                .enumerate()
                .skip(from.libraries)
                .find(|(_, l)| l.uri == stable.uri)
            else {
                return false;
            };
            let now = slots::Elements::capture(program, LibraryId(id as u32), from);
            if !slots.keep(&stable.slots, &now) {
                if std::env::var_os("DARTFORGE_ENGINE_TIMES").is_some() {
                    eprintln!(
                        "engine: named slot repair refused for {} (library {} -> {id})",
                        stable.uri, stable.id.0
                    );
                }
                return false;
            }
        }
        slots.apply(program, from)
    }

    /// The files of the reloaded libraries that kept ones point to: the load
    /// does not reach them through the kept libraries.
    pub(crate) fn files(&self) -> Vec<std::path::PathBuf> {
        self.stable.iter().filter_map(|s| s.path.clone()).collect()
    }

    /// Detaches the kept libraries' pointers past the cut (to an id no
    /// library has), before the load renumbers what comes after it.
    pub(crate) fn detach(&self, program: &mut Program) {
        for (l, export, i, _) in &self.pointers {
            let lib = &mut program.libraries[l.0 as usize];
            if *export {
                lib.exports[*i].library = LibraryId(u32::MAX);
            } else {
                lib.imports[*i].library = LibraryId(u32::MAX);
            }
        }
    }
}

/// The names an API change made different, from the old and the new API of
/// a unit: `Err(())` when it cannot be told (other directives, a changed
/// declaration used without its name). The first set is the names that
/// matter to any signature naming them; the second, the classes whose
/// members (and only them) changed, which matter to the classes extending,
/// implementing or mixing them in.
#[allow(clippy::type_complexity)]
fn changed_names(
    old: &ApiDigest,
    old_headers: &HashMap<String, u64>,
    new: &ApiDigest,
    new_headers: &HashMap<String, u64>,
) -> Result<(HashSet<u64>, HashSet<u64>), ()> {
    if old.directives != new.directives {
        return Err(());
    }
    fn by_name(a: &ApiDigest) -> BTreeMap<(Option<&str>, &str), Vec<(u64, bool, bool)>> {
        let mut m: BTreeMap<(Option<&str>, &str), Vec<(u64, bool, bool)>> = BTreeMap::new();
        for d in &a.declarations {
            m.entry((d.owner.as_deref(), d.name.as_str()))
                .or_default()
                .push((d.digest, d.implicit, d.class_like));
        }
        m
    }
    let (before, after) = (by_name(old), by_name(new));
    let mut names = HashSet::new();
    let mut members_of = HashSet::new();
    let keys: HashSet<&(Option<&str>, &str)> = before.keys().chain(after.keys()).collect();
    for key in keys {
        let (a, b) = (before.get(key), after.get(key));
        if a == b {
            continue;
        }
        let entries = a.into_iter().chain(b).flatten();
        let mut class_like = false;
        for &(_, implicit, cl) in entries {
            if implicit {
                return Err(());
            }
            class_like |= cl;
        }
        let (owner, name) = *key;
        match owner {
            Some(owner) => {
                names.insert(word_hash(name));
                members_of.insert(word_hash(owner));
            }
            None if class_like
                && a.is_some()
                && b.is_some()
                && old_headers.get(name) == new_headers.get(name) =>
            {
                // Only its members changed (each of them is a changed
                // name of its own).
                members_of.insert(word_hash(name));
            }
            None => {
                names.insert(word_hash(name));
            }
        }
    }
    Ok((names, members_of))
}

/// Says why a reload in place was refused (with `DARTFORGE_ENGINE_TIMES`).
fn refused(why: &str) {
    if std::env::var_os("DARTFORGE_ENGINE_TIMES").is_some() {
        eprintln!("engine: reload in place refused: {why}");
    }
}

/// A type alias or superclass header can keep its text while the declaration
/// it names changes. Follow those names to a fixpoint before checking the
/// kept signatures: otherwise an old supertype can point at a retired slot
/// that a different class now occupies. Include kept non-SDK aliases too,
/// since an inheritor may reach the fresh class through a kept alias chain.
fn propagate_type_names(
    program: &Program,
    from: &Checkpoint,
    names: &mut HashSet<u64>,
    members_of: &mut HashSet<u64>,
) -> Option<()> {
    if names.is_empty() && members_of.is_empty() {
        return Some(());
    }
    let mut dependencies = Vec::new();
    for alias in &program.typedefs {
        if program.libraries[alias.library.0 as usize].is_sdk {
            continue;
        }
        let unit = program.units.get(alias.decl.unit.0 as usize)?;
        let declaration = unit.ast.decl(alias.decl.decl);
        let DeclKind::Typedef(alias) = &declaration.kind else {
            return None;
        };
        let name = unit
            .source
            .get(alias.name.span.start..alias.name.span.end)?;
        let text = unit
            .source
            .get(declaration.span.start..declaration.span.end)?;
        dependencies.push((word_hash(name), text_words(text)));
    }
    for unit in &program.units[from.units..] {
        dependencies.extend(
            class_headers(&unit.source, &unit.ast)
                .into_iter()
                .map(|(name, header)| (word_hash(&name), text_words(&header))),
        );
    }
    loop {
        let before = names.len() + members_of.len();
        for (name, words) in &dependencies {
            if words.iter().any(|word| names.contains(word)) {
                names.insert(*name);
            }
            if words.iter().any(|word| members_of.contains(word)) {
                members_of.insert(*name);
            }
        }
        if before == names.len() + members_of.len() {
            return Some(());
        }
    }
}

/// Checks the new declarations before resolving their namespaces. Kept
/// pointers are rebound only after every check succeeds; the returned scope
/// repair joins the fresh libraries' export fixpoint. `None` aborts the
/// partial outline so the caller can reload from [`Plan::fallback`].
pub(crate) fn prepare(
    plan: &Plan,
    program: &mut Program,
    from: &Checkpoint,
    words: &mut Words,
) -> Option<OutlineRepair> {
    if !plan.reorder(program, from) {
        refused("named slots cannot preserve the kept IDs");
        return None;
    }
    let by_uri: HashMap<&str, LibraryId> = program
        .libraries
        .iter()
        .enumerate()
        .skip(plan.fallback.libraries)
        .map(|(i, l)| (l.uri.as_str(), LibraryId(i as u32)))
        .collect();
    let mut names = HashSet::new();
    let mut members_of = HashSet::new();
    for s in &plan.stable {
        let Some(&id) = by_uri.get(s.uri.as_str()) else {
            refused("a stable library is gone");
            return None;
        };
        let lib = &program.libraries[id.0 as usize];
        if lib.units.len() != 1 {
            refused("a stable library has parts now");
            return None;
        }
        let now = slots::Elements::capture(program, id, from);
        if !s.slots.preserved(&now) {
            refused("its surviving elements moved");
            return None;
        }
        if let Some((old_api, old_headers)) = &s.old_api {
            let unit = &program.units[lib.units[0].0 as usize];
            let Some(new_api) = api_digest(&unit.source, &unit.ast, &unit.unit) else {
                refused("its new text has no API digest");
                return None;
            };
            let new_headers: HashMap<String, u64> = class_headers(&unit.source, &unit.ast)
                .into_iter()
                .map(|(name, text)| (name, word_hash(&text)))
                .collect();
            match changed_names(old_api, old_headers, &new_api, &new_headers) {
                Ok((n, m)) => {
                    names.extend(n);
                    members_of.extend(m);
                }
                Err(()) => {
                    refused("an implicit declaration or the directives changed");
                    return None;
                }
            }
        }
    }
    if propagate_type_names(program, from, &mut names, &mut members_of).is_none() {
        refused("a type dependency has no source declaration");
        return None;
    }
    let Some(pointed): Option<Vec<LibraryId>> = plan
        .pointers
        .iter()
        .map(|(_, _, _, uri)| by_uri.get(uri.as_str()).copied())
        .collect()
    else {
        refused("a pointer has no library");
        return None;
    };
    drop(by_uri);
    // No kept dependent's signature names what changed.
    if !names.is_empty() || !members_of.is_empty() {
        for &d in &plan.dependents {
            for &u in &program.libraries[d.0 as usize].units {
                let Some(w) = words.of(program, u) else {
                    refused("a dependent has no API digest");
                    return None;
                };
                if names.iter().any(|n| w.signatures.binary_search(n).is_ok())
                    || members_of
                        .iter()
                        .any(|n| w.headers.binary_search(n).is_ok())
                {
                    refused("a dependent names what changed");
                    return None;
                }
            }
        }
    }
    // The kept libraries point to the stable ones' new ids.
    for ((l, export, i, _), to) in plan.pointers.iter().zip(pointed) {
        let lib = &mut program.libraries[l.0 as usize];
        if *export {
            lib.exports[*i].library = to;
        } else {
            lib.imports[*i].library = to;
        }
    }
    words.settled += 1;
    Some(OutlineRepair {
        exporters: plan.exporters.clone(),
        importers: plan.importers.clone(),
    })
}

/// The files of `changed` (keys) whose library another library of `program`
/// imports or exports, besides the synthetic entry: the ones to lay out last
/// (`Program::layout_hot`).
pub(crate) fn depended_on(
    program: &Program,
    from: usize,
    changed: &HashSet<std::path::PathBuf>,
    key: &dyn Fn(&std::path::Path) -> std::path::PathBuf,
) -> Vec<std::path::PathBuf> {
    let entry = program.entry.map(|e| e.0 as usize);
    let mut pointed = vec![false; program.libraries.len()];
    for l in from..program.libraries.len() {
        if Some(l) == entry {
            continue;
        }
        for (t, _) in targets(program, l) {
            if t != l
                && let Some(p) = pointed.get_mut(t)
            {
                *p = true;
            }
        }
    }
    let mut out = Vec::new();
    for u in &program.units[..] {
        if !pointed[u.library.0 as usize] {
            continue;
        }
        let Some(p) = &u.path else { continue };
        let k = dartforge_elements::generated::key_of(p);
        if changed.contains(&*k) && !out.iter().any(|o: &std::path::PathBuf| *o == *k) {
            out.push(key(p));
        }
    }
    out
}

#[cfg(test)]
mod header_tests {
    use super::class_headers;

    fn headers(source: &str) -> Vec<(String, String)> {
        let mut names = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_with(
            source,
            &mut names,
            dartforge_frontend::LibraryFeatures::current(),
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        class_headers(source, &parsed.ast)
    }

    #[test]
    fn header_spans_include_nested_braces_and_exclude_bodies() {
        for (source, old, new) in [
            (
                "@Note({'key': 1}) class C<T extends num> { int bodyOnly = 0; }",
                "extends num",
                "extends String",
            ),
            (
                "class C<T extends ({int field})> { int bodyOnly = 0; }",
                "int field",
                "String field",
            ),
            (
                "mixin C<T extends ({int field})> on Base<T> { int bodyOnly = 0; }",
                "int field",
                "String field",
            ),
            (
                "enum C<T extends ({int field})> { value; static int bodyOnly = 0; }",
                "int field",
                "String field",
            ),
            (
                "class C([Map<String, Object> options = const {'key': 1}]) { this { var bodyOnly = 0; } }",
                "'key': 1",
                "'key': 2",
            ),
            (
                "extension type C(({int field}) value) { static int bodyOnly = 0; }",
                "int field",
                "String field",
            ),
        ] {
            let before = headers(source);
            assert_eq!(before.len(), 1, "{source}");
            assert_ne!(before, headers(&source.replacen(old, new, 1)), "{source}");
            assert_eq!(
                before,
                headers(&source.replace("bodyOnly = 0", "bodyOnly = 1")),
                "{source}"
            );
            assert!(!before[0].1.contains("bodyOnly"), "{source}");
        }
    }
}
