//! `enum_without_constants` as in the `ErrorVerifier` of analyzer 3.6.2: the
//! main declaration uses the values of the whole augmented enum.

use crate::Unit;
use dartforge_diagnostics::Diagnostic;
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_frontend::ast::DeclKind;
use dartforge_intern::SymbolId;
use std::collections::HashSet;

/// Returns the unit's index along with each diagnostic.
///
/// `element.augmented.constants`: those of the declaration itself and those of
/// the `augment enum`s with the same name. A second declaration without
/// `augment` (the `augment` read as an identifier without the experiment) is
/// another element, with only its own.
pub fn without_constants(units: &[Unit<'_>]) -> Vec<(usize, Diagnostic)> {
    let mut augmented_with_constants = HashSet::<SymbolId>::new();
    for unit in units {
        for &id in &unit.unit.declarations {
            let decl = unit.ast.decl(id);
            if let DeclKind::Enum(e) = &decl.kind
                && decl.augment
                && !e.constants.is_empty()
            {
                augmented_with_constants.insert(e.name.sym);
            }
        }
    }

    let mut out = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        for &id in &unit.unit.declarations {
            let decl = unit.ast.decl(id);
            if let DeclKind::Enum(e) = &decl.kind
                && !decl.augment
                && e.constants.is_empty()
                && !augmented_with_constants.contains(&e.name.sym)
            {
                out.push((
                    i,
                    Diagnostic::with_code(
                        c::ENUM_WITHOUT_CONSTANTS,
                        e.name.span,
                        std::iter::empty::<&str>(),
                    ),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_frontend::features::{Feature, LanguageVersion, LibraryFeatures};
    use dartforge_frontend::parser::{parse, parse_with};
    use dartforge_intern::Interner;

    #[test]
    fn empty_enum_matches_the_3_6_oracle() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/diagnostics/analyzer/enum_without_constants/EnumWithoutConstants__noConstants.dart"
        ));
        let mut names = Interner::new();
        let parsed = parse(source, &mut names);
        let found = without_constants(&[Unit {
            ast: &parsed.ast,
            unit: &parsed.unit,
            source,
        }]);
        assert_eq!(found.len(), 1, "{found:?}");
        let d = &found[0].1;
        assert_eq!(d.code, Some(c::ENUM_WITHOUT_CONSTANTS));
        assert_eq!((d.span.start, d.span.end), (5, 6));
        assert_eq!(d.message, "The enum must have at least one constant.");
        assert_eq!(d.correction().as_deref(), Some("Try declaring a constant."));
    }

    #[test]
    fn constant_in_augmentation_completes_base_enum() {
        let sources = ["enum E {}", "augment enum E { v }", "augment enum F {}"];
        let mut names = Interner::new();
        let features = LibraryFeatures::new(LanguageVersion::FLOOR, &[Feature::Augmentations]);
        let parsed: Vec<_> = sources
            .iter()
            .map(|source| parse_with(source, &mut names, features))
            .collect();
        let units: Vec<_> = sources
            .iter()
            .zip(&parsed)
            .map(|(source, parsed)| Unit {
                ast: &parsed.ast,
                unit: &parsed.unit,
                source,
            })
            .collect();
        assert!(without_constants(&units).is_empty());

        let source = "enum E {} augment enum E {}";
        let parsed = parse_with(source, &mut names, features);
        let found = without_constants(&[Unit {
            ast: &parsed.ast,
            unit: &parsed.unit,
            source,
        }]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].1.span.start, 5);
    }
}
