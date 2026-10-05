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
fn requerido(p: &ast::Parameter, a: &ast::Ast, interner: &Interner) -> Option<Option<String>> {
    for m in p.metadata.iter() {
        let nomes: Vec<&str> = m.name.iter().map(|n| interner.resolve(n.sym)).collect();
        match &m.arguments {
            None if nomes.last() == Some(&"required") => return Some(None),
            Some(args) if nomes.contains(&"Required") => {
                let motivo = args.args.iter().find(|x| x.name.is_none()).and_then(|x| match &a.expr(x.value).kind {
                    ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                    _ => None,
                });
                return Some(motivo.filter(|m| !m.is_empty()));
            }
            _ => {}
        }
    }
    None
}

/// Os `missing_required_param` da unidade `u`.
pub fn requeridos_ausentes(program: &Program, interner: &Interner, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Sem nenhuma das duas anotações internadas, nada a fazer.
    if interner.lookup("required").is_none() && interner.lookup("Required").is_none() {
        return out;
    }
    let a = &program.unit(u).ast;
    let resolvido = |e: ast::ExprId| corpo.resolved.get(e.0 as usize).and_then(|r| r.as_ref());
    let funcao_de = |r: &Resolved| match r {
        Resolved::Constructor(f) | Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => Some(*f),
        _ => None,
    };
    for (i, e) in a.exprs.iter().enumerate() {
        let id = ast::ExprId(i as u32);
        // A função chamada, os argumentos e a entidade do relato: o nome do
        // método, ou o nome do construtor inteiro.
        let (f, argumentos, entidade): (FunctionElementId, &ast::Arguments, Span) = match &e.kind {
            ExprKind::Call { target, arguments } => {
                let Some(r) = resolvido(*target).or_else(|| resolvido(id)) else { continue };
                let Some(f) = funcao_de(r) else { continue };
                let entidade = match (&a.expr(*target).kind, r) {
                    (_, Resolved::Constructor(_)) => a.expr(*target).span,
                    (ExprKind::Identifier(n), _) | (ExprKind::Property { name: n, .. }, _) => n.span,
                    _ => continue,
                };
                (f, arguments, entidade)
            }
            ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                let Some(Resolved::Constructor(f)) = resolvido(id) else { continue };
                let tipo = &a.types[ty.0 as usize];
                // Sem os argumentos de tipo: o nome do tipo e o do construtor.
                let fim_do_tipo = match &tipo.kind {
                    TypeKind::Named { name, .. } => name.last().map_or(tipo.span.end, |n| n.span.end),
                    _ => tipo.span.end,
                };
                let fim = constructor.map_or(tipo.span.end.max(fim_do_tipo), |k| k.span.end);
                (*f, arguments, Span { start: tipo.span.start, end: fim })
            }
            _ => continue,
        };
        let Some((lista, arvore)) = parametros(program, f) else { continue };
        for p in lista.iter().filter(|p| p.kind == ParameterKind::Named && !p.required) {
            let Some(nome) = p.name else { continue };
            let Some(motivo) = requerido(p, arvore, interner) else { continue };
            if argumentos.args.iter().any(|x| x.name.is_some_and(|n| n.sym == nome.sym)) {
                continue;
            }
            let texto = interner.resolve(nome.sym);
            out.push(match motivo {
                Some(m) => Diagnostic::com_codigo(w::MISSING_REQUIRED_PARAM_WITH_DETAILS, entidade, [texto, m.as_str()]),
                None => Diagnostic::com_codigo(w::MISSING_REQUIRED_PARAM, entidade, [texto]),
            });
        }
    }
    out
}
