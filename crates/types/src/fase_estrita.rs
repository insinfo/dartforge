//! `inference_failure_on_untyped_parameter` e
//! `inference_failure_on_function_return_type` (docs/ANALYZER-ESPECIFICACAO-INFRA.md,
//! lote II.7 e §4.6): o `BestPracticesVerifier` da 3.6.2
//! (`_checkStrictInferenceInParameters` e `_checkStrictInferenceReturnType`,
//! `analyzer/lib/src/error/best_practices_verifier.dart:1359-1431`).
//!
//! Só valem com `analyzer: language: strict-inference: true`; a fase relata
//! sempre e quem publica (`dartforge_paridade::diagnosticos_json`) tira os
//! dois códigos sem a opção.
//!
//! * Parâmetro simples sem tipo (nem `this.`, nem `super.`, nem forma de
//!   função), citado: o nó do parâmetro sem o valor padrão, com o nome. Nos
//!   construtores (`visitConstructorDeclaration`), nas declarações de função
//!   de topo e locais (`visitFunctionDeclaration`), nos métodos que não
//!   sobrescrevem (`visitMethodDeclaration`), nas expressões de função cujo
//!   contexto não era um tipo de função (`visitFunctionExpression`), nos
//!   typedefs antigos (`visitFunctionTypeAlias`) e nos parâmetros-função
//!   (`visitFunctionTypedFormalParameter`, em qualquer lugar da árvore,
//!   também dentro de método que sobrescreve, de expressão de função e de
//!   tipo `Function(…)`).
//! * Sem tipo de retorno: a função de topo (não setter) e o método (não
//!   setter, não sobrescrevendo), no nome; o typedef antigo e o
//!   parâmetro-função, no nó; o tipo `Function(…)` que não é o do typedef
//!   `= Function(…)`, no nó, com o `toSource` dele
//!   (`dartforge_frontend::fonte`); o typedef `= Function(…)`, no nó, com o
//!   nome.
//!
//! "Citado" é o `isParameterReferenced`: sem corpo (ou corpo `;`) e sem
//! lista de inicializadores (typedef, abstrato, externo, parâmetro-função),
//! sempre; senão, o `_UsedParameterVisitor` sobre o corpo e os
//! inicializadores: um identificador cujo local resolvido é o próprio
//! parâmetro (a declaração dele, `UnitBodyTypes::declaracao_local`). O
//! construtor tem sempre a lista de inicializadores (nunca nula no
//! analyzer), então é sempre visitado; o corpo `native` também.
//! "Sobrescreve" é o `getOverridden2` (`fase_override::sobrescreve`: a
//! chave, `nome` ou `nome_=`, em algum supertipo, com o nome privado só da
//! mesma biblioteca; o `overridden` do analyzer são os candidatos herdados,
//! então vale também para o membro estático). A expressão de função é
//! conferida quando o contexto não era um tipo de função
//! (`UnitBodyTypes::com_tipo_de_funcao`, gravado pela inferência como o
//! `wasFunctionTypeSupplied`). Os relatos repetidos saem uma vez, como no
//! conjunto de erros do analyzer.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, Parameter, TypeKind, TypedefKind};
use dartforge_frontend::fonte::fim_do_parametro_funcao;
use dartforge_intern::{Interner, SymbolId};

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// As regiões que o `_UsedParameterVisitor` percorre, ou `None` quando o
/// `isParameterReferenced` dá todo parâmetro por citado: sem corpo (ou com
/// o corpo `;`) e sem lista de inicializadores.
fn regioes_de(a: &ast::Ast, corpo: &FunctionBody, inicializadores: Option<&[Initializer]>) -> Option<Vec<Span>> {
    if matches!(corpo, FunctionBody::Empty) && inicializadores.is_none() {
        return None;
    }
    let mut v = Vec::new();
    match corpo {
        FunctionBody::Block(s) => v.push(a.stmt(*s).span),
        FunctionBody::Expression(e) => v.push(a.expr(*e).span),
        // O corpo `native 'x';` só tem o literal: nenhum identificador.
        FunctionBody::Empty | FunctionBody::Native(_) => {}
    }
    for i in inicializadores.unwrap_or(&[]) {
        let sp = match i {
            Initializer::Assert { span, .. } | Initializer::Redirect { span, .. } | Initializer::Super { span, .. } => *span,
            Initializer::Field { value, .. } => a.expr(*value).span,
        };
        v.push(sp);
    }
    Some(v)
}

/// Os relatos da unidade `u` (sem olhar a opção).
#[allow(clippy::too_many_arguments)]
pub fn falhas_de_inferencia(
    program: &Program,
    interner: &Interner,
    table: &mut crate::table::TypeTable,
    core: &crate::table::CoreTypes,
    outline: &OutlineTypes,
    corpo: &UnitBodyTypes,
    u: UnitId,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();
    let nome = |s: SymbolId| interner.resolve(s).to_string();

    // `_UsedParameterVisitor`: um identificador nas regiões cujo local
    // resolvido é o parâmetro declarado em `declaracao`.
    let citado = |regioes: &[Span], declaracao: usize| {
        a.exprs.iter().enumerate().any(|(k, e)| {
            matches!(e.kind, ExprKind::Identifier(_))
                && regioes.iter().any(|&r| dentro(e.span, r))
                && corpo.declaracao_local(ExprId(k as u32)) == Some(declaracao)
        })
    };
    // `_checkStrictInferenceInParameters`: os parâmetros simples da lista
    // (também dentro do `DefaultFormalParameter`).
    let parametros = |ps: &[Parameter], regioes: Option<&[Span]>, out: &mut Vec<Diagnostic>| {
        for p in ps {
            if p.ty.is_some() || p.this_ || p.super_ || p.function_parameters.is_some() {
                continue;
            }
            let Some(n) = p.name else { continue };
            let referido = match regioes {
                None => true,
                Some(r) => citado(r, n.span.start),
            };
            if referido {
                // O nó do `SimpleFormalParameter`, sem o valor padrão: o
                // nome é o último token dele.
                let fim = if p.default_value.is_some() { n.span.end } else { p.span.end };
                out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_UNTYPED_PARAMETER, Span { start: p.span.start, end: fim }, [nome(n.sym)]));
            }
        }
    };
    // A classe da declaração (o `InterfaceElement` que a contém).
    let classe_da_decl = |d: ast::DeclId| -> Option<ClassId> {
        program.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == u && r.decl == d)).map(|i| ClassId(i as u32))
    };
    // `getOverridden2` (o mesmo de `fase_override`), pela chave do membro.
    let lib = unidade.library;
    let mut heranca = crate::heranca::Heranca::default();
    let mut provedor = crate::heranca::ProvedorDoOutline { program, interner, core, outline, table };
    let mut sobrescreve =
        |c: ClassId, chave: Option<SymbolId>| crate::fase_override::sobrescreve(&mut heranca, &mut provedor, c, lib, chave);

    // `visitFunctionDeclaration`: as funções de topo e as locais.
    let mut declaradas: std::collections::HashSet<ast::FunctionId> = std::collections::HashSet::new();
    for d in a.decls.iter() {
        match &d.kind {
            DeclKind::Function(f) => {
                declaradas.insert(*f);
                let g = a.function(*f);
                // Só a de topo tem o retorno conferido.
                if g.return_type.is_none() && g.kind != ast::FunctionKind::Setter && let Some(n) = g.name {
                    out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, n.span, [nome(n.sym)]));
                }
                if let Some(ps) = &g.parameters {
                    parametros(ps, regioes_de(a, &g.body, None).as_deref(), &mut out);
                }
            }
            // `visitFunctionTypeAlias`: o retorno e os parâmetros, sem corpo.
            DeclKind::Typedef(t) => match &t.kind {
                TypedefKind::Legacy { return_type, parameters } => {
                    if return_type.is_none() {
                        out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, d.span, [nome(t.name.sym)]));
                    }
                    parametros(parameters, None, &mut out);
                }
                // `visitGenericTypeAlias`.
                TypedefKind::Alias(ty) => {
                    if let TypeKind::Function { return_type: None, .. } = &a.ty(*ty).kind {
                        out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, d.span, [nome(t.name.sym)]));
                    }
                }
            },
            _ => {}
        }
    }
    for s in a.stmts.iter() {
        if let ast::StmtKind::Function(f) = &s.kind {
            declaradas.insert(*f);
            let g = a.function(*f);
            if let Some(ps) = &g.parameters {
                parametros(ps, regioes_de(a, &g.body, None).as_deref(), &mut out);
            }
        }
    }
    // `visitMethodDeclaration` e `visitConstructorDeclaration`.
    for (di, d) in a.decls.iter().enumerate() {
        let membros: &[ast::MemberId] = match &d.kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Extension(x) => &x.members,
            DeclKind::ExtensionType(x) => &x.members,
            _ => continue,
        };
        let classe = classe_da_decl(ast::DeclId(di as u32));
        for &m in membros {
            match &a.member(m).kind {
                MemberKind::Method(f) => {
                    let g = a.function(*f);
                    let Some(n) = g.name else { continue };
                    // O `Name` do elemento: o do setter é `nome=`.
                    let chave = if g.kind == ast::FunctionKind::Setter { interner.lookup(&format!("{}_=", nome(n.sym))) } else { Some(n.sym) };
                    if classe.is_some_and(|c| sobrescreve(c, chave)) {
                        continue;
                    }
                    if g.return_type.is_none() && g.kind != ast::FunctionKind::Setter {
                        out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, n.span, [nome(n.sym)]));
                    }
                    if let Some(ps) = &g.parameters {
                        parametros(ps, regioes_de(a, &g.body, None).as_deref(), &mut out);
                    }
                }
                MemberKind::Constructor(k) => {
                    parametros(&k.parameters, regioes_de(a, &k.body, Some(&k.initializers[..])).as_deref(), &mut out);
                }
                MemberKind::Field(_) => {}
            }
        }
    }
    // `visitFunctionExpression`: só sem tipo de função do contexto.
    for e in a.exprs.iter() {
        let ExprKind::FunctionExpression(f) = &e.kind else { continue };
        if declaradas.contains(f) || corpo.com_tipo_de_funcao.contains(f) {
            continue;
        }
        let g = a.function(*f);
        if let Some(ps) = &g.parameters {
            parametros(ps, regioes_de(a, &g.body, None).as_deref(), &mut out);
        }
    }
    // `visitFunctionTypedFormalParameter`, em toda lista de parâmetros da
    // unidade: funções (de topo, locais, métodos, expressões de função),
    // construtores, typedefs antigos e tipos `Function(…)`.
    let mut listas: Vec<&[Parameter]> = Vec::new();
    listas.extend(a.functions.iter().filter_map(|f| f.parameters.as_deref()));
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            listas.push(&k.parameters[..]);
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Typedef(t) = &d.kind
            && let TypedefKind::Legacy { parameters, .. } = &t.kind
        {
            listas.push(&parameters[..]);
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { parameters, .. } = &t.kind {
            listas.push(&parameters[..]);
        }
    }
    let mut pilha: Vec<&Parameter> = listas.into_iter().flatten().filter(|p| p.function_parameters.is_some()).collect();
    while let Some(q) = pilha.pop() {
        let Some(fs) = &q.function_parameters else { continue };
        // `this.f(…)` e `super.f(…)` não são `FunctionTypedFormalParameter`,
        // mas a lista deles é percorrida.
        if !q.this_
            && !q.super_
            && let Some(n) = q.name
        {
            if q.ty.is_none() {
                let fim = if q.default_value.is_some() { fim_do_parametro_funcao(fonte, n.span.end) } else { q.span.end };
                out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, Span { start: q.span.start, end: fim }, [nome(n.sym)]));
            }
            parametros(fs, None, &mut out);
        }
        pilha.extend(fs.iter().filter(|x| x.function_parameters.is_some()));
    }
    // `visitGenericFunctionType`: o tipo `Function(…)` sem retorno que não é
    // o do typedef `= Function(…)`, com o `node.toString()`.
    let dos_typedefs: Vec<ast::TypeId> = a
        .decls
        .iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Typedef(t) => match &t.kind {
                TypedefKind::Alias(ty) => Some(*ty),
                _ => None,
            },
            _ => None,
        })
        .collect();
    for (k, t) in a.types.iter().enumerate() {
        let id = ast::TypeId(k as u32);
        if let TypeKind::Function { return_type: None, .. } = &t.kind
            && !dos_typedefs.contains(&id)
        {
            let texto = dartforge_frontend::fonte::de_tipo(a, fonte, interner, id);
            out.push(Diagnostic::com_codigo(w::INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE, t.span, [texto]));
        }
    }
    // O conjunto de erros do analyzer: o mesmo relato sai uma vez.
    out.sort_by_key(|d| (d.span.start, d.span.end));
    out.dedup_by(|x, y| x.span == y.span && x.code == y.code && x.message == y.message);
    out
}
