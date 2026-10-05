//! O décimo segundo lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_if_elements_to_conditional_expressions`,
//! `prefer_for_elements_to_map_fromIterable`, `prefer_final_parameters`,
//! `prefer_foreach` e `prefer_asserts_in_initializer_lists`.
//!
//! Diferenças conhecidas:
//! - `prefer_for_elements_to_map_fromIterable` reconhece o `Map` do
//!   `dart:core` pelo nome escrito.
//! - `prefer_final_parameters` procura a mutação do parâmetro pelo nome no
//!   corpo (atribuição, `++`/`--`, padrão de atribuição, `for (x in …)`).
//! - `prefer_foreach` compara o argumento e o alvo com a variável do laço
//!   pelo nome.
//! - `prefer_asserts_in_initializer_lists` decide "usa a instância" pelo
//!   nome: `this`, ou um identificador com o nome de um membro de instância
//!   da classe (ou das superclasses declaradas na unidade) que nenhum
//!   parâmetro do construtor sombreia. Com uma superclasse ou mixin de fora
//!   da unidade, o construtor não é olhado.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::regras9::variaveis_do_padrao;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CollectionElement, DeclKind, ExprId, ExprKind, ForInTarget, FunctionBody, MemberKind, Parameter, PatternId, PatternKind,
    StmtKind, TypeKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// As mutações por nome da unidade, pela posição: atribuição, `++`/`--`,
/// padrão de atribuição e `for (x in …)`.
fn mutacoes(a: &Ast) -> Vec<(usize, SymbolId)> {
    let nome_de = |e: ExprId| match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(n.sym),
        _ => None,
    };
    let mut v: Vec<(usize, SymbolId)> = Vec::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Assign { target, .. } => v.extend(nome_de(*target).map(|s| (e.span.start, s))),
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                v.extend(nome_de(*operand).map(|s| (e.span.start, s)));
            }
            ExprKind::PatternAssign { pattern, .. } => {
                let mut variaveis: Vec<PatternId> = Vec::new();
                variaveis_do_padrao(a, *pattern, &mut variaveis);
                for p in variaveis {
                    if let PatternKind::Variable { name, .. } = &a.pattern(p).kind {
                        v.push((e.span.start, name.sym));
                    }
                }
            }
            _ => {}
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::ForIn { target: ForInTarget::Expression(x), .. } = &s.kind {
            v.extend(nome_de(*x).map(|n| (s.span.start, n)));
        }
    }
    v.sort_by_key(|x| x.0);
    v
}

/// A região do corpo (vazia no corpo `;`).
fn regiao_do_corpo(a: &Ast, b: &FunctionBody) -> Option<Span> {
    match b {
        FunctionBody::Block(s) => Some(a.stmt(*s).span),
        FunctionBody::Expression(e) => Some(a.expr(*e).span),
        _ => None,
    }
}

/// O corpo de uma única expressão (`=> e`, ou `{ return e; }`).
fn corpo_de_uma_expressao(a: &Ast, b: &FunctionBody) -> bool {
    match b {
        FunctionBody::Expression(_) => true,
        FunctionBody::Block(s) => match &a.stmt(*s).kind {
            StmtKind::Block(l) => matches!(&l[..], [x] if matches!(a.stmt(*x).kind, StmtKind::Return(_))),
            _ => false,
        },
        _ => false,
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

    // `prefer_if_elements_to_conditional_expressions`: `c ? a : b`
    // (talvez entre parênteses) como elemento direto de lista ou conjunto.
    if ligada("prefer_if_elements_to_conditional_expressions") {
        for e in a.exprs.iter() {
            let (elementos, conjunto) = match &e.kind {
                ExprKind::List { elements, .. } => (elements, true),
                ExprKind::SetOrMap { elements, type_args, .. } => {
                    let mapa = type_args.len() == 2
                        || (type_args.is_empty()
                            && (elements.is_empty() || elements.iter().any(|x| matches!(x, CollectionElement::MapEntry { .. }))));
                    (elements, !mapa)
                }
                _ => continue,
            };
            if !conjunto {
                continue;
            }
            for el in elements_iter(elementos) {
                if let CollectionElement::Expression(x) = el
                    && matches!(a.expr(sem_parenteses(a, *x)).kind, ExprKind::Conditional { .. })
                {
                    relatar(&c::PREFER_IF_ELEMENTS_TO_CONDITIONAL_EXPRESSIONS, a.expr(*x).span, &[]);
                }
            }
        }
    }

    // `prefer_for_elements_to_map_fromIterable`.
    if ligada("prefer_for_elements_to_map_fromIterable") {
        let fecho = |nome: &str, arg: &ast::Argument| -> bool {
            if arg.name.is_none_or(|n| interner.resolve(n.sym) != nome) {
                return false;
            }
            let ExprKind::FunctionExpression(f) = a.expr(sem_parenteses(a, arg.value)).kind else { return false };
            let f = a.function(f);
            let um_requerido = f.parameters.as_ref().is_some_and(|ps| {
                ps.len() == 1 && matches!(ps[0].kind, ast::ParameterKind::Required)
            });
            um_requerido && corpo_de_uma_expressao(a, &f.body)
        };
        for e in a.exprs.iter() {
            // `Map.fromIterable(…)` com ou sem `new`.
            let argumentos = match &e.kind {
                ExprKind::InstanceCreation { ty, constructor: Some(nome), arguments, .. }
                    if interner.resolve(nome.sym) == "fromIterable"
                        && matches!(&a.ty(*ty).kind, TypeKind::Named { name, .. } if name.last().is_some_and(|n| interner.resolve(n.sym) == "Map")) =>
                {
                    arguments
                }
                ExprKind::Call { target, arguments } => match &a.expr(*target).kind {
                    ExprKind::Property { target: alvo, name, .. }
                        if interner.resolve(name.sym) == "fromIterable"
                            && matches!(&a.expr(*alvo).kind, ExprKind::Identifier(n) if interner.resolve(n.sym) == "Map") =>
                    {
                        arguments
                    }
                    _ => continue,
                },
                _ => continue,
            };
            let [_, segundo, terceiro] = &argumentos.args[..] else { continue };
            let chave = fecho("key", segundo) || fecho("key", terceiro);
            let valor = fecho("value", terceiro) || fecho("value", segundo);
            if chave && valor {
                relatar(&c::PREFER_FOR_ELEMENTS_TO_MAP_FROMITERABLE, e.span, &[]);
            }
        }
    }

    // `prefer_final_parameters`.
    if ligada("prefer_final_parameters") {
        let muts = mutacoes(a);
        let mutado = |regiao: Span, s: SymbolId| {
            let (de, ate) = (muts.partition_point(|x| x.0 < regiao.start), muts.partition_point(|x| x.0 < regiao.end));
            muts[de..ate.max(de)].iter().any(|x| x.1 == s)
        };
        let mut achados: Vec<(Span, String)> = Vec::new();
        let mut conferir = |ps: &[Parameter], corpo: Option<Span>| {
            for p in ps {
                if p.final_ || p.const_ || p.this_ || p.super_ {
                    continue;
                }
                let Some(n) = p.name else { continue };
                if corpo.is_some_and(|r| mutado(r, n.sym)) {
                    continue;
                }
                // O nó do parâmetro sem o valor padrão.
                let fim = if p.default_value.is_some() && p.function_parameters.is_none() { n.span.end } else { p.span.end };
                achados.push((Span { start: p.span.start, end: fim }, interner.resolve(n.sym).to_string()));
            }
        };
        // Funções de topo, locais, expressões de função e métodos.
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                conferir(ps, regiao_do_corpo(a, &f.body));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && !k.parte_primaria
            {
                conferir(&k.parameters, regiao_do_corpo(a, &k.body));
            }
        }
        achados.sort_by_key(|x| (x.0.start, x.0.end));
        achados.dedup_by_key(|x| x.0);
        for (s, nome) in achados {
            relatar(&c::PREFER_FINAL_PARAMETERS, s, &[&nome]);
        }
    }

    // `prefer_foreach`: `for (final x in e) f(x);`.
    if ligada("prefer_foreach") {
        let identificador = |e: ExprId, s: SymbolId| matches!(&a.expr(e).kind, ExprKind::Identifier(n) if n.sym == s);
        let cita = |e: ExprId, s: SymbolId| {
            let r = a.expr(e).span;
            a.exprs.iter().any(|x| matches!(&x.kind, ExprKind::Identifier(n) if n.sym == s) && dentro(x.span, r))
        };
        for st in a.stmts.iter() {
            let StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, body, .. } = &st.kind else { continue };
            let mut corpo = *body;
            if let StmtKind::Block(l) = &a.stmt(corpo).kind {
                let [unico] = &l[..] else { continue };
                corpo = *unico;
            }
            let StmtKind::Expression(x) = &a.stmt(corpo).kind else { continue };
            let ExprKind::Call { target, arguments } = &a.expr(sem_parenteses(a, *x)).kind else { continue };
            let [arg] = &arguments.args[..] else { continue };
            if arg.name.is_some() || !identificador(arg.value, name.sym) {
                continue;
            }
            // Na chamada de método, o alvo não pode citar a variável.
            let alvo_cita = match &a.expr(*target).kind {
                ExprKind::Property { target: alvo, .. } => cita(*alvo, name.sym),
                _ => false,
            };
            if !alvo_cita {
                relatar(&c::PREFER_FOREACH, st.span, &[]);
            }
        }
    }

    // `prefer_asserts_in_initializer_lists`.
    if ligada("prefer_asserts_in_initializer_lists") {
        // As classes da unidade, pelo nome.
        let classes: std::collections::HashMap<SymbolId, &ast::ClassDecl> = a
            .decls
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Class(x) => Some((x.name.sym, x)),
                _ => None,
            })
            .collect();
        let nome_do_tipo = |t: ast::TypeId| match &a.ty(t).kind {
            TypeKind::Named { name, .. } if name.len() == 1 => Some(name[0].sym),
            _ => None,
        };
        for d in a.decls.iter() {
            let DeclKind::Class(classe) = &d.kind else { continue };
            // Os membros de instância da classe e das superclasses e mixins
            // declarados na unidade; `None` com algum de fora.
            let mut membros: std::collections::HashSet<SymbolId> = std::collections::HashSet::new();
            let mut fila: Vec<&ast::ClassDecl> = vec![classe];
            let mut vistos = std::collections::HashSet::new();
            let mut de_fora = false;
            while let Some(x) = fila.pop() {
                if !vistos.insert(x.name.sym) {
                    continue;
                }
                for &m in &x.members {
                    match &a.member(m).kind {
                        MemberKind::Field(v) if !v.static_ => membros.extend(v.variables.iter().map(|w| w.name.sym)),
                        MemberKind::Method(f) => {
                            let f = a.function(*f);
                            if !f.static_ && let Some(n) = f.name {
                                membros.insert(n.sym);
                            }
                        }
                        _ => {}
                    }
                }
                for t in x.extends.iter().chain(x.with.iter()) {
                    match nome_do_tipo(*t).and_then(|s| classes.get(&s)) {
                        Some(sup) => fila.push(*sup),
                        None => de_fora = true,
                    }
                }
            }
            if de_fora {
                continue;
            }
            for &m in &classe.members {
                let MemberKind::Constructor(k) = &a.member(m).kind else { continue };
                if k.factory {
                    continue;
                }
                let FunctionBody::Block(b) = &k.body else { continue };
                let StmtKind::Block(comandos) = &a.stmt(*b).kind else { continue };
                let parametros: std::collections::HashSet<SymbolId> = k.parameters.iter().filter_map(|p| p.name.map(|n| n.sym)).collect();
                for &s in comandos.iter() {
                    if !matches!(a.stmt(s).kind, StmtKind::Assert { .. }) {
                        break;
                    }
                    let r = a.stmt(s).span;
                    let usa_instancia = a.exprs.iter().filter(|e| dentro(e.span, r)).any(|e| match &e.kind {
                        ExprKind::This => true,
                        ExprKind::Identifier(n) => membros.contains(&n.sym) && !parametros.contains(&n.sym),
                        _ => false,
                    });
                    if !usa_instancia {
                        relatar(&c::PREFER_ASSERTS_IN_INITIALIZER_LISTS, Span { start: r.start, end: r.start + "assert".len() }, &[]);
                    }
                }
            }
        }
    }

    let _ = fonte;
    out
}

/// Os elementos diretos de um literal (os de `if` e `for` não são diretos).
fn elements_iter(l: &[CollectionElement]) -> impl Iterator<Item = &CollectionElement> {
    l.iter()
}
