//! The written target of a redirecting factory (`factory C() = D.name;`).
//!
//! The parser reads the target as a type and only splits off the final
//! `.name` when a dot is left over: `D.name` arrives as the type `[D, name]`,
//! which may be either the type `name` from import prefix `D` or class `D`
//! and constructor `name` (the analyzer's `AstRewriter` decides by scope).
//! The two cannot coexist in the same scope
//! (`PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER`), so the order of the attempts
//! does not change the result.

use dartforge_elements::model::{ClassId, Element, Program, UnitId};
use dartforge_frontend::ast;

/// The target class and the written constructor name (`None`: the unnamed
/// one).
pub fn class_and_constructor(
    program: &Program,
    unit: UnitId,
    r: &ast::RedirectTarget,
) -> Option<(ClassId, Option<ast::Name>)> {
    let ast::TypeKind::Named { name, .. } = &program.unit(unit).ast.ty(r.ty).kind else {
        return None;
    };
    let (b, constructor) = match (&name[..], r.constructor) {
        ([n], ctor) => (program.lookup_in_unit(unit, n.sym), ctor),
        ([p, n], None) => match program.lookup_prefixed_in_unit(unit, p.sym, n.sym) {
            Some(b) => (Some(b), None),
            None => (program.lookup_in_unit(unit, p.sym), Some(*n)),
        },
        ([p, n], ctor) => (program.lookup_prefixed_in_unit(unit, p.sym, n.sym), ctor),
        ([p, n, c], None) => (
            program.lookup_prefixed_in_unit(unit, p.sym, n.sym),
            Some(*c),
        ),
        _ => (None, None),
    };
    match b?.getter? {
        Element::Class(c) => Some((c, constructor)),
        _ => None,
    }
}
