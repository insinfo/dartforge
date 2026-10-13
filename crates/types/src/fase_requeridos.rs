//! A parte do `RequiredParametersVerifier` do analyzer que trata o
//! `@required` do `package:meta`
//! (`analyzer/lib/src/error/required_parameters_verifier.dart`, lido por
//! inteiro na 6.11.0; docs/ANALYZER-ESPECIFICACAO.md, família A):
//! `missing_required_param`, com e sem detalhes, na chamada que não passa
//! um parâmetro nomeado opcional anotado com `@required` ou
//! `@Required('motivo')`. O `missing_required_argument` (o `required` da
//! linguagem) já é emitido pela inferência.
//!
//! Cobre chamadas de função e de método, criações de instância (com e sem
//! `new`). Fora: anotações, constantes de enum, invocações de expressão de
//! função e as chamadas `this(...)` e `super(...)` de construtor. A
//! anotação é reconhecida pelo nome.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, FunctionElementId, FunctionRef, Program, UnitId};
use dartforge_frontend::ast::{self, ExprKind, MemberKind, ParameterKind, TypeKind};
use dartforge_intern::Interner;

/// Os parâmetros declarados de uma função, método ou construtor, com a
/// árvore em que estão.
fn parametros(program: &Program, f: FunctionElementId) -> Option<(&[ast::Parameter], &ast::Ast)> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            a.function(function).parameters.as_deref().map(|p| (p, a))
        }
        FunctionRef::Constructor { unit, member } => {
            let a = &program.unit(unit).ast;
            match &a.member(member).kind {
                MemberKind::Constructor(k) => Some((&k.parameters[..], a)),
                _ => None,
            }
        }
        FunctionRef::None => None,
    }
}

/// `_requiredAnnotation`: `Some(motivo)` se o parâmetro tem `@required` ou
/// `@Required(...)`; o motivo é o primeiro argumento posicional, se string
/// não vazia.
fn the_required(p: &ast::Parameter, a: &ast::Ast, interner: &Interner) -> Option<Option<String>> {
    for m in p.metadata.iter() {
        let names: Vec<&str> = m.name.iter().map(|n| interner.resolve(n.sym)).collect();
        match &m.arguments {
            None if names.last() == Some(&"required") => return Some(None),
            Some(args) if names.contains(&"Required") => {
                let reason = args.args.iter().find(|x| x.name.is_none()).and_then(|x| match &a.expr(x.value).kind {
                    ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                    _ => None,
                });
                return Some(reason.filter(|m| !m.is_empty()));
            }
            _ => {}
        }
    }
    None
}

/// Os `missing_required_param` da unidade `u`.
pub fn requeridos_ausentes(program: &Program, interner: &Interner, body: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Sem nenhuma das duas anotações internadas, nada a fazer.
    if interner.lookup("required").is_none() && interner.lookup("Required").is_none() {
        return out;
    }
    let a = &program.unit(u).ast;
    let resolved = |e: ast::ExprId| body.resolved.get(e.0 as usize).and_then(|r| r.as_ref());
    let function_of = |r: &Resolved| match r {
        Resolved::Constructor(f) | Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => Some(*f),
        _ => None,
    };
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ast::ExprId(i as u32);
        // A função chamada, os argumentos e a entidade do relato: o nome do
        // método, ou o nome do construtor inteiro.
        let (f, args_list, entity): (FunctionElementId, &ast::Arguments, Span) = match &e.kind {
            ExprKind::Call { target, arguments } => {
                let Some(r) = resolved(*target).or_else(|| resolved(id)) else { continue };
                let Some(f) = function_of(r) else { continue };
                let entity = match (&a.expr(*target).kind, r) {
                    (_, Resolved::Constructor(_)) => a.expr(*target).span,
                    (ExprKind::Identifier(n), _) | (ExprKind::Property { name: n, .. }, _) => n.span,
                    _ => continue,
                };
                (f, arguments, entity)
            }
            ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                let Some(Resolved::Constructor(f)) = resolved(id) else { continue };
                let typ = &a.types[ty.0 as usize];
                // Sem os argumentos de tipo: o nome do tipo e o do construtor.
                let type_end = match &typ.kind {
                    TypeKind::Named { name, .. } => name.last().map_or(typ.span.end, |n| n.span.end),
                    _ => typ.span.end,
                };
                let end = constructor.map_or(typ.span.end.max(type_end), |k| k.span.end);
                (*f, arguments, Span { start: typ.span.start, end: end })
            }
            _ => continue,
        };
        let Some((list, tree)) = parametros(program, f) else { continue };
        for p in list.iter().filter(|p| p.kind == ParameterKind::Named && !p.required) {
            let Some(the_name) = p.name else { continue };
            let Some(reason) = the_required(p, tree, interner) else { continue };
            if args_list.args.iter().any(|x| x.name.is_some_and(|n| n.sym == the_name.sym)) {
                continue;
            }
            let text = interner.resolve(the_name.sym);
            out.push(match reason {
                Some(m) => Diagnostic::com_codigo(w::MISSING_REQUIRED_PARAM_WITH_DETAILS, entity, [text, m.as_str()]),
                None => Diagnostic::com_codigo(w::MISSING_REQUIRED_PARAM, entity, [text]),
            });
        }
    }
    out
}
