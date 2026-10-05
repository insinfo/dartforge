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

/// Os nomes das variáveis que um padrão declara. Num padrão refutável
/// (`case`, `if-case`), o nome solto (sem `var`, `final` nem tipo) é
/// constante, não variável.
fn declaradas_no_padrao(a: &ast::Ast, p: PatternId, refutavel: bool, saida: &mut Vec<ast::Name>) {
    let mut todas: Vec<ast::Name> = Vec::new();
    variaveis_do_padrao(a, p, &mut todas);
    if !refutavel {
        saida.extend(todas);
        return;
    }
    // As variáveis com palavra-chave ou tipo.
    let mut pilha = vec![p];
    while let Some(q) = pilha.pop() {
        match &a.pattern(q).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                if *final_ || *var_ || ty.is_some() {
                    saida.push(*name);
                }
            }
            PatternKind::Or(l, r) | PatternKind::And(l, r) => pilha.extend([*l, *r]),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => pilha.push(*x),
            PatternKind::List { elements, .. } => pilha.extend(elements.iter().filter_map(|e| match e {
                ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => Some(*x),
                ast::ListPatternElement::Rest(None) => None,
            })),
            PatternKind::Map { entries, .. } => pilha.extend(entries.iter().map(|e| e.value)),
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => pilha.extend(fields.iter().map(|f| f.pattern)),
            _ => {}
        }
    }
}

/// O trecho de um elemento de coleção (das expressões dele).
fn trecho_do_elemento(a: &ast::Ast, el: &ast::CollectionElement) -> Option<Span> {
    use ast::CollectionElement as E;
    let junta = |x: Option<Span>, y: Option<Span>| match (x, y) {
        (Some(p), Some(q)) => Some(Span { start: p.start.min(q.start), end: p.end.max(q.end) }),
        (p, q) => p.or(q),
    };
    match el {
        E::Expression(e) | E::NullAwareExpression(e) | E::Spread { value: e, .. } => Some(a.expr(*e).span),
        E::MapEntry { key, value, .. } => junta(Some(a.expr(*key).span), Some(a.expr(*value).span)),
        E::If { condition, then, else_, .. } => {
            let mut s = junta(Some(a.expr(*condition).span), trecho_do_elemento(a, then));
            if let Some(x) = else_ {
                s = junta(s, trecho_do_elemento(a, x));
            }
            s
        }
        E::For { body, .. } | E::ForIn { body, .. } => trecho_do_elemento(a, body),
    }
}

/// Os escopos locais dos elementos de coleção: (trecho, nomes declarados).
fn escopos_de_colecao(a: &ast::Ast, el: &ast::CollectionElement, saida: &mut Vec<(Span, Vec<ast::Name>)>) {
    use ast::CollectionElement as E;
    match el {
        E::If { case_pattern, then, else_, .. } => {
            if let Some(p) = case_pattern
                && let Some(t) = trecho_do_elemento(a, then)
            {
                let mut nomes = Vec::new();
                declaradas_no_padrao(a, *p, true, &mut nomes);
                saida.push((Span { start: a.pattern(*p).span.start, end: t.end }, nomes));
            }
            escopos_de_colecao(a, then, saida);
            if let Some(x) = else_ {
                escopos_de_colecao(a, x, saida);
            }
        }
        E::For { init, condition, updates, body, .. } => {
            let mut nomes = Vec::new();
            let mut inicio: Option<usize> = None;
            match init {
                Some(ForInit::Variables(l)) => {
                    nomes.extend(l.variables.iter().map(|v| v.name));
                    inicio = l.variables.first().map(|v| v.name.span.start);
                }
                Some(ForInit::Pattern { pattern, .. }) => {
                    declaradas_no_padrao(a, *pattern, false, &mut nomes);
                    inicio = Some(a.pattern(*pattern).span.start);
                }
                _ => {}
            }
            let fim = trecho_do_elemento(a, body).map(|s| s.end).or(condition.map(|c| a.expr(c).span.end)).or(updates.last().map(|u| a.expr(*u).span.end));
            if let (Some(i), Some(f)) = (inicio, fim) {
                saida.push((Span { start: i, end: f }, nomes));
            }
            escopos_de_colecao(a, body, saida);
        }
        E::ForIn { target, body, .. } => {
            let mut nomes = Vec::new();
            match target {
                ForInTarget::Declared { name, .. } => nomes.push(*name),
                ForInTarget::Pattern { pattern, .. } => declaradas_no_padrao(a, *pattern, false, &mut nomes),
                ForInTarget::Expression(_) => {}
            }
            if let Some(t) = trecho_do_elemento(a, body) {
                saida.push((t, nomes));
            }
            escopos_de_colecao(a, body, saida);
        }
        _ => {}
    }
}

/// O nome `n` está declarado num escopo local ou de membro que contém a
/// posição `pos` da unidade (a busca léxica do analyzer pararia antes do
/// escopo da biblioteca).
pub fn sombreado(a: &ast::Ast, interner: &Interner, pos: usize, n: SymbolId) -> bool {
    let em = |s: Span| s.start <= pos && pos < s.end;
    let declara = |nomes: &[ast::Name]| nomes.iter().any(|x| x.sym == n && x.span.start < pos);
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
    // Os locais declarados antes de `pos`, no escopo que os contém.
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
                declaradas_no_padrao(a, *pattern, false, &mut nomes);
                bloco_de(s.span)
            }
            // As variáveis do laço valem no próprio laço.
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                nomes.extend(l.variables.iter().map(|v| v.name));
                s.span
            }
            StmtKind::For { init: Some(ForInit::Pattern { pattern, .. }), .. } => {
                declaradas_no_padrao(a, *pattern, false, &mut nomes);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => {
                nomes.push(*name);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Pattern { pattern, .. }, .. } => {
                declaradas_no_padrao(a, *pattern, false, &mut nomes);
                s.span
            }
            // As variáveis do `if-case` valem na guarda e no `then`.
            StmtKind::If { case_pattern: Some(p), then, .. } => {
                declaradas_no_padrao(a, *p, true, &mut nomes);
                Span { start: a.pattern(*p).span.start, end: a.stmt(*then).span.end }
            }
            // As de cada caso valem na guarda e no corpo do caso.
            StmtKind::Switch { cases, .. } => {
                for c in cases.iter() {
                    let Some(p) = c.pattern else { continue };
                    let mut vs = Vec::new();
                    declaradas_no_padrao(a, p, true, &mut vs);
                    if em(c.span) && declara(&vs) {
                        return true;
                    }
                }
                continue;
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
        if em(escopo) && declara(&nomes) {
            return true;
        }
    }
    // Os casos de `switch` de expressão e os elementos de coleção.
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Switch { cases, .. } => {
                for c in cases.iter() {
                    let mut vs = Vec::new();
                    declaradas_no_padrao(a, c.pattern, true, &mut vs);
                    if em(c.span) && declara(&vs) {
                        return true;
                    }
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                if !em(e.span) {
                    continue;
                }
                let mut escopos = Vec::new();
                for el in elements.iter() {
                    escopos_de_colecao(a, el, &mut escopos);
                }
                if escopos.iter().any(|(s, nomes)| em(*s) && declara(nomes)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    // Os parâmetros (no corpo) e os parâmetros de tipo (em toda a função)
    // das funções que contêm `pos`.
    let comum = |ps: &[ast::Parameter]| ps.iter().any(|p| !p.this_ && !p.super_ && p.name.is_some_and(|x| x.sym == n));
    for f in a.functions.iter() {
        if em(f.span) && f.type_params.iter().any(|t| t.name.sym == n) {
            return true;
        }
        let corpo = match &f.body {
            FunctionBody::Block(s) => a.stmt(*s).span,
            FunctionBody::Expression(e) => a.expr(*e).span,
            _ => continue,
        };
        if em(corpo) && f.parameters.as_deref().is_some_and(comum) {
            return true;
        }
    }
    // O construtor: na lista de inicializadores valem todos os parâmetros
    // (os `this.x` e `super.x` também); no corpo, os comuns.
    for m in a.members.iter() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let nos_inicializadores = k.initializers.iter().any(|i| {
            em(match i {
                Initializer::Field { span, .. } | Initializer::Super { span, .. } | Initializer::Redirect { span, .. } | Initializer::Assert { span, .. } => *span,
            })
        });
        if nos_inicializadores && k.parameters.iter().any(|p| p.name.is_some_and(|x| x.sym == n)) {
            return true;
        }
        let corpo = match &k.body {
            FunctionBody::Block(s) => Some(a.stmt(*s).span),
            FunctionBody::Expression(e) => Some(a.expr(*e).span),
            _ => None,
        };
        if corpo.is_some_and(em) && comum(&k.parameters) {
            return true;
        }
    }
    // Os parâmetros de tipo de um tipo `Function<T>(…)` e de um
    // parâmetro-função `f<T>(…)`.
    fn tipos_de_parametro(ps: &[ast::Parameter], pos: usize, n: SymbolId) -> bool {
        ps.iter().any(|p| {
            (p.span.start <= pos && pos < p.span.end && p.function_type_params.iter().any(|t| t.name.sym == n))
                || p.function_parameters.as_deref().is_some_and(|fs| tipos_de_parametro(fs, pos, n))
        })
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind
            && ((em(t.span) && type_params.iter().any(|x| x.name.sym == n)) || tipos_de_parametro(parameters, pos, n))
        {
            return true;
        }
    }
    for f in a.functions.iter() {
        if f.parameters.as_deref().is_some_and(|ps| tipos_de_parametro(ps, pos, n)) {
            return true;
        }
    }
    // Os membros declarados (e os parâmetros de tipo) do tipo que contém
    // `pos` depois do nome dele; os parâmetros de tipo do typedef.
    let valores = interner.lookup("values");
    for d in a.decls.iter() {
        let (nome, tipos, membros, extras): (Option<ast::Name>, &[ast::TypeParameter], &[ast::MemberId], Vec<SymbolId>) = match &d.kind {
            DeclKind::Class(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::Mixin(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            // O `values` sintético do enum também está no escopo.
            DeclKind::Enum(x) => (Some(x.name), &x.type_params[..], &x.members[..], x.constants.iter().map(|c| c.name.sym).chain(valores).collect()),
            DeclKind::Extension(x) => (x.name, &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::ExtensionType(x) => (Some(x.name), &x.type_params[..], &x.members[..], vec![x.representation_name.sym]),
            DeclKind::Typedef(x) => {
                if em(d.span) && x.type_params.iter().any(|t| t.name.sym == n) {
                    return true;
                }
                continue;
            }
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
pub fn elemento_da_anotacao(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<Element> {
    let a = &program.unit(u).ast;
    let pos = m.span.start;
    let primeiro = m.name.first()?;
    if sombreado(a, interner, pos, primeiro.sym) {
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

/// `ElementAnnotation.element`: o construtor invocado, a classe citada sem
/// argumentos, ou o getter lido (o da variável de topo ou estática, ou o
/// explícito).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementoInvocado {
    Construtor(dartforge_elements::model::ClassId, dartforge_elements::model::FunctionElementId),
    Classe(dartforge_elements::model::ClassId),
    Getter(dartforge_elements::model::FunctionElementId),
    Outro,
}

/// O [`ElementoInvocado`] da anotação `m`, escrita na unidade `u`.
pub fn elemento_invocado(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> ElementoInvocado {
    use dartforge_elements::model::FunctionKind;
    let a = &program.unit(u).ast;
    let Some(primeiro) = m.name.first() else { return ElementoInvocado::Outro };
    if sombreado(a, interner, m.span.start, primeiro.sym) {
        return ElementoInvocado::Outro;
    }
    let (alvo, membro) = match &m.name[..] {
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
        if e_getter { ElementoInvocado::Getter(f) } else { ElementoInvocado::Outro }
    };
    match (alvo, m.arguments.is_some(), membro) {
        (Some(Element::Class(c)), true, _) => {
            let chave = membro.or_else(|| interner.lookup(""));
            match chave.and_then(|k| program.class(c).constructors.get(&k)) {
                Some(&f) => ElementoInvocado::Construtor(c, f),
                None => ElementoInvocado::Outro,
            }
        }
        (Some(Element::Class(c)), false, None) => ElementoInvocado::Classe(c),
        (Some(Element::Class(c)), false, Some(n)) => match program.class(c).static_members.get(&n) {
            Some(&f) => getter(f),
            None => ElementoInvocado::Outro,
        },
        (Some(Element::Variable(v)), _, None) => match program.variable(v).getter {
            Some(f) => ElementoInvocado::Getter(f),
            None => ElementoInvocado::Outro,
        },
        (Some(Element::Function(g)), _, None) => getter(g),
        _ => ElementoInvocado::Outro,
    }
}

/// O nome da biblioteca (`library a.b;`), como o `LibraryElement.name`.
pub fn nome_da_biblioteca(program: &Program, interner: &Interner, l: dartforge_elements::model::LibraryId) -> String {
    program.library(l).name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
}

/// `_isTopGetter`: a anotação lê o getter `nome` de uma biblioteca chamada
/// `biblioteca` (`_isPackageMetaGetter` com `meta`).
pub fn e_getter_de(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, biblioteca: &str, nome: &str) -> bool {
    match elemento_invocado(program, interner, u, m) {
        ElementoInvocado::Getter(f) => {
            let e = program.function(f);
            interner.resolve(e.name) == nome && nome_da_biblioteca(program, interner, e.library) == biblioteca
        }
        _ => false,
    }
}

/// `_isConstructor`: a anotação invoca um construtor da classe `classe` de
/// uma biblioteca chamada `biblioteca`.
pub fn e_construtor_de(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, biblioteca: &str, classe: &str) -> bool {
    match elemento_invocado(program, interner, u, m) {
        ElementoInvocado::Construtor(c, _) => {
            let k = program.class(c);
            interner.resolve(k.name) == classe && nome_da_biblioteca(program, interner, k.library) == biblioteca
        }
        _ => false,
    }
}

/// A anotação `m` (escrita na unidade `u`), sem argumentos, é a variável de
/// topo `nome` de uma biblioteca cujo URI satisfaz `da_biblioteca`.
pub fn anotacao_e_variavel(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, nome: &str, da_biblioteca: &dyn Fn(&str) -> bool) -> bool {
    if m.arguments.is_some() || m.name.last().is_none_or(|n| interner.resolve(n.sym) != nome) {
        return false;
    }
    match elemento_da_anotacao(program, interner, u, m) {
        Some(Element::Variable(v)) => {
            let x = program.variable(v);
            interner.resolve(x.name) == nome && da_biblioteca(&program.library(x.library).uri)
        }
        _ => false,
    }
}

/// `ElementAnnotation.isDeprecated`: a variável `deprecated` do `dart:core`
/// (sem argumentos), ou um construtor da classe `Deprecated` do `dart:core`
/// (`@Deprecated('…')`, com argumentos).
pub fn e_deprecated(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> bool {
    match elemento_da_anotacao(program, interner, u, m) {
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
    anotacao_e_variavel(program, interner, u, m, "deprecated", &|uri| uri == "dart:core")
}
