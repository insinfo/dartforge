//! `assignment_of_do_not_store` e `return_of_do_not_store`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.7): o
//! `BestPracticesVerifier` da 3.6.2 (`_checkForAssignmentOfDoNotStore`,
//! `_checkForReturnOfDoNotStore` e `_getSubExpressionsMarkedDoNotStore`,
//! `analyzer/lib/src/error/best_practices_verifier.dart`).
//!
//! As subexpressões "marcadas": a propriedade, o identificador e a chamada
//! de método cujo elemento tem `@doNotStore` (nele, na classe ou extensão
//! que o contém, ou na biblioteca), exceto o tear-off de função ou método;
//! o `?:` e o operador binário descem nos dois lados; a expressão de função
//! `=> e` desce no corpo. O inicializador de variável de topo e de campo dá
//! `assignment_of_do_not_store`; o `return e;` e o `=> e`, fora de membro
//! `@doNotStore`, dão `return_of_do_not_store` com o nome da função ou
//! método que os contém (sem um, nada). Nada num diretório `test`.
//!
//! A anotação é a do `package:meta`, resolvida pelo escopo da unidade em que
//! está escrita (`fase_resultado::anotacao_do_meta`). Diferença conhecida: o
//! nome no relato é o escrito na expressão.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, MemberKind, StmtKind};
use dartforge_intern::Interner;

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A lista, escrita na unidade `unit`, tem o `@doNotStore` do `package:meta`.
fn marcada(program: &Program, interner: &Interner, unit: UnitId, metadata: &[ast::Annotation]) -> bool {
    metadata.iter().any(|m| crate::fase_resultado::anotacao_do_meta(program, interner, unit, m, "doNotStore"))
}

/// A biblioteca da unidade `u` tem `@doNotStore` na diretiva `library`.
fn biblioteca_marcada(program: &Program, interner: &Interner, u: UnitId) -> bool {
    let lib = program.library(program.unit(u).library);
    lib.units.first().is_some_and(|&d| {
        program
            .unit(d)
            .unit
            .directives
            .iter()
            .any(|x| matches!(x.kind, DirectiveKind::Library { .. }) && marcada(program, interner, d, &x.metadata))
    })
}

/// `hasOrInheritsDoNotStore` do elemento resolvido `r`.
fn elemento_marcado(program: &Program, interner: &Interner, r: &Resolved) -> bool {
    let (metadata, onde) = crate::fase_resultado::anotacoes_do_elemento(program, r);
    if onde.is_some_and(|w| marcada(program, interner, w, metadata)) {
        return true;
    }
    // A classe ou a extensão que o contém.
    match r {
        Resolved::Member { class, .. } => {
            if let Some(d) = program.class(*class).decl {
                let a = &program.unit(d.unit).ast;
                if marcada(program, interner, d.unit, &a.decl(d.decl).metadata) {
                    return true;
                }
                return biblioteca_marcada(program, interner, d.unit);
            }
            false
        }
        Resolved::ExtensionMember { extension, .. } => {
            let d = program.extension(*extension).decl;
            let a = &program.unit(d.unit).ast;
            marcada(program, interner, d.unit, &a.decl(d.decl).metadata) || biblioteca_marcada(program, interner, d.unit)
        }
        _ => onde.is_some_and(|u| biblioteca_marcada(program, interner, u)),
    }
}

/// O elemento é uma função ou um método (não getter, setter nem acessor de
/// variável): o identificador e a propriedade que o citam são tear-off.
fn e_tear_off(program: &Program, r: &Resolved) -> bool {
    let f = match r {
        Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => *f,
        Resolved::ExtensionMember { member, .. } => *member,
        _ => return false,
    };
    let e = program.function(f);
    e.variable.is_none()
        && matches!(e.kind, dartforge_elements::model::FunctionKind::Function | dartforge_elements::model::FunctionKind::Operator)
}

/// `_getSubExpressionsMarkedDoNotStore`: as subexpressões marcadas, com o
/// nome a relatar.
fn marcadas(program: &Program, interner: &Interner, a: &ast::Ast, corpo: &UnitBodyTypes, e: ExprId, saida: &mut Vec<(Span, String)>) {
    let resolvido = |x: ExprId| corpo.resolved.get(x.0 as usize).and_then(|r| r.as_ref());
    let expr = a.expr(e);
    let achado: Option<(&Resolved, String)> = match &expr.kind {
        ExprKind::Property { name, .. } => resolvido(e).filter(|r| !e_tear_off(program, r)).map(|r| (r, interner.resolve(name.sym).to_string())),
        ExprKind::Identifier(n) => resolvido(e).filter(|r| !e_tear_off(program, r)).map(|r| (r, interner.resolve(n.sym).to_string())),
        ExprKind::Call { target, .. } => match &a.expr(*target).kind {
            ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                resolvido(*target).or_else(|| resolvido(e)).map(|r| (r, interner.resolve(n.sym).to_string()))
            }
            _ => None,
        },
        ExprKind::Conditional { then, else_, .. } => {
            marcadas(program, interner, a, corpo, *else_, saida);
            marcadas(program, interner, a, corpo, *then, saida);
            None
        }
        ExprKind::Binary { left, right, .. } => {
            marcadas(program, interner, a, corpo, *left, saida);
            marcadas(program, interner, a, corpo, *right, saida);
            None
        }
        ExprKind::FunctionExpression(f) => {
            if let FunctionBody::Expression(x) = &a.function(*f).body {
                marcadas(program, interner, a, corpo, *x, saida);
            }
            None
        }
        _ => None,
    };
    if let Some((r, nome)) = achado
        && elemento_marcado(program, interner, r)
    {
        saida.push((expr.span, nome));
    }
}

/// Os relatos da unidade `u`.
pub fn guardados_e_devolvidos(program: &Program, interner: &Interner, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if interner.lookup("doNotStore").is_none() {
        return out;
    }
    let unidade = program.unit(u);
    // `_inTestDirectory`.
    if unidade.path.as_ref().is_some_and(|p| p.components().any(|c| c.as_os_str() == "test")) {
        return out;
    }
    let a = &unidade.ast;

    // `assignment_of_do_not_store`: os inicializadores de topo e de campo.
    let mut iniciais: Vec<ExprId> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            iniciais.extend(l.variables.iter().filter_map(|v| v.initializer));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            iniciais.extend(l.variables.iter().filter_map(|v| v.initializer));
        }
    }
    for e in iniciais {
        let mut achados = Vec::new();
        marcadas(program, interner, a, corpo, e, &mut achados);
        for (s, nome) in achados {
            out.push(Diagnostic::com_codigo(w::ASSIGNMENT_OF_DO_NOT_STORE, s, [nome.as_str()]));
        }
    }

    // `return_of_do_not_store`.
    // As declarações de função e de método, com o nome e se são marcadas.
    let mut declaracoes: Vec<(Span, String, bool)> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Function(f) = &d.kind {
            let g = a.function(*f);
            declaracoes.push((g.span, g.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(), marcada(program, interner, u, &d.metadata)));
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Function(f) = &s.kind {
            let g = a.function(*f);
            declaracoes.push((g.span, g.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(), false));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Method(f) = &m.kind {
            let g = a.function(*f);
            declaracoes.push((g.span, g.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(), marcada(program, interner, u, &m.metadata)));
        }
    }
    // Os contêineres marcados (classe, mixin, extensão, tipo de extensão).
    let mut marcados: Vec<Span> = declaracoes.iter().filter(|x| x.2).map(|x| x.0).collect();
    for d in a.decls.iter() {
        if matches!(d.kind, DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Enum(_))
            && marcada(program, interner, u, &d.metadata)
        {
            marcados.push(d.span);
        }
    }
    let biblioteca = biblioteca_marcada(program, interner, u);
    let mut devolvidas: Vec<ExprId> = Vec::new();
    for s in a.stmts.iter() {
        if let StmtKind::Return(Some(e)) = &s.kind {
            devolvidas.push(*e);
        }
    }
    for f in a.functions.iter() {
        if let FunctionBody::Expression(e) = &f.body {
            devolvidas.push(*e);
        }
    }
    if !biblioteca {
        for e in devolvidas {
            let sp = a.expr(e).span;
            if marcados.iter().any(|&m| dentro(sp, m)) {
                continue;
            }
            // A declaração de função ou método mais de dentro.
            let Some(dono) = declaracoes.iter().filter(|x| dentro(sp, x.0)).min_by_key(|x| x.0.end - x.0.start) else { continue };
            let mut achados = Vec::new();
            marcadas(program, interner, a, corpo, e, &mut achados);
            for (s, nome) in achados {
                out.push(Diagnostic::com_codigo(w::RETURN_OF_DO_NOT_STORE, s, [nome.as_str(), dono.1.as_str()]));
            }
        }
    }
    out
}
