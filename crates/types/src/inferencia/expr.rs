//! Inferência de expressões: contexto (esquema) para baixo, tipo estático
//! para cima (`inference.md`, "Expression inference").
//!
//! Cadeias com `?.` fazem *null-shorting*: os nós internos da cadeia recebem
//! o tipo não anulável e só o nó que termina a cadeia recebe o `?`
//! ([`inferir_no`] devolve se a cadeia está em curto).

use super::atalhos;
use super::chamadas;
use super::colecoes;
use super::corpo::{Base, Corpo, Local, Nome};
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
    let marca = cx.cadeias.len();
    let (t, curto) = inferir_no(inf, cx, e, ctx, false);
    fechar_cadeia(inf, cx, marca);
    if curto {
        let t = inf.anulavel(t);
        registrar(inf, cx, e, t);
        t
    } else {
        t
    }
}

/// Fim de uma cadeia com `?.`: a promoção do receptor valeu só dentro dela;
/// depois, o fluxo é a junção do "era nulo" (o de antes do primeiro `?.`) com
/// o de ter percorrido a cadeia.
fn fechar_cadeia(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, marca: usize) {
    if cx.cadeias.len() > marca {
        let antes = cx.cadeias[marca].clone();
        cx.cadeias.truncate(marca);
        let depois = cx.fluxo.clone();
        cx.fluxo = inf.juntar(&antes, &depois);
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
    /// Typedef para classe: argumentos da classe (`None`: o typedef só
    /// renomeia os parâmetros, e os argumentos são os explícitos ou
    /// inferidos como os da classe).
    Alias(ClassId, Option<Vec<TypeId>>, dartforge_elements::model::TypedefId),
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
                RefTipo::Alias(_, _, td) => {
                    let ex: Vec<TypeId> = targs.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect();
                    alias_de(inf, td, Some(ex))
                }
                _ => None,
            };
        }
        _ => return None,
    };
    match el {
        Element::Class(c) => Some(RefTipo::Classe(c, None)),
        Element::Extension(x) => Some(RefTipo::Extensao(x)),
        Element::Typedef(td) => alias_de(inf, td, None),
        _ => None,
    }
}

/// Classe e argumentos de um typedef usado como classe, com os argumentos
/// explícitos do typedef (ou sem eles).
fn alias_de(inf: &mut BodyInferrer<'_>, td: dartforge_elements::model::TypedefId, explicitos: Option<Vec<TypeId>>) -> Option<RefTipo> {
    let alvo = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let (class, args) = match inf.table.get(alvo).clone() {
        Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (class, args),
        _ => return None,
    };
    if params.is_empty() {
        return Some(RefTipo::Alias(class, Some(args.to_vec()), td));
    }
    let inst = match explicitos {
        Some(ex) if ex.len() == params.len() => ex,
        _ => {
            // Typedef que só renomeia (`typedef M<K, V> = _M<K, V>`): os
            // argumentos se inferem como os da classe.
            let renomeia = args.len() == params.len()
                && args.iter().zip(params.iter()).all(|(&a, &p)| matches!(inf.table.get(a), Type::TypeParameter { param, nullable: false } if *param == p));
            if renomeia {
                return Some(RefTipo::Alias(class, None, td));
            }
            inf.instanciar_para_limites(&params)
        }
    };
    let mapa = inf.mapa(&params, &inst);
    let args = args.iter().map(|a| inf.subst(*a, &mapa)).collect();
    Some(RefTipo::Alias(class, Some(args), td))
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
                        return leitura_de_campo(inf, cx, e, Base::This, m.tipo);
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
                    leitura_de_campo(inf, cx, e, Base::This, m.tipo)
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
        // Só curingas declaram `_` aqui (3.7): usar `_` é erro
        // (`Undefined name '_'` no CFE).
        RefNome::Nenhum if cx.curinga == Some(n.sym) => {
            inf.erro_de_linguagem(cx.unit, n.span, WILDCARD_NAO_LIGA.template.to_string());
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
pub(crate) fn alvo_de_promocao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> Option<LocalId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            alvo_de_promocao(inf, cx, i)
        }
        ExprKind::Identifier(n) => match cx.buscar(n.sym) {
            Some(Nome::Local(id)) if !cx.local(id).funcao_local => Some(id),
            Some(_) => None,
            // `_x` implícito: `this._x`.
            None => alvo_de_campo(inf, cx, e, Base::This),
        },
        ExprKind::Property { target, null_aware: false, .. } => {
            let t = *target;
            let base = match &a.expr(t).kind {
                ExprKind::This => Base::This,
                ExprKind::Identifier(n) => match cx.buscar(n.sym) {
                    Some(Nome::Local(id)) if !cx.local(id).funcao_local && !cx.local(id).late => Base::Local(id),
                    _ => return None,
                },
                _ => return None,
            };
            alvo_de_campo(inf, cx, e, base)
        }
        _ => None,
    }
}

/// Local sintético do campo promovível que `e` (já inferida) lê.
fn alvo_de_campo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, base: Base) -> Option<LocalId> {
    let v = campo_promovivel(inf, cx, e)?;
    if let Some(&id) = cx.campos.get(&(base, v)) {
        return Some(id);
    }
    // Tipo declarado do campo visto pelo receptor: o tipo da leitura antes
    // de qualquer promoção (a primeira leitura, que criou nada ainda).
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.dynamic_);
    let nome = inf.program.variable(v).name;
    let id = cx.declarar_sintetico(Local { nome, tipo: t, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.campos.insert((base, v), id);
    Some(id)
}

/// O campo que `e` lê, se é promovível (Dart 3.2): de instância, `final`,
/// privado, não `external`, e nenhuma outra declaração da biblioteca com o
/// mesmo nome o impede (getter concreto ou campo não final).
fn campo_promovivel(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<dartforge_elements::model::VariableId> {
    // Só em biblioteca com versão de linguagem 3.2 ou mais (o recurso
    // `inference-update-2`; pacote antigo como o built_collection não
    // promove, e o `!` dele é necessário).
    if inf.program.library(cx.lib).features.versao() < dartforge_frontend::LanguageVersion::new(3, 2) {
        return None;
    }
    let r = inf.body_types.units[cx.unit.0 as usize].get_resolved(e)?.clone();
    let Resolved::Member { member: MemberRef::Function(f), .. } = r else { return None };
    let fe = inf.program.function(f);
    if fe.kind != FunctionKind::ImplicitAccessor {
        return None;
    }
    let v = fe.variable?;
    inf.campo_e_promovivel(v).then_some(v)
}

impl<'a> BodyInferrer<'a> {
    /// Regra de promoção de campos (`inference-update-2`, Dart 3.2).
    pub(crate) fn campo_e_promovivel(&mut self, v: dartforge_elements::model::VariableId) -> bool {
        if let Some(&r) = self.promoviveis.get(&v.0) {
            return r;
        }
        let ve = self.program.variable(v);
        let nome = self.interner.resolve(ve.name);
        let lib = ve.library;
        let mut ok = ve.final_ && !ve.static_ && !ve.external && ve.class.is_some() && nome.starts_with('_');
        if ok {
            // Nenhuma outra declaração homônima na biblioteca que o impeça.
            let sym = ve.name;
            for (ci, c) in self.program.classes.iter().enumerate() {
                if c.library != lib {
                    continue;
                }
                if let Some(&g) = c.instance_members.get(&sym) {
                    let ge = self.program.function(g);
                    let impede = match (ge.kind, ge.variable) {
                        (FunctionKind::ImplicitAccessor, Some(w)) => {
                            let we = self.program.variable(w);
                            !(we.final_ && !we.external)
                        }
                        (FunctionKind::Getter, _) => !ge.abstract_,
                        _ => false,
                    };
                    if impede {
                        ok = false;
                        break;
                    }
                }
                let _ = ci;
            }
        }
        self.promoviveis.insert(v.0, ok);
        ok
    }
}

/// Tipo lido de um campo promovível (promovido pelo fluxo, se houver).
fn leitura_de_campo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, base: Base, t: TypeId) -> TypeId {
    let Some(v) = campo_promovivel(inf, cx, e) else { return t };
    match cx.campos.get(&(base, v)) {
        Some(&id) => cx.fluxo.tipo_atual(id, t),
        None => t,
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
    atalhos::registrar_cadeia(inf, cx, e, ctx);
    let t = match &expr.kind {
        ExprKind::DotShorthand { name, .. } => atalhos::valor(inf, cx, e, name.sym, ctx),
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
            let marca = cx.cadeias.len();
            let (t, c) = inferir_no(inf, cx, *i, ctx, false);
            fechar_cadeia(inf, cx, marca);
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
            let (t, c) = ler_indice(inf, cx, e, *target, *index, *null_aware, ctx);
            curto = c;
            t
        }
        ExprKind::Call { .. } => {
            if let Some(t) = atalhos::construcao(inf, cx, e, ctx) {
                t
            } else {
                let (t, c) = chamadas::chamada(inf, cx, e, ctx);
                curto = c;
                t
            }
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
        // Promove o receptor dentro da cadeia; `fechar_cadeia` desfaz no fim.
        cx.cadeias.push(cx.fluxo.clone());
        if let Some(id) = alvo_de_promocao(inf, cx, r) {
            let decl = cx.local(id).tipo;
            let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
            inf.promover_nao_nulo(&mut f, id, decl);
            cx.fluxo = f;
        }
        let nn = inf.nao_nulo(t);
        (nn, true)
    } else {
        (t, c)
    }
}

/// Busca de membro no receptor `target`: sobreposição explícita de
/// extensão (`E(x).m`) consulta só a extensão; senão a busca normal.
pub(crate) fn buscar_membro_do_alvo(inf: &mut BodyInferrer<'_>, cx: &Corpo, target: ExprId, recv: TypeId, nome: SymbolId, setter: bool) -> Busca {
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        return match inf.membro_de_extensao_explicita(x, &args, nome, setter) {
            Some(m) => Busca::Achado(m),
            None => Busca::Ausente,
        };
    }
    inf.buscar_membro(cx.lib, recv, nome, setter)
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
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        if inf.membro_de_extensao_explicita(x, &args, name.sym, false).is_none()
            && inf.membro_estatico_de_extensao(x, name.sym, false).is_some()
        {
            inf.aviso(EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(), name.span);
            return (inf.core.dynamic_, curto);
        }
    }
    let base = if null_aware {
        None
    } else {
        match &a.expr(target).kind {
            ExprKind::This => Some(Base::This),
            ExprKind::Identifier(n) => match cx.buscar(n.sym) {
                Some(Nome::Local(id)) if !cx.local(id).late => Some(Base::Local(id)),
                _ => None,
            },
            _ => None,
        }
    };
    let t = match buscar_membro_do_alvo(inf, cx, target, recv, name.sym, false) {
        Busca::Achado(m) => {
            resolver(inf, cx, e, m.resolved.clone());
            match base {
                Some(b) => leitura_de_campo(inf, cx, e, b, m.tipo),
                None => m.tipo,
            }
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
            if let Some((x, _)) = cx.sobreposicoes.get(&target).cloned() {
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_GETTER.template, inf.interner.resolve(name.sym), extensao);
                inf.aviso(msg, name.span);
                return (inf.core.dynamic_, curto);
            }
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
            None => {
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_GETTER.template, inf.interner.resolve(name.sym), extensao);
                inf.aviso(msg, name.span);
                inf.core.dynamic_
            }
        },
        RefTipo::Classe(c, targs) => {
            if let Some(m) = inf.membro_estatico(c, name.sym, false) {
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            let args = targs.map(|v| v.iter().map(|&t| inf.tipo_de_anotacao(cx, t)).collect::<Vec<_>>());
            tearoff_de_construtor(inf, cx, e, c, args, name)
        }
        RefTipo::Alias(c, args, _) => {
            if let Some(m) = inf.membro_estatico(c, name.sym, false) {
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            tearoff_de_construtor(inf, cx, e, c, args, name)
        }
    }
}

/// `C.nome` / `C.new` como valor: tipo de função do construtor (genérico
/// sobre os parâmetros da classe se não instanciado).
fn tearoff_de_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, c: ClassId, args: Option<Vec<TypeId>>, name: ast::Name) -> TypeId {
    let chave = if Some(name.sym) == inf.sym.new_ { inf.sym.vazio } else { Some(name.sym) };
    let Some(chave) = chave else { return inf.core.dynamic_ };
    let Some(f) = inf.construtor_de(c, chave) else {
        let msg = format!("{}: getter '{}' não definido para a classe", UNDEFINED_GETTER.template, inf.interner.resolve(name.sym));
        inf.aviso(msg, name.span);
        return inf.core.dynamic_;
    };
    if inf.program.function(f).class == Some(c) {
        resolver(inf, cx, e, Resolved::Constructor(f));
    }
    let sig = inf.assinatura_construtor(c, f);
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
fn texto_operador(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
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
    })
}

fn simbolo_operador(inf: &BodyInferrer<'_>, op: BinaryOp) -> Option<SymbolId> {
    inf.interner.lookup(texto_operador(op)?)
}

fn pular_espacos_e_comentarios(trecho: &str) -> usize {
    let bytes = trecho.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
        } else if bytes.get(pos..pos + 2) == Some(b"/*") {
            pos += 2;
            while pos + 1 < bytes.len() && &bytes[pos..pos + 2] != b"*/" { pos += 1; }
            pos = (pos + 2).min(bytes.len());
        } else if bytes.get(pos..pos + 2) == Some(b"//") {
            while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
        } else {
            break;
        }
    }
    pos
}

fn span_indice(inf: &BodyInferrer<'_>, cx: &Corpo, alvo: ExprId, target: ExprId) -> dartforge_diagnostics::Span {
    let todo = inf.span_expr(cx.unit, alvo);
    let inicio = inf.span_expr(cx.unit, target).end;
    let entre = inf.program.unit(cx.unit).source.get(inicio..todo.end).unwrap_or("");
    let pos = pular_espacos_e_comentarios(entre);
    let colchete = inicio + pos + entre.get(pos..).unwrap_or("").find('[').unwrap_or(0);
    dartforge_diagnostics::Span { start: colchete, end: todo.end }
}

fn avisar_operador_de_extensao(inf: &mut BodyInferrer<'_>, x: ExtensionId, nome: &str, span: dartforge_diagnostics::Span) {
    let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
    let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, nome, extensao);
    inf.aviso(msg, span);
}

fn ler_indice(
    inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, target: ExprId,
    index: ExprId, null_aware: bool, ctx: TypeId,
) -> (TypeId, bool) {
    let (recv, curto) = receptor(inf, cx, target, null_aware);
    let op = inf.sym.indice;
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        if let Some((s, m)) = op.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false).map(|m| (s, m))) {
            return (operador_binario_com_membro(inf, cx, recv, s, index, ctx, Some(alvo), m).0, curto);
        }
        inferir_livre(inf, cx, index);
        let span = span_indice(inf, cx, alvo, target);
        avisar_operador_de_extensao(inf, x, "[]", span);
        return (inf.core.dynamic_, curto);
    }
    let span = inf.span_expr(cx.unit, alvo);
    (operador_binario(inf, cx, recv, op, index, ctx, span, Some(alvo)).0, curto)
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
    let Some(op) = op else {
        inferir_livre(inf, cx, arg);
        return (inf.core.dynamic_, None);
    };
    match inf.buscar_membro(cx.lib, recv, op, false) {
        Busca::Achado(m) => operador_binario_com_membro(inf, cx, recv, op, arg, ctx, no, m),
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

fn operador_binario_com_membro(
    inf: &mut BodyInferrer<'_>, cx: &mut Corpo, recv: TypeId, op: SymbolId,
    arg: ExprId, ctx: TypeId, no: Option<ExprId>, m: Membro,
) -> (TypeId, Option<Membro>) {
    if let Some(n) = no {
        resolver(inf, cx, n, m.resolved.clone());
    }
    let (param, ret) = match inf.table.get(m.tipo).clone() {
        Type::Function { positional, optional, ret, .. } => (positional.first().or(optional.first()).copied(), ret),
        _ => (None, inf.core.dynamic_),
    };
    let ctx_arg = match param {
        Some(p) => contexto_numerico(inf, recv, &m, op, ctx, p),
        None => inf.core.unknown,
    };
    let ta = inferir(inf, cx, arg, ctx_arg);
    if let Some(p) = param {
        let sp = inf.span_expr(cx.unit, arg);
        inf.verificar_atribuivel(ta, p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
    }
    let t = refinar_numerico(inf, recv, &m, op, &[ta], ret);
    (t, Some(m))
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
            if let Some((x, args)) = cx.sobreposicoes.get(&left).cloned()
                && let Some(texto) = texto_operador(op)
            {
                if let Some((s, m)) = sym.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false).map(|m| (s, m))) {
                    return operador_binario_com_membro(inf, cx, l, s, right, ctx, Some(e), m).0;
                }
                let inicio = inf.span_expr(cx.unit, left).end;
                let fim = inf.span_expr(cx.unit, right).start;
                let trecho = inf.program.unit(cx.unit).source.get(inicio..fim).unwrap_or("");
                let pos = pular_espacos_e_comentarios(trecho);
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, texto, extensao);
                if trecho.get(pos..).is_some_and(|resto| resto.starts_with(texto)) {
                    let offset = inicio + pos;
                    inf.aviso(msg, dartforge_diagnostics::Span { start: offset, end: offset + texto.len() });
                }
                inferir_livre(inf, cx, right);
                return inf.core.dynamic_;
            }
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
            // `-1` com literal: o literal recebe o contexto (`double x = -1`).
            let literal = op == UnaryOp::Neg && matches!(inf.program.unit(cx.unit).ast.expr(operand).kind, ExprKind::Int(_));
            let c = if literal { _ctx } else { inf.core.unknown };
            let t = inferir(inf, cx, operand, c);
            let sym = if op == UnaryOp::Neg { inf.sym.menos_unario } else { inf.sym.til };
            if let Some((x, args)) = cx.sobreposicoes.get(&operand).cloned() {
                if let Some(m) = sym.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false)) {
                    resolver(inf, cx, e, m.resolved.clone());
                    return match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    };
                }
                let operador = if op == UnaryOp::Neg { "unary-" } else { "~" };
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, operador, extensao);
                inf.aviso(msg, dartforge_diagnostics::Span { start: span.start, end: span.start + 1 });
                return inf.core.dynamic_;
            }
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
                let alvo_span = inf.span_expr(cx.unit, operand);
                check_final_local(inf, cx, id, alvo_span);
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
    if l.const_ {
        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), span);
    } else if l.final_ && !l.late {
        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
        inf.aviso(msg, span);
    }
}

/// Diagnóstico quando a recuperação de uma escrita encontra apenas o getter.
/// O getter explícito é uma propriedade sem setter; o getter implícito é um
/// campo `final` ou `const`. `late final` sem inicializador ainda aceita escrita.
fn avisar_membro_sem_setter(inf: &mut BodyInferrer<'_>, nome: ast::Name, f: dartforge_elements::model::FunctionElementId) -> bool {
    let fe = inf.program.function(f);
    match fe.kind {
        FunctionKind::Getter => {
            let dono = fe.class.map(|c| inf.program.class(c).name)
                .or_else(|| fe.extension.and_then(|e| inf.program.extension(e).name));
            let Some(dono) = dono else { return false };
            let msg = format!(
                "{}: '{}' na classe '{}'",
                ASSIGNMENT_TO_FINAL_NO_SETTER.template,
                inf.interner.resolve(nome.sym),
                inf.interner.resolve(dono)
            );
            inf.aviso(msg, nome.span);
            true
        }
        FunctionKind::ImplicitAccessor => {
            let Some(v) = fe.variable else { return false };
            let ve = inf.program.variable(v);
            if ve.const_ {
                inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), nome.span);
            } else if ve.final_ && !(ve.late && inf.inicializador(v).is_none()) {
                let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(nome.sym));
                inf.aviso(msg, nome.span);
            } else {
                return false;
            }
            true
        }
        _ => false,
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
            let (leitura, curto_lido) = propriedade(inf, cx, alvo, target, name, null_aware);
            let recv_lido = inf.body_types.units[cx.unit.0 as usize].get_type(target).map(|t| {
                (if null_aware { inf.nao_nulo(t) } else { t }, curto_lido)
            });
            let mut curto = false;
            let escrita = escrita_propriedade(inf, cx, alvo, target, name, null_aware, &mut curto, recv_lido);
            registrar(inf, cx, alvo, leitura);
            (leitura, escrita, None)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (target, index, null_aware) = (*target, *index, *null_aware);
            let u = inf.core.unknown;
            let (t, _) = ler_indice(inf, cx, alvo, target, index, null_aware, u);
            if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned()
                && inf.sym.indice_set.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false)).is_none()
            {
                let span = span_indice(inf, cx, alvo, target);
                avisar_operador_de_extensao(inf, x, "[]=", span);
            }
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
                    if ve.const_ {
                        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), n.span);
                    } else if ve.final_ {
                        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(n.sym));
                        inf.aviso(msg, n.span);
                    }
                    inf.tipo_variavel(v)
                }
                Element::Function(f) => {
                    let fe = inf.program.function(f);
                    match (fe.kind, fe.variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                        (FunctionKind::Setter, _) => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        (FunctionKind::Getter, _) => {
                            let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(n.sym));
                            inf.aviso(msg, n.span);
                            inf.outline.functions[f.0 as usize].return_type
                        }
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
            avisar_membro_sem_setter(inf, n, f);
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
                    if let Busca::Achado(getter) = inf.buscar_membro(cx.lib, this, n.sym, false) {
                        if let Some(f) = getter.funcao {
                            if avisar_membro_sem_setter(inf, n, f) {
                                resolver(inf, cx, alvo, getter.resolved);
                                return getter.tipo;
                            }
                        }
                    }
                    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
                    inf.aviso(msg, n.span);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        RefNome::ConstanteEnum(v) => inf.tipo_variavel(v),
        // Só curingas declaram `_` aqui (3.7): usar `_` é erro
        // (`Undefined name '_'` no CFE).
        RefNome::Nenhum if cx.curinga == Some(n.sym) => {
            inf.erro_de_linguagem(cx.unit, n.span, WILDCARD_NAO_LIGA.template.to_string());
            inf.core.dynamic_
        }
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
                                if l.const_ {
                                    inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), n.span);
                                } else {
                                    let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
                                    inf.aviso(msg, n.span);
                                }
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
                    let t = escrita_propriedade(inf, cx, alvo, target, name, null_aware, curto, None);
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
                cx.esquecer_campos_de(id);
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
                let alvo_span = inf.span_expr(cx.unit, alvo);
                check_final_local(inf, cx, id, alvo_span);
            }
            t
        }
    }
}

/// `r.x = …`: tipo do setter.
fn escrita_propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, target: ExprId, name: ast::Name, null_aware: bool, curto: &mut bool, recv_lido: Option<(TypeId, bool)>) -> TypeId {
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
        let m = match &rt {
            RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => inf.membro_estatico(*c, name.sym, true),
            RefTipo::Extensao(x) => inf.membro_estatico_de_extensao(*x, name.sym, true),
        };
        return match m {
            Some(m) => {
                resolver(inf, cx, alvo, m.resolved.clone());
                m.tipo
            }
            None => {
                let getter = match rt {
                    RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => inf.membro_estatico(c, name.sym, false),
                    RefTipo::Extensao(x) => inf.membro_estatico_de_extensao(x, name.sym, false),
                };
                if let Some(getter) = getter {
                    if let Some(f) = getter.funcao {
                        if avisar_membro_sem_setter(inf, name, f) {
                            resolver(inf, cx, alvo, getter.resolved);
                            return getter.tipo;
                        }
                    }
                }
                inf.core.dynamic_
            }
        };
    }
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
        registrar(inf, cx, target, this);
        return membro_super(inf, cx, alvo, name, true);
    }
    let (recv, c) = recv_lido.unwrap_or_else(|| receptor(inf, cx, target, null_aware));
    *curto = c;
    // `E(valor).m` força a extensão nomeada: na falta de setter ela emite
    // `undefined_extension_setter`, mesmo que a extensão tenha um getter `m`.
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        if let Some(m) = inf.membro_de_extensao_explicita(x, &args, name.sym, true) {
            resolver(inf, cx, alvo, m.resolved);
            return m.tipo;
        }
        if inf.membro_estatico_de_extensao(x, name.sym, true).is_some() {
            // `+=` resolve leitura e escrita do mesmo nome. O analyzer relata
            // o acesso estático uma vez, mesmo quando ambos os lados resolvem.
            let ja_reportado = inf.diagnostics.iter().zip(&inf.unidades_dos_avisos).any(|(d, unidade)| {
                *unidade == inf.unidade_corrente
                    && d.span == name.span
                    && d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template
            });
            if !ja_reportado {
                inf.aviso(EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(), name.span);
            }
            return inf.core.dynamic_;
        }
        let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_SETTER.template, inf.interner.resolve(name.sym), extensao);
        inf.aviso(msg, name.span);
        return inf.core.dynamic_;
    }
    match inf.buscar_membro(cx.lib, recv, name.sym, true) {
        Busca::Achado(m) => {
            resolver(inf, cx, alvo, m.resolved.clone());
            m.tipo
        }
        Busca::Ausente => {
            // O analyzer recupera o getter quando a escrita não encontra um
            // setter. Um getter declarado numa classe tem diagnóstico próprio;
            // sem getter, continua sendo um setter indefinido.
            if let Busca::Achado(getter) = inf.buscar_membro(cx.lib, recv, name.sym, false) {
                if let Some(f) = getter.funcao {
                    if avisar_membro_sem_setter(inf, name, f) {
                        resolver(inf, cx, alvo, getter.resolved);
                        return getter.tipo;
                    }
                }
            }
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
    let target = match &ast(inf, cx).expr(alvo).kind {
        ExprKind::Index { target, .. } => Some(*target),
        _ => None,
    };
    let busca = if let Some(target) = target
        && let Some((x, args)) = cx.sobreposicoes.get(&target).cloned()
    {
        match inf.sym.indice_set.and_then(|op| inf.membro_de_extensao_explicita(x, &args, op, false)) {
            Some(m) => Busca::Achado(m),
            None => {
                inferir_livre(inf, cx, index);
                let sp = span_indice(inf, cx, alvo, target);
                avisar_operador_de_extensao(inf, x, "[]=", sp);
                return inf.core.dynamic_;
            }
        }
    } else {
        let Some(op) = inf.sym.indice_set else {
            inferir_livre(inf, cx, index);
            return inf.core.dynamic_;
        };
        inf.buscar_membro(cx.lib, recv, op, false)
    };
    match busca {
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
            // Leitura de variável de condição não reescrita (§7.10).
            if let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind
                && let Some(Nome::Local(id)) = cx.buscar(n.sym)
                && let Some((sim, nao, versao)) = cx.condicoes.get(&id).cloned()
                && f.versao(id) == Some(versao)
                && !f.modelo(id).is_some_and(|m| m.capturada)
            {
                let v = inf.reaplicar(&f, &sim);
                let fa = inf.reaplicar(&f, &nao);
                return (v, fa);
            }
            (f.clone(), f)
        }
    }
}

/// Expressão cuja informação de condição não é trivial (`==`, `!=`, `&&`,
/// `||`, `!`, `is`), para guardar numa variável de condição.
pub(crate) fn e_forma_de_condicao(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    match &ast(inf, cx).expr(e).kind {
        ExprKind::Parenthesized(i) => e_forma_de_condicao(inf, cx, *i),
        ExprKind::Unary { op: UnaryOp::Not, .. } | ExprKind::Is { .. } => true,
        ExprKind::Binary { op, .. } => matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq),
        _ => false,
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
            // `e == .x`: o atalho à direita usa o tipo de `e` (3.10).
            atalhos::registrar_igualdade(inf, cx, right, tl);
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
            // O alvo (um campo) pode ter sido criado agora: o fluxo corrente o tem.
            let depois = if alvo.is_some() { cx.fluxo.clone() } else { depois };
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
    let alvo = alvo_de_promocao(inf, cx, value);
    let depois = cx.fluxo.clone();
    let (mut sim, mut nao) = (depois.clone(), depois.clone());
    if let Some(id) = alvo {
        let decl = cx.local(id).tipo;
        inf.promover(&mut sim, id, decl, t);
        let fatorado = fator(inf, v, t);
        inf.promover_testado(&mut nao, id, decl, fatorado, t);
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

impl<'a> BodyInferrer<'a> {
    /// Expressão constante (especificação, "Constants"), sobre a resolução
    /// já feita: literais, constantes referidas, construtores e coleções
    /// `const`, operadores sobre constantes, `?:`, interpolação, tipos.
    pub(crate) fn e_constante(&self, cx: &Corpo, e: ExprId) -> bool {
        let a = &self.program.unit(cx.unit).ast;
        let bt = &self.body_types.units[cx.unit.0 as usize];
        let var_const = |v: dartforge_elements::model::VariableId| self.program.variable(v).const_;
        let fun_const = |f: dartforge_elements::model::FunctionElementId| {
            let fe = self.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => var_const(v),
                (FunctionKind::Getter | FunctionKind::Setter, _) => false,
                // Tear-off de função de topo ou estática é constante.
                _ => fe.static_ || fe.class.is_none(),
            }
        };
        let ref_const = |r: Option<&Resolved>| match r {
            Some(Resolved::Local(id)) => cx.locais.get(id.0 as usize).is_some_and(|l| l.const_),
            Some(Resolved::Element(Element::Variable(v))) => var_const(*v),
            Some(Resolved::Element(Element::Function(f))) => fun_const(*f),
            Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))) => true,
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => var_const(*v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => fun_const(*f),
            Some(Resolved::Constructor(_)) => true,
            _ => false,
        };
        match &a.expr(e).kind {
            ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Symbol(_) => true,
            ExprKind::String(lit) => lit.parts.iter().all(|p| match p {
                ast::StringPart::Interpolation(x) => self.e_constante(cx, *x),
                _ => true,
            }),
            ExprKind::Parenthesized(x) => self.e_constante(cx, *x),
            ExprKind::Identifier(_) => ref_const(bt.get_resolved(e)),
            ExprKind::Property { target, name, .. } => {
                if ref_const(bt.get_resolved(e)) {
                    return true;
                }
                // `s.length` de string constante.
                self.interner.resolve(name.sym) == "length" && self.e_constante(cx, *target)
            }
            ExprKind::TypeArguments { .. } => true,
            ExprKind::List { const_, elements, .. } | ExprKind::SetOrMap { const_, elements, .. } => {
                *const_ || elements.iter().all(|el| match el {
                    ast::CollectionElement::Expression(x) => self.e_constante(cx, *x),
                    ast::CollectionElement::MapEntry { key, value, .. } => self.e_constante(cx, *key) && self.e_constante(cx, *value),
                    _ => false,
                })
            }
            ExprKind::Record { positional, named, .. } => {
                positional.iter().all(|x| self.e_constante(cx, *x)) && named.iter().all(|(_, x)| self.e_constante(cx, *x))
            }
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                matches!(keyword, Some(ast::CreationKeyword::Const)) || arguments.args.iter().all(|x| self.e_constante(cx, x.value)) && self.construtor_const(bt.get_resolved(e))
            }
            ExprKind::Call { target, arguments } => {
                // Construtor `const` sem `new` em contexto constante, ou `identical`.
                let ok_args = arguments.args.iter().all(|x| self.e_constante(cx, x.value));
                if !ok_args {
                    return false;
                }
                if self.construtor_const(bt.get_resolved(e)) {
                    return true;
                }
                matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "identical")
            }
            ExprKind::Unary { op, operand } => {
                matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot) && self.e_constante(cx, *operand)
            }
            ExprKind::Binary { left, right, .. } => self.e_constante(cx, *left) && self.e_constante(cx, *right),
            ExprKind::Conditional { condition, then, else_ } => {
                self.e_constante(cx, *condition) && self.e_constante(cx, *then) && self.e_constante(cx, *else_)
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.e_constante(cx, *value),
            _ => false,
        }
    }

    fn construtor_const(&self, r: Option<&Resolved>) -> bool {
        match r {
            Some(Resolved::Constructor(f)) => self.program.function(*f).const_,
            _ => false,
        }
    }
}
