//! `assignment_of_do_not_store` e `return_of_do_not_store`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.7): o
//! `BestPracticesVerifier` da 3.6.2 (`_checkForAssignmentOfDoNotStore`,
//! `_checkForReturnOfDoNotStore` e `_getSubExpressionsMarkedDoNotStore`,
//! `analyzer/lib/src/error/best_practices_verifier.dart`).
//!
//! As subexpressões "marcadas": a propriedade (`PropertyAccess` e
//! `PrefixedIdentifier`), o identificador e a chamada de método
//! (`MethodInvocation`, não a criação de instância) cujo elemento tem
//! `hasOrInheritsDoNotStore` (nele; na classe, mixin, enum, tipo de extensão
//! ou extensão que o contém; ou, para o que está numa unidade, na
//! biblioteca), exceto o tear-off de função ou método; o acessor sintético
//! vale pela variável. O `?:` e o operador binário descem nos dois lados; a
//! expressão de função `=> e` desce no corpo; o parêntese não desce. O nome
//! relatado é o `name` do elemento (o identificador escrito).
//!
//! O inicializador de variável de topo e de campo dá
//! `assignment_of_do_not_store`; o `return e;` e o `=> e`, fora de
//! `_inDoNotStoreMember`, dão `return_of_do_not_store` com o nome da
//! declaração de função ou método mais próxima que os contém (sem uma,
//! nada). O `_inDoNotStoreMember` começa com a biblioteca e é ligado só pela
//! classe (`visitClassDeclaration`), pela função de topo ou local
//! (`visitFunctionDeclaration`) e pelo método (`visitMethodDeclaration`)
//! marcados. Nada numa unidade em diretório de teste (`inTestDir`: `test`,
//! `integration_test`, `test_driver`, `testing`).
//!
//! A anotação é a do `package:meta`, resolvida pelo escopo da unidade em que
//! está escrita (`fase_resultado::anotacao_do_meta`). A metadata da
//! biblioteca é a da diretiva `library` ou, sem ela, a da primeira diretiva
//! (`ElementBuilder._buildLibraryMetadata`). A de variável local e de
//! função local vem de `Ast::metadados_locais`; a de parâmetro, do nó.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprId, ExprKind, ForInTarget, FunctionBody, MemberKind, StmtId, StmtKind};
use dartforge_intern::Interner;

/// Os diretórios de teste do `CompilationUnitExtension.inTestDir`.
const DIRETORIOS_DE_TESTE: [&str; 4] = ["test", "integration_test", "test_driver", "testing"];

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A lista, escrita na unidade `unit`, tem o `@doNotStore` do `package:meta`.
fn marcada(program: &Program, interner: &Interner, unit: UnitId, metadata: &[ast::Annotation]) -> bool {
    metadata.iter().any(|m| crate::fase_resultado::anotacao_do_meta(program, interner, unit, m, "doNotStore"))
}

/// `LibraryElement.hasDoNotStore` da biblioteca da unidade `u`: a metadata
/// da diretiva `library` da unidade que define a biblioteca ou, sem ela, a
/// da primeira diretiva.
fn biblioteca_marcada(program: &Program, interner: &Interner, u: UnitId) -> bool {
    let lib = program.library(program.unit(u).library);
    lib.units.first().is_some_and(|&d| {
        let ds = &program.unit(d).unit.directives;
        let alvo = ds.iter().find(|x| matches!(x.kind, DirectiveKind::Library { .. })).or_else(|| ds.first());
        alvo.is_some_and(|x| marcada(program, interner, d, &x.metadata))
    })
}

/// O que a declaração de um local é: a metadata e se é uma função local.
struct DeclaracaoLocal<'a> {
    metadata: &'a [ast::Annotation],
    funcao: bool,
}

/// A metadata de uma instrução (`Ast::metadados_locais`).
fn metadata_da_instrucao(a: &ast::Ast, s: StmtId) -> &[ast::Annotation] {
    a.metadados_locais.iter().find(|(x, _)| *x == s).map(|(_, m)| &**m).unwrap_or(&[])
}

/// O parâmetro de `ps` declarado no deslocamento `decl`.
fn parametro_em(ps: &[ast::Parameter], decl: usize) -> Option<&ast::Parameter> {
    ps.iter().find(|p| p.name.is_some_and(|n| n.span.start == decl))
}

/// A declaração do local declarado no deslocamento `decl`: parâmetro,
/// variável local, função local ou variável do `for-in`.
fn declaracao_local(a: &ast::Ast, decl: usize) -> Option<DeclaracaoLocal<'_>> {
    for f in a.functions.iter() {
        if let Some(p) = f.parameters.as_deref().and_then(|ps| parametro_em(ps, decl)) {
            return Some(DeclaracaoLocal { metadata: &p.metadata, funcao: false });
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let Some(p) = parametro_em(&k.parameters, decl)
        {
            return Some(DeclaracaoLocal { metadata: &p.metadata, funcao: false });
        }
    }
    for (i, s) in a.stmts.iter().enumerate() {
        let id = StmtId(i as u32);
        match &s.kind {
            StmtKind::Variables(l) if l.variables.iter().any(|v| v.name.span.start == decl) => {
                return Some(DeclaracaoLocal { metadata: metadata_da_instrucao(a, id), funcao: false });
            }
            StmtKind::Function(f) if a.function(*f).name.is_some_and(|n| n.span.start == decl) => {
                return Some(DeclaracaoLocal { metadata: metadata_da_instrucao(a, id), funcao: true });
            }
            StmtKind::ForIn { target: ForInTarget::Declared { metadata, name, .. }, .. } if name.span.start == decl => {
                return Some(DeclaracaoLocal { metadata, funcao: false });
            }
            _ => {}
        }
    }
    None
}

/// O parâmetro `nome` da função ou do construtor mais de dentro que contém
/// `e` e o declara (`Resolved::Parameter`).
fn declaracao_do_parametro(a: &ast::Ast, e: Span, nome: dartforge_intern::SymbolId) -> Option<DeclaracaoLocal<'_>> {
    let mut melhor: Option<(usize, &ast::Parameter)> = None;
    let mut listas: Vec<(Span, &[ast::Parameter])> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            listas.push((f.span, ps));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            listas.push((m.span, &k.parameters[..]));
        }
    }
    for (sp, ps) in listas {
        if !dentro(e, sp) {
            continue;
        }
        if let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == nome)) {
            let tamanho = sp.end - sp.start;
            if melhor.is_none_or(|(t, _)| tamanho < t) {
                melhor = Some((tamanho, p));
            }
        }
    }
    melhor.map(|(_, p)| DeclaracaoLocal { metadata: &p.metadata, funcao: false })
}

/// `hasOrInheritsDoNotStore` do elemento resolvido `r` da expressão `e`
/// (`u` é a unidade da expressão, para os locais).
fn elemento_marcado(program: &Program, interner: &Interner, u: UnitId, corpo: &UnitBodyTypes, e: ExprId, r: &Resolved) -> bool {
    let a = &program.unit(u).ast;
    match r {
        // O elemento local: só a própria metadata (o que o contém é uma
        // função, nem classe nem unidade).
        Resolved::Local(_) => corpo
            .declaracao_local(e)
            .and_then(|d| declaracao_local(a, d))
            .is_some_and(|d| marcada(program, interner, u, d.metadata)),
        Resolved::Parameter { name, .. } => {
            declaracao_do_parametro(a, a.expr(e).span, *name).is_some_and(|d| marcada(program, interner, u, d.metadata))
        }
        _ => {
            let (metadata, onde) = crate::fase_resultado::anotacoes_do_elemento(program, r);
            if onde.is_some_and(|w| marcada(program, interner, w, metadata)) {
                return true;
            }
            // A classe ou a extensão que o contém; depois, a biblioteca.
            match r {
                Resolved::Member { class, .. } => match program.class(*class).decl {
                    Some(d) => {
                        let a = &program.unit(d.unit).ast;
                        marcada(program, interner, d.unit, &a.decl(d.decl).metadata) || biblioteca_marcada(program, interner, d.unit)
                    }
                    None => false,
                },
                Resolved::ExtensionMember { extension, .. } => {
                    let d = program.extension(*extension).decl;
                    let a = &program.unit(d.unit).ast;
                    marcada(program, interner, d.unit, &a.decl(d.decl).metadata) || biblioteca_marcada(program, interner, d.unit)
                }
                _ => onde.is_some_and(|w| biblioteca_marcada(program, interner, w)),
            }
        }
    }
}

/// O elemento é uma função ou um método (`FunctionElement`,
/// `MethodElement`; não getter, setter nem acessor de variável): o
/// identificador e a propriedade que o citam são tear-off.
fn e_tear_off(program: &Program, a: &ast::Ast, corpo: &UnitBodyTypes, e: ExprId, r: &Resolved) -> bool {
    let f = match r {
        Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => *f,
        Resolved::ExtensionMember { member, .. } => *member,
        // A função local é um `FunctionElement`.
        Resolved::Local(_) => return corpo.declaracao_local(e).and_then(|d| declaracao_local(a, d)).is_some_and(|d| d.funcao),
        _ => return false,
    };
    let g = program.function(f);
    g.variable.is_none() && matches!(g.kind, dartforge_elements::model::FunctionKind::Function | dartforge_elements::model::FunctionKind::Operator)
}

/// `_getSubExpressionsMarkedDoNotStore`: as subexpressões marcadas, com o
/// nome a relatar.
fn marcadas(program: &Program, interner: &Interner, u: UnitId, corpo: &UnitBodyTypes, e: ExprId, saida: &mut Vec<(Span, String)>) {
    let a = &program.unit(u).ast;
    let resolvido = |x: ExprId| corpo.get_resolved(x);
    let expr = a.expr(e);
    // (a expressão cujo elemento conta, o elemento, o nome)
    let achado: Option<(ExprId, &Resolved, String)> = match &expr.kind {
        ExprKind::Property { name, .. } => {
            resolvido(e).filter(|r| !e_tear_off(program, a, corpo, e, r)).map(|r| (e, r, interner.resolve(name.sym).to_string()))
        }
        ExprKind::Identifier(n) => {
            resolvido(e).filter(|r| !e_tear_off(program, a, corpo, e, r)).map(|r| (e, r, interner.resolve(n.sym).to_string()))
        }
        // `MethodInvocation`: `f(…)`, `o.m(…)`; a criação de instância sem
        // `new` (`C()`, `C.nome()`, `p.C()`) é `InstanceCreationExpression`
        // no analyzer e não conta.
        ExprKind::Call { target, .. } if !matches!(resolvido(e), Some(Resolved::Constructor(_))) => match &a.expr(*target).kind {
            ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                match resolvido(*target) {
                    Some(r) => Some((*target, r, interner.resolve(n.sym).to_string())),
                    None => resolvido(e).map(|r| (e, r, interner.resolve(n.sym).to_string())),
                }
            }
            _ => None,
        },
        ExprKind::Conditional { then, else_, .. } => {
            marcadas(program, interner, u, corpo, *else_, saida);
            marcadas(program, interner, u, corpo, *then, saida);
            None
        }
        ExprKind::Binary { left, right, .. } => {
            marcadas(program, interner, u, corpo, *left, saida);
            marcadas(program, interner, u, corpo, *right, saida);
            None
        }
        ExprKind::FunctionExpression(f) => {
            if let FunctionBody::Expression(x) = &a.function(*f).body {
                marcadas(program, interner, u, corpo, *x, saida);
            }
            None
        }
        _ => None,
    };
    if let Some((x, r, nome)) = achado
        && elemento_marcado(program, interner, u, corpo, x, r)
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
    // `inTestDir`: um diretório da lista no caminho.
    let em_teste = unidade
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .is_some_and(|d| d.components().any(|c| DIRETORIOS_DE_TESTE.iter().any(|t| c.as_os_str() == *t)));
    if em_teste {
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
        marcadas(program, interner, u, corpo, e, &mut achados);
        for (s, nome) in achados {
            out.push(Diagnostic::com_codigo(w::ASSIGNMENT_OF_DO_NOT_STORE, s, [nome.as_str()]));
        }
    }

    // `return_of_do_not_store`. As declarações de função (de topo e locais)
    // e de método: o trecho, o `displayName` e se ligam o
    // `_inDoNotStoreMember`.
    let nome_de = |g: &ast::Function| g.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default();
    let mut declaracoes: Vec<(Span, String, bool)> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Function(f) = &d.kind {
            let g = a.function(*f);
            declaracoes.push((g.span, nome_de(g), marcada(program, interner, u, &d.metadata)));
        }
    }
    for (i, s) in a.stmts.iter().enumerate() {
        if let StmtKind::Function(f) = &s.kind {
            let g = a.function(*f);
            declaracoes.push((g.span, nome_de(g), marcada(program, interner, u, metadata_da_instrucao(a, StmtId(i as u32)))));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Method(f) = &m.kind {
            let g = a.function(*f);
            declaracoes.push((g.span, nome_de(g), marcada(program, interner, u, &m.metadata)));
        }
    }
    // Os trechos com `_inDoNotStoreMember` ligado: as declarações marcadas e
    // as classes marcadas (só `visitClassDeclaration` o liga).
    let mut marcados: Vec<Span> = declaracoes.iter().filter(|x| x.2).map(|x| x.0).collect();
    for d in a.decls.iter() {
        if matches!(d.kind, DeclKind::Class(_)) && marcada(program, interner, u, &d.metadata) {
            marcados.push(d.span);
        }
    }
    if biblioteca_marcada(program, interner, u) {
        return out;
    }
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
    for e in devolvidas {
        let sp = a.expr(e).span;
        if marcados.iter().any(|&m| dentro(sp, m)) {
            continue;
        }
        let mut achados = Vec::new();
        marcadas(program, interner, u, corpo, e, &mut achados);
        if achados.is_empty() {
            continue;
        }
        // A declaração de função ou método mais próxima que contém o retorno.
        let Some(dono) = declaracoes.iter().filter(|x| dentro(sp, x.0)).min_by_key(|x| x.0.end - x.0.start) else { continue };
        for (s, nome) in achados {
            out.push(Diagnostic::com_codigo(w::RETURN_OF_DO_NOT_STORE, s, [nome.as_str(), dono.1.as_str()]));
        }
    }
    out
}
