//! O décimo quinto lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), do conjunto
//! `core`, pela semântica da unidade:
//!
//! * `implicit_call_tearoffs`: as expressões que o resolvedor troca por um
//!   `ImplicitCallReference` (`UnitBodyTypes::chamadas_implicitas`), fora as
//!   formas que o `_shouldSkipImplicitCallReferenceDueToForm` pula (alvo de
//!   cascata, ramo de `?:`, operando de `??`, através de parênteses).
//! * `prefer_iterable_whereType`: `alvo.where((x) => x is T)` com o alvo
//!   (o `realTarget`) de tipo de interface que é ou implementa o `Iterable`
//!   do `dart:core`.
//! * `null_check_on_nullable_type_parameter`: o `!` (pós-fixo e de padrão)
//!   sobre um parâmetro de tipo anulável, com o `getExpectedType` de
//!   `unnecessary_null_checks.dart` (retorno e corpo de expressão pela
//!   `FunctionExpression` mais próxima, que um método não é; `yield`;
//!   atribuição `=`; declaração de variável; operando direito de binário;
//!   elemento direto de lista e de conjunto; entrada de mapa; argumento, pelo
//!   `staticParameterElement`), e o `isNullable`/`isNonNullable` sintáticos
//!   do `TypeSystemImpl`.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionRef, VariableRef};
use dartforge_frontend::ast::{
    AssignOp, AsyncModifier, Ast, BinaryOp, CollectionElement, DeclKind, ExprId, ExprKind, ForInit, FunctionBody, FunctionId, MemberKind,
    PatternId, PatternKind, StmtKind, UnaryOp,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId, TypeTable};
use std::collections::HashMap;

/// `TypeSystemImpl.isNullable` (sintático).
fn anulavel(table: &TypeTable, core: &dartforge_types::table::CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => anulavel(table, core, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, core, *arg),
        _ => t == core.unknown,
    }
}

/// `TypeSystemImpl.isNonNullable` (sintático).
fn nao_anulavel(table: &TypeTable, core: &dartforge_types::table::CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => nao_anulavel(table, core, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && nao_anulavel(table, core, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && nao_anulavel(table, core, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        _ => t != core.unknown,
    }
}

/// `promoteToNonNull(a) == promoteToNonNull(b)` quando `a` é parâmetro de
/// tipo: o mesmo parâmetro (e o mesmo limite promovido) sem o `?`.
fn mesmo_sem_nulo(table: &TypeTable, a: TypeId, b: TypeId) -> bool {
    let chave = |t: TypeId| match table.get(t) {
        Type::TypeParameter { param, .. } => Some((*param, None)),
        Type::Intersection { param, bound } => Some((*param, Some(*bound))),
        _ => None,
    };
    match (chave(a), chave(b)) {
        (Some((pa, ba)), Some((pb, bb))) => pa == pb && ba == bb,
        _ => false,
    }
}

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

/// A função (`FunctionExpression` do analyzer) e o tipo dela: literal,
/// local ou de topo (o método não é `FunctionExpression`).
pub(super) struct Funcoes {
    literais: HashMap<FunctionId, ExprId>,
    locais: HashMap<FunctionId, usize>,
    de_topo: HashMap<FunctionId, dartforge_elements::model::FunctionElementId>,
    metodos: std::collections::HashSet<FunctionId>,
}

impl Funcoes {
    pub(super) fn de(s: &super::Semantica<'_>, a: &Ast) -> Funcoes {
        let mut f = Funcoes { literais: HashMap::new(), locais: HashMap::new(), de_topo: HashMap::new(), metodos: Default::default() };
        for (k, e) in a.exprs.iter().enumerate() {
            if let ExprKind::FunctionExpression(g) = &e.kind {
                f.literais.insert(*g, ExprId(k as u32));
            }
        }
        for st in a.stmts.iter() {
            if let StmtKind::Function(g) = &st.kind
                && let Some(n) = a.function(*g).name
            {
                f.locais.insert(*g, n.span.start);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(g) = &m.kind {
                f.metodos.insert(*g);
            }
        }
        for (k, fe) in s.program.functions.iter().enumerate() {
            if let FunctionRef::Function { unit, function } = fe.node
                && unit == s.unidade
                && !f.metodos.contains(&function)
            {
                f.de_topo.insert(function, dartforge_elements::model::FunctionElementId(k as u32));
            }
        }
        f
    }

    /// O tipo estático da `FunctionExpression` de `g`, se `g` é uma.
    fn tipo(&self, s: &super::Semantica<'_>, g: FunctionId) -> Option<Option<TypeId>> {
        if self.metodos.contains(&g) {
            return None;
        }
        if let Some(&e) = self.literais.get(&g) {
            return Some(s.corpo.get_type(e));
        }
        if let Some(&pos) = self.locais.get(&g) {
            return Some(s.corpo.tipo_local(pos));
        }
        if let Some(fe) = self.de_topo.get(&g) {
            return Some(s.outline.functions.get(fe.0 as usize).map(|d| d.signature));
        }
        None
    }
}

/// O tipo declarado do elemento de uma variável (`declaredElement.type`).
fn tipo_da_variavel(s: &super::Semantica<'_>, ref_: VariableRef, nome_pos: usize, inicializador: ExprId) -> Option<TypeId> {
    let v = s.program.variables.iter().position(|x| x.node == ref_)?;
    let d = s.outline.variables.get(v)?;
    d.declared_type.or(d.inferred).or_else(|| s.corpo.get_type(inicializador)).or_else(|| s.corpo.tipo_local(nome_pos))
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let algum = ["implicit_call_tearoffs", "prefer_iterable_whereType", "null_check_on_nullable_type_parameter"].iter().any(|r| ligada(r));
    if !algum {
        return out;
    }
    // O pai de cada expressão e os ancestrais dos `!`.
    let mut pai: HashMap<ExprId, No> = HashMap::new();
    let mut ancestrais_do_bang: HashMap<ExprId, Vec<No>> = HashMap::new();
    andar(u, &mut |no, ancestrais| {
        if let No::Expr(e) = no {
            if let Some(p) = ancestrais.last() {
                pai.insert(e, *p);
            }
            if matches!(a.expr(e).kind, ExprKind::Unary { op: UnaryOp::NullAssert, .. }) {
                ancestrais_do_bang.insert(e, ancestrais.to_vec());
            }
        }
    });
    let pai_expr = |e: ExprId| match pai.get(&e) {
        Some(No::Expr(p)) => Some(*p),
        _ => None,
    };
    // O `realTarget` de uma seção de cascata.
    let mut cascatas: HashMap<ExprId, ExprId> = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &sec in sections.iter() {
            let mut x = sec;
            loop {
                let prox = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        cascatas.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = prox;
            }
        }
    }
    let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascatas.get(&x).copied() } else { Some(x) };
    let table = s.table;
    let core = s.core;

    // `implicit_call_tearoffs`.
    if ligada("implicit_call_tearoffs") {
        let mut implicitas: Vec<ExprId> = s.corpo.chamadas_implicitas.iter().copied().collect();
        implicitas.sort_by_key(|e| (a.expr(*e).span.start, a.expr(*e).span.end));
        for e in implicitas {
            let mut x = e;
            let mut p = pai_expr(x);
            while let Some(q) = p
                && matches!(a.expr(q).kind, ExprKind::Parenthesized(_))
            {
                x = q;
                p = pai_expr(q);
            }
            let pula = p.is_some_and(|q| match &a.expr(q).kind {
                ExprKind::Cascade { target, .. } => *target == x,
                ExprKind::Conditional { then, else_, .. } => *then == x || *else_ == x,
                ExprKind::Binary { op: BinaryOp::IfNull, .. } => true,
                _ => false,
            });
            if !pula {
                relatar(&c::IMPLICIT_CALL_TEAROFFS, a.expr(e).span, &[]);
            }
        }
    }

    // `prefer_iterable_whereType`.
    if ligada("prefer_iterable_whereType") {
        let implementa_iterable = |t: TypeId| match table.get(t) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                let e_iterable = |k: dartforge_elements::model::ClassId| {
                    let cl = s.program.class(k);
                    interner.resolve(cl.name) == "Iterable" && s.program.library(cl.library).uri == "dart:core"
                };
                e_iterable(*class)
                    || (s.program.class(*class).decl.is_some()
                        && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| e_iterable(*k))))
            }
            _ => false,
        };
        for e in a.exprs.iter() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
            if interner.resolve(name.sym) != "where" {
                continue;
            }
            let Some(alvo) = real(*alvo) else { continue };
            if !s.corpo.get_type(alvo).is_some_and(implementa_iterable) {
                continue;
            }
            let [arg] = &arguments.args[..] else { continue };
            let ExprKind::FunctionExpression(f) = &a.expr(arg.value).kind else { continue };
            let f = a.function(*f);
            let Some(ps) = &f.parameters else { continue };
            let [p] = &ps[..] else { continue };
            let expressao = match &f.body {
                FunctionBody::Block(b) => match &a.stmt(*b).kind {
                    StmtKind::Block(cmds) if cmds.len() == 1 => match &a.stmt(cmds[0]).kind {
                        StmtKind::Return(v) => *v,
                        _ => None,
                    },
                    _ => None,
                },
                FunctionBody::Expression(x) => Some(*x),
                _ => None,
            };
            let Some(x) = expressao.map(|x| sem_parenteses(a, x)) else { continue };
            if let ExprKind::Is { value, negated: false, .. } = &a.expr(x).kind
                && let ExprKind::Identifier(n) = &a.expr(*value).kind
                && p.name.is_some_and(|pn| pn.sym == n.sym)
            {
                relatar(&c::PREFER_ITERABLE_WHERETYPE, name.span, &[]);
            }
        }
    }

    // `null_check_on_nullable_type_parameter`.
    if ligada("null_check_on_nullable_type_parameter") {
        let e_parametro_anulavel = |t: TypeId| matches!(table.get(t), Type::TypeParameter { .. } | Type::Intersection { .. }) && anulavel(table, core, t);
        // O padrão `p!`: o tipo casado.
        for (k, p) in a.patterns.iter().enumerate() {
            if let PatternKind::NullAssert(_) = &p.kind
                && let Some(&t) = s.corpo.tipos_casados.get(&PatternId(k as u32))
                && e_parametro_anulavel(t)
            {
                relatar(&c::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER, Span { start: p.span.end - 1, end: p.span.end }, &[]);
            }
        }
        let funcoes = Funcoes::de(s, a);
        let mut bangs: Vec<(&ExprId, &Vec<No>)> = ancestrais_do_bang.iter().collect();
        bangs.sort_by_key(|(e, _)| a.expr(**e).span.start);
        for (&no, ancestrais) in bangs {
            let ExprKind::Unary { operand, .. } = &a.expr(no).kind else { continue };
            let Some(t) = s.corpo.get_type(*operand) else { continue };
            if !e_parametro_anulavel(t) {
                continue;
            }
            let esperado = tipo_esperado(s, a, interner, &funcoes, &pai, ancestrais, no, &real);
            if let Some(esp) = esperado
                && !nao_anulavel(table, core, esp)
                && mesmo_sem_nulo(table, t, esp)
            {
                let sp = a.expr(no).span;
                relatar(&c::NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER, Span { start: sp.end - 1, end: sp.end }, &[]);
            }
        }
    }

    out
}

/// O argumento de tipo `i` de um tipo de interface (de `Future`, `Iterable`…).
fn argumento(table: &TypeTable, t: TypeId, i: usize) -> Option<TypeId> {
    match table.get(t) {
        Type::Interface { args, .. } => args.get(i).copied(),
        Type::FutureOr { arg, .. } if i == 0 => Some(*arg),
        _ => None,
    }
}

/// O argumento `i` de `receptor.asInstanceOf(dc)`, quando o receptor é de
/// interface: direto na própria classe; pelo supertipo instanciado em termos
/// dos parâmetros da classe do receptor, nos outros casos.
fn argumento_da_superclasse(s: &super::Semantica<'_>, receptor: TypeId, dc: ClassId, i: usize) -> Option<TypeId> {
    let table = s.table;
    let (rc, args) = match table.get(receptor) {
        Type::Interface { class, args, .. } => (*class, args.to_vec()),
        _ => return None,
    };
    if rc == dc {
        return args.get(i).copied();
    }
    let dados = s.outline.hierarchy.get(rc)?;
    let molde = *dados.supertypes.get(&dc)?;
    let Type::Interface { args: margs, .. } = table.get(molde) else { return None };
    let m = *margs.get(i)?;
    match table.get(m) {
        Type::TypeParameter { param, .. } => dados.type_params.iter().position(|q| q == param).and_then(|j| args.get(j).copied()),
        _ => Some(m),
    }
}

/// `getExpectedType` (`unnecessary_null_checks.dart:15`).
#[allow(clippy::too_many_arguments)]
pub(super) fn tipo_esperado(
    s: &super::Semantica<'_>,
    a: &Ast,
    interner: &Interner,
    funcoes: &Funcoes,
    pai: &HashMap<ExprId, No>,
    ancestrais: &[No],
    no: ExprId,
    real_alvo: &dyn Fn(ExprId) -> Option<ExprId>,
) -> Option<TypeId> {
    let table = s.table;
    let core = s.core;
    let classe_e = |t: TypeId, nome: &str, lib: &str| match table.get(t) {
        Type::Interface { class, .. } => {
            let cl = s.program.class(*class);
            interner.resolve(cl.name) == nome && s.program.library(cl.library).uri == lib
        }
        _ => false,
    };
    // `realNode`: o parêntese mais de fora.
    let mut real = no;
    while let Some(No::Expr(p)) = pai.get(&real)
        && matches!(a.expr(*p).kind, ExprKind::Parenthesized(_))
    {
        real = *p;
    }
    let mut filho = real;
    let mut pai_no = pai.get(&real).copied();
    let com_await = matches!(pai_no, Some(No::Expr(p)) if matches!(a.expr(p).kind, ExprKind::Await(_)));
    if com_await && let Some(No::Expr(p)) = pai_no {
        filho = p;
        pai_no = pai.get(&p).copied();
    }
    // A `FunctionExpression` mais próxima (o método não é uma).
    let funcao_mais_proxima = || -> Option<(FunctionId, Option<TypeId>)> {
        for n in ancestrais.iter().rev() {
            if let No::Funcao(g) = n {
                return funcoes.tipo(s, *g).map(|t| (*g, t));
            }
        }
        None
    };
    let retorno_de = |t: Option<TypeId>| -> Option<TypeId> {
        match table.get(t?) {
            Type::Function { ret, .. } => Some(*ret),
            _ => None,
        }
    };
    // Retorno e corpo de expressão.
    let e_retorno = match pai_no {
        Some(No::Stmt(st)) => matches!(a.stmt(st).kind, StmtKind::Return(Some(v)) if v == filho),
        Some(No::Funcao(g)) => matches!(a.function(g).body, FunctionBody::Expression(v) if v == filho),
        _ => false,
    };
    if e_retorno {
        let (g, t) = funcao_mais_proxima()?;
        let ret = retorno_de(t)?;
        if com_await || matches!(a.function(g).modifier, AsyncModifier::Async | AsyncModifier::AsyncStar) {
            return if classe_e(ret, "Future", "dart:async") || matches!(table.get(ret), Type::FutureOr { .. }) { argumento(table, ret, 0) } else { None };
        }
        return Some(ret);
    }
    // `yield`.
    if let Some(No::Stmt(st)) = pai_no
        && matches!(a.stmt(st).kind, StmtKind::Yield { .. })
    {
        let (_, t) = funcao_mais_proxima()?;
        let ret = retorno_de(t)?;
        return if classe_e(ret, "Iterable", "dart:core") || classe_e(ret, "Stream", "dart:async") { argumento(table, ret, 0) } else { None };
    }
    let Some(pai_no) = pai_no else { return None };
    match pai_no {
        No::Expr(p) => match &a.expr(p).kind {
            // Atribuição `=` (fora `x = x!`).
            ExprKind::Assign { op: AssignOp::Assign, target, .. } => {
                let nome = |e: ExprId| -> Option<String> {
                    match &a.expr(e).kind {
                        ExprKind::Identifier(n) => Some(interner.resolve(n.sym).to_string()),
                        ExprKind::Property { target, name, null_aware: false } => match &a.expr(*target).kind {
                            ExprKind::Identifier(m) => Some(format!("{}.{}", interner.resolve(m.sym), interner.resolve(name.sym))),
                            _ => None,
                        },
                        _ => None,
                    }
                };
                let ExprKind::Unary { operand, .. } = &a.expr(no).kind else { return None };
                match (nome(*target), nome(*operand)) {
                    (Some(x), Some(y)) if x == y => None,
                    _ => s.corpo.tipos_de_escrita.get(&p).copied(),
                }
            }
            // Operando direito de binário: o primeiro parâmetro do operador.
            ExprKind::Binary { right, left, .. } if *right == real => {
                let Some(Resolved::Member { member: dartforge_types::resolved::MemberRef::Function(f), .. }) = s.corpo.get_resolved(p) else {
                    return None;
                };
                let sig = s.outline.functions.get(f.0 as usize)?.signature;
                let Type::Function { positional, .. } = table.get(sig) else { return None };
                let declarado = *positional.first()?;
                // O membro do tipo do receptor (`MethodMember`): um parâmetro
                // de tipo da classe que declara o operador vira o argumento
                // correspondente do receptor (o resto não pode ser igual a um
                // parâmetro de tipo do `!`).
                let dono = s.program.function(*f).class;
                match (table.get(declarado), dono, s.corpo.get_type(*left)) {
                    (Type::TypeParameter { param, .. }, Some(dc), Some(receptor)) => {
                        let i = s.outline.classes.get(dc.0 as usize).and_then(|d| d.type_params.iter().position(|q| q == param));
                        match i {
                            Some(i) => argumento_da_superclasse(s, receptor, dc, i).or(Some(declarado)),
                            None => Some(declarado),
                        }
                    }
                    _ => Some(declarado),
                }
            }
            // Elemento direto de lista e de conjunto.
            ExprKind::List { elements, .. } => {
                if !elements.iter().any(|el| matches!(el, CollectionElement::Expression(x) if *x == real)) {
                    return None;
                }
                argumento(table, s.corpo.get_type(p)?, 0)
            }
            ExprKind::SetOrMap { elements, .. } => {
                if elements.iter().any(|el| matches!(el, CollectionElement::Expression(x) if *x == real)) {
                    let t = s.corpo.get_type(p)?;
                    return if classe_e(t, "Set", "dart:core") { argumento(table, t, 0) } else { None };
                }
                // A entrada de mapa (também dentro de `for`/`if`).
                fn entrada(els: &[CollectionElement], x: ExprId) -> Option<(ExprId, ExprId)> {
                    for el in els {
                        let achado = match el {
                            CollectionElement::MapEntry { key, value, .. } if *key == x || *value == x => Some((*key, *value)),
                            CollectionElement::If { then, else_, .. } => {
                                entrada(std::slice::from_ref(then), x).or_else(|| else_.as_ref().and_then(|e| entrada(std::slice::from_ref(e), x)))
                            }
                            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => entrada(std::slice::from_ref(body), x),
                            _ => None,
                        };
                        if achado.is_some() {
                            return achado;
                        }
                    }
                    None
                }
                let (chave, _) = entrada(elements, real)?;
                let t = s.corpo.get_type(p)?;
                argumento(table, t, if chave == no { 0 } else { 1 })
            }
            // Argumento (posicional ou nomeado).
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } if arguments.args.iter().any(|x| x.value == real) => {
                if let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(p) {
                    // `Future.value`.
                    let g = s.program.function(*f);
                    if let Some(cl) = g.class
                        && interner.resolve(g.name) == "value"
                    {
                        let k = s.program.class(cl);
                        if interner.resolve(k.name) == "Future" && s.program.library(k.library).uri == "dart:async" {
                            return None;
                        }
                    }
                } else if let ExprKind::Call { target, .. } = &a.expr(p).kind
                    && let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind
                    && interner.resolve(name.sym) == "complete"
                    && let Some(alvo) = real_alvo(*alvo)
                    && let Some(t) = s.corpo.get_type(alvo)
                    && classe_e(t, "Completer", "dart:async")
                {
                    return None;
                }
                s.corpo.tipos_de_parametros.get(&real).copied()
            }
            _ => None,
        },
        // Declaração de variável.
        No::Stmt(st) => {
            let lista = match &a.stmt(st).kind {
                StmtKind::Variables(l) => l,
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => l,
                _ => return None,
            };
            let v = lista.variables.iter().find(|v| v.initializer == Some(real))?;
            s.corpo.tipo_local(v.name.span.start).or_else(|| s.corpo.get_type(real)).filter(|t| *t != core.unknown)
        }
        No::Membro(m) => {
            let MemberKind::Field(l) = &a.member(m).kind else { return None };
            let index = l.variables.iter().position(|v| v.initializer == Some(real))?;
            tipo_da_variavel(s, VariableRef::Field { unit: s.unidade, member: m, index }, l.variables[index].name.span.start, real)
        }
        No::Decl(d) => {
            let DeclKind::Variables(l) = &a.decl(d).kind else { return None };
            let index = l.variables.iter().position(|v| v.initializer == Some(real))?;
            tipo_da_variavel(s, VariableRef::TopLevel { unit: s.unidade, decl: d, index }, l.variables[index].name.span.start, real)
        }
        _ => None,
    }
}
