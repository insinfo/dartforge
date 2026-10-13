//! The 3.6.2 linter's `hasObviousType` and `elementTypeOfIterable`
//! (`util/obvious_types.dart`), over the unit's tree and types. Substituting
//! the type arguments does not create types (`intern_lookup`): a type the
//! table does not have is not equal to any other in it.
//! Written without compiling or running (2026-10-05).

use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{Ast, BinaryOp, CollectionElement, ExprId, ExprKind, TypeKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeParamId, TypeTable};
use std::collections::HashMap;

/// `t` with the parameters of `map` replaced, without creating new types.
pub fn substitute_without_creating(table: &TypeTable, t: TypeId, map: &HashMap<TypeParamId, TypeId>) -> Option<TypeId> {
    let new_type = match table.get(t).clone() {
        Type::TypeParameter { param, nullable } => {
            let Some(&x) = map.get(&param) else { return Some(t) };
            if !nullable {
                return Some(x);
            }
            match table.get(x).clone() {
                Type::Interface { class, args, .. } => Type::Interface { class, args, nullable: true },
                Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable: true },
                Type::ExtensionType { decl, args, .. } => Type::ExtensionType { decl, args, nullable: true },
                Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable: true },
                Type::Dynamic | Type::Void | Type::Null => return Some(x),
                other => other,
            }
        }
        Type::Interface { class, args, nullable } => {
            let new_args: Option<Vec<TypeId>> = args.iter().map(|a| substitute_without_creating(table, *a, map)).collect();
            Type::Interface { class, args: new_args?.into(), nullable }
        }
        Type::ExtensionType { decl, args, nullable } => {
            let new_args: Option<Vec<TypeId>> = args.iter().map(|a| substitute_without_creating(table, *a, map)).collect();
            Type::ExtensionType { decl, args: new_args?.into(), nullable }
        }
        Type::FutureOr { arg, nullable } => Type::FutureOr { arg: substitute_without_creating(table, arg, map)?, nullable },
        _ => return Some(t),
    };
    table.intern_lookup(new_type)
}

/// Argument `i` of `t.asInstanceOf(target)`, without creating types.
pub fn argument_as_instance(s: &super::Semantics<'_>, t: TypeId, target: ClassId, i: usize) -> Option<TypeId> {
    let table = s.table;
    let (Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. }) = table.get(t) else { return None };
    if *class == target {
        return args.get(i).copied();
    }
    let data = s.outline.hierarchy.get(*class)?;
    let template = *data.supertypes.get(&target)?;
    let Type::Interface { args: margs, .. } = table.get(template) else { return None };
    let map: HashMap<TypeParamId, TypeId> = data.type_params.iter().copied().zip(args.iter().copied()).collect();
    substitute_without_creating(table, *margs.get(i)?, &map)
}

/// `elementTypeOfIterable`: the argument of `Iterable` among the implemented
/// interfaces (the type itself and its supertypes).
pub fn iterable_element_type(s: &super::Semantics<'_>, interner: &Interner, t: Option<TypeId>) -> Option<TypeId> {
    let t = t?;
    let iterable = s.core.iterable_class?;
    let _ = interner;
    match s.table.get(t) {
        Type::Interface { class, .. } => {
            let has = *class == iterable || s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&iterable));
            if !has {
                return None;
            }
            argument_as_instance(s, t, iterable, 0)
        }
        _ => None,
    }
}

/// The type of a collection element (`elementType`): the expression's, that
/// of an `if`'s `then`, that of a `...`'s iterable; nothing for the others.
fn element_type(s: &super::Semantics<'_>, interner: &Interner, el: &CollectionElement) -> Option<TypeId> {
    match el {
        CollectionElement::Expression(e) => s.body.get_type(*e),
        CollectionElement::If { then, .. } => element_type(s, interner, then),
        CollectionElement::Spread { value, .. } => iterable_element_type(s, interner, s.body.get_type(*value)),
        _ => None,
    }
}

fn key_and_value(s: &super::Semantics<'_>, el: &CollectionElement) -> (Option<TypeId>, Option<TypeId>) {
    match el {
        CollectionElement::MapEntry { key, value, .. } => (s.body.get_type(*key), s.body.get_type(*value)),
        _ => (None, None),
    }
}

fn obvious_element(s: &super::Semantics<'_>, interner: &Interner, a: &Ast, el: &CollectionElement) -> bool {
    match el {
        CollectionElement::Expression(e) => has_obvious_type(s, interner, a, *e),
        CollectionElement::MapEntry { key, value, .. } => has_obvious_type(s, interner, a, *key) && has_obvious_type(s, interner, a, *value),
        CollectionElement::If { then, else_, .. } => obvious_element(s, interner, a, then) && else_.as_ref().is_none_or(|x| obvious_element(s, interner, a, x)),
        CollectionElement::Spread { value, .. } => has_obvious_type(s, interner, a, *value),
        CollectionElement::NullAwareExpression(e) => has_obvious_type(s, interner, a, *e),
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => false,
    }
}

/// `Expression.hasObviousType`.
pub fn has_obvious_type(s: &super::Semantics<'_>, interner: &Interner, a: &Ast, e: ExprId) -> bool {
    let same_or_none = |x: Option<TypeId>, y: Option<TypeId>| x.is_none() || y.is_none() || x == y;
    match &a.expr(e).kind {
        ExprKind::List { type_args, elements, .. } | ExprKind::SetOrMap { type_args, elements, .. } => {
            if !type_args.is_empty() {
                return true;
            }
            let (mut obvious, mut key, mut value): (Option<TypeId>, Option<TypeId>, Option<TypeId>) = (None, None, None);
            for el in elements.iter() {
                if !obvious_element(s, interner, a, el) {
                    return false;
                }
                let te = element_type(s, interner, el);
                let (tk, tv) = key_and_value(s, el);
                obvious = obvious.or(te);
                key = key.or(tk);
                value = value.or(tv);
                if !same_or_none(obvious, te) || !same_or_none(key, tk) || !same_or_none(value, tv) {
                    return false;
                }
            }
            iterable_element_type(s, interner, s.body.get_type(e)) == obvious
        }
        ExprKind::Record { positional, named, .. } => {
            positional.iter().all(|x| has_obvious_type(s, interner, a, *x)) && named.iter().all(|(_, x)| has_obvious_type(s, interner, a, *x))
        }
        ExprKind::Int(_) => false,
        ExprKind::Bool(_) | ExprKind::Double(_) | ExprKind::Null | ExprKind::String(_) | ExprKind::Symbol(_) => true,
        ExprKind::Identifier(_) => {
            // A local variable (not a parameter) without promotion.
            if !matches!(s.body.get_resolved(e), Some(Resolved::Local(_))) {
                return false;
            }
            let Some(decl) = s.body.declaracao_local(e) else { return false };
            let is_function = a.stmts.iter().any(|x| matches!(&x.kind, dartforge_frontend::ast::StmtKind::Function(f) if a.function(*f).name.is_some_and(|n| n.span.start == decl)));
            if is_function {
                return false;
            }
            s.body.get_type(e).is_some() && s.body.get_type(e) == s.body.tipo_local(decl)
        }
        ExprKind::InstanceCreation { ty, .. } => match &a.ty(*ty).kind {
            TypeKind::Named { args, .. } if !args.is_empty() => true,
            _ => match s.body.get_type(e).map(|t| s.table.get(t)) {
                Some(Type::Interface { class, .. }) => s.outline.classes.get(class.0 as usize).is_none_or(|d| d.type_params.is_empty()),
                Some(_) => true,
                None => false,
            },
        },
        ExprKind::Call { target, .. } if matches!(s.body.get_resolved(e), Some(Resolved::Constructor(_))) => {
            if matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. }) {
                return true;
            }
            match s.body.get_type(e).map(|t| s.table.get(t)) {
                Some(Type::Interface { class, .. }) => s.outline.classes.get(class.0 as usize).is_none_or(|d| d.type_params.is_empty()),
                Some(_) => true,
                None => false,
            }
        }
        ExprKind::Cascade { target, .. } => has_obvious_type(s, interner, a, *target),
        ExprKind::As { .. } | ExprKind::Is { .. } | ExprKind::Throw(_) | ExprKind::This => true,
        ExprKind::Conditional { then, else_, .. } => {
            has_obvious_type(s, interner, a, *then) && has_obvious_type(s, interner, a, *else_) && s.body.get_type(*then) == s.body.get_type(*else_)
        }
        ExprKind::Parenthesized(x) => has_obvious_type(s, interner, a, *x),
        ExprKind::Property { name, .. } => interner.resolve(name.sym) == "hashCode",
        ExprKind::Call { target, .. } => match &a.expr(*target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => interner.resolve(name.sym) == "toString",
            _ => false,
        },
        ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, .. } => true,
        _ => false,
    }
}
