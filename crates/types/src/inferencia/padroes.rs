//! Padrões (Dart 3): esquema de tipo de um padrão (contexto do valor
//! casado), tipagem dos subpadrões pelo tipo casado, variáveis de padrão,
//! `switch` como expressão, `if-case`, declarações e atribuições por padrão.

use super::corpo::{Corpo, Local, Nome};
use super::expr::{self, declarar_local, inferir, inferir_livre};
use super::fluxo::Fluxo;
use super::membros::Busca;
use super::BodyInferrer;
use crate::constraints::GenericInferrer;
use crate::table::{Type, TypeId};
use dartforge_elements::model::Element;
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};

/// Esquema de tipo de um padrão (`_` onde nada restringe).
pub(crate) fn esquema(inf: &mut BodyInferrer<'_>, cx: &Corpo, p: PatternId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let u = inf.core.unknown;
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty } | PatternKind::Variable { ty, .. } => match ty {
            Some(t) => inf.tipo_de_anotacao(cx, *t),
            None => u,
        },
        PatternKind::Constant(_) | PatternKind::Relational { .. } | PatternKind::Cast { .. } | PatternKind::Or(..) => u,
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            let a1 = esquema(inf, cx, x);
            let a2 = esquema(inf, cx, y);
            if inf.e_desconhecido(a1) {
                a2
            } else {
                a1
            }
        }
        PatternKind::NullCheck(x) => {
            let s = esquema(inf, cx, *x);
            if inf.e_desconhecido(s) {
                s
            } else {
                inf.anulavel(s)
            }
        }
        PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => esquema(inf, cx, *x),
        PatternKind::List { type_args, elements } => {
            let el = if let Some(t) = type_args.first() {
                inf.tipo_de_anotacao(cx, *t)
            } else {
                let mut acc = u;
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) = e {
                        let s = esquema(inf, cx, *x);
                        acc = inf.down(acc, s);
                    }
                }
                acc
            };
            inf.lista(el)
        }
        PatternKind::Map { type_args, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.tipo_de_anotacao(cx, type_args[0]), inf.tipo_de_anotacao(cx, type_args[1]))
            } else {
                (u, u)
            };
            inf.tipo_mapa(k, v)
        }
        PatternKind::Record { fields } => {
            let mut pos = Vec::new();
            let mut nm = Vec::new();
            let fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            for (n, x) in fields {
                let s = esquema(inf, cx, x);
                match n {
                    Some(n) => nm.push((n.sym, s)),
                    None => match nome_implicito(inf, cx, x) {
                        Some(sym) if campo_nomeado_implicito(inf, cx, p, x) => nm.push((sym, s)),
                        _ => pos.push(s),
                    },
                }
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        PatternKind::Object { ty, .. } => {
            let t = inf.tipo_de_anotacao(cx, *ty);
            t
        }
    }
}

/// `:x` num campo nomeado de record/objeto usa o nome da variável.
fn nome_implicito(inf: &BodyInferrer<'_>, cx: &Corpo, p: PatternId) -> Option<dartforge_intern::SymbolId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.pattern(p).kind {
        PatternKind::Variable { name, .. } => Some(name.sym),
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => nome_implicito(inf, cx, *x),
        PatternKind::Cast { pattern, .. } => nome_implicito(inf, cx, *pattern),
        _ => None,
    }
}

/// O campo foi escrito como `:x` (nome omitido mas com dois-pontos): o parser
/// registra `name = None` com o span do campo começando em `:`.
fn campo_nomeado_implicito(inf: &BodyInferrer<'_>, cx: &Corpo, pai: PatternId, filho: PatternId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    let fields = match &a.pattern(pai).kind {
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields,
        _ => return false,
    };
    fields.iter().any(|f| f.pattern == filho && f.name.is_none() && inf.program.unit(cx.unit).source.as_bytes().get(f.span.start) == Some(&b':'))
}

/// Tipa o padrão `p` contra o tipo casado `t`, declarando variáveis.
/// `decl`: contexto de declaração (variáveis sem tipo são declaradas com o
/// tipo casado); em atribuição, os identificadores são variáveis existentes.
fn tipar(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, t: TypeId, final_: bool, atribuicao: bool) {
    let a = &inf.program.unit(cx.unit).ast;
    let u = inf.core.unknown;
    match &a.pattern(p).kind {
        PatternKind::Wildcard { .. } => {}
        PatternKind::Variable { final_: f2, ty, name, .. } => {
            let (f2, ty, name) = (*f2, *ty, *name);
            if atribuicao {
                if let Some(Nome::Local(id)) = cx.buscar(name.sym) {
                    let decl = cx.local(id).tipo;
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    inf.atribuir_fluxo(&mut f, id, decl, t);
                    cx.fluxo = f;
                }
                return;
            }
            let tipo = match ty {
                Some(x) => inf.tipo_de_anotacao(cx, x),
                None => t,
            };
            declarar_local(inf, cx, Local { nome: name.sym, tipo, final_: final_ || f2, late: false, const_: false, offset: name.span.start, funcao_local: false }, true);
        }
        PatternKind::Constant(e) => {
            inferir(inf, cx, *e, t);
        }
        PatternKind::Relational { op, value } => {
            let (op, value) = (*op, *value);
            let nome = match op {
                ast::BinaryOp::Eq | ast::BinaryOp::NotEq => "==",
                ast::BinaryOp::Lt => "<",
                ast::BinaryOp::Gt => ">",
                ast::BinaryOp::LtEq => "<=",
                ast::BinaryOp::GtEq => ">=",
                _ => "",
            };
            let ctx = match inf.interner.lookup(nome).map(|s| inf.buscar_membro(cx.lib, t, s, false)) {
                Some(Busca::Achado(m)) => match inf.table.get(m.tipo) {
                    Type::Function { positional, .. } => positional.first().copied().unwrap_or(u),
                    _ => u,
                },
                _ => u,
            };
            inferir(inf, cx, value, ctx);
        }
        PatternKind::Or(x, y) => {
            let (x, y) = (*x, *y);
            tipar(inf, cx, x, t, final_, atribuicao);
            tipar(inf, cx, y, t, final_, atribuicao);
        }
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            tipar(inf, cx, x, t, final_, atribuicao);
            tipar(inf, cx, y, t, final_, atribuicao);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) => {
            let x = *x;
            let nn = inf.nao_nulo(t);
            tipar(inf, cx, x, nn, final_, atribuicao);
        }
        PatternKind::Parenthesized(x) => {
            let x = *x;
            tipar(inf, cx, x, t, final_, atribuicao);
        }
        PatternKind::Cast { pattern, ty } => {
            let (pattern, ty) = (*pattern, *ty);
            let c = inf.tipo_de_anotacao(cx, ty);
            tipar(inf, cx, pattern, c, final_, atribuicao);
        }
        PatternKind::List { type_args, elements } => {
            let el = if let Some(x) = type_args.first() {
                inf.tipo_de_anotacao(cx, *x)
            } else if inf.e_dynamic(t) {
                inf.core.dynamic_
            } else {
                inf.como_instancia_de(t, inf.core.list_class).map(|a| a[0]).unwrap_or(inf.core.object_nullable)
            };
            let els: Vec<ListPatternElement> = elements.iter().map(|e| match e {
                ListPatternElement::Pattern(x) => ListPatternElement::Pattern(*x),
                ListPatternElement::Rest(x) => ListPatternElement::Rest(*x),
            }).collect();
            for e in els {
                match e {
                    ListPatternElement::Pattern(x) => tipar(inf, cx, x, el, final_, atribuicao),
                    ListPatternElement::Rest(Some(x)) => {
                        let l = inf.lista(el);
                        tipar(inf, cx, x, l, final_, atribuicao);
                    }
                    ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { type_args, entries, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.tipo_de_anotacao(cx, type_args[0]), inf.tipo_de_anotacao(cx, type_args[1]))
            } else if inf.e_dynamic(t) {
                (inf.core.dynamic_, inf.core.dynamic_)
            } else {
                match inf.como_instancia_de(t, inf.core.map_class) {
                    Some(a) => (a[0], a[1]),
                    None => (inf.core.object_nullable, inf.core.object_nullable),
                }
            };
            let es: Vec<(ExprId, PatternId)> = entries.iter().map(|e| (e.key, e.value)).collect();
            for (key, val) in es {
                inferir(inf, cx, key, k);
                tipar(inf, cx, val, v, final_, atribuicao);
            }
        }
        PatternKind::Record { fields } => {
            let fields: Vec<(Option<ast::Name>, PatternId, dartforge_diagnostics::Span)> = fields.iter().map(|f| (f.name, f.pattern, f.span)).collect();
            let tnn = inf.nao_nulo(t);
            let rec = match inf.table.get(tnn).clone() {
                Type::Record { positional, named, .. } => Some((positional, named)),
                _ => None,
            };
            let mut ipos = 0;
            for (n, x, _) in fields {
                let nome = n.map(|n| n.sym).or_else(|| if campo_nomeado_implicito(inf, cx, p, x) { nome_implicito(inf, cx, x) } else { None });
                let ft = match (&rec, nome) {
                    (Some((_, named)), Some(nm)) => named.iter().find(|(s, _)| *s == nm).map(|(_, t)| *t),
                    (Some((pos, _)), None) => {
                        let r = pos.get(ipos).copied();
                        ipos += 1;
                        r
                    }
                    _ => None,
                };
                let ft = ft.unwrap_or(if inf.e_dynamic(t) { inf.core.dynamic_ } else { inf.core.object_nullable });
                tipar(inf, cx, x, ft, final_, atribuicao);
            }
        }
        PatternKind::Object { ty, fields } => {
            let ty = *ty;
            let fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            let obj = tipo_do_padrao_objeto(inf, cx, ty, t);
            for (n, x) in fields {
                let nome = n.map(|n| n.sym).or_else(|| nome_implicito(inf, cx, x));
                let ft = match nome {
                    Some(nm) => match inf.buscar_membro(cx.lib, obj, nm, false) {
                        Busca::Achado(m) => m.tipo,
                        Busca::Nunca => inf.core.never,
                        _ => inf.core.dynamic_,
                    },
                    None => inf.core.dynamic_,
                };
                tipar(inf, cx, x, ft, final_, atribuicao);
            }
        }
    }
}

/// Tipo de um padrão objeto `C(...)`: com `C` genérico cru, os argumentos
/// vêm do tipo casado (`Option<int>` casado com `Some()` → `Some<int>`).
fn tipo_do_padrao_objeto(inf: &mut BodyInferrer<'_>, cx: &Corpo, ty: ast::TypeId, casado: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let node = a.ty(ty);
    if let ast::TypeKind::Named { name, args } = &node.kind {
        if args.is_empty() {
            let binding = if name.len() == 2 {
                inf.program.lookup_prefixed(cx.lib, name[0].sym, name[1].sym)
            } else {
                inf.program.lookup(cx.lib, name[0].sym)
            };
            if let Some(Element::Class(c)) = binding.and_then(|b| b.getter) {
                let params = inf.outline.classes[c.0 as usize].type_params.clone();
                if !params.is_empty() && !inf.e_dynamic(casado) {
                    let this = inf.tipo_this_classe(c);
                    let mut gi = GenericInferrer::new(&params);
                    let mut env = inf.env();
                    gi.constrain_return(this, casado, &mut env);
                    let args = gi.choose_final(&mut env);
                    drop(env);
                    let mapa = inf.mapa(&params, &args);
                    let t = inf.subst(this, &mapa);
                    return if node.nullable { inf.anulavel(t) } else { t };
                }
            }
        }
    }
    inf.tipo_de_anotacao(cx, ty)
}

/// `case p when g`: tipa o padrão e a guarda, devolve `(casou, não casou)`.
pub(crate) fn caso(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, t: TypeId, guarda: Option<ExprId>) -> (Fluxo, Fluxo) {
    let antes = cx.fluxo.clone();
    tipar(inf, cx, p, t, false, false);
    let mut sim = cx.fluxo.clone();
    let mut nao = antes;
    if let Some(g) = guarda {
        let (gv, gf) = expr::condicao_verificada(inf, cx, g);
        sim = gv;
        nao = inf.juntar(&nao, &gf);
    }
    cx.fluxo = sim.clone();
    (sim, nao)
}

/// `var (a, b) = e;` / `final [x] = e;`
pub(crate) fn declaracao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, final_: bool, p: PatternId, valor: ExprId) {
    let s = esquema(inf, cx, p);
    let t = inferir(inf, cx, valor, s);
    tipar(inf, cx, p, t, final_, false);
}

/// Declara as variáveis de um padrão casado com `t` (`for (var (a, b) in …)`).
pub(crate) fn declarar_por_tipo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, final_: bool, p: PatternId, t: TypeId) {
    tipar(inf, cx, p, t, final_, false);
}

/// `(a, b) = e` — atribuição por padrão a variáveis existentes.
pub(crate) fn atribuicao_de_padrao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, valor: ExprId) -> TypeId {
    let s = esquema_de_atribuicao(inf, cx, p);
    let t = inferir(inf, cx, valor, s);
    tipar(inf, cx, p, t, false, true);
    t
}

/// Esquema de um padrão de atribuição: variáveis contribuem com o tipo declarado.
fn esquema_de_atribuicao(inf: &mut BodyInferrer<'_>, cx: &Corpo, p: PatternId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.pattern(p).kind {
        PatternKind::Variable { name, ty: None, .. } => match cx.buscar(name.sym) {
            Some(Nome::Local(id)) => cx.local(id).tipo,
            _ => inf.core.unknown,
        },
        PatternKind::Record { fields } => {
            let fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            let mut pos = Vec::new();
            let mut nm = Vec::new();
            for (n, x) in fields {
                let s = esquema_de_atribuicao(inf, cx, x);
                match n {
                    Some(n) => nm.push((n.sym, s)),
                    None => pos.push(s),
                }
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        _ => esquema(inf, cx, p),
    }
}

/// `switch (v) { p => e, ... }` como expressão.
pub(crate) fn expressao_switch(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, valor: ExprId, casos: &[ast::SwitchExprCase], ctx: TypeId) -> TypeId {
    let t = inferir_livre(inf, cx, valor);
    let antes = cx.fluxo.clone();
    let mut nao_casou = antes.clone();
    let mut tipos: Vec<TypeId> = Vec::new();
    let mut saidas: Vec<Fluxo> = Vec::new();
    for c in casos {
        cx.fluxo = nao_casou.clone();
        cx.empurrar_escopo();
        let (sim, nao) = caso(inf, cx, c.pattern, t, c.guard);
        cx.fluxo = sim;
        let tc = inferir(inf, cx, c.body, ctx);
        tipos.push(tc);
        saidas.push(cx.fluxo.clone());
        cx.tirar_escopo();
        nao_casou = nao;
    }
    cx.fluxo = inf.juntar_todos(&antes, &saidas);
    if tipos.is_empty() {
        return inf.core.never;
    }
    let mut acc = tipos[0];
    for &x in &tipos[1..] {
        acc = inf.up(acc, x);
    }
    if !inf.e_desconhecido(ctx) {
        let s = inf.fecho_maior(ctx);
        if !inf.sub(acc, s) && tipos.iter().all(|&x| inf.sub(x, s)) {
            return s;
        }
    }
    acc
}
