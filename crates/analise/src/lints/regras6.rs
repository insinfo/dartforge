//! O sexto lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `control_flow_in_finally`, `throw_in_finally`, `avoid_final_parameters`,
//! `avoid_void_async`, `avoid_returning_null_for_void`,
//! `prefer_asserts_with_message`, `unnecessary_library_directive`,
//! `no_self_assignments`, `avoid_annotating_with_dynamic`,
//! `unnecessary_constructor_name`, `unnecessary_breaks`,
//! `prefer_expression_function_bodies`, `combinators_ordering`,
//! `prefer_null_aware_operators`, `one_member_abstracts` e
//! `avoid_private_typedef_functions`.
//!
//! Diferenças conhecidas:
//! - `avoid_returning_null_for_void` só olha funções e métodos com o tipo de
//!   retorno escrito (e o setter sem tipo): o original usa o tipo do
//!   elemento, que numa função literal vem da inferência e num método sem
//!   tipo vem da sobrescrita. `Future` é reconhecido pelo nome.
//! - `unnecessary_breaks` não confere a versão de linguagem (o original só
//!   roda com padrões, 3.0 ou mais).
//! - `unnecessary_constructor_name` conhece a criação sem `new`/`const`
//!   pela forma `X.new(…)`, sem resolver `X`.
//! - `avoid_private_typedef_functions` conta os usos só nesta unidade, e por
//!   isso não roda em unidade com `part` ou `part of` (o original conta na
//!   biblioteca inteira).
//! - `avoid_annotating_with_dynamic` não tem a isenção de augmentation.
//! - `unnecessary_library_directive` acha o comentário de documentação pelo
//!   texto que precede a diretiva.
//! - `prefer_null_aware_operators` compara os textos sem os brancos, não o
//!   `toString` dos nós.
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, AssignOp, Ast, AsyncModifier, BinaryOp, Combinator, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionId, FunctionKind,
    Initializer, MemberId, MemberKind, StmtKind, TypeKind, TypedefKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

/// O que sai do `finally`: `break`/`continue` (com o rótulo, se houver) ou
/// `return`/`throw`.
enum Fluxo {
    Break(Option<SymbolId>),
    Continue(Option<SymbolId>),
    Saida,
}

/// `ControlFlowInFinallyBlockReporter.reportIfFinallyAncestorExists`: o nó
/// está no `finally` do `try` mais próximo, e nada dentro do `finally` o
/// contém: nem uma função (para `return` e `throw`), nem o alvo do desvio
/// (para `break` e `continue`).
fn foge_do_finally(a: &Ast, pilha: &[No], fluxo: &Fluxo) -> bool {
    let Some(t) = pilha.iter().rposition(|n| matches!(n, No::Stmt(s) if matches!(a.stmt(*s).kind, StmtKind::Try { .. }))) else {
        return false;
    };
    let No::Stmt(tentativa) = pilha[t] else { return false };
    let StmtKind::Try { finally_: Some(fim), .. } = &a.stmt(tentativa).kind else { return false };
    if pilha.get(t + 1) != Some(&No::Stmt(*fim)) {
        return false;
    }
    let dentro = &pilha[t + 2..];
    if dentro.iter().any(|n| matches!(n, No::Funcao(_))) {
        return false;
    }
    let rotulo = match fluxo {
        Fluxo::Saida => return true,
        Fluxo::Break(r) | Fluxo::Continue(r) => *r,
    };
    let de_break = matches!(fluxo, Fluxo::Break(_));
    let e_alvo = |n: &No| {
        let No::Stmt(s) = n else { return false };
        match (&a.stmt(*s).kind, rotulo) {
            (StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. }, None) => true,
            (StmtKind::Switch { .. }, None) => de_break,
            (StmtKind::Labeled { labels, .. }, Some(r)) => labels.iter().any(|l| l.sym == r),
            (StmtKind::Switch { cases, .. }, Some(r)) => cases.iter().any(|k| k.labels.iter().any(|l| l.sym == r)),
            _ => false,
        }
    };
    !dentro.iter().any(e_alvo)
}

/// A função declara que não devolve valor: `void` (ou setter sem tipo) se
/// não é `async`; `Future<void>` se é.
fn devolve_void(a: &Ast, f: &ast::Function, interner: &Interner) -> bool {
    let assincrona = match f.modifier {
        AsyncModifier::None => false,
        AsyncModifier::Async => true,
        AsyncModifier::AsyncStar | AsyncModifier::SyncStar => return false,
    };
    let Some(t) = f.return_type else { return f.kind == FunctionKind::Setter && !assincrona };
    match &a.ty(t).kind {
        TypeKind::Void => !assincrona,
        TypeKind::Named { name, args } => {
            assincrona
                && name.last().is_some_and(|n| interner.resolve(n.sym) == "Future")
                && args.len() == 1
                && matches!(a.ty(args[0]).kind, TypeKind::Void)
        }
        _ => false,
    }
}

/// O intervalo do corpo `=> e;` (o `ExpressionFunctionBody`): do `async`,
/// se houver, ou do `=>`, até o `;`.
fn corpo_de_seta(a: &Ast, fonte: &str, e: ExprId) -> Span {
    let s = a.expr(e).span;
    let antes = fonte.get(..s.start).unwrap_or("").trim_end();
    let mut inicio = s.start;
    if antes.ends_with("=>") {
        inicio = antes.len() - 2;
        let mais = antes[..inicio].trim_end();
        if mais.ends_with("async") {
            inicio = mais.len() - 5;
        }
    }
    let depois = fonte.get(s.end..).unwrap_or("");
    let aparado = depois.trim_start();
    let fim = if aparado.starts_with(';') { s.end + (depois.len() - aparado.len()) + 1 } else { s.end };
    Span { start: inicio, end: fim }
}

/// O intervalo do corpo em bloco (o `BlockFunctionBody`): começa na palavra
/// `async`, `async*` ou `sync*`, se houver.
fn corpo_em_bloco(fonte: &str, bloco: Span, modificador: AsyncModifier) -> Span {
    if modificador == AsyncModifier::None {
        return bloco;
    }
    let mut antes = fonte.get(..bloco.start).unwrap_or("").trim_end();
    if let Some(sem) = antes.strip_suffix('*') {
        antes = sem.trim_end();
    }
    let palavra = if modificador == AsyncModifier::SyncStar { "sync" } else { "async" };
    if antes.ends_with(palavra) { Span { start: antes.len() - palavra.len(), end: bloco.end } } else { bloco }
}

/// Há comentário de documentação logo antes de `inicio`.
fn documentado(fonte: &str, inicio: usize) -> bool {
    let antes = fonte.get(..inicio).unwrap_or("").trim_end();
    if antes.rsplit('\n').next().unwrap_or("").trim_start().starts_with("///") {
        return true;
    }
    if antes.ends_with("*/")
        && let Some(abre) = antes[..antes.len() - 2].rfind("/*")
    {
        let texto = &antes[abre..];
        return texto.starts_with("/**") && !texto.starts_with("/**/");
    }
    false
}

/// `Identifier`: `x` ou `p.x` (o `PrefixedIdentifier`).
fn identificador(a: &Ast, e: ExprId) -> Option<(Option<SymbolId>, SymbolId)> {
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some((None, n.sym)),
        ExprKind::Property { target, name, null_aware: false } => match &a.expr(*target).kind {
            ExprKind::Identifier(p) => Some((Some(p.sym), name.sym)),
            _ => None,
        },
        _ => None,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, _sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let de_metodo: HashSet<FunctionId> = a
        .members
        .iter()
        .filter_map(|m| match &m.kind {
            MemberKind::Method(f) => Some(*f),
            _ => None,
        })
        .collect();
    let nulo = |e: ExprId| matches!(a.expr(e).kind, ExprKind::Null);
    // O texto de uma expressão sem os brancos.
    let compacto = |e: ExprId| -> String {
        let s = a.expr(e).span;
        fonte.get(s.start..s.end).unwrap_or("").chars().filter(|x| !x.is_whitespace()).collect()
    };

    // `prefer_asserts_with_message`: o comando e o inicializador.
    if ligada("prefer_asserts_with_message") {
        for s in a.stmts.iter() {
            if matches!(s.kind, StmtKind::Assert { message: None, .. }) {
                relatar(&c::PREFER_ASSERTS_WITH_MESSAGE, s.span, &[]);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let Initializer::Assert { span, message: None, .. } = i {
                        relatar(&c::PREFER_ASSERTS_WITH_MESSAGE, *span, &[]);
                    }
                }
            }
        }
    }
    // `no_self_assignments`: `x = x` e `p.x = p.x`.
    if ligada("no_self_assignments") {
        for e in a.exprs.iter() {
            if let ExprKind::Assign { op: AssignOp::Assign, target, value } = &e.kind
                && let (Some(x), Some(y)) = (identificador(a, *target), identificador(a, *value))
                && x == y
            {
                relatar(&c::NO_SELF_ASSIGNMENTS, e.span, &[]);
            }
        }
    }
    // `avoid_final_parameters`: os parâmetros de construtor, método e função
    // (declarada ou literal); não os de dentro de um parâmetro-função.
    if ligada("avoid_final_parameters") {
        let mut checar = |ps: &[ast::Parameter]| {
            for p in ps.iter().filter(|p| p.final_) {
                relatar(&c::AVOID_FINAL_PARAMETERS, p.span, &[]);
            }
        };
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                checar(&ps[..]);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                checar(&k.parameters[..]);
            }
        }
    }
    // `avoid_void_async`: função ou método `async` (não gerador) de retorno
    // `void` escrito; `main` de topo ou local é isenta.
    if ligada("avoid_void_async") {
        for (i, f) in a.functions.iter().enumerate() {
            let Some(nome) = f.name else { continue };
            if f.modifier != AsyncModifier::Async || !f.return_type.is_some_and(|t| matches!(a.ty(t).kind, TypeKind::Void)) {
                continue;
            }
            if !de_metodo.contains(&FunctionId(i as u32)) && interner.resolve(nome.sym) == "main" {
                continue;
            }
            relatar(&c::AVOID_VOID_ASYNC, nome.span, &[]);
        }
    }
    // `avoid_annotating_with_dynamic`: todo parâmetro comum, `this.x` ou
    // `super.x` com o tipo `dynamic` escrito, em qualquer lista.
    if ligada("avoid_annotating_with_dynamic") {
        fn listas<'x>(lista: &'x [ast::Parameter], saida: &mut Vec<&'x [ast::Parameter]>) {
            saida.push(lista);
            for p in lista {
                if let Some(internos) = &p.function_parameters {
                    listas(internos, saida);
                }
            }
        }
        let mut todas: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                listas(ps, &mut todas);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                listas(&k.parameters, &mut todas);
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Function { parameters, .. } = &t.kind {
                listas(parameters, &mut todas);
            }
        }
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                listas(parameters, &mut todas);
            }
        }
        for lista in todas {
            for p in lista.iter().filter(|p| p.function_parameters.is_none()) {
                let Some(t) = p.ty else { continue };
                let ty = a.ty(t);
                if let TypeKind::Named { name, args } = &ty.kind
                    && let [unico] = &name[..]
                    && args.is_empty()
                    && interner.resolve(unico.sym) == "dynamic"
                {
                    // O nó é o parâmetro sem o valor padrão.
                    let fim = p.name.map_or(ty.span.end, |n| n.span.end).max(ty.span.end);
                    relatar(&c::AVOID_ANNOTATING_WITH_DYNAMIC, Span { start: p.span.start, end: fim }, &[]);
                }
            }
        }
    }
    // `unnecessary_library_directive`: sem `part`, sem documentação e sem
    // anotação.
    if ligada("unnecessary_library_directive") && !u.unit.directives.iter().any(|d| matches!(d.kind, DirectiveKind::Part { .. })) {
        for d in u.unit.directives.iter() {
            if matches!(d.kind, DirectiveKind::Library { .. }) && d.metadata.is_empty() && !documentado(fonte, d.span.start) {
                relatar(&c::UNNECESSARY_LIBRARY_DIRECTIVE, d.span, &[]);
            }
        }
    }
    // `unnecessary_constructor_name`: `.new` na declaração, no construtor
    // primário de extension type e na criação.
    if ligada("unnecessary_constructor_name") {
        let e_new = |n: ast::Name| interner.resolve(n.sym) == "new";
        // Os construtores de um extension type sem nome no primário não são
        // conferidos.
        let mut isentos: HashSet<MemberId> = HashSet::new();
        for d in a.decls.iter() {
            if let DeclKind::ExtensionType(x) = &d.kind {
                match x.constructor {
                    Some(n) if e_new(n) => relatar(&c::UNNECESSARY_CONSTRUCTOR_NAME, n.span, &[]),
                    Some(_) => {}
                    None => isentos.extend(x.members.iter().copied()),
                }
            }
        }
        for (i, m) in a.members.iter().enumerate() {
            if let MemberKind::Constructor(k) = &m.kind
                && let Some(n) = k.name
                && e_new(n)
                && !isentos.contains(&MemberId(i as u32))
            {
                relatar(&c::UNNECESSARY_CONSTRUCTOR_NAME, n.span, &[]);
            }
        }
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::InstanceCreation { constructor: Some(n), .. } if e_new(*n) => relatar(&c::UNNECESSARY_CONSTRUCTOR_NAME, n.span, &[]),
                ExprKind::Call { target, .. } => {
                    if let ExprKind::Property { name, null_aware: false, .. } = &a.expr(*target).kind
                        && e_new(*name)
                    {
                        relatar(&c::UNNECESSARY_CONSTRUCTOR_NAME, name.span, &[]);
                    }
                }
                _ => {}
            }
        }
    }
    // `unnecessary_breaks`: o `break` sem rótulo que fecha um caso de mais
    // de um comando.
    if ligada("unnecessary_breaks") {
        for s in a.stmts.iter() {
            let StmtKind::Switch { cases, .. } = &s.kind else { continue };
            for k in cases.iter() {
                if k.body.len() > 1
                    && let Some(&ultimo) = k.body.last()
                    && matches!(a.stmt(ultimo).kind, StmtKind::Break(None))
                {
                    relatar(&c::UNNECESSARY_BREAKS, a.stmt(ultimo).span, &[]);
                }
            }
        }
    }
    // `prefer_expression_function_bodies`: bloco de um só `return e;`.
    if ligada("prefer_expression_function_bodies") {
        let mut checar = |corpo: &FunctionBody, modificador: AsyncModifier| {
            if let FunctionBody::Block(s) = corpo
                && let StmtKind::Block(comandos) = &a.stmt(*s).kind
                && let [unico] = &comandos[..]
                && matches!(a.stmt(*unico).kind, StmtKind::Return(Some(_)))
            {
                relatar(&c::PREFER_EXPRESSION_FUNCTION_BODIES, corpo_em_bloco(fonte, a.stmt(*s).span, modificador), &[]);
            }
        };
        for f in a.functions.iter() {
            checar(&f.body, f.modifier);
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                checar(&k.body, AsyncModifier::None);
            }
        }
    }
    // `combinators_ordering`: os nomes de cada `show`/`hide` em ordem.
    if ligada("combinators_ordering") {
        for d in u.unit.directives.iter() {
            let combinadores = match &d.kind {
                DirectiveKind::Import { combinators, .. } | DirectiveKind::Export { combinators, .. } => combinators,
                _ => continue,
            };
            for k in combinadores.iter() {
                let (palavra, nomes) = match k {
                    Combinator::Show(n) => ("show", n),
                    Combinator::Hide(n) => ("hide", n),
                };
                let (Some(primeiro), Some(ultimo)) = (nomes.first(), nomes.last()) else { continue };
                if nomes.windows(2).all(|w| interner.resolve(w[0].sym) <= interner.resolve(w[1].sym)) {
                    continue;
                }
                let inicio = fonte
                    .get(..primeiro.span.start)
                    .and_then(|t| t.rfind(palavra))
                    .filter(|&i| i >= d.span.start)
                    .unwrap_or(primeiro.span.start);
                relatar(&c::COMBINATORS_ORDERING, Span { start: inicio, end: ultimo.span.end }, &[]);
            }
        }
    }
    // `prefer_null_aware_operators`: `a == null ? null : a.b` e
    // `a != null ? a.b : null`.
    if ligada("prefer_null_aware_operators") {
        for e in a.exprs.iter() {
            let ExprKind::Conditional { condition, then, else_ } = &e.kind else { continue };
            let ExprKind::Binary { op, left, right } = &a.expr(*condition).kind else { continue };
            let testada = if nulo(*left) {
                *right
            } else if nulo(*right) {
                *left
            } else {
                continue;
            };
            let mut resultado = match op {
                BinaryOp::Eq if nulo(*then) => *else_,
                BinaryOp::NotEq if nulo(*else_) => *then,
                _ => continue,
            };
            let alvo = compacto(testada);
            loop {
                // O prefixo, o alvo da chamada de método, o operando de `!`
                // ou o alvo do acesso.
                let proximo = match &a.expr(resultado).kind {
                    ExprKind::Property { target, .. } => Some(*target),
                    ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                        ExprKind::Property { target, .. } => Some(*target),
                        ExprKind::TypeArguments { target, .. } => match &a.expr(*target).kind {
                            ExprKind::Property { target, .. } => Some(*target),
                            _ => None,
                        },
                        _ => None,
                    },
                    ExprKind::Unary { op: UnaryOp::NullAssert, operand } => Some(*operand),
                    _ => None,
                };
                let Some(p) = proximo else { break };
                if compacto(p) == alvo {
                    relatar(&c::PREFER_NULL_AWARE_OPERATORS, e.span, &[]);
                    break;
                }
                resultado = p;
            }
        }
    }
    // `one_member_abstracts`: classe abstrata sem cláusulas, sem campo nem
    // acessor, com um só método, e ele abstrato.
    if ligada("one_member_abstracts") {
        for d in a.decls.iter().filter(|d| !d.augment) {
            let DeclKind::Class(x) = &d.kind else { continue };
            if !x.modifiers.abstract_ || x.modifiers.macro_ || x.mixin_application || x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty() {
                continue;
            }
            let mut tem_campo = false;
            let mut metodos: Vec<&ast::Function> = Vec::new();
            for &m in x.members.iter() {
                match &a.member(m).kind {
                    MemberKind::Field(_) => tem_campo = true,
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        match f.kind {
                            FunctionKind::Getter | FunctionKind::Setter => tem_campo = true,
                            FunctionKind::Function | FunctionKind::Operator => metodos.push(f),
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            if let (false, [unico]) = (tem_campo, &metodos[..])
                && matches!(unico.body, FunctionBody::Empty)
                && !unico.external
                && !unico.static_
            {
                relatar(&c::ONE_MEMBER_ABSTRACTS, x.name.span, &[unico.name.map_or("", |n| interner.resolve(n.sym))]);
            }
        }
    }
    // `avoid_private_typedef_functions`: o alias privado de tipo função
    // citado no máximo uma vez.
    if ligada("avoid_private_typedef_functions")
        && !u.unit.directives.iter().any(|d| matches!(d.kind, DirectiveKind::Part { .. } | DirectiveKind::PartOf { .. }))
    {
        for d in a.decls.iter() {
            let DeclKind::Typedef(x) = &d.kind else { continue };
            let elegivel = match &x.kind {
                TypedefKind::Legacy { .. } => true,
                TypedefKind::Alias(t) => x.type_params.is_empty() && matches!(a.ty(*t).kind, TypeKind::Function { .. }),
            };
            if !elegivel || !interner.resolve(x.name.sym).starts_with('_') {
                continue;
            }
            let usos = a
                .types
                .iter()
                .filter(|t| matches!(&t.kind, TypeKind::Named { name, .. } if name.last().is_some_and(|n| n.sym == x.name.sym)))
                .count();
            if usos <= 1 {
                relatar(&c::AVOID_PRIVATE_TYPEDEF_FUNCTIONS, x.name.span, &[]);
            }
        }
    }

    // As regras que perguntam pelos ancestrais.
    let (do_finally, do_throw, do_nulo) = (ligada("control_flow_in_finally"), ligada("throw_in_finally"), ligada("avoid_returning_null_for_void"));
    if do_finally || do_throw || do_nulo {
        // O código do `return null`: o da função declarada mais próxima, se
        // ela declara que não devolve valor. Função literal não entra.
        let codigo_do_nulo = |pilha: &[No], propria: Option<FunctionId>| -> Option<&'static CodigoLint> {
            let f = propria.or_else(|| {
                pilha.iter().rev().find_map(|n| match n {
                    No::Funcao(f) => Some(*f),
                    _ => None,
                })
            })?;
            let funcao = a.function(f);
            if funcao.name.is_none() || !devolve_void(a, funcao, interner) {
                return None;
            }
            Some(if de_metodo.contains(&f) {
                &c::AVOID_RETURNING_NULL_FOR_VOID_FROM_METHOD
            } else {
                &c::AVOID_RETURNING_NULL_FOR_VOID_FROM_FUNCTION
            })
        };
        andar(u, &mut |no, pilha| match no {
            No::Stmt(s) => {
                let n = a.stmt(s);
                match &n.kind {
                    StmtKind::Break(r) if do_finally => {
                        if foge_do_finally(a, pilha, &Fluxo::Break(r.map(|x| x.sym))) {
                            relatar(&c::CONTROL_FLOW_IN_FINALLY, n.span, &["break"]);
                        }
                    }
                    StmtKind::Continue(r) if do_finally => {
                        if foge_do_finally(a, pilha, &Fluxo::Continue(r.map(|x| x.sym))) {
                            relatar(&c::CONTROL_FLOW_IN_FINALLY, n.span, &["continue"]);
                        }
                    }
                    StmtKind::Return(valor) => {
                        if do_finally && foge_do_finally(a, pilha, &Fluxo::Saida) {
                            relatar(&c::CONTROL_FLOW_IN_FINALLY, n.span, &["return"]);
                        }
                        if do_nulo
                            && valor.is_some_and(nulo)
                            && let Some(codigo) = codigo_do_nulo(pilha, None)
                        {
                            relatar(codigo, n.span, &[]);
                        }
                    }
                    _ => {}
                }
            }
            No::Expr(e) => {
                if do_throw && matches!(a.expr(e).kind, ExprKind::Throw(_)) && foge_do_finally(a, pilha, &Fluxo::Saida) {
                    relatar(&c::THROW_IN_FINALLY, a.expr(e).span, &["throw"]);
                }
            }
            No::Funcao(f) => {
                if do_nulo
                    && let FunctionBody::Expression(v) = &a.function(f).body
                    && nulo(*v)
                    && let Some(codigo) = codigo_do_nulo(pilha, Some(f))
                {
                    relatar(codigo, corpo_de_seta(a, fonte, *v), &[]);
                }
            }
            _ => {}
        });
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|_| true, None);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.unico, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    fn so(regra: &str, fonte: &str) -> Vec<String> {
        achados(fonte).into_iter().filter(|(c, _)| c.starts_with(regra)).map(|(_, t)| t).collect()
    }

    #[test]
    fn fluxo_no_finally() {
        let fonte = "int f(List<int> l) {\n  for (var x in l) {\n    try {\n    } finally {\n      if (x > 0) break;\n      for (;;) {\n        break;\n      }\n      l.forEach((y) {\n        return;\n      });\n      return 1;\n    }\n  }\n  return 0;\n}\n";
        assert_eq!(so("control_flow_in_finally", fonte), vec!["break;".to_string(), "return 1;".to_string()]);
        let fonte = "void f() {\n  try {\n    throw 1;\n  } finally {\n    throw 2;\n  }\n}\n";
        assert_eq!(so("throw_in_finally", fonte), vec!["throw 2".to_string()]);
        // O `try` mais próximo não tem `finally`: nada.
        let fonte = "void f() {\n  try {\n  } finally {\n    try {\n      return;\n    } catch (e) {}\n  }\n}\n";
        assert!(so("control_flow_in_finally", fonte).is_empty());
    }

    #[test]
    fn nulo_para_void() {
        let fonte = "void f() {\n  return null;\n}\nFuture<void> g() async => null;\nclass A {\n  void m() => null;\n  set s(int v) {\n    return null;\n  }\n  int n() {\n    return null;\n  }\n}\n";
        assert_eq!(
            achados(fonte).into_iter().filter(|(c, _)| c.starts_with("avoid_returning_null_for_void")).collect::<Vec<_>>(),
            vec![
                ("avoid_returning_null_for_void_from_function", "return null;".to_string()),
                ("avoid_returning_null_for_void_from_function", "async => null;".to_string()),
                ("avoid_returning_null_for_void_from_method", "=> null;".to_string()),
                ("avoid_returning_null_for_void_from_method", "return null;".to_string()),
            ]
        );
    }

    #[test]
    fn declaracoes() {
        assert_eq!(so("avoid_void_async", "void main() async {}\nvoid f() async {}\nFuture<void> g() async {}\n"), vec!["f".to_string()]);
        assert_eq!(so("avoid_final_parameters", "void f(final int a, int b) {}\n"), vec!["final int a".to_string()]);
        assert_eq!(so("avoid_annotating_with_dynamic", "void f(dynamic a, [dynamic b = 1]) {}\n"), vec!["dynamic a".to_string(), "dynamic b".to_string()]);
        assert_eq!(so("one_member_abstracts", "abstract class A {\n  void m();\n}\nabstract class B {\n  void m();\n  int get x;\n}\n"), vec!["A".to_string()]);
        assert_eq!(so("avoid_private_typedef_functions", "typedef _F = void Function();\ntypedef _G = void Function();\n_G? a;\n_G? b;\n_F? c;\n"), vec!["_F".to_string()]);
        assert_eq!(so("unnecessary_constructor_name", "class A {\n  A.new();\n}\nvar a = A.new();\nvar t = A.new;\n"), vec!["new".to_string(), "new".to_string()]);
        assert_eq!(so("unnecessary_library_directive", "library a;\n"), vec!["library a;".to_string()]);
        assert!(so("unnecessary_library_directive", "/// Doc.\nlibrary a;\n").is_empty());
        assert_eq!(so("combinators_ordering", "import 'dart:math' show max, min;\nimport 'dart:async' hide Timer, Future;\n"), vec!["hide Timer, Future".to_string()]);
    }

    #[test]
    fn comandos_e_expressoes() {
        assert_eq!(so("prefer_asserts_with_message", "void f(int x) {\n  assert(x > 0);\n  assert(x > 0, 'm');\n}\n"), vec!["assert(x > 0);".to_string()]);
        assert_eq!(so("no_self_assignments", "class A {\n  int x = 0;\n  void m(int x, A o) {\n    x = x;\n    this.x = x;\n    o.x = o.x;\n  }\n}\n"), vec!["x = x".to_string(), "o.x = o.x".to_string()]);
        assert_eq!(
            so("unnecessary_breaks", "void f(int x) {\n  switch (x) {\n    case 1:\n      f(2);\n      break;\n    case 2:\n      break;\n  }\n}\n"),
            vec!["break;".to_string()]
        );
        assert_eq!(so("prefer_expression_function_bodies", "int f() {\n  return 1;\n}\nFuture<int> g() async {\n  return 1;\n}\nvoid h() {\n  return;\n}\n"), vec![
            "{\n  return 1;\n}".to_string(),
            "async {\n  return 1;\n}".to_string()
        ]);
        assert_eq!(
            so("prefer_null_aware_operators", "int? f(String? a) => a == null ? null : a.length;\nint? g(String? a) => a != null ? a.length : null;\nString? h(String? a) => a == null ? null : a;\n"),
            vec!["a == null ? null : a.length".to_string(), "a != null ? a.length : null".to_string()]
        );
    }
}
