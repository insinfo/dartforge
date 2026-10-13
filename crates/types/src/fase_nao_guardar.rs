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
const TEST_DIRECTORIES: [&str; 4] = ["test", "integration_test", "test_driver", "testing"];

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// A lista, escrita na unidade `unit`, tem o `@doNotStore` do `package:meta`.
fn the_marked(program: &Program, interner: &Interner, unit: UnitId, metadata: &[ast::Annotation]) -> bool {
    metadata.iter().any(|m| crate::fase_resultado::meta_annotation(program, interner, unit, m, "doNotStore"))
}

/// `LibraryElement.hasDoNotStore` da biblioteca da unidade `u`: a metadata
/// da diretiva `library` da unidade que define a biblioteca ou, sem ela, a
/// da primeira diretiva.
fn marked_library(program: &Program, interner: &Interner, u: UnitId) -> bool {
    let lib = program.library(program.unit(u).library);
    lib.units.first().is_some_and(|&d| {
        let ds = &program.unit(d).unit.directives;
        let tgt = ds.iter().find(|x| matches!(x.kind, DirectiveKind::Library { .. })).or_else(|| ds.first());
        tgt.is_some_and(|x| the_marked(program, interner, d, &x.metadata))
    })
}

/// O que a declaração de um local é: a metadata e se é uma função local.
struct LocalDeclaration<'a> {
    metadata: &'a [ast::Annotation],
    funcao: bool,
}

/// A metadata de uma instrução (`Ast::metadados_locais`).
fn statement_metadata(a: &ast::Ast, s: StmtId) -> &[ast::Annotation] {
    a.metadados_locais.iter().find(|(x, _)| *x == s).map(|(_, m)| &**m).unwrap_or(&[])
}

/// O parâmetro de `ps` declarado no deslocamento `decl`.
fn parametro_em(ps: &[ast::Parameter], decl: usize) -> Option<&ast::Parameter> {
    ps.iter().find(|p| p.name.is_some_and(|n| n.span.start == decl))
}

/// A declaração do local declarado no deslocamento `decl`: parâmetro,
/// variável local, função local ou variável do `for-in`.
fn declaracao_local(a: &ast::Ast, decl: usize) -> Option<LocalDeclaration<'_>> {
    for f in a.functions.iter() {
        if let Some(p) = f.parameters.as_deref().and_then(|ps| parametro_em(ps, decl)) {
            return Some(LocalDeclaration { metadata: &p.metadata, funcao: false });
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let Some(p) = parametro_em(&k.parameters, decl)
        {
            return Some(LocalDeclaration { metadata: &p.metadata, funcao: false });
        }
    }
    for (i, s) in a.stmts.iter().enumerate() {
        let id = StmtId(i as u32);
        match &s.kind {
            StmtKind::Variables(l) if l.variables.iter().any(|v| v.name.span.start == decl) => {
                return Some(LocalDeclaration { metadata: statement_metadata(a, id), funcao: false });
            }
            StmtKind::Function(f) if a.function(*f).name.is_some_and(|n| n.span.start == decl) => {
                return Some(LocalDeclaration { metadata: statement_metadata(a, id), funcao: true });
            }
            StmtKind::ForIn { target: ForInTarget::Declared { metadata, name, .. }, .. } if name.span.start == decl => {
                return Some(LocalDeclaration { metadata, funcao: false });
            }
            _ => {}
        }
    }
    None
}

/// O parâmetro `nome` da função ou do construtor mais de dentro que contém
/// `e` e o declara (`Resolved::Parameter`).
fn parameter_declaration(a: &ast::Ast, e: Span, the_name: dartforge_intern::SymbolId) -> Option<LocalDeclaration<'_>> {
    let mut best: Option<(usize, &ast::Parameter)> = None;
    let mut lists: Vec<(Span, &[ast::Parameter])> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            lists.push((f.span, ps));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            lists.push((m.span, &k.parameters[..]));
        }
    }
    for (sp, ps) in lists {
        if !dentro(e, sp) {
            continue;
        }
        if let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == the_name)) {
            let size = sp.end - sp.start;
            if best.is_none_or(|(t, _)| size < t) {
                best = Some((size, p));
            }
        }
    }
    best.map(|(_, p)| LocalDeclaration { metadata: &p.metadata, funcao: false })
}

/// `hasOrInheritsDoNotStore` do elemento resolvido `r` da expressão `e`
/// (`u` é a unidade da expressão, para os locais).
fn marked_element(program: &Program, interner: &Interner, u: UnitId, body: &UnitBodyTypes, e: ExprId, r: &Resolved) -> bool {
    let a = &program.unit(u).ast;
    match r {
        // O elemento local: só a própria metadata (o que o contém é uma
        // função, nem classe nem unidade).
        Resolved::Local(_) => body
            .declaracao_local(e)
            .and_then(|d| declaracao_local(a, d))
            .is_some_and(|d| the_marked(program, interner, u, d.metadata)),
        Resolved::Parameter { name, .. } => {
            parameter_declaration(a, a.expr(e).span, *name).is_some_and(|d| the_marked(program, interner, u, d.metadata))
        }
        _ => {
            let (metadata, where_) = crate::fase_resultado::element_annotations(program, r);
            if where_.is_some_and(|w| the_marked(program, interner, w, metadata)) {
                return true;
            }
            // A classe ou a extensão que o contém; depois, a biblioteca.
            match r {
                Resolved::Member { class, .. } => match program.class(*class).decl {
                    Some(d) => {
                        let a = &program.unit(d.unit).ast;
                        the_marked(program, interner, d.unit, &a.decl(d.decl).metadata) || marked_library(program, interner, d.unit)
                    }
                    None => false,
                },
                Resolved::ExtensionMember { extension, .. } => {
                    let d = program.extension(*extension).decl;
                    let a = &program.unit(d.unit).ast;
                    the_marked(program, interner, d.unit, &a.decl(d.decl).metadata) || marked_library(program, interner, d.unit)
                }
                _ => where_.is_some_and(|w| marked_library(program, interner, w)),
            }
        }
    }
}

/// O elemento é uma função ou um método (`FunctionElement`,
/// `MethodElement`; não getter, setter nem acessor de variável): o
/// identificador e a propriedade que o citam são tear-off.
fn e_tear_off(program: &Program, a: &ast::Ast, body: &UnitBodyTypes, e: ExprId, r: &Resolved) -> bool {
    let f = match r {
        Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => *f,
        Resolved::ExtensionMember { member, .. } => *member,
        // A função local é um `FunctionElement`.
        Resolved::Local(_) => return body.declaracao_local(e).and_then(|d| declaracao_local(a, d)).is_some_and(|d| d.funcao),
        _ => return false,
    };
    let g = program.function(f);
    g.variable.is_none() && matches!(g.kind, dartforge_elements::model::FunctionKind::Function | dartforge_elements::model::FunctionKind::Operator)
}

/// `_getSubExpressionsMarkedDoNotStore`: as subexpressões marcadas, com o
/// nome a relatar.
fn marked_(program: &Program, interner: &Interner, u: UnitId, body: &UnitBodyTypes, e: ExprId, output: &mut Vec<(Span, String)>) {
    let a = &program.unit(u).ast;
    let resolved = |x: ExprId| body.get_resolved(x);
    let expr = a.expr(e);
    // (a expressão cujo elemento conta, o elemento, o nome)
    let finding: Option<(ExprId, &Resolved, String)> = match &expr.kind {
        ExprKind::Property { name, .. } => {
            resolved(e).filter(|r| !e_tear_off(program, a, body, e, r)).map(|r| (e, r, interner.resolve(name.sym).to_string()))
        }
        ExprKind::Identifier(n) => {
            resolved(e).filter(|r| !e_tear_off(program, a, body, e, r)).map(|r| (e, r, interner.resolve(n.sym).to_string()))
        }
        // `MethodInvocation`: `f(…)`, `o.m(…)`; a criação de instância sem
        // `new` (`C()`, `C.nome()`, `p.C()`) é `InstanceCreationExpression`
        // no analyzer e não conta.
        ExprKind::Call { target, .. } if !matches!(resolved(e), Some(Resolved::Constructor(_))) => match &a.expr(*target).kind {
            ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                match resolved(*target) {
                    Some(r) => Some((*target, r, interner.resolve(n.sym).to_string())),
                    None => resolved(e).map(|r| (e, r, interner.resolve(n.sym).to_string())),
                }
            }
            _ => None,
        },
        ExprKind::Conditional { then, else_, .. } => {
            marked_(program, interner, u, body, *else_, output);
            marked_(program, interner, u, body, *then, output);
            None
        }
        ExprKind::Binary { left, right, .. } => {
            marked_(program, interner, u, body, *left, output);
            marked_(program, interner, u, body, *right, output);
            None
        }
        ExprKind::FunctionExpression(f) => {
            if let FunctionBody::Expression(x) = &a.function(*f).body {
                marked_(program, interner, u, body, *x, output);
            }
            None
        }
        _ => None,
    };
    if let Some((x, r, the_name)) = finding
        && marked_element(program, interner, u, body, x, r)
    {
        output.push((expr.span, the_name));
    }
}

/// Os relatos da unidade `u`.
pub fn guardados_e_devolvidos(program: &Program, interner: &Interner, body: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if interner.lookup("doNotStore").is_none() {
        return out;
    }
    let unit_id = program.unit(u);
    // `inTestDir`: um diretório da lista no caminho.
    let in_test = unit_id
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .is_some_and(|d| d.components().any(|c| TEST_DIRECTORIES.iter().any(|t| c.as_os_str() == *t)));
    if in_test {
        return out;
    }
    let a = &unit_id.ast;

    // `assignment_of_do_not_store`: os inicializadores de topo e de campo.
    let mut initial: Vec<ExprId> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            initial.extend(l.variables.iter().filter_map(|v| v.initializer));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            initial.extend(l.variables.iter().filter_map(|v| v.initializer));
        }
    }
    for e in initial {
        let mut findings = Vec::new();
        marked_(program, interner, u, body, e, &mut findings);
        for (s, the_name) in findings {
            out.push(Diagnostic::com_codigo(w::ASSIGNMENT_OF_DO_NOT_STORE, s, [the_name.as_str()]));
        }
    }

    // `return_of_do_not_store`. As declarações de função (de topo e locais)
    // e de método: o trecho, o `displayName` e se ligam o
    // `_inDoNotStoreMember`.
    let name_of = |g: &ast::Function| g.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default();
    let mut declarations: Vec<(Span, String, bool)> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Function(f) = &d.kind {
            let g = a.function(*f);
            declarations.push((g.span, name_of(g), the_marked(program, interner, u, &d.metadata)));
        }
    }
    for (i, s) in a.stmts.iter().enumerate() {
        if let StmtKind::Function(f) = &s.kind {
            let g = a.function(*f);
            declarations.push((g.span, name_of(g), the_marked(program, interner, u, statement_metadata(a, StmtId(i as u32)))));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Method(f) = &m.kind {
            let g = a.function(*f);
            declarations.push((g.span, name_of(g), the_marked(program, interner, u, &m.metadata)));
        }
    }
    // Os trechos com `_inDoNotStoreMember` ligado: as declarações marcadas e
    // as classes marcadas (só `visitClassDeclaration` o liga).
    let mut marked: Vec<Span> = declarations.iter().filter(|x| x.2).map(|x| x.0).collect();
    for d in a.decls.iter() {
        if matches!(d.kind, DeclKind::Class(_)) && the_marked(program, interner, u, &d.metadata) {
            marked.push(d.span);
        }
    }
    if marked_library(program, interner, u) {
        return out;
    }
    let mut returned: Vec<ExprId> = Vec::new();
    for s in a.stmts.iter() {
        if let StmtKind::Return(Some(e)) = &s.kind {
            returned.push(*e);
        }
    }
    for f in a.functions.iter() {
        if let FunctionBody::Expression(e) = &f.body {
            returned.push(*e);
        }
    }
    for e in returned {
        let sp = a.expr(e).span;
        if marked.iter().any(|&m| dentro(sp, m)) {
            continue;
        }
        let mut findings = Vec::new();
        marked_(program, interner, u, body, e, &mut findings);
        if findings.is_empty() {
            continue;
        }
        // A declaração de função ou método mais próxima que contém o retorno.
        let Some(owner) = declarations.iter().filter(|x| dentro(sp, x.0)).min_by_key(|x| x.0.end - x.0.start) else { continue };
        for (s, the_name) in findings {
            out.push(Diagnostic::com_codigo(w::RETURN_OF_DO_NOT_STORE, s, [the_name.as_str(), owner.1.as_str()]));
        }
    }
    out
}
