//! O `hasObviousType` e o `elementTypeOfIterable` do linter 3.6.2
//! (`util/obvious_types.dart`), sobre a árvore e os tipos da unidade. A
//! substituição dos argumentos de tipo não cria tipos (`intern_lookup`): um
//! tipo que a tabela não tem não é igual a nenhum outro dela.
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{Ast, BinaryOp, CollectionElement, ExprId, ExprKind, TypeKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeParamId, TypeTable};
use std::collections::HashMap;

/// `t` com os parâmetros de `mapa` trocados, sem criar tipos novos.
pub fn substituir_sem_criar(table: &TypeTable, t: TypeId, mapa: &HashMap<TypeParamId, TypeId>) -> Option<TypeId> {
    let novo = match table.get(t).clone() {
        Type::TypeParameter { param, nullable } => {
            let Some(&x) = mapa.get(&param) else { return Some(t) };
            if !nullable {
                return Some(x);
            }
            match table.get(x).clone() {
                Type::Interface { class, args, .. } => Type::Interface { class, args, nullable: true },
                Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable: true },
                Type::ExtensionType { decl, args, .. } => Type::ExtensionType { decl, args, nullable: true },
                Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable: true },
                Type::Dynamic | Type::Void | Type::Null => return Some(x),
                outro => outro,
            }
        }
        Type::Interface { class, args, nullable } => {
            let novos: Option<Vec<TypeId>> = args.iter().map(|a| substituir_sem_criar(table, *a, mapa)).collect();
            Type::Interface { class, args: novos?.into(), nullable }
        }
        Type::ExtensionType { decl, args, nullable } => {
            let novos: Option<Vec<TypeId>> = args.iter().map(|a| substituir_sem_criar(table, *a, mapa)).collect();
            Type::ExtensionType { decl, args: novos?.into(), nullable }
        }
        Type::FutureOr { arg, nullable } => Type::FutureOr { arg: substituir_sem_criar(table, arg, mapa)?, nullable },
        _ => return Some(t),
    };
    table.intern_lookup(novo)
}

/// O argumento `i` de `t.asInstanceOf(alvo)`, sem criar tipos.
pub fn argumento_como_instancia(s: &super::Semantica<'_>, t: TypeId, alvo: ClassId, i: usize) -> Option<TypeId> {
    let table = s.table;
    let (Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. }) = table.get(t) else { return None };
    if *class == alvo {
        return args.get(i).copied();
    }
    let dados = s.outline.hierarchy.get(*class)?;
    let molde = *dados.supertypes.get(&alvo)?;
    let Type::Interface { args: margs, .. } = table.get(molde) else { return None };
    let mapa: HashMap<TypeParamId, TypeId> = dados.type_params.iter().copied().zip(args.iter().copied()).collect();
    substituir_sem_criar(table, *margs.get(i)?, &mapa)
}

/// `elementTypeOfIterable`: o argumento do `Iterable` entre as interfaces
/// implementadas (ela própria e os supertipos).
pub fn tipo_do_elemento_do_iteravel(s: &super::Semantica<'_>, interner: &Interner, t: Option<TypeId>) -> Option<TypeId> {
    let t = t?;
    let iteravel = s.core.iterable_class?;
    let _ = interner;
    match s.table.get(t) {
        Type::Interface { class, .. } => {
            let tem = *class == iteravel || s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&iteravel));
            if !tem {
                return None;
            }
            argumento_como_instancia(s, t, iteravel, 0)
        }
        _ => None,
    }
}

/// O tipo de um elemento de coleção (`elementType`): o da expressão, o do
/// `then` de um `if`, o do iterável de um `...`; nada para os outros.
fn tipo_do_elemento(s: &super::Semantica<'_>, interner: &Interner, el: &CollectionElement) -> Option<TypeId> {
    match el {
        CollectionElement::Expression(e) => s.corpo.get_type(*e),
        CollectionElement::If { then, .. } => tipo_do_elemento(s, interner, then),
        CollectionElement::Spread { value, .. } => tipo_do_elemento_do_iteravel(s, interner, s.corpo.get_type(*value)),
        _ => None,
    }
}

fn chave_e_valor(s: &super::Semantica<'_>, el: &CollectionElement) -> (Option<TypeId>, Option<TypeId>) {
    match el {
        CollectionElement::MapEntry { key, value, .. } => (s.corpo.get_type(*key), s.corpo.get_type(*value)),
        _ => (None, None),
    }
}

fn elemento_obvio(s: &super::Semantica<'_>, interner: &Interner, a: &Ast, el: &CollectionElement) -> bool {
    match el {
        CollectionElement::Expression(e) => tem_tipo_obvio(s, interner, a, *e),
        CollectionElement::MapEntry { key, value, .. } => tem_tipo_obvio(s, interner, a, *key) && tem_tipo_obvio(s, interner, a, *value),
        CollectionElement::If { then, else_, .. } => elemento_obvio(s, interner, a, then) && else_.as_ref().is_none_or(|x| elemento_obvio(s, interner, a, x)),
        CollectionElement::Spread { value, .. } => tem_tipo_obvio(s, interner, a, *value),
        CollectionElement::NullAwareExpression(e) => tem_tipo_obvio(s, interner, a, *e),
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => false,
    }
}

/// `Expression.hasObviousType`.
pub fn tem_tipo_obvio(s: &super::Semantica<'_>, interner: &Interner, a: &Ast, e: ExprId) -> bool {
    let mesmo_ou_nada = |x: Option<TypeId>, y: Option<TypeId>| x.is_none() || y.is_none() || x == y;
    match &a.expr(e).kind {
        ExprKind::List { type_args, elements, .. } | ExprKind::SetOrMap { type_args, elements, .. } => {
            if !type_args.is_empty() {
                return true;
            }
            let (mut obvio, mut chave, mut valor): (Option<TypeId>, Option<TypeId>, Option<TypeId>) = (None, None, None);
            for el in elements.iter() {
                if !elemento_obvio(s, interner, a, el) {
                    return false;
                }
                let te = tipo_do_elemento(s, interner, el);
                let (tk, tv) = chave_e_valor(s, el);
                obvio = obvio.or(te);
                chave = chave.or(tk);
                valor = valor.or(tv);
                if !mesmo_ou_nada(obvio, te) || !mesmo_ou_nada(chave, tk) || !mesmo_ou_nada(valor, tv) {
                    return false;
                }
            }
            tipo_do_elemento_do_iteravel(s, interner, s.corpo.get_type(e)) == obvio
        }
        ExprKind::Record { positional, named, .. } => {
            positional.iter().all(|x| tem_tipo_obvio(s, interner, a, *x)) && named.iter().all(|(_, x)| tem_tipo_obvio(s, interner, a, *x))
        }
        ExprKind::Int(_) => false,
        ExprKind::Bool(_) | ExprKind::Double(_) | ExprKind::Null | ExprKind::String(_) | ExprKind::Symbol(_) => true,
        ExprKind::Identifier(_) => {
            // Uma variável local (não parâmetro) sem promoção.
            if !matches!(s.corpo.get_resolved(e), Some(Resolved::Local(_))) {
                return false;
            }
            let Some(decl) = s.corpo.declaracao_local(e) else { return false };
            let e_funcao = a.stmts.iter().any(|x| matches!(&x.kind, dartforge_frontend::ast::StmtKind::Function(f) if a.function(*f).name.is_some_and(|n| n.span.start == decl)));
            if e_funcao {
                return false;
            }
            s.corpo.get_type(e).is_some() && s.corpo.get_type(e) == s.corpo.tipo_local(decl)
        }
        ExprKind::InstanceCreation { ty, .. } => match &a.ty(*ty).kind {
            TypeKind::Named { args, .. } if !args.is_empty() => true,
            _ => match s.corpo.get_type(e).map(|t| s.table.get(t)) {
                Some(Type::Interface { class, .. }) => s.outline.classes.get(class.0 as usize).is_none_or(|d| d.type_params.is_empty()),
                Some(_) => true,
                None => false,
            },
        },
        ExprKind::Call { target, .. } if matches!(s.corpo.get_resolved(e), Some(Resolved::Constructor(_))) => {
            if matches!(a.expr(*target).kind, ExprKind::TypeArguments { .. }) {
                return true;
            }
            match s.corpo.get_type(e).map(|t| s.table.get(t)) {
                Some(Type::Interface { class, .. }) => s.outline.classes.get(class.0 as usize).is_none_or(|d| d.type_params.is_empty()),
                Some(_) => true,
                None => false,
            }
        }
        ExprKind::Cascade { target, .. } => tem_tipo_obvio(s, interner, a, *target),
        ExprKind::As { .. } | ExprKind::Is { .. } | ExprKind::Throw(_) | ExprKind::This => true,
        ExprKind::Conditional { then, else_, .. } => {
            tem_tipo_obvio(s, interner, a, *then) && tem_tipo_obvio(s, interner, a, *else_) && s.corpo.get_type(*then) == s.corpo.get_type(*else_)
        }
        ExprKind::Parenthesized(x) => tem_tipo_obvio(s, interner, a, *x),
        ExprKind::Property { name, .. } => interner.resolve(name.sym) == "hashCode",
        ExprKind::Call { target, .. } => match &a.expr(*target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => interner.resolve(name.sym) == "toString",
            _ => false,
        },
        ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, .. } => true,
        _ => false,
    }
}
