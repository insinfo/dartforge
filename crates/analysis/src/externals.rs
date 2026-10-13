//! Initializers on `abstract` fields and `external` declarations, as in the
//! analyzer `ErrorVerifier`'s `_checkForAbstractOrExternalVariableInitializer`
//! and `_checkForAbstractOrExternalFieldConstructorInitializer`. The error is
//! on the variable/field name.

use crate::Unit;
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{DeclKind, Initializer, MemberId, MemberKind};
use dartforge_intern::SymbolId;
use std::collections::HashSet;

fn report(code: dartforge_diagnostics::Code, span: Span) -> Diagnostic {
    Diagnostic::with_code(code, span, std::iter::empty::<&str>())
}

/// This unit's diagnostics that do not need type inference.
pub fn initializers(unit: Unit<'_>) -> Vec<Diagnostic> {
    let ast = unit.ast;
    let mut out = Vec::new();
    for &id in &unit.unit.declarations {
        match &ast.decl(id).kind {
            DeclKind::Variables(v) => {
                if v.external {
                    for var in v.variables.iter() {
                        if var.initializer.is_some() {
                            out.push(report(c::EXTERNAL_VARIABLE_INITIALIZER, var.name.span));
                        }
                    }
                }
            }
            kind => {
                let members: &[MemberId] = match kind {
                    DeclKind::Class(x) => &x.members,
                    DeclKind::Mixin(x) => &x.members,
                    DeclKind::Enum(x) => &x.members,
                    DeclKind::Extension(x) => &x.members,
                    DeclKind::ExtensionType(x) => &x.members,
                    DeclKind::Typedef(_) | DeclKind::Function(_) | DeclKind::Variables(_) => {
                        continue
                    }
                };
                let mut external_fields = HashSet::<SymbolId>::new();
                let mut abstract_fields = HashSet::<SymbolId>::new();
                for &m in members {
                    if let MemberKind::Field(v) = &ast.member(m).kind {
                        for var in v.variables.iter() {
                            if !v.static_ {
                                if v.external {
                                    external_fields.insert(var.name.sym);
                                }
                                if v.abstract_ {
                                    abstract_fields.insert(var.name.sym);
                                }
                            }
                            if var.initializer.is_some() {
                                if v.external {
                                    out.push(report(c::EXTERNAL_FIELD_INITIALIZER, var.name.span));
                                }
                                if v.abstract_ {
                                    out.push(report(c::ABSTRACT_FIELD_INITIALIZER, var.name.span));
                                }
                            }
                        }
                    }
                }
                for &m in members {
                    if let MemberKind::Constructor(k) = &ast.member(m).kind {
                        for p in k.parameters.iter() {
                            if p.this_ {
                                if let Some(name) = p.name {
                                    if external_fields.contains(&name.sym) {
                                        out.push(report(
                                            c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER,
                                            name.span,
                                        ));
                                    }
                                    if abstract_fields.contains(&name.sym) {
                                        out.push(report(
                                            c::ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER,
                                            name.span,
                                        ));
                                    }
                                }
                            }
                        }
                        for init in &k.initializers {
                            if let Initializer::Field { name, .. } = init {
                                if external_fields.contains(&name.sym) {
                                    out.push(report(
                                        c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER,
                                        name.span,
                                    ));
                                }
                                if abstract_fields.contains(&name.sym) {
                                    out.push(report(
                                        c::ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER,
                                        name.span,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_frontend::parser::parse;
    use dartforge_intern::Interner;

    #[test]
    fn three_shapes_match_recorded_oracle() {
        for (source, code, start, message, correction) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/variable_not_initialized/VariableNotInitialized__topLevelVariabl_9e68b99b.dart")),
                c::EXTERNAL_VARIABLE_INITIALIZER,
                13,
                "External variables can't have initializers.",
                "Try removing the initializer or the 'external' keyword.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_2eafc447.dart")),
                c::EXTERNAL_FIELD_INITIALIZER,
                31,
                "External fields can't have initializers.",
                "Try removing the initializer or the 'external' keyword.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/external_field_constructor_initializer/ExternalFieldConstructorInitializer__ex_7c1a26a5.dart")),
                c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER,
                43,
                "External fields can't have initializers.",
                "Try removing the field initializer or the 'external' keyword from the field declaration.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/external_field_constructor_initializer/ExternalFieldConstructorInitializer__ex_4fd7b47b.dart")),
                c::EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER,
                42,
                "External fields can't have initializers.",
                "Try removing the field initializer or the 'external' keyword from the field declaration.",
            ),
        ] {
            let mut names = Interner::new();
            let parsed = parse(source, &mut names);
            let found = initializers(Unit { ast: &parsed.ast, unit: &parsed.unit, source });
            assert!(found.iter().any(|d|
                d.code == Some(code) && d.span.start == start && d.span.end == start + 1
                    && d.message == message && d.correction().as_deref() == Some(correction)
            ), "{source}: {found:?}");
        }
    }

    #[test]
    fn external_without_initializer_and_static_field_report_nothing() {
        let source = "external int topo; class A { external static int x; A() : x = 1; }";
        let mut names = Interner::new();
        let parsed = parse(source, &mut names);
        let found = initializers(Unit {
            ast: &parsed.ast,
            unit: &parsed.unit,
            source,
        });
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn abstract_field_initializers_match_oracle() {
        for (source, code, fragment, correction) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/abstract_field_initializer/AbstractFieldInitializer__abstract_fiel_92732177.dart")),
                c::ABSTRACT_FIELD_INITIALIZER,
                "x = 0",
                "Try removing the initializer or the 'abstract' keyword.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/abstract_field_initializer/AbstractFieldInitializer__abstract_fiel_2551b9f0.dart")),
                c::ABSTRACT_FIELD_INITIALIZER,
                "x = 0",
                "Try removing the initializer or the 'abstract' keyword.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/abstract_field_constructor_initializer/AbstractFieldConstructorInitializer__ab_d542ca00.dart")),
                c::ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER,
                "x);",
                "Try removing the field initializer or the 'abstract' keyword from the field declaration.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnostics/analyzer/abstract_field_constructor_initializer/AbstractFieldConstructorInitializer__ab_204a2986.dart")),
                c::ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER,
                "x = 0",
                "Try removing the field initializer or the 'abstract' keyword from the field declaration.",
            ),
        ] {
            let mut names = Interner::new();
            let parsed = parse(source, &mut names);
            let found = initializers(Unit { ast: &parsed.ast, unit: &parsed.unit, source });
            let start = source.rfind(fragment).unwrap();
            assert!(found.iter().any(|d|
                d.code == Some(code) && d.span.start == start && d.span.end == start + 1
                    && d.message == "Abstract fields can't have initializers."
                    && d.correction().as_deref() == Some(correction)
            ), "{source}: {found:?}");
        }
    }

    #[test]
    fn abstract_field_without_initializer_reports_nothing() {
        let source = "abstract class A { abstract int x; A(); }";
        let mut names = Interner::new();
        let parsed = parse(source, &mut names);
        let found = initializers(Unit {
            ast: &parsed.ast,
            unit: &parsed.unit,
            source,
        });
        assert!(found.is_empty(), "{found:?}");
    }
}
