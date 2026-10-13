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
use dartforge_frontend::ast::{self, DeclKind, ExprKind, ForInTarget, ForInit, FunctionBody, Initializer, MemberKind, PatternId, PatternKind, StmtKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};

/// Os nomes declarados por um padrão (as variáveis dele).
fn variaveis_do_padrao(a: &ast::Ast, p: PatternId, output: &mut Vec<ast::Name>) {
    match &a.pattern(p).kind {
        PatternKind::Variable { name, .. } => output.push(*name),
        PatternKind::Or(l, r) | PatternKind::And(l, r) => {
            variaveis_do_padrao(a, *l, output);
            variaveis_do_padrao(a, *r, output);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => {
            variaveis_do_padrao(a, *x, output)
        }
        PatternKind::List { elements, .. } => {
            for e in elements.iter() {
                match e {
                    ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => variaveis_do_padrao(a, *x, output),
                    ast::ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for e in entries.iter() {
                variaveis_do_padrao(a, e.value, output);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for f in fields.iter() {
                variaveis_do_padrao(a, f.pattern, output);
            }
        }
        _ => {}
    }
}

/// Os nomes das variáveis que um padrão declara. Num padrão refutável
/// (`case`, `if-case`), o nome solto (sem `var`, `final` nem tipo) é
/// constante, não variável.
fn declared_in_pattern(a: &ast::Ast, p: PatternId, refutable: bool, output: &mut Vec<ast::Name>) {
    let mut all: Vec<ast::Name> = Vec::new();
    variaveis_do_padrao(a, p, &mut all);
    if !refutable {
        output.extend(all);
        return;
    }
    // As variáveis com palavra-chave ou tipo.
    let mut stack = vec![p];
    while let Some(q) = stack.pop() {
        match &a.pattern(q).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                if *final_ || *var_ || ty.is_some() {
                    output.push(*name);
                }
            }
            PatternKind::Or(l, r) | PatternKind::And(l, r) => stack.extend([*l, *r]),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => stack.push(*x),
            PatternKind::List { elements, .. } => stack.extend(elements.iter().filter_map(|e| match e {
                ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => Some(*x),
                ast::ListPatternElement::Rest(None) => None,
            })),
            PatternKind::Map { entries, .. } => stack.extend(entries.iter().map(|e| e.value)),
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => stack.extend(fields.iter().map(|f| f.pattern)),
            _ => {}
        }
    }
}

/// O trecho de um elemento de coleção (das expressões dele).
fn element_snippet(a: &ast::Ast, el: &ast::CollectionElement) -> Option<Span> {
    use ast::CollectionElement as E;
    let joins_with = |x: Option<Span>, y: Option<Span>| match (x, y) {
        (Some(p), Some(q)) => Some(Span { start: p.start.min(q.start), end: p.end.max(q.end) }),
        (p, q) => p.or(q),
    };
    match el {
        E::Expression(e) | E::NullAwareExpression(e) | E::Spread { value: e, .. } => Some(a.expr(*e).span),
        E::MapEntry { key, value, .. } => joins_with(Some(a.expr(*key).span), Some(a.expr(*value).span)),
        E::If { condition, then, else_, .. } => {
            let mut s = joins_with(Some(a.expr(*condition).span), element_snippet(a, then));
            if let Some(x) = else_ {
                s = joins_with(s, element_snippet(a, x));
            }
            s
        }
        E::For { body, .. } | E::ForIn { body, .. } => element_snippet(a, body),
    }
}

/// Os escopos locais dos elementos de coleção: (trecho, nomes declarados).
fn collection_scopes(a: &ast::Ast, el: &ast::CollectionElement, output: &mut Vec<(Span, Vec<ast::Name>)>) {
    use ast::CollectionElement as E;
    match el {
        E::If { case_pattern, then, else_, .. } => {
            if let Some(p) = case_pattern
                && let Some(t) = element_snippet(a, then)
            {
                let mut names = Vec::new();
                declared_in_pattern(a, *p, true, &mut names);
                output.push((Span { start: a.pattern(*p).span.start, end: t.end }, names));
            }
            collection_scopes(a, then, output);
            if let Some(x) = else_ {
                collection_scopes(a, x, output);
            }
        }
        E::For { init, condition, updates, body, .. } => {
            let mut names = Vec::new();
            let mut start: Option<usize> = None;
            match init {
                Some(ForInit::Variables(l)) => {
                    names.extend(l.variables.iter().map(|v| v.name));
                    start = l.variables.first().map(|v| v.name.span.start);
                }
                Some(ForInit::Pattern { pattern, .. }) => {
                    declared_in_pattern(a, *pattern, false, &mut names);
                    start = Some(a.pattern(*pattern).span.start);
                }
                _ => {}
            }
            let end = element_snippet(a, body).map(|s| s.end).or(condition.map(|c| a.expr(c).span.end)).or(updates.last().map(|u| a.expr(*u).span.end));
            if let (Some(i), Some(f)) = (start, end) {
                output.push((Span { start: i, end: f }, names));
            }
            collection_scopes(a, body, output);
        }
        E::ForIn { target, body, .. } => {
            let mut names = Vec::new();
            match target {
                ForInTarget::Declared { name, .. } => names.push(*name),
                ForInTarget::Pattern { pattern, .. } => declared_in_pattern(a, *pattern, false, &mut names),
                ForInTarget::Expression(_) => {}
            }
            if let Some(t) = element_snippet(a, body) {
                output.push((t, names));
            }
            collection_scopes(a, body, output);
        }
        _ => {}
    }
}

/// O nome `n` está declarado num escopo local ou de membro que contém a
/// posição `pos` da unidade (a busca léxica do analyzer pararia antes do
/// escopo da biblioteca).
pub fn sombreado(a: &ast::Ast, interner: &Interner, pos: usize, n: SymbolId) -> bool {
    let in_ = |s: Span| s.start <= pos && pos < s.end;
    let declares = |names: &[ast::Name]| names.iter().any(|x| x.sym == n && x.span.start < pos);
    // O menor bloco que contém o trecho `s` (o escopo de uma declaração
    // local); sem bloco, o próprio trecho.
    let block_of = |s: Span| {
        a.stmts
            .iter()
            .filter(|x| matches!(x.kind, StmtKind::Block(_)) && x.span.start < s.start && s.end <= x.span.end)
            .map(|x| x.span)
            .min_by_key(|x| x.end - x.start)
            .unwrap_or(s)
    };
    // Os locais declarados antes de `pos`, no escopo que os contém.
    for s in a.stmts.iter() {
        let mut names: Vec<ast::Name> = Vec::new();
        // O local de bloco vale no bloco inteiro, também antes da declaração
        // (o uso antes dela é `REFERENCED_BEFORE_DECLARATION`, e um prefixo
        // de mesmo nome fica sombreado: `PREFIX_SHADOWED_BY_LOCAL_DECLARATION`).
        let mut in_whole_block = false;
        let scope = match &s.kind {
            StmtKind::Variables(l) => {
                names.extend(l.variables.iter().map(|v| v.name));
                in_whole_block = true;
                block_of(s.span)
            }
            StmtKind::Function(f) => {
                names.extend(a.function(*f).name);
                in_whole_block = true;
                block_of(s.span)
            }
            StmtKind::PatternVariables { pattern, .. } => {
                declared_in_pattern(a, *pattern, false, &mut names);
                in_whole_block = true;
                block_of(s.span)
            }
            // As variáveis do laço valem no próprio laço.
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                names.extend(l.variables.iter().map(|v| v.name));
                s.span
            }
            StmtKind::For { init: Some(ForInit::Pattern { pattern, .. }), .. } => {
                declared_in_pattern(a, *pattern, false, &mut names);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => {
                names.push(*name);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Pattern { pattern, .. }, .. } => {
                declared_in_pattern(a, *pattern, false, &mut names);
                s.span
            }
            // As variáveis do `if-case` valem na guarda e no `then`.
            StmtKind::If { case_pattern: Some(p), then, .. } => {
                declared_in_pattern(a, *p, true, &mut names);
                Span { start: a.pattern(*p).span.start, end: a.stmt(*then).span.end }
            }
            // As de cada caso valem na guarda e no corpo do caso.
            StmtKind::Switch { cases, .. } => {
                for c in cases.iter() {
                    let Some(p) = c.pattern else { continue };
                    let mut vs = Vec::new();
                    declared_in_pattern(a, p, true, &mut vs);
                    if in_(c.span) && declares(&vs) {
                        return true;
                    }
                }
                continue;
            }
            StmtKind::Try { catches, .. } => {
                for c in catches.iter() {
                    let the_body = a.stmt(c.body).span;
                    if in_(the_body) && c.exception.iter().chain(c.stack_trace.iter()).any(|x| x.sym == n) {
                        return true;
                    }
                }
                continue;
            }
            _ => continue,
        };
        if in_(scope) && (if in_whole_block { names.iter().any(|x| x.sym == n) } else { declares(&names) }) {
            return true;
        }
    }
    // Os casos de `switch` de expressão e os elementos de coleção.
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Switch { cases, .. } => {
                for c in cases.iter() {
                    let mut vs = Vec::new();
                    declared_in_pattern(a, c.pattern, true, &mut vs);
                    if in_(c.span) && declares(&vs) {
                        return true;
                    }
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                if !in_(e.span) {
                    continue;
                }
                let mut scopes = Vec::new();
                for el in elements.iter() {
                    collection_scopes(a, el, &mut scopes);
                }
                if scopes.iter().any(|(s, names)| in_(*s) && declares(names)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    // Os parâmetros (no corpo) e os parâmetros de tipo (em toda a função)
    // das funções que contêm `pos`.
    let common = |ps: &[ast::Parameter]| ps.iter().any(|p| !p.this_ && !p.super_ && p.name.is_some_and(|x| x.sym == n));
    for f in a.functions.iter() {
        if in_(f.span) && f.type_params.iter().any(|t| t.name.sym == n) {
            return true;
        }
        let the_body = match &f.body {
            FunctionBody::Block(s) => a.stmt(*s).span,
            FunctionBody::Expression(e) => a.expr(*e).span,
            _ => continue,
        };
        if in_(the_body) && f.parameters.as_deref().is_some_and(common) {
            return true;
        }
    }
    // O construtor: na lista de inicializadores valem todos os parâmetros
    // (os `this.x` e `super.x` também); no corpo, os comuns.
    for m in a.members.iter() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let in_initializers = k.initializers.iter().any(|i| {
            in_(match i {
                Initializer::Field { span, .. } | Initializer::Super { span, .. } | Initializer::Redirect { span, .. } | Initializer::Assert { span, .. } => *span,
            })
        });
        if in_initializers && k.parameters.iter().any(|p| p.name.is_some_and(|x| x.sym == n)) {
            return true;
        }
        let the_body = match &k.body {
            FunctionBody::Block(s) => Some(a.stmt(*s).span),
            FunctionBody::Expression(e) => Some(a.expr(*e).span),
            _ => None,
        };
        if the_body.is_some_and(in_) && common(&k.parameters) {
            return true;
        }
    }
    // Os parâmetros de tipo de um tipo `Function<T>(…)` e de um
    // parâmetro-função `f<T>(…)`.
    fn parameter_types(ps: &[ast::Parameter], pos: usize, n: SymbolId) -> bool {
        ps.iter().any(|p| {
            (p.span.start <= pos && pos < p.span.end && p.function_type_params.iter().any(|t| t.name.sym == n))
                || p.function_parameters.as_deref().is_some_and(|fs| parameter_types(fs, pos, n))
        })
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind
            && ((in_(t.span) && type_params.iter().any(|x| x.name.sym == n)) || parameter_types(parameters, pos, n))
        {
            return true;
        }
    }
    for f in a.functions.iter() {
        if f.parameters.as_deref().is_some_and(|ps| parameter_types(ps, pos, n)) {
            return true;
        }
    }
    // Os membros declarados (e os parâmetros de tipo) do tipo que contém
    // `pos` depois do nome dele; os parâmetros de tipo do typedef.
    let values = interner.lookup("values");
    for d in a.decls.iter() {
        let (the_name, types, members, extras): (Option<ast::Name>, &[ast::TypeParameter], &[ast::MemberId], Vec<SymbolId>) = match &d.kind {
            DeclKind::Class(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::Mixin(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            // O `values` sintético do enum também está no escopo.
            DeclKind::Enum(x) => (Some(x.name), &x.type_params[..], &x.members[..], x.constants.iter().map(|c| c.name.sym).chain(values).collect()),
            DeclKind::Extension(x) => (x.name, &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::ExtensionType(x) => (Some(x.name), &x.type_params[..], &x.members[..], vec![x.representation_name.sym]),
            DeclKind::Typedef(x) => {
                if in_(d.span) && x.type_params.iter().any(|t| t.name.sym == n) {
                    return true;
                }
                continue;
            }
            _ => continue,
        };
        let after_name = the_name.map_or(d.span.start, |x| x.span.end);
        if !(pos > after_name && pos < d.span.end) {
            continue;
        }
        if types.iter().any(|t| t.name.sym == n) || extras.contains(&n) {
            return true;
        }
        for &mid in members {
            let declares = match &a.member(mid).kind {
                MemberKind::Method(f) => a.function(*f).name.is_some_and(|x| x.sym == n),
                MemberKind::Field(l) => l.variables.iter().any(|v| v.name.sym == n),
                MemberKind::Constructor(_) => false,
            };
            if declares {
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
pub fn annotation_element(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<Element> {
    let a = &program.unit(u).ast;
    let pos = m.span.start;
    let first = m.name.first()?;
    if sombreado(a, interner, pos, first.sym) {
        return None;
    }
    let in_scope = program.lookup_na_unidade(u, first.sym).and_then(|b| b.getter);
    match &m.name[..] {
        [_] => in_scope,
        // `p.x` com `p` prefixo de importação, ou `C.x`.
        [p, x, ..] => match in_scope {
            Some(e) => Some(e),
            None => program.lookup_prefixed_na_unidade(u, p.sym, x.sym).and_then(|b| b.getter),
        },
        [] => None,
    }
}

/// `ElementAnnotation.element`: o construtor invocado, a classe citada sem
/// argumentos, ou o getter lido (o da variável de topo ou estática, ou o
/// explícito).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvokedElement {
    Construtor(dartforge_elements::model::ClassId, dartforge_elements::model::FunctionElementId),
    Classe(dartforge_elements::model::ClassId),
    Getter(dartforge_elements::model::FunctionElementId),
    Outro,
}

/// O [`ElementoInvocado`] da anotação `m`, escrita na unidade `u`.
pub fn invoked_element(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> InvokedElement {
    use dartforge_elements::model::FunctionKind;
    let a = &program.unit(u).ast;
    let Some(first) = m.name.first() else { return InvokedElement::Outro };
    if sombreado(a, interner, m.span.start, first.sym) {
        return InvokedElement::Outro;
    }
    let (tgt, member) = match &m.name[..] {
        [c] => (program.lookup_na_unidade(u, c.sym).and_then(|b| b.getter), None),
        [x, y] => match program.lookup_na_unidade(u, x.sym).and_then(|b| b.getter) {
            Some(el @ Element::Class(_)) => (Some(el), Some(y.sym)),
            _ => (program.lookup_prefixed_na_unidade(u, x.sym, y.sym).and_then(|b| b.getter), None),
        },
        [x, c, n] => (program.lookup_prefixed_na_unidade(u, x.sym, c.sym).and_then(|b| b.getter), Some(n.sym)),
        _ => (None, None),
    };
    let getter = |f: dartforge_elements::model::FunctionElementId| {
        let e = program.function(f);
        let e_getter = match e.kind {
            FunctionKind::Getter => true,
            FunctionKind::ImplicitAccessor => e.variable.is_some_and(|v| program.variable(v).getter == Some(f)),
            _ => false,
        };
        if e_getter { InvokedElement::Getter(f) } else { InvokedElement::Outro }
    };
    match (tgt, m.arguments.is_some(), member) {
        (Some(Element::Class(c)), true, _) => {
            let the_key = member.or_else(|| interner.lookup(""));
            match the_key.and_then(|k| program.class(c).constructors.get(&k)) {
                Some(&f) => InvokedElement::Construtor(c, f),
                None => InvokedElement::Outro,
            }
        }
        (Some(Element::Class(c)), false, None) => InvokedElement::Classe(c),
        (Some(Element::Class(c)), false, Some(n)) => match program.class(c).static_members.get(&n) {
            Some(&f) => getter(f),
            None => InvokedElement::Outro,
        },
        (Some(Element::Variable(v)), _, None) => match program.variable(v).getter {
            Some(f) => InvokedElement::Getter(f),
            None => InvokedElement::Outro,
        },
        (Some(Element::Function(g)), _, None) => getter(g),
        _ => InvokedElement::Outro,
    }
}

/// O nome da biblioteca (`library a.b;`), como o `LibraryElement.name`.
pub fn nome_da_biblioteca(program: &Program, interner: &Interner, l: dartforge_elements::model::LibraryId) -> String {
    program.library(l).name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
}

/// `_isTopGetter`: a anotação lê o getter `nome` de uma biblioteca chamada
/// `biblioteca` (`_isPackageMetaGetter` com `meta`).
pub fn e_getter_de(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, library: &str, the_name: &str) -> bool {
    match invoked_element(program, interner, u, m) {
        InvokedElement::Getter(f) => {
            let e = program.function(f);
            interner.resolve(e.name) == the_name && nome_da_biblioteca(program, interner, e.library) == library
        }
        _ => false,
    }
}

/// `_isConstructor`: a anotação invoca um construtor da classe `classe` de
/// uma biblioteca chamada `biblioteca`.
pub fn e_construtor_de(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, library: &str, class: &str) -> bool {
    match invoked_element(program, interner, u, m) {
        InvokedElement::Construtor(c, _) => {
            let k = program.class(c);
            interner.resolve(k.name) == class && nome_da_biblioteca(program, interner, k.library) == library
        }
        _ => false,
    }
}

/// A anotação `m` (escrita na unidade `u`), sem argumentos, é a variável de
/// topo `nome` de uma biblioteca cujo URI satisfaz `da_biblioteca`.
pub fn annotation_and_variable(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, the_name: &str, of_library: &dyn Fn(&str) -> bool) -> bool {
    if m.arguments.is_some() || m.name.last().is_none_or(|n| interner.resolve(n.sym) != the_name) {
        return false;
    }
    match annotation_element(program, interner, u, m) {
        Some(Element::Variable(v)) => {
            let x = program.variable(v);
            interner.resolve(x.name) == the_name && of_library(&program.library(x.library).uri)
        }
        _ => false,
    }
}

/// `ElementAnnotation.isDeprecated`: a variável `deprecated` do `dart:core`
/// (sem argumentos), ou um construtor da classe `Deprecated` do `dart:core`
/// (`@Deprecated('…')`, com argumentos).
pub fn e_deprecated(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> bool {
    match annotation_element(program, interner, u, m) {
        Some(Element::Variable(v)) => {
            let x = program.variable(v);
            m.arguments.is_none() && interner.resolve(x.name) == "deprecated" && program.library(x.library).uri == "dart:core"
        }
        Some(Element::Class(c)) => {
            let x = program.class(c);
            m.arguments.is_some() && interner.resolve(x.name) == "Deprecated" && program.library(x.library).uri == "dart:core"
        }
        _ => false,
    }
}

/// `ElementAnnotation.isDeprecated` sem argumentos: a variável
/// `deprecated` do `dart:core`.
pub fn e_deprecated_do_core(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> bool {
    annotation_and_variable(program, interner, u, m, "deprecated", &|uri| uri == "dart:core")
}
