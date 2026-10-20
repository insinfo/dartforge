//! `non_const_call_to_literal_constructor` (and the `_USING_NEW` variant)
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.7): the 3.6.2
//! `BestPracticesVerifier._checkForLiteralConstructorUse`
//! (`analyzer/lib/src/error/best_practices_verifier.dart`).
//!
//! A creation without `const` of a constructor marked with the `@literal` of
//! `package:meta` (`phase_result::meta_annotation`) that could be `const`
//! reports the whole node with the constructor name as written
//! (`constructorName.type.qualifiedName` and `.the_name`); with `new`, the
//! `_USING_NEW` variant. "Could be `const`" is the analyzer linter's
//! `canBeConst`: the `const` constructor and the constant check of the
//! creation, as if it had `const`, without a constant error
//! (`const_eval::verifier::creation_can_be_const`).
//! Written without compiling or running (2026-10-05).

use crate::const_eval::evaluation::Engine;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, Resolved};
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::Diagnostic;
use dartforge_diagnostics::codes::warning as w;
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, CreationKeyword, ExprId, ExprKind};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// A candidate creation: the unit, the expression, the name to report and
/// whether it was written with `new`.
struct Candidate {
    unit: UnitId,
    lib: LibraryId,
    expr: ExprId,
    name: String,
    with_new: bool,
}

/// The reports of the libraries `libs` (our own, whose bodies `bodies` holds).
#[allow(clippy::too_many_arguments)]
pub fn literal_constructors(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    libs: &[LibraryId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut out = Vec::new();
    if interner.lookup("literal").is_none() {
        return out;
    }
    // The creations without `const` of a `@literal` constructor.
    let mut candidates: Vec<Candidate> = Vec::new();
    for &lib in libs {
        for &u in &program.library(lib).units {
            let Some(body) = bodies.units.get(u.0 as usize) else {
                continue;
            };
            let unit = program.unit(u);
            let a = &unit.ast;
            // `InstanceCreationExpression.isConst`: also without the keyword in a
            // constant context (`const m = Module(...)`).
            let mut parents = None;
            for (k, e) in a.exprs.iter().enumerate() {
                let id = ExprId(k as u32);
                let (with_new, the_name) = match &e.kind {
                    ExprKind::InstanceCreation {
                        keyword,
                        ty,
                        constructor,
                        ..
                    } => {
                        if matches!(keyword, Some(CreationKeyword::Const)) {
                            continue;
                        }
                        let ast::TypeKind::Named { name, .. } = &a.ty(*ty).kind else {
                            continue;
                        };
                        let typ = name
                            .iter()
                            .map(|n| interner.resolve(n.sym))
                            .collect::<Vec<_>>()
                            .join(".");
                        let the_name = match constructor {
                            Some(c) => format!("{typ}.{}", interner.resolve(c.sym)),
                            None => typ,
                        };
                        (matches!(keyword, Some(CreationKeyword::New)), the_name)
                    }
                    // A creation without `new` that resolution says is of a constructor: the
                    // written name, without the type arguments.
                    ExprKind::Call { target, .. } => {
                        if !matches!(body.get_resolved(id), Some(Resolved::Constructor(_))) {
                            continue;
                        }
                        (
                            false,
                            written_name(interner, a, *target).unwrap_or_default(),
                        )
                    }
                    _ => continue,
                };
                let Some(Resolved::Constructor(f)) = body.get_resolved(id) else {
                    continue;
                };
                let Some(where_) = crate::phase_result::function_unit(program, *f) else {
                    continue;
                };
                let marked = crate::phase_result::function_annotations(program, *f)
                    .iter()
                    .any(|m| {
                        crate::phase_result::meta_annotation(
                            program, interner, where_, m, "literal",
                        )
                    });
                let implicitly_const = !with_new
                    && parents
                        .get_or_insert_with(|| crate::typed_lints::unit_parents(program, u))
                        .in_constant_context(a, id);
                if marked && !implicitly_const {
                    candidates.push(Candidate {
                        unit: u,
                        lib,
                        expr: id,
                        name: the_name,
                        with_new,
                    });
                }
            }
        }
    }
    if candidates.is_empty() {
        return out;
    }
    let mut engine = Engine::new(
        program,
        interner,
        table,
        core,
        outline,
        bodies,
        the_inferred,
    );
    for c in candidates {
        if !crate::const_eval::verifier::creation_can_be_const(&mut engine, c.lib, c.unit, c.expr) {
            continue;
        }
        let code = if c.with_new {
            w::NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR_USING_NEW
        } else {
            w::NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR
        };
        let span = program.unit(c.unit).ast.expr(c.expr).span;
        out.push((c.unit, Diagnostic::with_code(code, span, [c.name.as_str()])));
    }
    out
}

/// The name written in the target of a creation without `new`: `C`, `p.C`,
/// `C.the_name`, `p.C.the_name`, without the type arguments.
fn written_name(interner: &Interner, a: &ast::Ast, e: ExprId) -> Option<String> {
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(interner.resolve(n.sym).to_string()),
        ExprKind::Property { target, name, .. } => Some(format!(
            "{}.{}",
            written_name(interner, a, *target)?,
            interner.resolve(name.sym)
        )),
        ExprKind::TypeArguments { target, .. } => written_name(interner, a, *target),
        _ => None,
    }
}
