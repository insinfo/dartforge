//! O vigésimo sétimo lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `cascade_invocations`: em cada bloco com duas ou mais instruções, a
//!   instrução que poderia ser juntada à anterior por cascata
//!   (`_CascadableExpression`), pelo `canonicalElement` do analyzer (o
//!   acessor vira a variável; o lado esquerdo simples de uma atribuição não
//!   tem elemento) e pelas dependências críticas (o elemento da declaração
//!   ou da atribuição anterior usado nos argumentos, no lado direito ou nas
//!   seções da atual).
//! * `require_trailing_commas`: as listas de argumentos (de chamadas,
//!   criações, anotações, constantes de enum, `super(…)`/`this(…)`), de
//!   parâmetros, de `assert` e os literais de coleção que ocupam mais de uma
//!   linha sem vírgula final, com as exceções do último nó (closure de bloco,
//!   string, chamada de closure numa linha, coleção), pelos tokens do léxico.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::regras16::e_invocacao_de_metodo;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, VariableId};
use dartforge_frontend::ast::{self, Ast, AssignOp, CollectionElement, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, StmtKind};
use dartforge_frontend::token::{Kind, Op, Token};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolved::{MemberRef, Resolved};
use std::collections::HashSet;

/// O `canonicalElement` de um elemento.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Canonico {
    Local(usize),
    Parametro(usize),
    Variavel(VariableId),
    Funcao(FunctionElementId),
    /// O par getter/setter escrito sem variável no modelo: a variável
    /// sintética do analyzer.
    Acessor(Option<ClassId>, Option<ExtensionId>, LibraryId, SymbolId),
    Elemento(Element),
}

/// O parâmetro `nome` da função ou do construtor mais de dentro que contém
/// `e` (o offset do nome na declaração).
fn parametro_declarado(a: &Ast, e: Span, nome: SymbolId) -> Option<usize> {
    let mut melhor: Option<(usize, usize)> = None;
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
        if e.start < sp.start || e.end > sp.end {
            continue;
        }
        if let Some(n) = ps.iter().find_map(|p| p.name.filter(|n| n.sym == nome)) {
            let tamanho = sp.end - sp.start;
            if melhor.is_none_or(|(t, _)| tamanho < t) {
                melhor = Some((tamanho, n.span.start));
            }
        }
    }
    melhor.map(|(_, d)| d)
}

fn canonico_da_funcao(s: &super::Semantica<'_>, f: FunctionElementId) -> Canonico {
    let g = s.program.function(f);
    match g.kind {
        FunctionKind::Getter | FunctionKind::Setter => match g.variable {
            Some(v) => Canonico::Variavel(v),
            None => Canonico::Acessor(g.class, g.extension, g.library, g.name),
        },
        _ => Canonico::Funcao(f),
    }
}

/// `AstNode.canonicalElement`: o identificador (simples ou prefixado) e o
/// `PropertyAccess`, sem parênteses.
fn canonico(s: &super::Semantica<'_>, a: &Ast, mut e: ExprId) -> Option<Canonico> {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    if !matches!(a.expr(e).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return None;
    }
    elemento(s, a, e)
}

/// O `staticElement?.canonicalElement` do nó `e` (identificador ou nome de
/// propriedade).
fn elemento(s: &super::Semantica<'_>, a: &Ast, e: ExprId) -> Option<Canonico> {
    Some(match s.corpo.get_resolved(e)? {
        Resolved::Local(_) => Canonico::Local(s.corpo.declaracao_local(e)?),
        Resolved::Parameter { name, .. } => Canonico::Parametro(parametro_declarado(a, a.expr(e).span, *name)?),
        Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => Canonico::Variavel(*v),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => canonico_da_funcao(s, *f),
        Resolved::ExtensionMember { member, .. } => canonico_da_funcao(s, *member),
        Resolved::Constructor(f) => Canonico::Funcao(*f),
        Resolved::Element(x) => Canonico::Elemento(*x),
        _ => return None,
    })
}

/// `_CascadableExpression`.
#[derive(Clone, Default)]
struct Cascavel {
    elemento: Option<Canonico>,
    /// Os intervalos dos nós críticos.
    criticos: Vec<Span>,
    junta: bool,
    recebe: bool,
    cascateavel: bool,
    critico: bool,
}

fn de_instrucao(s: &super::Semantica<'_>, a: &Ast, st: &ast::Stmt) -> Cascavel {
    match &st.kind {
        StmtKind::Variables(l) => {
            let elemento = match &l.variables[..] {
                [v] if !v.initializer.is_some_and(|i| matches!(a.expr(i).kind, ExprKind::Await(_))) => Some(Canonico::Local(v.name.span.start)),
                _ => None,
            };
            Cascavel { elemento, recebe: true, critico: true, ..Default::default() }
        }
        StmtKind::Expression(e) => {
            let mut e = *e;
            while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
                e = *x;
            }
            de_expressao(s, a, e)
        }
        _ => Cascavel::default(),
    }
}

fn de_expressao(s: &super::Semantica<'_>, a: &Ast, e: ExprId) -> Cascavel {
    match &a.expr(e).kind {
        ExprKind::Assign { op, target, value } => {
            let mut esq = *target;
            while let ExprKind::Parenthesized(x) = &a.expr(esq).kind {
                esq = *x;
            }
            let criticos = vec![a.expr(*value).span];
            if matches!(a.expr(esq).kind, ExprKind::Identifier(_)) {
                // O lado esquerdo simples não tem `staticElement`.
                return Cascavel { elemento: None, criticos, recebe: *op != AssignOp::Compound(dartforge_frontend::ast::BinaryOp::IfNull), critico: true, ..Default::default() };
            }
            // Setter: o prefixo de `a.b` (o `PropertyAccess` só com `.` e alvo
            // simples).
            let variavel = match &a.expr(esq).kind {
                ExprKind::Property { target, null_aware: false, .. } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) => elemento(s, a, *target),
                _ => None,
            };
            let e_variavel_de_instancia = match variavel {
                Some(Canonico::Local(_) | Canonico::Parametro(_)) => true,
                Some(Canonico::Variavel(v)) => !s.program.variable(v).static_,
                _ => false,
            };
            Cascavel { elemento: variavel, criticos, junta: true, recebe: *op != AssignOp::Compound(dartforge_frontend::ast::BinaryOp::IfNull) && e_variavel_de_instancia, cascateavel: true, critico: false }
        }
        ExprKind::Call { target, arguments } if e_invocacao_de_metodo(s, a, *target) => {
            // `_getExecutableElementFromMethodInvocation`: só com `.`.
            let ExprKind::Property { target: alvo, null_aware: false, .. } = &a.expr(*target).kind else { return Cascavel::default() };
            let nao_estatico = match elemento(s, a, *target) {
                Some(Canonico::Funcao(f)) => {
                    let g = s.program.function(f);
                    !g.static_ && (g.class.is_some() || g.extension.is_some())
                }
                _ => false,
            };
            if !nao_estatico {
                return Cascavel::default();
            }
            let simples = matches!(a.expr(*alvo).kind, ExprKind::Identifier(_));
            Cascavel { elemento: canonico(s, a, *alvo), criticos: vec![arguments.span], junta: simples, recebe: simples, cascateavel: true, critico: false }
        }
        ExprKind::Cascade { target, sections, .. } => {
            let simples = matches!(a.expr(*target).kind, ExprKind::Identifier(_));
            Cascavel {
                elemento: canonico(s, a, *target),
                criticos: sections.iter().map(|x| a.expr(*x).span).collect(),
                junta: simples,
                recebe: simples,
                cascateavel: true,
                critico: false,
            }
        }
        ExprKind::Property { target, null_aware: false, .. } => {
            if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                // `PrefixedIdentifier`.
                Cascavel { elemento: elemento(s, a, *target), junta: true, recebe: true, cascateavel: true, ..Default::default() }
            } else {
                // `PropertyAccess` com `.`.
                Cascavel { elemento: canonico(s, a, *target), cascateavel: true, ..Default::default() }
            }
        }
        _ => Cascavel::default(),
    }
}

/// O número da linha de um offset (`LineInfo`: `\n`, `\r\n` e `\r`).
struct Linhas {
    inicios: Vec<usize>,
}

impl Linhas {
    fn de(fonte: &str) -> Linhas {
        let b = fonte.as_bytes();
        let mut inicios = vec![0];
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\n' => inicios.push(i + 1),
                b'\r' => {
                    if b.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    inicios.push(i + 1);
                }
                _ => {}
            }
            i += 1;
        }
        Linhas { inicios }
    }

    fn linha(&self, offset: usize) -> usize {
        self.inicios.partition_point(|&x| x <= offset)
    }
}

/// Os tokens da unidade, para achar os vizinhos de um offset.
struct Tokens {
    t: Vec<Token>,
}

impl Tokens {
    /// O índice do primeiro token que começa em `offset` ou depois.
    fn a_partir(&self, offset: usize) -> usize {
        self.t.partition_point(|x| x.span.start < offset)
    }

    fn e_op(&self, i: usize, op: Op) -> bool {
        self.t.get(i).is_some_and(|x| x.kind == Kind::Op(op))
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let _ = interner;
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let mut relatar = |codigo: &'static CodigoLint, span: Span| {
        out.push(RelatoDeLint { codigo, span, args: Vec::new() });
    };

    // `cascade_invocations`.
    if ligada("cascade_invocations")
        && let Some(s) = sem
    {
        for st in a.stmts.iter() {
            let StmtKind::Block(cmds) = &st.kind else { continue };
            if cmds.len() < 2 {
                continue;
            }
            let mut anterior = Cascavel::default();
            for &k in cmds.iter() {
                let x = a.stmt(k);
                let atual = de_instrucao(s, a, x);
                // `compatibleWith` e `_hasCriticalDependencies`.
                let compativel = atual.elemento.is_some()
                    && anterior.recebe
                    && atual.junta
                    && (atual.cascateavel || anterior.cascateavel)
                    && atual.elemento == anterior.elemento
                    && !(anterior.critico
                        && atual.criticos.iter().any(|sp| {
                            a.exprs.iter().enumerate().any(|(i, y)| {
                                y.span.start >= sp.start
                                    && y.span.end <= sp.end
                                    && matches!(y.kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                                    && elemento(s, a, ExprId(i as u32)) == anterior.elemento
                            })
                        }));
                if compativel {
                    relatar(&c::CASCADE_INVOCATIONS, x.span);
                }
                anterior = atual;
            }
        }
    }

    // `require_trailing_commas`.
    if ligada("require_trailing_commas")
        && let Ok(t) = dartforge_frontend::lexer::lex(u.fonte)
    {
        let tk = Tokens { t };
        let linhas = Linhas::de(u.fonte);
        let mesma_linha = |inicio: usize, fim: usize| linhas.linha(inicio) == linhas.linha(fim);
        let mut achados: HashSet<(usize, usize)> = HashSet::new();
        // `_checkTrailingComma`: `abre` é o offset do token de abertura, `fecha`
        // o token de fechamento, `ultimo` o intervalo do último nó, `excecao` se
        // o último nó admite exceção e `erro` o token relatado.
        let mut checar = |abre: usize, fecha: Span, ultimo: Span, excecao: &dyn Fn() -> bool, erro: Span| {
            let i = tk.a_partir(ultimo.end);
            if tk.e_op(i, Op::Comma) {
                return;
            }
            if mesma_linha(abre, fecha.end) {
                return;
            }
            if !mesma_linha(ultimo.start, ultimo.end) && excecao() {
                return;
            }
            achados.insert((erro.start, erro.end));
        };
        // A exceção de uma expressão como último nó.
        let excecao_de = |e: ExprId| -> bool {
            match &a.expr(e).kind {
                ExprKind::FunctionExpression(f) => matches!(a.function(*f).body, FunctionBody::Block(_)),
                ExprKind::String(_) => true,
                ExprKind::Call { target, arguments } if matches!(a.expr(*target).kind, ExprKind::FunctionExpression(_)) => {
                    mesma_linha(arguments.span.start, arguments.span.end)
                }
                ExprKind::List { .. } | ExprKind::SetOrMap { .. } => true,
                _ => false,
            }
        };
        let fecha_de = |args: &ast::Arguments| Span { start: args.span.end.saturating_sub(1), end: args.span.end };
        // `visitArgumentList`.
        let mut listas: Vec<&ast::Arguments> = Vec::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => listas.push(arguments),
                _ => {}
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } = i {
                        listas.push(arguments);
                    }
                }
            }
        }
        for d in a.decls.iter() {
            if let ast::DeclKind::Enum(x) = &d.kind {
                listas.extend(x.constants.iter().filter_map(|k| k.arguments.as_ref()));
            }
        }
        for m in dartforge_frontend::pais::todas_as_anotacoes(a, u.unit) {
            if let Some(args) = &m.arguments {
                listas.push(args);
            }
        }
        for args in listas {
            let Some(ultimo) = args.args.last() else { continue };
            let inicio = ultimo.name.map_or(a.expr(ultimo.value).span.start, |n| n.span.start);
            let sp = Span { start: inicio, end: a.expr(ultimo.value).span.end };
            let nomeado = ultimo.name.is_some();
            let v = ultimo.value;
            checar(args.span.start, fecha_de(args), sp, &|| !nomeado && excecao_de(v), fecha_de(args));
        }
        // `visitAssertStatement` e `visitAssertInitializer`: os parênteses
        // depois do `assert`.
        let mut asserts: Vec<(Span, ExprId, Option<ExprId>)> = Vec::new();
        for st in a.stmts.iter() {
            if let StmtKind::Assert { condition, message } = &st.kind {
                asserts.push((st.span, *condition, *message));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Assert { span, condition, message } = i {
                        asserts.push((*span, *condition, *message));
                    }
                }
            }
        }
        for (sp, cond, msg) in asserts {
            let i = tk.a_partir(sp.start) + 1;
            if !tk.e_op(i, Op::LParen) {
                continue;
            }
            let abre = tk.t[i].span.start;
            let ultimo = msg.unwrap_or(cond);
            let usp = a.expr(ultimo).span;
            // O `)`: depois do último nó e da vírgula opcional.
            let mut j = tk.a_partir(usp.end);
            if tk.e_op(j, Op::Comma) {
                j += 1;
            }
            if !tk.e_op(j, Op::RParen) {
                continue;
            }
            let fecha = tk.t[j].span;
            checar(abre, fecha, usp, &|| excecao_de(ultimo), fecha);
        }
        // `visitFormalParameterList`: o erro vai no `]`/`}` quando há.
        let mut parametros: Vec<&[ast::Parameter]> = Vec::new();
        fn com_internos<'a>(ps: &'a [ast::Parameter], v: &mut Vec<&'a [ast::Parameter]>) {
            v.push(ps);
            for p in ps {
                if let Some(internos) = &p.function_parameters {
                    com_internos(internos, v);
                }
            }
        }
        for f in a.functions.iter() {
            if let Some(ps) = f.parameters.as_deref() {
                com_internos(ps, &mut parametros);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                com_internos(&k.parameters, &mut parametros);
            }
        }
        for t in a.types.iter() {
            if let ast::TypeKind::Function { parameters, .. } = &t.kind {
                com_internos(parameters, &mut parametros);
            }
        }
        for d in a.decls.iter() {
            if let ast::DeclKind::Typedef(x) = &d.kind
                && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                com_internos(parameters, &mut parametros);
            }
        }
        for ps in parametros {
            let (Some(primeiro), Some(ultimo)) = (ps.first(), ps.last()) else { continue };
            // O `(`: antes do primeiro parâmetro, pulando o `[`/`{` de abertura.
            let mut i = tk.a_partir(primeiro.span.start);
            if i == 0 {
                continue;
            }
            i -= 1;
            if (tk.e_op(i, Op::LBracket) || tk.e_op(i, Op::LBrace)) && i > 0 {
                i -= 1;
            }
            if !tk.e_op(i, Op::LParen) {
                continue;
            }
            let abre = tk.t[i].span.start;
            let mut j = tk.a_partir(ultimo.span.end);
            if tk.e_op(j, Op::Comma) {
                j += 1;
            }
            let mut delimitador = None;
            if tk.e_op(j, Op::RBracket) || tk.e_op(j, Op::RBrace) {
                delimitador = Some(tk.t[j].span);
                j += 1;
            }
            if !tk.e_op(j, Op::RParen) {
                continue;
            }
            let fecha = tk.t[j].span;
            // Um parâmetro não é nenhum dos nós com exceção.
            checar(abre, fecha, ultimo.span, &|| false, delimitador.unwrap_or(fecha));
        }
        // `visitListLiteral` e `visitSetOrMapLiteral`.
        for e in a.exprs.iter() {
            let (ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. }) = &e.kind else { continue };
            let Some(ultimo) = elements.last() else { continue };
            let (usp, expr) = match ultimo {
                CollectionElement::Expression(x) => (a.expr(*x).span, Some(*x)),
                CollectionElement::NullAwareExpression(x) => (Span { start: tk.t[tk.a_partir(a.expr(*x).span.start).saturating_sub(1)].span.start, end: a.expr(*x).span.end }, None),
                CollectionElement::MapEntry { key, value, .. } => (Span { start: a.expr(*key).span.start, end: a.expr(*value).span.end }, None),
                CollectionElement::Spread { value, .. } => {
                    let i = tk.a_partir(a.expr(*value).span.start).saturating_sub(1);
                    (Span { start: tk.t[i].span.start, end: a.expr(*value).span.end }, None)
                }
                CollectionElement::If { .. } | CollectionElement::For { .. } | CollectionElement::ForIn { .. } => match fim_de_elemento(a, ultimo) {
                    Some(fim) => (Span { start: inicio_de_elemento(a, &tk, u.fonte, ultimo), end: fim }, None),
                    None => continue,
                },
            };
            // O `]`/`}` do literal: o último token do nó.
            let fecha = Span { start: e.span.end.saturating_sub(1), end: e.span.end };
            // O `[`/`{` do literal: depois dos argumentos de tipo e do `const`.
            let i = tk.a_partir(e.span.start);
            let abre = (i..tk.t.len()).find(|&k| tk.e_op(k, Op::LBracket) || tk.e_op(k, Op::LBrace)).map_or(e.span.start, |k| tk.t[k].span.start);
            checar(abre, fecha, usp, &|| expr.is_some_and(excecao_de), fecha);
        }
        let mut v: Vec<(usize, usize)> = achados.into_iter().collect();
        v.sort();
        for (i, f) in v {
            relatar(&c::REQUIRE_TRAILING_COMMAS, Span { start: i, end: f });
        }
    }

    out
}

/// O início do elemento `if`/`for` de coleção: o token `if`/`for` (ou o
/// `await` de `await for`) antes da condição.
fn inicio_de_elemento(a: &Ast, tk: &Tokens, fonte: &str, x: &CollectionElement) -> usize {
    let primeiro = match x {
        CollectionElement::If { condition, .. } => a.expr(*condition).span.start,
        CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
            // A primeira expressão de dentro do cabeçalho.
            match primeira_do_for(a, x) {
                Some(p) => p,
                None => return 0,
            }
        }
        _ => return 0,
    };
    // Volta até o `if`/`for`: o `(` antes da condição e o token antes dele.
    let mut i = tk.a_partir(primeiro);
    while i > 0 {
        i -= 1;
        if let Kind::Keyword(k) = tk.t[i].kind
            && matches!(k, dartforge_frontend::token::Keyword::If | dartforge_frontend::token::Keyword::For)
        {
            if i > 0 && tk.t[i - 1].kind == Kind::Ident && &fonte[tk.t[i - 1].span.start..tk.t[i - 1].span.end] == "await" {
                return tk.t[i - 1].span.start;
            }
            return tk.t[i].span.start;
        }
    }
    0
}

fn primeira_do_for(a: &Ast, x: &CollectionElement) -> Option<usize> {
    match x {
        CollectionElement::ForIn { iterable, .. } => Some(a.expr(*iterable).span.start),
        CollectionElement::For { init, condition, updates, body, .. } => match init {
            Some(ast::ForInit::Expression(e)) => Some(a.expr(*e).span.start),
            Some(ast::ForInit::Variables(l)) => l.variables.first().map(|v| v.name.span.start),
            Some(ast::ForInit::Pattern { pattern, .. }) => Some(a.pattern(*pattern).span.start),
            None => condition.map(|c| a.expr(c).span.start).or_else(|| updates.first().map(|u| a.expr(*u).span.start)).or_else(|| fim_de_elemento(a, body)),
        },
        _ => None,
    }
}

/// O fim de um elemento de coleção.
fn fim_de_elemento(a: &Ast, x: &CollectionElement) -> Option<usize> {
    Some(match x {
        CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) => a.expr(*e).span.end,
        CollectionElement::MapEntry { value, .. } => a.expr(*value).span.end,
        CollectionElement::Spread { value, .. } => a.expr(*value).span.end,
        CollectionElement::If { then, else_, .. } => match else_ {
            Some(x) => fim_de_elemento(a, x)?,
            None => fim_de_elemento(a, then)?,
        },
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => fim_de_elemento(a, body)?,
    })
}
