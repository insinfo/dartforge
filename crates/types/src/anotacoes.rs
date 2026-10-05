//! O elemento a que uma anotação se refere pelo nome, pelos escopos com que
//! o analyzer 3.6.2 a resolve (`analyzer/lib/src/dart/element/scope.dart`):
//! os locais declarados antes dela no bloco que os contém, os parâmetros e
//! os parâmetros de tipo das funções que a contêm (`FormalParameterScope`,
//! `TypeParameterScope`), os membros declarados da classe, mixin, enum, tipo
//! de extensão ou extensão que a contém (`InstanceScope`: os acessores e os
//! métodos dela, não os herdados) com os parâmetros de tipo dela, e por fim
//! a unidade e a biblioteca (com o prefixo de importação).
//!
//! Um nome achado num escopo local ou de membro não é o elemento de topo de
//! mesmo nome: a anotação não é a constante procurada.
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ForInTarget, ForInit, FunctionBody, Initializer, MemberKind, PatternId, PatternKind, StmtKind};
use dartforge_intern::{Interner, SymbolId};

/// Os nomes declarados por um padrão (as variáveis dele).
fn variaveis_do_padrao(a: &ast::Ast, p: PatternId, saida: &mut Vec<ast::Name>) {
    match &a.pattern(p).kind {
        PatternKind::Variable { name, .. } => saida.push(*name),
        PatternKind::Or(l, r) | PatternKind::And(l, r) => {
            variaveis_do_padrao(a, *l, saida);
            variaveis_do_padrao(a, *r, saida);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => {
            variaveis_do_padrao(a, *x, saida)
        }
        PatternKind::List { elements, .. } => {
            for e in elements.iter() {
                match e {
                    ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => variaveis_do_padrao(a, *x, saida),
                    ast::ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for e in entries.iter() {
                variaveis_do_padrao(a, e.value, saida);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for f in fields.iter() {
                variaveis_do_padrao(a, f.pattern, saida);
            }
        }
        _ => {}
    }
}

/// O nome `n` está declarado num escopo local ou de membro que contém a
/// posição `pos` da unidade.
fn sombreado(a: &ast::Ast, pos: usize, n: SymbolId) -> bool {
    let em = |s: Span| s.start <= pos && pos < s.end;
    // O menor bloco que contém o trecho `s` (o escopo de uma declaração
    // local); sem bloco, o próprio trecho.
    let bloco_de = |s: Span| {
        a.stmts
            .iter()
            .filter(|x| matches!(x.kind, StmtKind::Block(_)) && x.span.start < s.start && s.end <= x.span.end)
            .map(|x| x.span)
            .min_by_key(|x| x.end - x.start)
            .unwrap_or(s)
    };
    // Os locais declarados antes de `pos`, no bloco que os contém.
    for s in a.stmts.iter() {
        let mut nomes: Vec<ast::Name> = Vec::new();
        let escopo = match &s.kind {
            StmtKind::Variables(l) => {
                nomes.extend(l.variables.iter().map(|v| v.name));
                bloco_de(s.span)
            }
            StmtKind::Function(f) => {
                nomes.extend(a.function(*f).name);
                bloco_de(s.span)
            }
            StmtKind::PatternVariables { pattern, .. } => {
                variaveis_do_padrao(a, *pattern, &mut nomes);
                bloco_de(s.span)
            }
            // As variáveis do laço valem no próprio laço.
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                nomes.extend(l.variables.iter().map(|v| v.name));
                s.span
            }
            StmtKind::For { init: Some(ForInit::Pattern { pattern, .. }), .. } => {
                variaveis_do_padrao(a, *pattern, &mut nomes);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => {
                nomes.push(*name);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Pattern { pattern, .. }, .. } => {
                variaveis_do_padrao(a, *pattern, &mut nomes);
                s.span
            }
            StmtKind::Try { catches, .. } => {
                for c in catches.iter() {
                    let corpo = a.stmt(c.body).span;
                    if em(corpo) && c.exception.iter().chain(c.stack_trace.iter()).any(|x| x.sym == n) {
                        return true;
                    }
                }
                continue;
            }
            _ => continue,
        };
        if em(escopo) && nomes.iter().any(|x| x.sym == n && x.span.start < pos) {
            return true;
        }
    }
    // Os parâmetros e os parâmetros de tipo das funções cujo corpo contém
    // `pos`.
    let parametro = |ps: &[ast::Parameter]| ps.iter().any(|p| !p.this_ && !p.super_ && p.name.is_some_and(|x| x.sym == n));
    for f in a.functions.iter() {
        let corpo = match &f.body {
            FunctionBody::Block(s) => a.stmt(*s).span,
            FunctionBody::Expression(e) => a.expr(*e).span,
            _ => continue,
        };
        if em(corpo) && (f.parameters.as_deref().is_some_and(parametro) || f.type_params.iter().any(|t| t.name.sym == n)) {
            return true;
        }
    }
    for m in a.members.iter() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let mut regioes: Vec<Span> = k
            .initializers
            .iter()
            .map(|i| match i {
                Initializer::Field { span, .. } | Initializer::Super { span, .. } | Initializer::Redirect { span, .. } | Initializer::Assert { span, .. } => *span,
            })
            .collect();
        match &k.body {
            FunctionBody::Block(s) => regioes.push(a.stmt(*s).span),
            FunctionBody::Expression(e) => regioes.push(a.expr(*e).span),
            _ => {}
        }
        if regioes.iter().any(|r| em(*r)) && parametro(&k.parameters) {
            return true;
        }
    }
    // Os membros declarados (e os parâmetros de tipo) do tipo que contém
    // `pos` depois do nome dele.
    for d in a.decls.iter() {
        let (nome, tipos, membros, extras): (Option<ast::Name>, &[ast::TypeParameter], &[ast::MemberId], Vec<SymbolId>) = match &d.kind {
            DeclKind::Class(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::Mixin(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::Enum(x) => (Some(x.name), &x.type_params[..], &x.members[..], x.constants.iter().map(|c| c.name.sym).collect()),
            DeclKind::Extension(x) => (x.name, &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::ExtensionType(x) => (Some(x.name), &x.type_params[..], &x.members[..], vec![x.representation_name.sym]),
            _ => continue,
        };
        let depois_do_nome = nome.map_or(d.span.start, |x| x.span.end);
        if !(pos > depois_do_nome && pos < d.span.end) {
            continue;
        }
        if tipos.iter().any(|t| t.name.sym == n) || extras.contains(&n) {
            return true;
        }
        for &mid in membros {
            let declara = match &a.member(mid).kind {
                MemberKind::Method(f) => a.function(*f).name.is_some_and(|x| x.sym == n),
                MemberKind::Field(l) => l.variables.iter().any(|v| v.name.sym == n),
                MemberKind::Constructor(_) => false,
            };
            if declara {
                return true;
            }
        }
    }
    false
}

/// O elemento de topo da anotação `m`, escrita na unidade `u`: o nome
/// simples, o prefixado (`p.x`, `p.C.ctor`) ou o de classe (`C.ctor`, `C.x`,
/// que devolve a classe). `None` quando o nome é de um escopo local ou de
/// membro, ou não resolve.
pub fn elemento_da_anotacao(program: &Program, u: UnitId, m: &ast::Annotation) -> Option<Element> {
    let a = &program.unit(u).ast;
    let pos = m.span.start;
    let primeiro = m.name.first()?;
    if sombreado(a, pos, primeiro.sym) {
        return None;
    }
    let no_escopo = program.lookup_na_unidade(u, primeiro.sym).and_then(|b| b.getter);
    match &m.name[..] {
        [_] => no_escopo,
        // `p.x` com `p` prefixo de importação, ou `C.x`.
        [p, x, ..] => match no_escopo {
            Some(e) => Some(e),
            None => program.lookup_prefixed_na_unidade(u, p.sym, x.sym).and_then(|b| b.getter),
        },
        [] => None,
    }
}

/// A anotação `m` (escrita na unidade `u`), sem argumentos, é a variável de
/// topo `nome` de uma biblioteca cujo URI satisfaz `da_biblioteca`.
pub fn anotacao_e_variavel(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, nome: &str, da_biblioteca: &dyn Fn(&str) -> bool) -> bool {
    if m.arguments.is_some() || m.name.last().is_none_or(|n| interner.resolve(n.sym) != nome) {
        return false;
    }
    match elemento_da_anotacao(program, u, m) {
        Some(Element::Variable(v)) => {
            let x = program.variable(v);
            interner.resolve(x.name) == nome && da_biblioteca(&program.library(x.library).uri)
        }
        _ => false,
    }
}

/// `ElementAnnotation.isDeprecated` sem argumentos: a variável
/// `deprecated` do `dart:core`.
pub fn e_deprecated_do_core(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> bool {
    anotacao_e_variavel(program, interner, u, m, "deprecated", &|uri| uri == "dart:core")
}
