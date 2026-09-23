//! Inferência de expressões: contexto (esquema) para baixo, tipo estático
//! para cima (`inference.md`, "Expression inference").
//!
//! Cadeias com `?.` fazem *null-shorting*: os nós internos da cadeia recebem
//! o tipo não anulável e só o nó que termina a cadeia recebe o `?`
//! ([`inferir_no`] devolve se a cadeia está em curto).

use super::chamadas;
use super::colecoes;
use super::corpo::{Corpo, Local, Nome};
use super::fluxo::Fluxo;
use super::funcoes;
use super::membros::{Busca, Membro};
use super::padroes;
use super::BodyInferrer;
use crate::codes::*;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionKind};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infere `e` no contexto `ctx` (o desconhecido `_` = sem contexto).
pub(crate) fn inferir(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> TypeId {
    let (t, curto) = inferir_no(inf, cx, e, ctx, false);
    if curto {
        let t = inf.anulavel(t);
        registrar(inf, cx, e, t);
        t
    } else {
        t
    }
}

/// Infere sem contexto.
pub(crate) fn inferir_livre(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> TypeId {
    let u = inf.core.unknown;
    inferir(inf, cx, e, u)
}

pub(crate) fn registrar(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId) {
    inf.body_types.units[cx.unit.0 as usize].set_type(e, t);
}

pub(crate) fn resolver(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, r: Resolved) {
    inf.body_types.units[cx.unit.0 as usize].set_resolved(e, r);
}

fn ast<'p>(inf: &BodyInferrer<'p>, cx: &Corpo) -> &'p ast::Ast {
    &inf.program.unit(cx.unit).ast
}

/// O que um nome simples denota no ponto atual.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RefNome {
    Local(LocalId),
    TipoParam(crate::table::TypeParamId),
    Elemento(Element),
    /// Membro declarado no corpo da classe/extensão envolvente.
    MembroLexico(dartforge_elements::model::FunctionElementId, bool),
    ConstanteEnum(dartforge_elements::model::VariableId),
    Prefixo,
    /// Não achado lexicamente, mas há `this`: `this.nome`.
    ThisImplicito,
    /// Local do bloco declarado depois deste uso.
    Adiante,
    Nenhum,
}

/// Busca léxica de um nome (`setter` = contexto de escrita).
pub(crate) fn resolver_nome(inf: &mut BodyInferrer<'_>, cx: &Corpo, nome: SymbolId, setter: bool) -> RefNome {
    match cx.buscar(nome) {
        Some(Nome::Local(id)) => return RefNome::Local(id),
        Some(Nome::TipoParam(p)) => return RefNome::TipoParam(p),
        Some(Nome::Adiante) => return RefNome::Adiante,
        None => {}
    }
    // Membros declarados no corpo da classe/extensão.
    if cx.classe.is_some() || cx.extensao.is_some() {
        let chave_setter = if setter { inf.chave_setter(nome) } else { None };
        let ordem: Vec<Option<SymbolId>> = if setter { vec![chave_setter, Some(nome)] } else { vec![Some(nome), chave_setter] };
        for chave in ordem.into_iter().flatten() {
            if let Some((f, estatico)) = inf.membro_declarado_lexico(cx.classe, cx.extensao, chave, None) {
                return RefNome::MembroLexico(f, estatico);
            }
        }
        if let Some(c) = cx.classe {
            if let Some(&v) = inf.program.class(c).enum_constants.iter().find(|&&v| inf.program.variable(v).name == nome) {
                return RefNome::ConstanteEnum(v);
            }
        }
        if let Some(x) = cx.extensao {
            if let Some(&v) = inf.program.extension(x).fields.iter().find(|&&v| inf.program.variable(v).name == nome) {
                return RefNome::Elemento(Element::Variable(v));
            }
        }
    }
    if let Some(b) = inf.program.lookup(cx.lib, nome) {
        let el = if setter { b.setter.or(b.getter) } else { b.getter.or(b.setter) };
        if let Some(el) = el {
            if let Element::Prefix(..) = el {
                return RefNome::Prefixo;
            }
            return RefNome::Elemento(el);
        }
    }
    if inf.program.library(cx.lib).prefixes.contains_key(&nome) {
        return RefNome::Prefixo;
    }
    if cx.tipo_this.is_some() && !cx.estatico {
        return RefNome::ThisImplicito;
    }
    RefNome::Nenhum
}

/// Uma referência a tipo usada como receptor (`C.m`, `p.C.m`, `C<T>.m`).
#[derive(Debug, Clone)]
pub(crate) enum RefTipo {
    Classe(ClassId, Option<Vec<ast::TypeId>>),
    /// Alias de tipo que nomeia uma classe (`typedef A = B<int>`).
    Alias(ClassId, Vec<TypeId>),
    Extensao(ExtensionId),
}

/// Se `e` nomeia uma classe/extensão/typedef (não um valor), qual.
pub(crate) fn referencia_a_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<RefTipo> {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolver_nome(inf, cx, n.sym, false) {
            RefNome::Elemento(el) => el,
            _ => return None,
        },
        ExprKind::Property { target, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else { return None };
            if !matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                return None;
            }
            inf.program.lookup_prefixed(cx.lib, p.sym, name.sym)?.getter?
        }
        ExprKind::TypeArguments { target, type_args } => {
            let targs = type_args.to_vec();
            return match referencia_a_tipo(inf, cx, *target)? {
                RefTipo::Classe(c, None) => Some(RefTipo::Classe(c, Some(targs))),
                RefTipo::Alias(c, _) => Some(RefTipo::Alias(c, Vec::new())),
                _ => None,
            };
        }
        _ => return None,
    };
    match el {
        Element::Class(c) => Some(RefTipo::Classe(c, None)),
        Element::Extension(x) => Some(RefTipo::Extensao(x)),
        Element::Typedef(td) => {
            let alvo = inf.outline.typedefs[td.0 as usize].target_type;
            let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
            match inf.table.get(alvo).clone() {
                Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                    let args = if params.is_empty() {
                        args.to_vec()
                    } else {
                        let inst = inf.instanciar_para_limites(&params);
                        let mapa = inf.mapa(&params, &inst);
                        args.iter().map(|a| inf.subst(*a, &mapa)).collect()
                    };
                    Some(RefTipo::Alias(class, args))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Registra os nós de uma referência a tipo (tipo `Type`, resolução do elemento).
fn registrar_ref_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) {
    let a = ast(inf, cx);
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(b) = inf.program.lookup(cx.lib, n.sym) {
                if let Some(el) = b.getter {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            if let ExprKind::Identifier(p) = &a.expr(*target).kind {
                resolver(inf, cx, *target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf.program.lookup_prefixed(cx.lib, p.sym, name.sym).and_then(|b| b.getter) {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            registrar_ref_tipo(inf, cx, t);
            registrar(inf, cx, e, tt);
        }
        _ => {}
    }
}

/// Tipo de uma leitura de variável local (com checagem de atribuição definitiva).
fn ler_local(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, id: LocalId, span: dartforge_diagnostics::Span) -> TypeId {
    let l = cx.local(id).clone();
    if !l.late && !l.funcao_local && cx.fluxo.alcancavel && !cx.fluxo.atribuida(id) {
        let potencialmente_nao_nulo = l.final_ || inf.e_nao_anulavel(l.tipo);
        if potencialmente_nao_nulo && !inf.e_dynamic(l.tipo) {
            let msg = format!("{}: '{}'", DEFINITELY_UNASSIGNED_VARIABLE.template, inf.interner.resolve(l.nome));
            inf.aviso(msg, span);
        }
    }
    cx.fluxo.tipo_atual(id, l.tipo)
}

/// Tipo de uma leitura de elemento de topo.
fn ler_elemento(inf: &mut BodyInferrer<'_>, el: Element) -> TypeId {
    match el {
        Element::Class(_) | Element::Typedef(_) | Element::Extension(_) => inf.core.type_,
        Element::Variable(v) => inf.tipo_variavel(v),
        Element::Function(f) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                (FunctionKind::Getter, _) => inf.outline.functions[f.0 as usize].return_type,
                (FunctionKind::Setter, _) => {
                    inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_)
                }
                _ => inf.outline.functions[f.0 as usize].signature,
            }
        }
        Element::Prefix(..) => inf.core.dynamic_,
    }
}

fn resolved_de_membro_lexico(inf: &BodyInferrer<'_>, cx: &Corpo, f: dartforge_elements::model::FunctionElementId, estatico: bool) -> Resolved {
    let fe = inf.program.function(f);
    if let Some(x) = fe.extension.or(cx.extensao) {
        if fe.class.is_none() {
            return Resolved::ExtensionMember { extension: x, member: f };
        }
    }
    let class = fe.class.or(cx.classe).unwrap_or(ClassId(0));
    let member = match (fe.kind, fe.variable) {
        (FunctionKind::ImplicitAccessor, Some(v)) if estatico => MemberRef::Variable(v),
        _ => MemberRef::Function(f),
    };
    Resolved::Member { class, member, via_super: false }
}

/// Identificador como valor.
fn identificador(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, n: ast::Name) -> TypeId {
    match resolver_nome(inf, cx, n.sym, false) {
        RefNome::Local(id) => {
            resolver(inf, cx, e, Resolved::Local(id));
            if n.span.start < cx.local(id).offset && !cx.local(id).funcao_local {
                let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
                inf.aviso(msg, n.span);
            }
            ler_local(inf, cx, id, n.span)
        }
        RefNome::TipoParam(p) => {
            resolver(inf, cx, e, Resolved::TypeParameter(p));
            inf.core.type_
        }
        RefNome::Elemento(el) => {
            resolver(inf, cx, e, Resolved::Element(el));
            ler_elemento(inf, el)
        }
        RefNome::MembroLexico(f, estatico) => {
            let r = resolved_de_membro_lexico(inf, cx, f, estatico);
            resolver(inf, cx, e, r);
            if !estatico {
                // Membro de instância declarado aqui: pelo tipo `this` (a
                // substituição é a identidade).
                if let Some(this) = cx.tipo_this {
                    if let Busca::Achado(m) = inf.buscar_membro(cx.lib, this, n.sym, false) {
                        return m.tipo;
                    }
                }
            }
            inf.tipo_do_membro_declarado(f, false).0
        }
        RefNome::ConstanteEnum(v) => {
            let c = inf.program.variable(v).class;
            if let Some(c) = c {
                resolver(inf, cx, e, Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            inf.tipo_variavel(v)
        }
        RefNome::Prefixo => {
            resolver(inf, cx, e, Resolved::Prefix(cx.lib));
            inf.core.dynamic_
        }
        RefNome::ThisImplicito => {
            let this = cx.tipo_this.unwrap();
            match inf.buscar_membro(cx.lib, this, n.sym, false) {
                Busca::Achado(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    m.tipo
                }
                Busca::Dinamico => inf.core.dynamic_,
                Busca::Nunca => inf.core.never,
                Busca::Ausente => {
                    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
                    inf.aviso(msg, n.span);
                    inf.core.dynamic_
                }
            }
        }
        RefNome::Adiante => {
            let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
            inf.aviso(msg, n.span);
            inf.core.dynamic_
        }
        RefNome::Nenhum => {
            let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
            inf.aviso(msg, n.span);
            inf.core.dynamic_
        }
    }
}

/// O alvo de promoção (variável local) que `e` denota, sem parênteses.
pub(crate) fn alvo_de_promocao(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<LocalId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Parenthesized(i) => alvo_de_promocao(inf, cx, *i),
        ExprKind::Identifier(n) => match cx.buscar(n.sym) {
            Some(Nome::Local(id)) if !cx.local(id).funcao_local => Some(id),
            _ => None,
        },
        _ => None,
    }
}

/// Coerção de tear-off genérico para um contexto de função não genérico
/// (instanciação implícita).
fn instanciar_em_contexto(inf: &mut BodyInferrer<'_>, t: TypeId, ctx: TypeId) -> TypeId {
    let Type::Function { type_params, ret, positional, optional, named, nullable } = inf.table.get(t).clone() else { return t };
    if type_params.is_empty() || inf.e_desconhecido(ctx) {
        return t;
    }
    let k = inf.fecho_maior(ctx);
    let k = inf.nao_nulo(k);
    let Type::Function { type_params: tp_ctx, .. } = inf.table.get(k).clone() else { return t };
    if !tp_ctx.is_empty() {
        return t;
    }
    let sem = inf.table.intern(Type::Function {
        type_params: Box::new([]),
        ret,
        positional: positional.clone(),
        optional: optional.clone(),
        named: named.clone(),
        nullable,
    });
    let mut gi = crate::constraints::GenericInferrer::new(&type_params);
    let mut env = inf.env();
    gi.constrain_return(sem, k, &mut env);
    let args = gi.choose_final(&mut env);
    crate::constraints::instanciar_funcao(t, &args, &mut env)
}

/// Infere um nó; devolve `(tipo, curto)` onde `curto` diz que há `?.` na
/// cadeia abaixo (o chamador que termina a cadeia torna o tipo anulável).
pub(crate) fn inferir_no(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId, _cadeia: bool) -> (TypeId, bool) {
    let a = ast(inf, cx);
    let expr = a.expr(e);
    let span = expr.span;
    let mut curto = false;
    let t = match &expr.kind {
        ExprKind::Int(_) => {
            let s = inf.fecho_maior(ctx);
            let (int, double) = (inf.core.int, inf.core.double);
            if !inf.e_desconhecido(ctx) && inf.sub(double, s) && !inf.sub(int, s) {
                double
            } else {
                int
            }
        }
        ExprKind::Double(_) => inf.core.double,
        ExprKind::Bool(_) => inf.core.bool_,
        ExprKind::Null => inf.core.null,
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(i) = p {
                    inferir_livre(inf, cx, *i);
                }
            }
            inf.core.string
        }
        ExprKind::Symbol(_) => inf.core.symbol,
        // `$this` numa interpolação chega como identificador.
        ExprKind::Identifier(n) if Some(n.sym) == inf.sym.this_ => cx.tipo_this.unwrap_or(inf.core.dynamic_),
        ExprKind::Identifier(n) => {
            let t = identificador(inf, cx, e, *n);
            instanciar_em_contexto(inf, t, ctx)
        }
        ExprKind::This => cx.tipo_this.unwrap_or(inf.core.dynamic_),
        ExprKind::Super => cx.tipo_this.unwrap_or(inf.core.dynamic_),
        ExprKind::Parenthesized(i) => {
            let (t, c) = inferir_no(inf, cx, *i, ctx, false);
            let t = if c { inf.anulavel(t) } else { t };
            registrar(inf, cx, *i, t);
            t
        }
        ExprKind::List { .. } | ExprKind::SetOrMap { .. } => colecoes::literal(inf, cx, e, ctx),
        ExprKind::Record { positional, named, .. } => {
            let k = inf.fecho_maior_se_conhecido(ctx);
            let k = inf.nao_nulo(k);
            let (ctx_pos, ctx_nom) = match inf.table.get(k).clone() {
                Type::Record { positional: p, named: n, .. } if p.len() == positional.len() && n.len() == named.len() => (Some(p), Some(n)),
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let positional = positional.to_vec();
            let named: Vec<(ast::Name, ExprId)> = named.to_vec();
            let mut pos = Vec::new();
            for (i, p) in positional.iter().enumerate() {
                let c = ctx_pos.as_ref().map(|v| v[i]).unwrap_or(u);
                pos.push(inferir(inf, cx, *p, c));
            }
            let mut nm = Vec::new();
            for (n, x) in named.iter() {
                let c = ctx_nom.as_ref().and_then(|v| v.iter().find(|(s, _)| *s == n.sym).map(|(_, t)| *t)).unwrap_or(u);
                nm.push((n.sym, inferir(inf, cx, *x, c)));
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        ExprKind::InstanceCreation { .. } => chamadas::instanciacao(inf, cx, e, ctx),
        ExprKind::FunctionExpression(f) => funcoes::expressao_de_funcao(inf, cx, *f, ctx),
        ExprKind::Property { target, name, null_aware } => {
            let (t, c) = propriedade(inf, cx, e, *target, *name, *null_aware);
            curto = c;
            instanciar_em_contexto(inf, t, ctx)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (recv, c) = receptor(inf, cx, *target, *null_aware);
            curto = c;
            let op = inf.sym.indice;
            let (t, _) = operador_binario(inf, cx, recv, op, *index, ctx, span, Some(e));
            t
        }
        ExprKind::Call { .. } => {
            let (t, c) = chamadas::chamada(inf, cx, e, ctx);
            curto = c;
            t
        }
        ExprKind::TypeArguments { target, type_args } => {
            if referencia_a_tipo(inf, cx, e).is_some() {
                registrar_ref_tipo(inf, cx, e);
                inf.core.type_
            } else {
                let t = inferir_livre(inf, cx, *target);
                let targs: Vec<TypeId> = type_args.iter().map(|&x| inf.tipo_de_anotacao(cx, x)).collect();
                match inf.table.get(t).clone() {
                    Type::Function { type_params, .. } if type_params.len() == targs.len() => {
                        let mut env = inf.env();
                        crate::constraints::instanciar_funcao(t, &targs, &mut env)
                    }
                    _ => inf.core.dynamic_,
                }
            }
        }
        ExprKind::Unary { op, operand } => unario(inf, cx, e, *op, *operand, ctx, &mut curto),
        ExprKind::Binary { op, left, right } => binario(inf, cx, e, *op, *left, *right, ctx),
        ExprKind::Conditional { condition, then, else_ } => {
            let (vf, ff) = condicao_verificada(inf, cx, *condition);
            let antes = cx.fluxo.clone();
            cx.fluxo = vf;
            let t1 = inferir(inf, cx, *then, ctx);
            let depois1 = std::mem::replace(&mut cx.fluxo, ff);
            let t2 = inferir(inf, cx, *else_, ctx);
            let depois2 = std::mem::replace(&mut cx.fluxo, antes);
            cx.fluxo = inf.juntar(&depois1, &depois2);
            limite_superior_em_contexto(inf, t1, t2, ctx)
        }
        ExprKind::Is { value, ty, negated } => {
            let (vf, ff) = teste_de_tipo(inf, cx, *value, *ty, *negated, span);
            cx.fluxo = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        ExprKind::As { value, ty } => {
            let v = inferir_livre(inf, cx, *value);
            let t = inf.tipo_de_anotacao(cx, *ty);
            if v == t && !inf.e_dynamic(v) {
                inf.aviso(UNNECESSARY_CAST.template.to_string(), span);
            }
            if let Some(id) = alvo_de_promocao(inf, cx, *value) {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.promover(&mut f, id, decl, t);
                cx.fluxo = f;
            }
            t
        }
        ExprKind::Assign { op, target, value } => atribuicao(inf, cx, e, *op, *target, *value, &mut curto),
        ExprKind::PatternAssign { pattern, value } => padroes::atribuicao_de_padrao(inf, cx, *pattern, *value),
        ExprKind::Cascade { target, sections, null_aware } => {
            let t = inferir(inf, cx, *target, ctx);
            let r = if *null_aware { inf.nao_nulo(t) } else { t };
            cx.cascatas.push(r);
            let secs = sections.to_vec();
            for s in secs {
                inferir_livre(inf, cx, s);
            }
            cx.cascatas.pop();
            t
        }
        ExprKind::CascadeTarget => cx.cascatas.last().copied().unwrap_or(inf.core.dynamic_),
        ExprKind::Await(i) => {
            let k1 = contexto_de_await(inf, ctx);
            let t1 = inferir(inf, cx, *i, k1);
            inf.flatten(t1)
        }
        ExprKind::Throw(i) => {
            inferir_livre(inf, cx, *i);
            cx.fluxo.alcancavel = false;
            inf.core.never
        }
        ExprKind::Rethrow => {
            cx.fluxo.alcancavel = false;
            inf.core.never
        }
        ExprKind::Switch { value, cases } => padroes::expressao_switch(inf, cx, *value, cases, ctx),
    };
    registrar(inf, cx, e, t);
    if matches!(inf.table.get(t), Type::Never) {
        cx.fluxo.alcancavel = false;
    }
    (t, curto)
}

impl<'a> BodyInferrer<'a> {
    /// Fecho maior se o contexto não for `_` (senão o próprio `_`).
    pub(crate) fn fecho_maior_se_conhecido(&mut self, ctx: TypeId) -> TypeId {
        if self.e_desconhecido(ctx) {
            ctx
        } else {
            self.fecho_maior(ctx)
        }
    }
}

/// `K1` de `await e` em contexto `K` (`inference.md`, "Await expressions").
fn contexto_de_await(inf: &mut BodyInferrer<'_>, k: TypeId) -> TypeId {
    if inf.e_desconhecido(k) {
        return inf.futuro_ou(k);
    }
    match inf.table.get(k) {
        Type::FutureOr { .. } => k,
        Type::Dynamic => {
            let u = inf.core.unknown;
            inf.futuro_ou(u)
        }
        _ => inf.futuro_ou(k),
    }
}

/// UP dos ramos com a regra de `inference-update-3`: se UP não cabe no
/// contexto mas os dois ramos cabem, o tipo é o contexto.
pub(crate) fn limite_superior_em_contexto(inf: &mut BodyInferrer<'_>, t1: TypeId, t2: TypeId, ctx: TypeId) -> TypeId {
    let t = inf.up(t1, t2);
    if inf.e_desconhecido(ctx) {
        return t;
    }
    let s = inf.fecho_maior(ctx);
    if inf.sub(t, s) {
        t
    } else if inf.sub(t1, s) && inf.sub(t2, s) {
        s
    } else {
        t
    }
}

/// Infere o receptor de um acesso (`r.x`, `r[i]`, `r.m()`), tratando `?.` e
/// o *null-shorting*. Devolve o tipo do receptor para a busca e se há curto.
pub(crate) fn receptor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, r: ExprId, null_aware: bool) -> (TypeId, bool) {
    let u = inf.core.unknown;
    let (t, c) = inferir_no(inf, cx, r, u, true);
    if null_aware {
        if !inf.e_anulavel(t) && !inf.e_dynamic(t) {
            let sp = inf.span_expr(cx.unit, r);
            inf.aviso(INVALID_NULL_AWARE_OPERATOR.template.to_string(), sp);
        }
        // A promoção do receptor de `?.` vale só dentro da cadeia; como ela
        // não é modelada, não se promove (nada vaza para depois da cadeia).
        let nn = inf.nao_nulo(t);
        (nn, true)
    } else {
        (t, c)
    }
}

/// `target.name` (leitura).
fn propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, target: ExprId, name: ast::Name, null_aware: bool) -> (TypeId, bool) {
    let a = ast(inf, cx);
    // `p.nome` com prefixo de import.
    if let ExprKind::Identifier(p) = &a.expr(target).kind {
        if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
            resolver(inf, cx, target, Resolved::Prefix(cx.lib));
            match inf.program.lookup_prefixed(cx.lib, p.sym, name.sym).and_then(|b| b.getter.or(b.setter)) {
                Some(el) => {
                    resolver(inf, cx, e, Resolved::Element(el));
                    return (ler_elemento(inf, el), false);
                }
                None => {
                    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(name.sym));
                    inf.aviso(msg, name.span);
                    return (inf.core.dynamic_, false);
                }
            }
        }
    }
    // `C.x`: estático, constante de enum ou tear-off de construtor.
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        registrar_ref_tipo(inf, cx, target);
        return (acesso_estatico(inf, cx, e, rt, name), false);
    }
    // `super.x`.
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
        registrar(inf, cx, target, this);
        return (membro_super(inf, cx, e, name, false), false);
    }
    let (recv, curto) = receptor(inf, cx, target, null_aware);
    let t = match inf.buscar_membro(cx.lib, recv, name.sym, false) {
        Busca::Achado(m) => {
            resolver(inf, cx, e, m.resolved.clone());
            m.tipo
        }
        Busca::Dinamico => {
            resolver(inf, cx, e, Resolved::Dynamic);
            // `hashCode`, `runtimeType`, `toString`… de `Object` valem também em `dynamic`.
            let o = inf.core.object;
            match inf.membro_de_interface(o, name.sym, false) {
                Some(m) => m.tipo,
                None => inf.core.dynamic_,
            }
        }
        Busca::Nunca => inf.core.never,
        Busca::Ausente => {
            let msg = format!(
                "{}: getter '{}' não definido para o tipo '{}'",
                UNDEFINED_GETTER.template,
                inf.interner.resolve(name.sym),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.aviso(msg, name.span);
            inf.core.dynamic_
        }
    };
    (t, curto)
}

/// Acesso estático `C.x` / `E.x` / `C.new`.
fn acesso_estatico(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, rt: RefTipo, name: ast::Name) -> TypeId {
    match rt {
        RefTipo::Extensao(x) => match inf.membro_estatico_de_extensao(x, name.sym, false) {
            Some(m) => {
                resolver(inf, cx, e, m.resolved.clone());
                m.tipo
            }
            None => inf.core.dynamic_,
        },
        RefTipo::Classe(c, targs) => {
            if let Some(m) = inf.membro_estatico(c, name.sym, false) {
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            let args = targs.map(|v| v.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect::<Vec<_>>());
            tearoff_de_construtor(inf, cx, e, c, args, name)
        }
        RefTipo::Alias(c, args) => {
            if let Some(m) = inf.membro_estatico(c, name.sym, false) {
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            tearoff_de_construtor(inf, cx, e, c, Some(args), name)
        }
    }
}

/// `C.nome` / `C.new` como valor: tipo de função do construtor (genérico
/// sobre os parâmetros da classe se não instanciado).
fn tearoff_de_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, c: ClassId, args: Option<Vec<TypeId>>, name: ast::Name) -> TypeId {
    let chave = if Some(name.sym) == inf.sym.new_ { inf.sym.vazio } else { Some(name.sym) };
    let Some(chave) = chave else { return inf.core.dynamic_ };
    let Some(&f) = inf.program.class(c).constructors.get(&chave) else {
        let msg = format!("{}: getter '{}' não definido para a classe", UNDEFINED_GETTER.template, inf.interner.resolve(name.sym));
        inf.aviso(msg, name.span);
        return inf.core.dynamic_;
    };
    resolver(inf, cx, e, Resolved::Constructor(f));
    let sig = inf.outline.functions[f.0 as usize].signature;
    let params = inf.outline.classes[c.0 as usize].type_params.clone();
    match args {
        Some(args) if args.len() == params.len() => {
            let mapa = inf.mapa(&params, &args);
            inf.subst(sig, &mapa)
        }
        _ if params.is_empty() => sig,
        _ => {
            // Genérico: `C<T> Function<T>(...)` com parâmetros novos.
            let novos: Vec<crate::table::TypeParamId> = params
                .iter()
                .map(|&p| {
                    let d = inf.table.param(p).clone();
                    inf.table.alloc_type_param(d.name, crate::table::TypeParamOwner::GenericFunctionType, d.bound, d.variance)
                })
                .collect();
            let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
            let mapa = inf.mapa(&params, &tipos);
            let s = inf.subst(sig, &mapa);
            for &p in &novos {
                let b = inf.table.param(p).bound;
                let b = inf.subst(b, &mapa);
                inf.table.set_type_param_bound(p, b);
            }
            match inf.table.get(s).clone() {
                Type::Function { ret, positional, optional, named, nullable, .. } => inf.table.intern(Type::Function {
                    type_params: novos.into_boxed_slice(),
                    ret,
                    positional,
                    optional,
                    named,
                    nullable,
                }),
                _ => s,
            }
        }
    }
}

/// `super.nome` (leitura ou escrita).
pub(crate) fn membro_super(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, name: ast::Name, setter: bool) -> TypeId {
    let Some(c) = cx.classe else { return inf.core.dynamic_ };
    let Some(this) = cx.tipo_this else { return inf.core.dynamic_ };
    let chave = if setter { inf.chave_setter(name.sym) } else { Some(name.sym) };
    let Some(chave) = chave else { return inf.core.dynamic_ };
    for (sup, _) in crate::scope::supertipos_ordenados(inf.program, &inf.outline.hierarchy, c) {
        if let Some(&f) = inf.program.class(sup).instance_members.get(&chave) {
            let (t, _) = inf.tipo_do_membro_declarado(f, setter);
            let t = inf.substituir_do_dono(this, c, sup, t);
            resolver(inf, cx, e, Resolved::Member { class: sup, member: MemberRef::Function(f), via_super: true });
            return t;
        }
    }
    // Extensões sobre o tipo de `super` não se aplicam; `Object`.
    let o = inf.core.object;
    match inf.membro_de_interface(o, name.sym, setter) {
        Some(m) => m.tipo,
        None => inf.core.dynamic_,
    }
}

/// Símbolo do operador binário.
fn simbolo_operador(inf: &BodyInferrer<'_>, op: BinaryOp) -> Option<SymbolId> {
    let s = match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::Eq | BinaryOp::NotEq => "==",
        _ => return None,
    };
    inf.interner.lookup(s)
}

/// Invocação de um operador de um argumento (`a + b`, `a[i]`): busca o
/// membro no receptor, infere o argumento com o contexto do parâmetro
/// (com o refinamento numérico) e devolve `(tipo, membro)`.
pub(crate) fn operador_binario(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    recv: TypeId,
    op: Option<SymbolId>,
    arg: ExprId,
    ctx: TypeId,
    span: dartforge_diagnostics::Span,
    no: Option<ExprId>,
) -> (TypeId, Option<Membro>) {
    let u = inf.core.unknown;
    let Some(op) = op else {
        inferir_livre(inf, cx, arg);
        return (inf.core.dynamic_, None);
    };
    match inf.buscar_membro(cx.lib, recv, op, false) {
        Busca::Achado(m) => {
            if let Some(n) = no {
                resolver(inf, cx, n, m.resolved.clone());
            }
            let (param, ret) = match inf.table.get(m.tipo).clone() {
                Type::Function { positional, optional, ret, .. } => (positional.first().or(optional.first()).copied(), ret),
                _ => (None, inf.core.dynamic_),
            };
            let ctx_arg = match param {
                Some(p) => contexto_numerico(inf, recv, &m, op, ctx, p),
                None => u,
            };
            let ta = inferir(inf, cx, arg, ctx_arg);
            if let Some(p) = param {
                let sp = inf.span_expr(cx.unit, arg);
                inf.verificar_atribuivel(ta, p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
            }
            let t = refinar_numerico(inf, recv, &m, op, &[ta], ret);
            (t, Some(m))
        }
        Busca::Dinamico => {
            inferir_livre(inf, cx, arg);
            (inf.core.dynamic_, None)
        }
        Busca::Nunca => {
            inferir_livre(inf, cx, arg);
            (inf.core.never, None)
        }
        Busca::Ausente => {
            inferir_livre(inf, cx, arg);
            let msg = format!(
                "{}: operador '{}' para o tipo '{}'",
                UNDEFINED_METHOD.template,
                inf.interner.resolve(op),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.aviso(msg, span);
            (inf.core.dynamic_, None)
        }
    }
}

fn e_numerico_refinavel(inf: &BodyInferrer<'_>, m: &Membro, op: SymbolId) -> bool {
    if m.de_extensao {
        return false;
    }
    let s = inf.interner.resolve(op);
    matches!(s, "+" | "-" | "*" | "%" | "remainder")
}

/// Contexto do argumento de `e1 op e2` numérico (`_refineNumericInvocationContext`).
pub(crate) fn contexto_numerico(inf: &mut BodyInferrer<'_>, t: TypeId, m: &Membro, op: SymbolId, ctx: TypeId, atual: TypeId) -> TypeId {
    if !e_numerico_refinavel(inf, m, op) && Some(op) != inf.sym.clamp {
        return atual;
    }
    let num_q = inf.anulavel(inf.core.num);
    if !inf.sub(t, num_q) {
        return atual;
    }
    let c = inf.fecho_maior(ctx);
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let int_q = inf.anulavel(int);
    let double_q = inf.anulavel(double);
    if inf.sub(int, c) && !inf.sub(num, c) && inf.sub(t, int_q) {
        return int;
    }
    let clamp = Some(op) == inf.sym.clamp;
    if inf.sub(double, c) && !inf.sub(num, c) && (if clamp { inf.sub(t, double_q) } else { !inf.sub(t, double_q) }) {
        return double;
    }
    num
}

/// Tipo de `e1 op e2` numérico (`_refineNumericInvocationTypeNullSafe`).
pub(crate) fn refinar_numerico(inf: &mut BodyInferrer<'_>, t: TypeId, m: &Membro, op: SymbolId, args: &[TypeId], atual: TypeId) -> TypeId {
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let num_q = inf.anulavel(num);
    let int_q = inf.anulavel(int);
    let double_q = inf.anulavel(double);
    if e_numerico_refinavel(inf, m, op) && args.len() == 1 && inf.sub(t, num_q) {
        let s = args[0];
        if inf.sub(t, double_q) {
            return double;
        }
        let fundo = inf.e_fundo(s);
        if !fundo && inf.sub(s, double_q) {
            return double;
        }
        if !fundo && inf.sub(t, int_q) && inf.sub(s, int_q) {
            return int;
        }
        return num;
    }
    if Some(op) == inf.sym.clamp && !m.de_extensao && args.len() == 2 && inf.sub(t, num_q) {
        let (t2, t3) = (args[0], args[1]);
        if inf.e_fundo(t2) || inf.e_fundo(t3) {
            return atual;
        }
        if inf.sub(t, int_q) && inf.sub(t2, int_q) && inf.sub(t3, int_q) {
            return int;
        }
        if inf.sub(t, double_q) && inf.sub(t2, double_q) && inf.sub(t3, double_q) {
            return double;
        }
        return num;
    }
    atual
}

/// Operadores binários.
fn binario(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId, ctx: TypeId) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq => {
            let (vf, ff) = condicao_binaria(inf, cx, e, op, left, right);
            cx.fluxo = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        BinaryOp::IfNull => {
            let k_q = if inf.e_desconhecido(ctx) { ctx } else { inf.anulavel(ctx) };
            let t1 = inferir(inf, cx, left, k_q);
            let j = if inf.e_desconhecido(ctx) || inf.e_dynamic(ctx) { t1 } else { ctx };
            let antes = cx.fluxo.clone();
            if let Some(id) = alvo_de_promocao(inf, cx, left) {
                // No ramo em que `e1` é nulo nada se promove; depois, `e1` é não nulo.
                let _ = id;
            }
            let t2 = inferir(inf, cx, right, j);
            let depois = cx.fluxo.clone();
            cx.fluxo = inf.juntar(&antes, &depois);
            let nn = inf.nao_nulo(t1);
            limite_superior_em_contexto(inf, nn, t2, ctx)
        }
        _ => {
            let u = inf.core.unknown;
            let l = inferir(inf, cx, left, u);
            if matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, left, this);
            }
            let sym = simbolo_operador(inf, op);
            let (t, _) = operador_binario(inf, cx, l, sym, right, ctx, span, Some(e));
            t
        }
    }
}

/// Unários (`!`, `-`, `~`, `++`, `--`, `!` pós-fixo).
fn unario(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: UnaryOp, operand: ExprId, _ctx: TypeId, curto: &mut bool) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        UnaryOp::Not => {
            let (vf, ff) = condicao(inf, cx, operand);
            cx.fluxo = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        UnaryOp::NullAssert => {
            let u = inf.core.unknown;
            let (t, c) = inferir_no(inf, cx, operand, u, true);
            *curto = c;
            if !inf.e_anulavel(t) && !inf.e_dynamic(t) && !c {
                inf.aviso(INVALID_NULL_AWARE_OPERATOR.template.to_string(), span);
            }
            if let Some(id) = alvo_de_promocao(inf, cx, operand) {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.promover_nao_nulo(&mut f, id, decl);
                cx.fluxo = f;
            }
            inf.nao_nulo_promocao(t)
        }
        UnaryOp::Neg | UnaryOp::BitNot => {
            let u = inf.core.unknown;
            let t = inferir(inf, cx, operand, u);
            let sym = if op == UnaryOp::Neg { inf.sym.menos_unario } else { inf.sym.til };
            let Some(sym) = sym else { return inf.core.dynamic_ };
            match inf.buscar_membro(cx.lib, t, sym, false) {
                Busca::Achado(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    }
                }
                Busca::Nunca => inf.core.never,
                _ => inf.core.dynamic_,
            }
        }
        UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
            let prefixo = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec);
            let bop = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) { BinaryOp::Add } else { BinaryOp::Sub };
            let (leitura, escrita, local) = ler_para_escrita(inf, cx, operand);
            let sym = simbolo_operador(inf, bop);
            let int = inf.core.int;
            let res = match sym.map(|s| inf.buscar_membro(cx.lib, leitura, s, false)) {
                Some(Busca::Achado(m)) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    let ret = match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    };
                    refinar_numerico(inf, leitura, &m, sym.unwrap(), &[int], ret)
                }
                Some(Busca::Nunca) => inf.core.never,
                _ => inf.core.dynamic_,
            };
            if let Some(id) = local {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.atribuir_fluxo(&mut f, id, decl, res);
                cx.fluxo = f;
                check_final_local(inf, cx, id, span);
            }
            let _ = escrita;
            if prefixo {
                res
            } else {
                leitura
            }
        }
    }
}

fn check_final_local(inf: &mut BodyInferrer<'_>, cx: &Corpo, id: LocalId, span: dartforge_diagnostics::Span) {
    let l = cx.local(id);
    if (l.final_ || l.const_) && !l.late {
        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
        inf.aviso(msg, span);
    }
}

/// Para `x op= e` / `x++`: lê o alvo, devolvendo `(tipo lido, tipo de escrita, local)`.
fn ler_para_escrita(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId) -> (TypeId, TypeId, Option<LocalId>) {
    let a = ast(inf, cx);
    match &a.expr(alvo).kind {
        ExprKind::Identifier(n) => {
            let n = *n;
            match resolver_nome(inf, cx, n.sym, true) {
                RefNome::Local(id) => {
                    resolver(inf, cx, alvo, Resolved::Local(id));
                    let t = ler_local(inf, cx, id, n.span);
                    let decl = cx.local(id).tipo;
                    (t, decl, Some(id))
                }
                _ => {
                    let leitura = identificador(inf, cx, alvo, n);
                    let escrita = tipo_de_escrita_nome(inf, cx, alvo, n);
                    (leitura, escrita, None)
                }
            }
        }
        ExprKind::Property { target, name, null_aware } => {
            let (target, name, null_aware) = (*target, *name, *null_aware);
            let (leitura, _) = propriedade(inf, cx, alvo, target, name, null_aware);
            registrar(inf, cx, alvo, leitura);
            (leitura, leitura, None)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (target, index, null_aware) = (*target, *index, *null_aware);
            let (recv, _) = receptor(inf, cx, target, null_aware);
            let op = inf.sym.indice;
            let sp = inf.span_expr(cx.unit, alvo);
            let u = inf.core.unknown;
            let (t, _) = operador_binario(inf, cx, recv, op, index, u, sp, Some(alvo));
            registrar(inf, cx, alvo, t);
            (t, t, None)
        }
        _ => {
            let t = inferir_livre(inf, cx, alvo);
            (t, t, None)
        }
    }
}

/// Tipo de escrita (setter) de um nome não local.
fn tipo_de_escrita_nome(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, n: ast::Name) -> TypeId {
    match resolver_nome(inf, cx, n.sym, true) {
        RefNome::Local(id) => cx.local(id).tipo,
        RefNome::Elemento(el) => {
            resolver(inf, cx, alvo, Resolved::Element(el));
            match el {
                Element::Variable(v) => {
                    let ve = inf.program.variable(v);
                    if ve.final_ || ve.const_ {
                        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(n.sym));
                        inf.aviso(msg, n.span);
                    }
                    inf.tipo_variavel(v)
                }
                Element::Function(f) => {
                    let fe = inf.program.function(f);
                    match (fe.kind, fe.variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                        (FunctionKind::Setter, _) => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        _ => inf.core.dynamic_,
                    }
                }
                _ => inf.core.dynamic_,
            }
        }
        RefNome::MembroLexico(f, estatico) => {
            let r = resolved_de_membro_lexico(inf, cx, f, estatico);
            resolver(inf, cx, alvo, r);
            let fe = inf.program.function(f);
            if let (FunctionKind::ImplicitAccessor, Some(v)) = (fe.kind, fe.variable) {
                let ve = inf.program.variable(v);
                if (ve.final_ || ve.const_) && ve.setter.is_none() {
                    // Campo final sem setter: pode haver setter herdado.
                    if let Some(this) = cx.tipo_this {
                        if let Busca::Achado(m) = inf.buscar_membro(cx.lib, this, n.sym, true) {
                            return m.tipo;
                        }
                    }
                }
            }
            if !estatico {
                if let Some(this) = cx.tipo_this {
                    if let Busca::Achado(m) = inf.buscar_membro(cx.lib, this, n.sym, true) {
                        return m.tipo;
                    }
                }
            }
            inf.tipo_do_membro_declarado(f, true).0
        }
        RefNome::ThisImplicito => {
            let this = cx.tipo_this.unwrap();
            match inf.buscar_membro(cx.lib, this, n.sym, true) {
                Busca::Achado(m) => {
                    resolver(inf, cx, alvo, m.resolved.clone());
                    m.tipo
                }
                Busca::Ausente => {
                    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
                    inf.aviso(msg, n.span);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        RefNome::ConstanteEnum(v) => inf.tipo_variavel(v),
        RefNome::Nenhum => {
            let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
            inf.aviso(msg, n.span);
            inf.core.dynamic_
        }
        _ => inf.core.dynamic_,
    }
}

/// Atribuições: `=`, compostas e `??=`.
fn atribuicao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: AssignOp, alvo: ExprId, valor: ExprId, curto: &mut bool) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    let a = ast(inf, cx);
    let alvo_kind = &a.expr(alvo).kind;
    match op {
        AssignOp::Assign => {
            // Tipo de escrita e contexto.
            let (escrita, contexto, local) = match alvo_kind {
                ExprKind::Identifier(n) => {
                    let n = *n;
                    match resolver_nome(inf, cx, n.sym, true) {
                        RefNome::Local(id) => {
                            resolver(inf, cx, alvo, Resolved::Local(id));
                            let l = cx.local(id).clone();
                            if (l.final_ || l.const_) && (!l.late || cx.fluxo.atribuida(id)) && !cx.fluxo.nao_atribuida(id) {
                                let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
                                inf.aviso(msg, span);
                            }
                            let atual = cx.fluxo.tipo_atual(id, l.tipo);
                            (l.tipo, atual, Some(id))
                        }
                        _ => {
                            let t = tipo_de_escrita_nome(inf, cx, alvo, n);
                            (t, t, None)
                        }
                    }
                }
                ExprKind::Property { target, name, null_aware } => {
                    let (target, name, null_aware) = (*target, *name, *null_aware);
                    let t = escrita_propriedade(inf, cx, alvo, target, name, null_aware, curto);
                    (t, t, None)
                }
                ExprKind::Index { target, index, null_aware } => {
                    let (target, index, null_aware) = (*target, *index, *null_aware);
                    let (recv, c) = receptor(inf, cx, target, null_aware);
                    *curto = c;
                    let t = escrita_indice(inf, cx, alvo, recv, index, span);
                    (t, t, None)
                }
                _ => {
                    let t = inferir_livre(inf, cx, alvo);
                    (t, t, None)
                }
            };
            let tv = inferir(inf, cx, valor, contexto);
            if let Some(id) = local {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.atribuir_fluxo(&mut f, id, decl, tv);
                cx.fluxo = f;
            }
            if !inf.e_dynamic(escrita) {
                let sp = inf.span_expr(cx.unit, valor);
                inf.verificar_atribuivel(tv, escrita, sp, INVALID_ASSIGNMENT.template);
            }
            tv
        }
        AssignOp::Compound(bop) => {
            let (leitura, escrita, local) = ler_para_escrita(inf, cx, alvo);
            if bop == BinaryOp::IfNull {
                let tv = inferir(inf, cx, valor, escrita);
                let nn = inf.nao_nulo(leitura);
                let t = inf.up(nn, tv);
                if let Some(id) = local {
                    let decl = cx.local(id).tipo;
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    inf.atribuir_fluxo(&mut f, id, decl, t);
                    cx.fluxo = f;
                }
                return t;
            }
            let sym = simbolo_operador(inf, bop);
            let u = inf.core.unknown;
            let (t, _) = operador_binario(inf, cx, leitura, sym, valor, u, span, Some(e));
            if let Some(id) = local {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.atribuir_fluxo(&mut f, id, decl, t);
                cx.fluxo = f;
                check_final_local(inf, cx, id, span);
            }
            t
        }
    }
}

/// `r.x = …`: tipo do setter.
fn escrita_propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, target: ExprId, name: ast::Name, null_aware: bool, curto: &mut bool) -> TypeId {
    let a = ast(inf, cx);
    if let ExprKind::Identifier(p) = &a.expr(target).kind {
        if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
            resolver(inf, cx, target, Resolved::Prefix(cx.lib));
            if let Some(el) = inf.program.lookup_prefixed(cx.lib, p.sym, name.sym).and_then(|b| b.setter.or(b.getter)) {
                resolver(inf, cx, alvo, Resolved::Element(el));
                return match el {
                    Element::Variable(v) => inf.tipo_variavel(v),
                    Element::Function(f) => {
                        let fe = inf.program.function(f);
                        match (fe.kind, fe.variable) {
                            (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                            _ => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        }
                    }
                    _ => inf.core.dynamic_,
                };
            }
            return inf.core.dynamic_;
        }
    }
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        registrar_ref_tipo(inf, cx, target);
        let m = match rt {
            RefTipo::Classe(c, _) | RefTipo::Alias(c, _) => inf.membro_estatico(c, name.sym, true),
            RefTipo::Extensao(x) => inf.membro_estatico_de_extensao(x, name.sym, true),
        };
        return match m {
            Some(m) => {
                resolver(inf, cx, alvo, m.resolved.clone());
                m.tipo
            }
            None => inf.core.dynamic_,
        };
    }
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
        registrar(inf, cx, target, this);
        return membro_super(inf, cx, alvo, name, true);
    }
    let (recv, c) = receptor(inf, cx, target, null_aware);
    *curto = c;
    match inf.buscar_membro(cx.lib, recv, name.sym, true) {
        Busca::Achado(m) => {
            resolver(inf, cx, alvo, m.resolved.clone());
            m.tipo
        }
        Busca::Ausente => {
            let msg = format!(
                "{}: setter '{}' não definido para o tipo '{}'",
                UNDEFINED_SETTER.template,
                inf.interner.resolve(name.sym),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.aviso(msg, name.span);
            inf.core.dynamic_
        }
        _ => {
            resolver(inf, cx, alvo, Resolved::Dynamic);
            inf.core.dynamic_
        }
    }
}

/// `r[i] = …`: tipo do valor em `[]=`.
fn escrita_indice(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, recv: TypeId, index: ExprId, span: dartforge_diagnostics::Span) -> TypeId {
    let Some(op) = inf.sym.indice_set else {
        inferir_livre(inf, cx, index);
        return inf.core.dynamic_;
    };
    match inf.buscar_membro(cx.lib, recv, op, false) {
        Busca::Achado(m) => {
            resolver(inf, cx, alvo, m.resolved.clone());
            let (pi, pv) = match inf.table.get(m.tipo).clone() {
                Type::Function { positional, .. } => (positional.first().copied(), positional.get(1).copied()),
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let ti = inferir(inf, cx, index, pi.unwrap_or(u));
            if let Some(p) = pi {
                let sp = inf.span_expr(cx.unit, index);
                inf.verificar_atribuivel(ti, p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
            }
            pv.unwrap_or(inf.core.dynamic_)
        }
        Busca::Ausente => {
            inferir_livre(inf, cx, index);
            let msg = format!("{}: operador '[]=' para o tipo '{}'", UNDEFINED_METHOD.template, inf.table.format(recv, inf.interner, inf.program));
            inf.aviso(msg, span);
            inf.core.dynamic_
        }
        _ => {
            inferir_livre(inf, cx, index);
            inf.core.dynamic_
        }
    }
}

// -------------------------------------------------------------------
// Condições: modelos de fluxo verdadeiro/falso
// -------------------------------------------------------------------

/// Infere uma condição (contexto `bool`) e devolve `(true(E), false(E))`.
/// Deixa `cx.fluxo` no estado de antes (o chamador escolhe o ramo).
pub(crate) fn condicao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> (Fluxo, Fluxo) {
    let a = ast(inf, cx);
    let kind = &a.expr(e).kind;
    let span = a.expr(e).span;
    match kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            let r = condicao(inf, cx, i);
            let t = inf.body_types.units[cx.unit.0 as usize].get_type(i).unwrap_or(inf.core.bool_);
            registrar(inf, cx, e, t);
            r
        }
        ExprKind::Bool(b) => {
            let b = *b;
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            let f = cx.fluxo.clone();
            if b {
                (f.clone(), f.inalcancavel())
            } else {
                (f.inalcancavel(), f)
            }
        }
        ExprKind::Unary { op: UnaryOp::Not, operand } => {
            let o = *operand;
            let (v, f) = condicao(inf, cx, o);
            verificar_bool(inf, cx, o, NON_BOOL_NEGATION_EXPRESSION.template);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            (f, v)
        }
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq) => {
            let (op, left, right) = (*op, *left, *right);
            let r = condicao_binaria(inf, cx, e, op, left, right);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            r
        }
        ExprKind::Is { value, ty, negated } => {
            let (value, ty, negated) = (*value, *ty, *negated);
            let r = teste_de_tipo(inf, cx, value, ty, negated, span);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            r
        }
        _ => {
            let b = inf.core.bool_;
            inferir(inf, cx, e, b);
            let f = cx.fluxo.clone();
            (f.clone(), f)
        }
    }
}

/// Condição de `if`/`while`/`?:`/`assert`, com o diagnóstico de não-`bool`.
pub(crate) fn condicao_verificada(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> (Fluxo, Fluxo) {
    let r = condicao(inf, cx, e);
    verificar_bool(inf, cx, e, NON_BOOL_CONDITION.template);
    r
}

fn verificar_bool(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, template: &str) {
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.bool_);
    let b = inf.core.bool_;
    if !inf.atribuivel(t, b) {
        let sp = inf.span_expr(cx.unit, e);
        inf.aviso(template.to_string(), sp);
    }
}

fn condicao_binaria(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId) -> (Fluxo, Fluxo) {
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let antes = cx.fluxo.clone();
            let (lv, lf) = condicao(inf, cx, left);
            verificar_bool(inf, cx, left, NON_BOOL_CONDITION.template);
            cx.fluxo = if op == BinaryOp::And { lv.clone() } else { lf.clone() };
            let (rv, rf) = condicao(inf, cx, right);
            verificar_bool(inf, cx, right, NON_BOOL_CONDITION.template);
            cx.fluxo = antes;
            if op == BinaryOp::And {
                let falso = inf.juntar(&lf, &rf);
                (rv, falso)
            } else {
                let verdadeiro = inf.juntar(&lv, &rv);
                (verdadeiro, rf)
            }
        }
        _ => {
            // `==` / `!=`
            let u = inf.core.unknown;
            let tl = inferir(inf, cx, left, u);
            let tr = inferir(inf, cx, right, u);
            if matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, left, this);
            }
            if let Some(eq) = inf.sym.igual {
                if let Busca::Achado(m) = inf.buscar_membro(cx.lib, tl, eq, false) {
                    resolver(inf, cx, e, m.resolved);
                }
            }
            let depois = cx.fluxo.clone();
            let a = ast(inf, cx);
            let e_nulo = |x: ExprId| {
                let mut x = x;
                loop {
                    match &a.expr(x).kind {
                        ExprKind::Parenthesized(i) => x = *i,
                        ExprKind::Null => return true,
                        _ => return false,
                    }
                }
            };
            let alvo = if e_nulo(right) {
                alvo_de_promocao(inf, cx, left)
            } else if e_nulo(left) {
                alvo_de_promocao(inf, cx, right)
            } else {
                None
            };
            let _ = (tl, tr);
            let (mut igual, mut diferente) = (depois.clone(), depois.clone());
            if let Some(id) = alvo {
                let decl = cx.local(id).tipo;
                inf.promover_nao_nulo(&mut diferente, id, decl);
                let _ = &mut igual;
            }
            if op == BinaryOp::Eq {
                (igual, diferente)
            } else {
                (diferente, igual)
            }
        }
    }
}

/// `e is T` / `e is! T`: `(true, false)` com promoção.
fn teste_de_tipo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, value: ExprId, ty: ast::TypeId, negado: bool, span: dartforge_diagnostics::Span) -> (Fluxo, Fluxo) {
    let v = inferir_livre(inf, cx, value);
    let t = inf.tipo_de_anotacao(cx, ty);
    if t == inf.core.object && !inf.e_dynamic(v) && inf.sub(v, t) && !negado {
        inf.aviso(UNNECESSARY_TYPE_CHECK_TRUE.template.to_string(), span);
    }
    let depois = cx.fluxo.clone();
    let (mut sim, mut nao) = (depois.clone(), depois.clone());
    if let Some(id) = alvo_de_promocao(inf, cx, value) {
        let decl = cx.local(id).tipo;
        inf.promover(&mut sim, id, decl, t);
        let fatorado = fator(inf, v, t);
        if fatorado != v {
            inf.promover(&mut nao, id, decl, fatorado);
        }
    }
    if negado {
        (nao, sim)
    } else {
        (sim, nao)
    }
}

/// `factor(T, S)` (`flow-analysis.md`).
fn fator(inf: &mut BodyInferrer<'_>, t: TypeId, s: TypeId) -> TypeId {
    if inf.sub(t, s) {
        return inf.core.never;
    }
    let n = inf.core.null;
    match inf.table.get(t).clone() {
        ty if ty.is_declared_nullable() && !matches!(ty, Type::FutureOr { .. }) => {
            let r = inf.nao_nulo(t);
            let f = fator(inf, r, s);
            if inf.sub(n, s) {
                f
            } else {
                inf.anulavel(f)
            }
        }
        Type::FutureOr { arg, nullable: false } => {
            let fut = inf.futuro(arg);
            if inf.sub(fut, s) {
                fator(inf, arg, s)
            } else if inf.sub(arg, s) {
                fator(inf, fut, s)
            } else {
                t
            }
        }
        _ => t,
    }
}

/// Declara um local (com tabela lateral de tipo por offset).
pub(crate) fn declarar_local(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, local: Local, inicializado: bool) -> LocalId {
    let (offset, tipo) = (local.offset, local.tipo);
    let id = cx.declarar(local);
    if inicializado {
        cx.fluxo.inicializar(id);
    }
    inf.body_types.units[cx.unit.0 as usize].set_tipo_local(offset, tipo);
    id
}
