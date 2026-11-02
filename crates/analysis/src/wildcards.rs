//! `_` declarations a library without wildcard variables may not repeat.
//!
//! Wildcard variables are the language from Dart 3.7 (or the
//! `wildcard-variables` experiment): before that, `_` is an ordinary name, and
//! two parameters, catch variables, locals or type parameters named `_` in the
//! same scope are duplicate definitions, a record field `_` is a private name,
//! and a member `_` conflicts with a type variable `_`. The CFE refuses those
//! programs; the compilers refuse them too (`emit_native`, `emit_js`), with the
//! analyzer's checks of this crate (`duplicates`, `records`, `members`) kept to
//! the diagnostics on a `_` — the compilers' own checks already cover the
//! other names.

use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{LibraryId, Program, UnitId, UnitRole};
use dartforge_frontend::features::Feature;
use dartforge_intern::Interner;

/// The diagnostics on a `_` declaration of each program library (not the
/// SDK's) whose features do not include wildcard variables, as
/// `path:offset: message` lines, in order.
pub fn repeated_without_wildcards(program: &Program, interner: &Interner) -> Vec<String> {
    let mut out = Vec::new();
    for (i, library) in program.libraries.iter().enumerate() {
        if library.is_sdk || library.features.has(Feature::WildcardVariables) {
            continue;
        }
        let lib = LibraryId(i as u32);
        let ids: Vec<UnitId> = library
            .units
            .iter()
            .copied()
            .filter(|u| program.unit(*u).role != UnitRole::Patch)
            .collect();
        let units: Vec<crate::Unit<'_>> = ids
            .iter()
            .map(|u| crate::Unit {
                ast: &program.unit(*u).ast,
                unit: &program.unit(*u).unit,
                source: &program.unit(*u).source,
            })
            .collect();
        let paths: Vec<String> = ids.iter().map(|u| program.unit_path(*u)).collect();
        let new_syntax =
            program.library_reference(lib) == dartforge_diagnostics::AnalyzerReference::V3_13;
        let mut found: Vec<(UnitId, Diagnostic)> =
            crate::duplicates::duplicates_with_paths(&units, &paths, interner, false, !new_syntax)
                .into_iter()
                .map(|(k, d)| (ids[k], d))
                .collect();
        for (k, u) in units.iter().enumerate() {
            found.extend(
                crate::records::check(u, interner, library.features)
                    .into_iter()
                    .map(|d| (ids[k], d)),
            );
        }
        found.extend(crate::members::check(program, lib, interner));
        for (u, d) in found {
            let source = &program.unit(u).source;
            if source.get(d.span.start..d.span.end) != Some("_") {
                continue;
            }
            let line = format!("{}:{}: {}", program.unit_path(u), d.span.start, d.message);
            if !out.contains(&line) {
                out.push(line);
            }
        }
    }
    out
}
