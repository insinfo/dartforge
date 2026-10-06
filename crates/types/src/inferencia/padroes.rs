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
use dartforge_elements::model::{ClassId, Element};
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};

mod casamento;
pub(crate) use casamento::{variaveis_declaradas, Casamento};

/// O fluxo de padrões do analyzer (T6) está em uso neste corpo.
pub(crate) fn fluxo_de_padroes_ligado(inf: &BodyInferrer<'_>) -> bool {
    casamento::ligado(inf)
}

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
                inf.tipo_de_argumento_de_tipo(cx, *t)
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
                (inf.tipo_de_argumento_de_tipo(cx, type_args[0]), inf.tipo_de_argumento_de_tipo(cx, type_args[1]))
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
    // O tipo casado de cada padrão, lido pelo verificador de constantes
    // (`UnitBodyTypes::tipos_casados`); ainda sem a promoção do fluxo de
    // padrões (T6 da especificação).
    inf.body_types.units[cx.unit.0 as usize].tipos_casados.insert(p, t);
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            let r = inf.tipo_de_anotacao(cx, x);
            let inv = anotacao_invalida(inf, cx, x, r);
            registrar_tipo_de_padrao(inf, cx, p, r, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(x).span;
            nunca_casa(inf, cx, t, r, sp);
        }
        PatternKind::Wildcard { .. } => {}
        PatternKind::Variable { final_: f2, var_, ty, name } => {
            let (f2, ty, name) = (*f2, *ty, *name);
            if cx.padrao_refutavel && !*var_ && !f2 && ty.is_none() {
                // `case limite:`: constante (o nome de uma const), não uma
                // variável que esconde a constante e fica sem valor.
                return;
            }
            // Com `var`, `final` ou tipo, é declaração mesmo em atribuição
            // (`DeclaredVariablePattern`).
            if atribuicao && !*var_ && !f2 && ty.is_none() {
                if let Some(Nome::Local(id)) = cx.buscar(name.sym) {
                    super::expr::escrita_em_primario(inf, cx, id, name.span);
                    if inf.registrar_locais {
                        let offset = cx.local(id).offset;
                        inf.body_types.units[cx.unit.0 as usize].declaracoes_de_padroes.insert(p, offset);
                    }
                    let decl = cx.local(id).tipo;
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    let motivo = super::fluxo::MotivoDeNaoPromocao::Escrita { nome: name.sym, span: name.span };
                    inf.atribuir_fluxo(&mut f, id, decl, t, Some(motivo));
                    cx.fluxo = f;
                }
                return;
            }
            let tipo = match ty {
                Some(x) => {
                    let r = inf.tipo_de_anotacao(cx, x);
                    nunca_casa(inf, cx, t, r, inf.program.unit(cx.unit).ast.ty(x).span);
                    r
                }
                None => t,
            };
            let inv = ty.is_some_and(|x| anotacao_invalida(inf, cx, x, tipo));
            registrar_tipo_de_padrao(inf, cx, p, tipo, inv);
            declarar_local(inf, cx, Local { nome: name.sym, tipo, final_: final_ || f2, late: false, const_: false, offset: name.span.start, funcao_local: false }, true);
        }
        PatternKind::Constant(e) => {
            let e = *e;
            inferir(inf, cx, e, t);
        }
        PatternKind::Relational { op, value } => {
            let (op, value) = (*op, *value);
            padrao_relacional(inf, cx, p, op, value, t);
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
            let inv = anotacao_invalida(inf, cx, ty, c);
            registrar_tipo_de_padrao(inf, cx, p, c, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
            nunca_casa(inf, cx, t, c, sp);
            tipar(inf, cx, pattern, c, final_, atribuicao);
        }
        PatternKind::List { type_args, elements } => {
            let el = if let Some(x) = type_args.first() {
                inf.tipo_de_argumento_de_tipo(cx, *x)
            } else if inf.e_dynamic(t) {
                inf.core.dynamic_
            } else {
                inf.como_instancia_de(t, inf.core.list_class).map(|a| a[0]).unwrap_or(inf.core.object_nullable)
            };
            {
                let requerido = inf.lista(el);
                let inv = type_args.first().is_some_and(|&x| anotacao_invalida(inf, cx, x, el));
                registrar_tipo_de_padrao(inf, cx, p, requerido, inv);
                let sp = inf.program.unit(cx.unit).ast.pattern(p).span;
                nunca_casa(inf, cx, t, requerido, sp);
            }
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
                (inf.tipo_de_argumento_de_tipo(cx, type_args[0]), inf.tipo_de_argumento_de_tipo(cx, type_args[1]))
            } else if inf.e_dynamic(t) {
                (inf.core.dynamic_, inf.core.dynamic_)
            } else {
                match inf.como_instancia_de(t, inf.core.map_class) {
                    Some(a) => (a[0], a[1]),
                    None => (inf.core.object_nullable, inf.core.object_nullable),
                }
            };
            if let Some(mc) = inf.core.map_class {
                let requerido = inf.table.intern(Type::Interface { class: mc, args: vec![k, v].into_boxed_slice(), nullable: false });
                let inv = type_args.len() == 2 && (anotacao_invalida(inf, cx, type_args[0], k) || anotacao_invalida(inf, cx, type_args[1], v));
                registrar_tipo_de_padrao(inf, cx, p, requerido, inv);
            }
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
            let inv = anotacao_invalida(inf, cx, ty, obj);
            registrar_tipo_de_padrao(inf, cx, p, obj, inv);
            {
                let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
                nunca_casa(inf, cx, t, obj, sp);
            }
            for (n, x) in fields {
                let nome = n.map(|n| n.sym).or_else(|| nome_implicito(inf, cx, x));
                let ft = match nome {
                    Some(nm) => match inf.buscar_membro(cx.lib, obj, nm, false) {
                        Busca::Achado(m) => {
                            // Membro de extensão ou de tipo de extensão: a
                            // exaustividade o lê como `ExtensionKey` (o tipo
                            // do objeto é apagado).
                            let de_tipo_de_extensao = matches!(
                                &m.resolved,
                                crate::resolved::Resolved::Member { class, .. }
                                    if inf.program.class(*class).kind == dartforge_elements::model::ClassKind::ExtensionType
                            );
                            if m.de_extensao || de_tipo_de_extensao {
                                inf.body_types.units[cx.unit.0 as usize].campos_de_extensao.insert(x, m.tipo);
                            }
                            m.tipo
                        }
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

/// Guarda o tipo do padrão para a exaustividade (ver
/// [`crate::resolved::UnitBodyTypes::tipos_de_padroes`]); um tipo escrito
/// que não resolve (fica `dynamic` sem ser `dynamic`) marca o padrão como
/// inválido, como o `InvalidType` do analyzer.
fn registrar_tipo_de_padrao(inf: &mut BodyInferrer<'_>, cx: &Corpo, p: PatternId, t: TypeId, invalido: bool) {
    let b = &mut inf.body_types.units[cx.unit.0 as usize];
    b.tipos_de_padroes.insert(p, t);
    if invalido {
        b.padroes_invalidos.insert(p);
    }
}

/// A anotação `x`, resolvida para `r`, não resolve: `dynamic` sem ter sido
/// escrita `dynamic`.
fn anotacao_invalida(inf: &BodyInferrer<'_>, cx: &Corpo, x: ast::TypeId, r: TypeId) -> bool {
    if !inf.e_dynamic(r) {
        return false;
    }
    match &inf.program.unit(cx.unit).ast.ty(x).kind {
        ast::TypeKind::Named { name, .. } => !name.last().is_some_and(|n| inf.interner.resolve(n.sym) == "dynamic"),
        _ => true,
    }
}

/// `PATTERN_NEVER_MATCHES_VALUE_TYPE` (`checkPatternNeverMatchesValueType`,
/// `an611:src/generated/resolver.dart:613-640`): num contexto refutável, o
/// tipo casado não pode ser subtipo do tipo exigido pelo padrão. Relatado no
/// tipo escrito (variável, curinga, cast, objeto) ou no padrão (lista).
fn nunca_casa(inf: &mut BodyInferrer<'_>, cx: &Corpo, casado: TypeId, requerido: TypeId, span: dartforge_diagnostics::Span) {
    if !cx.padrao_refutavel || span.start == span.end {
        return;
    }
    if pode_ser_subtipo(inf, casado, requerido, 0) {
        return;
    }
    inf.aviso_com_args(
        dartforge_diagnostics::codigos::warning::PATTERN_NEVER_MATCHES_VALUE_TYPE,
        span,
        &[crate::exibicao::Arg::Tipo(casado), crate::exibicao::Arg::Tipo(requerido)],
    );
}

/// `TypeSystemImpl.canBeSubtypeOf` (`an611:src/dart/element/type_system.dart:
/// 147-315`): pode existir um valor de `left` que seja de `right`? Pelo lado
/// otimista: só diz "não" quando sabe (Null, funções contra interfaces,
/// enums, classes `final`/`sealed` com todos os subtipos na biblioteca,
/// registros).
pub(crate) fn pode_ser_subtipo(inf: &mut BodyInferrer<'_>, left: TypeId, right: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return true;
    }
    let left = inf.apagar_extensao(left);
    let right = inf.apagar_extensao(right);
    if matches!(inf.table.get(left), Type::Dynamic | Type::Void) || matches!(inf.table.get(right), Type::Dynamic | Type::Void) {
        return true;
    }
    let left_anulavel = !inf.e_nao_anulavel(left);
    let right_anulavel = !inf.e_nao_anulavel(right);
    if matches!(inf.table.get(left), Type::Null) {
        return right_anulavel;
    }
    if matches!(inf.table.get(right), Type::Null) {
        return left_anulavel;
    }
    if left_anulavel && right_anulavel {
        return true;
    }
    let left = inf.nao_nulo(left);
    let right = inf.nao_nulo(right);
    let e_function = |inf: &BodyInferrer<'_>, c: ClassId| Some(c) == inf.core.function_class;
    let e_object = |inf: &BodyInferrer<'_>, c: ClassId| Some(c) == inf.core.object_class;
    let e_record = |inf: &BodyInferrer<'_>, c: ClassId| inf.interner.resolve(inf.program.class(c).name) == "Record" && inf.program.library(inf.program.class(c).library).is_sdk;
    match (inf.table.get(left).clone(), inf.table.get(right).clone()) {
        (Type::Function { .. }, Type::Interface { class, .. }) => return e_function(inf, class) || e_object(inf, class),
        (Type::Interface { class, .. }, Type::Function { .. }) => return e_function(inf, class) || e_object(inf, class),
        _ => {}
    }
    if let Type::FutureOr { arg, .. } = inf.table.get(left).clone() {
        let fut = inf.futuro(arg);
        return pode_ser_subtipo(inf, arg, right, prof + 1) || pode_ser_subtipo(inf, fut, right, prof + 1);
    }
    if let Type::FutureOr { arg, .. } = inf.table.get(right).clone() {
        let fut = inf.futuro(arg);
        return pode_ser_subtipo(inf, left, arg, prof + 1) || pode_ser_subtipo(inf, left, fut, prof + 1);
    }
    if let (Type::Interface { class: lc, args: la, .. }, Type::Interface { class: rc, args: ra, .. }) =
        (inf.table.get(left).clone(), inf.table.get(right).clone())
    {
        let int = inf.core.int_class;
        let double = inf.core.double_class;
        if (Some(lc) == int && Some(rc) == double) || (Some(lc) == double && Some(rc) == int) {
            return true;
        }
        // Enum: os tipos de todas as instâncias são conhecidos.
        if inf.program.class(lc).kind == dartforge_elements::model::ClassKind::Enum {
            return inf.sub(left, right);
        }
        if lc == rc {
            return la.iter().zip(ra.iter()).all(|(&a, &b)| pode_ser_subtipo(inf, a, b, prof + 1));
        }
        if let Some(subtipos) = todos_os_subtipos(inf, lc) {
            for cand in std::iter::once(lc).chain(subtipos) {
                let tc = inf.tipo_this_classe(cand);
                if let Some(args) = inf.como_instancia_de(tc, Some(rc)) {
                    // `_canBeEqualArguments`: só classes diferentes impedem.
                    let iguais = args.iter().zip(ra.iter()).all(|(&a, &b)| {
                        match (inf.table.get(a), inf.table.get(b)) {
                            (Type::Interface { class: x, .. }, Type::Interface { class: y, .. }) => x == y,
                            _ => true,
                        }
                    });
                    if iguais {
                        return true;
                    }
                }
            }
            return false;
        }
        if let Some(subtipos) = todos_os_subtipos(inf, rc) {
            for cand in std::iter::once(rc).chain(subtipos) {
                let tc = if cand == rc { right } else { inf.tipo_this_classe(cand) };
                if let Some(args) = inf.como_instancia_de(tc, Some(lc)) {
                    if la.iter().zip(args.iter()).all(|(&a, &b)| pode_ser_subtipo(inf, a, b, prof + 1)) {
                        return true;
                    }
                }
            }
            return false;
        }
    }
    match (inf.table.get(left).clone(), inf.table.get(right).clone()) {
        (Type::Record { .. }, Type::Function { .. }) | (Type::Function { .. }, Type::Record { .. }) => return false,
        (Type::Record { .. }, Type::Interface { class, .. }) | (Type::Interface { class, .. }, Type::Record { .. }) => {
            return e_object(inf, class) || e_record(inf, class);
        }
        (Type::Record { positional: lp, named: ln, .. }, Type::Record { positional: rp, named: rn, .. }) => {
            if lp.len() != rp.len() || ln.len() != rn.len() {
                return false;
            }
            for (&a, &b) in lp.iter().zip(rp.iter()) {
                if !pode_ser_subtipo(inf, a, b, prof + 1) {
                    return false;
                }
            }
            for ((na, a), (nb, b)) in ln.iter().zip(rn.iter()) {
                if na != nb || !pode_ser_subtipo(inf, *a, *b, prof + 1) {
                    return false;
                }
            }
        }
        _ => {}
    }
    true
}

/// `ClassElementImpl.allSubtypes` (`an611:src/dart/element/element.dart:
/// 235-276`): de uma classe `final`, todas as classes da biblioteca que a
/// têm por supertipo; de uma `sealed`, idem, desde que cada uma seja
/// `final`/`sealed` ou enum (senão, desconhecido). Outras: desconhecido.
fn todos_os_subtipos(inf: &mut BodyInferrer<'_>, c: ClassId) -> Option<Vec<ClassId>> {
    let k = inf.program.class(c);
    if k.kind != dartforge_elements::model::ClassKind::Class {
        return None;
    }
    let (final_, sealed) = (k.modifiers.final_, k.modifiers.sealed);
    if !final_ && !sealed {
        return None;
    }
    let lib = k.library;
    let mut out = Vec::new();
    for i in 0..inf.program.classes.len() {
        let cand = ClassId(i as u32);
        if cand == c || inf.program.class(cand).library != lib || inf.program.class(cand).decl.is_none() {
            continue;
        }
        let tc = inf.tipo_this_classe(cand);
        if inf.como_instancia_de(tc, Some(c)).is_none() {
            continue;
        }
        if sealed && !final_ {
            let ck = inf.program.class(cand);
            let ok = match ck.kind {
                dartforge_elements::model::ClassKind::Enum => true,
                dartforge_elements::model::ClassKind::Class => ck.modifiers.final_ || ck.modifiers.sealed,
                _ => false,
            };
            if !ok {
                return None;
            }
        }
        out.push(cand);
    }
    Some(out)
}

/// Tipo de um padrão objeto `C(...)`: com `C` genérico cru, os argumentos
/// vêm do tipo casado (`Option<int>` casado com `Some()` → `Some<int>`).
fn tipo_do_padrao_objeto(inf: &mut BodyInferrer<'_>, cx: &Corpo, ty: ast::TypeId, casado: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let node = a.ty(ty);
    if let ast::TypeKind::Named { name, args } = &node.kind {
        if args.is_empty() {
            let binding = if name.len() == 2 {
                inf.program.lookup_prefixed_na_unidade(cx.unit, name[0].sym, name[1].sym)
            } else {
                inf.program.lookup_na_unidade(cx.unit, name[0].sym)
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

/// Tipo que o valor casado passa a ter quando `p` casa com um valor de
/// tipo `t` (`promoteForPattern`, R-FLU-14): tipo escrito de variável ou
/// curinga, `p?`/`p!`/`!= null` não nulo, cast, padrão objeto; `None` se o
/// padrão não estreita.
fn tipo_casado(inf: &mut BodyInferrer<'_>, cx: &Corpo, p: PatternId, t: TypeId) -> Option<TypeId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.pattern(p).kind {
        PatternKind::Variable { ty: Some(x), .. } | PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            Some(inf.tipo_de_anotacao(cx, x))
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) => {
            let x = *x;
            let nn = inf.nao_nulo(t);
            Some(tipo_casado(inf, cx, x, nn).unwrap_or(nn))
        }
        PatternKind::Relational { op: ast::BinaryOp::NotEq, value } if matches!(a.expr(*value).kind, ast::ExprKind::Null) => Some(inf.nao_nulo(t)),
        PatternKind::Parenthesized(x) => {
            let x = *x;
            tipo_casado(inf, cx, x, t)
        }
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            let tx = tipo_casado(inf, cx, x, t);
            tipo_casado(inf, cx, y, tx.unwrap_or(t)).or(tx)
        }
        PatternKind::Cast { ty, .. } => {
            let ty = *ty;
            Some(inf.tipo_de_anotacao(cx, ty))
        }
        PatternKind::Object { ty, .. } => {
            let ty = *ty;
            Some(tipo_do_padrao_objeto(inf, cx, ty, t))
        }
        _ => None,
    }
}

/// `case p when g`: tipa o padrão e a guarda, devolve `(casou, não casou)`.
/// `escrutinio`: a expressão casada; variável promovível é promovida ao
/// tipo casado no ramo que casa, antes da guarda (R-FLU-14).
pub(crate) fn caso(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, t: TypeId, guarda: Option<ExprId>, escrutinio: Option<ExprId>) -> (Fluxo, Fluxo) {
    if casamento::ligado(inf) {
        return casamento::caso(inf, cx, p, t, guarda, escrutinio);
    }
    let antes = cx.fluxo.clone();
    // O escrutínio é resolvido no escopo de fora, antes de o padrão declarar
    // as suas variáveis: em `switch (e) { Neg(:final e) => … }` o `e` do
    // padrão sombreia o escrutinado, e é o escrutinado (não a variável nova,
    // do tipo do campo) que o caso promove.
    let alvo = escrutinio.and_then(|e| expr::alvo_de_promocao(inf, cx, e));
    let refutavel_antes = std::mem::replace(&mut cx.padrao_refutavel, true);
    tipar(inf, cx, p, t, false, false);
    cx.padrao_refutavel = refutavel_antes;
    let mut sim = cx.fluxo.clone();
    if let Some(id) = alvo
        && let Some(tc) = tipo_casado(inf, cx, p, t)
    {
        let decl = cx.local(id).tipo;
        inf.promover(&mut sim, id, decl, tc);
        cx.fluxo = sim.clone();
    }
    // Padrão que casa com qualquer valor do tipo (`case _:`, `case var x:`,
    // `case Object o:` sobre `Object`): a falha do casamento é inalcançável,
    // como na análise de fluxo do analyzer.
    let mut nao = if irrefutavel(inf, cx, p, t) { antes.inalcancavel() } else { antes };
    if let Some(g) = guarda {
        let (gv, gf) = expr::condicao_verificada(inf, cx, g);
        sim = gv;
        nao = inf.juntar(&nao, &gf);
    }
    cx.fluxo = sim.clone();
    (sim, nao)
}

/// O padrão (num `case`) casa com todo valor de tipo `t`? Só as formas
/// simples: curinga e variável (com tipo que `t` satisfaz), `as`, `!`,
/// parênteses, `&&` e `||`. O identificador solto de um `case` é constante,
/// não variável.
fn irrefutavel(inf: &mut BodyInferrer<'_>, cx: &Corpo, p: PatternId, t: TypeId) -> bool {
    let kind = &inf.program.unit(cx.unit).ast.pattern(p).kind;
    match kind {
        PatternKind::Wildcard { ty: None } => true,
        PatternKind::Variable { ty: None, var_, final_, .. } => *var_ || *final_,
        PatternKind::Wildcard { ty: Some(a) } | PatternKind::Variable { ty: Some(a), .. } => {
            let a = *a;
            let tt = inf.tipo_de_anotacao(cx, a);
            inf.sub(t, tt)
        }
        PatternKind::Cast { .. } => true,
        PatternKind::Parenthesized(q) => {
            let q = *q;
            irrefutavel(inf, cx, q, t)
        }
        PatternKind::NullAssert(q) => {
            let q = *q;
            let nn = inf.nao_nulo(t);
            irrefutavel(inf, cx, q, nn)
        }
        PatternKind::And(a, b) => {
            let (a, b) = (*a, *b);
            irrefutavel(inf, cx, a, t) && irrefutavel(inf, cx, b, t)
        }
        PatternKind::Or(a, b) => {
            let (a, b) = (*a, *b);
            irrefutavel(inf, cx, a, t) || irrefutavel(inf, cx, b, t)
        }
        _ => false,
    }
}

/// `var (a, b) = e;` / `final [x] = e;`
pub(crate) fn declaracao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, final_: bool, p: PatternId, valor: ExprId) {
    let s = esquema(inf, cx, p);
    let t = inferir(inf, cx, valor, s);
    if casamento::ligado(inf) {
        casamento::irrefutavel(inf, cx, p, t, final_, false, Some(valor));
        return;
    }
    tipar(inf, cx, p, t, final_, false);
}

/// Declara as variáveis de um padrão casado com `t` (`for (var (a, b) in …)`).
pub(crate) fn declarar_por_tipo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, final_: bool, p: PatternId, t: TypeId) {
    if casamento::ligado(inf) {
        casamento::irrefutavel(inf, cx, p, t, final_, false, None);
        return;
    }
    tipar(inf, cx, p, t, final_, false);
}

/// `(a, b) = e` — atribuição por padrão a variáveis existentes.
pub(crate) fn atribuicao_de_padrao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, valor: ExprId) -> TypeId {
    let s = esquema_de_atribuicao(inf, cx, p);
    let t = inferir(inf, cx, valor, s);
    if casamento::ligado(inf) {
        casamento::irrefutavel(inf, cx, p, t, false, true, None);
        return t;
    }
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
    expr::uso_de_void(inf, cx, valor, t);
    let antes = cx.fluxo.clone();
    let mut nao_casou = antes.clone();
    let mut tipos: Vec<TypeId> = Vec::new();
    let mut saidas: Vec<Fluxo> = Vec::new();
    let escrutinio_de_fora = cx.escrutinio_de_switch.replace(None);
    for c in casos {
        cx.fluxo = nao_casou.clone();
        cx.empurrar_escopo();
        // O caso é um bloco básico (`flowEnd` em `resolver.dart:874`). Um
        // caso a que nenhum valor chega é código morto inteiro; um caso cujo
        // padrão nunca casa, do corpo ao fim (Apêndice B do §B).
        let novo = casamento::ligado(inf);
        if novo {
            super::instrucoes::entrar_fluxo(cx, c.span.end);
            if !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
                inf.aviso(crate::codes::DEAD_CODE.template.to_string(), c.span);
                cx.trecho_morto = Some(cx.fins_de_fluxo.len());
            }
        }
        let (sim, nao) = caso(inf, cx, c.pattern, t, c.guard, Some(valor));
        cx.fluxo = sim;
        if novo && !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
            let inicio = inf.span_expr(cx.unit, c.body).start;
            inf.aviso(crate::codes::DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: inicio, end: c.span.end });
            cx.trecho_morto = Some(cx.fins_de_fluxo.len());
        }
        let tc = inferir(inf, cx, c.body, ctx);
        if novo {
            super::instrucoes::sair_fluxo(cx);
        }
        tipos.push(tc);
        saidas.push(cx.fluxo.clone());
        cx.tirar_escopo();
        nao_casou = nao;
    }
    cx.escrutinio_de_switch = escrutinio_de_fora;
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

/// `analyzeRelationalPattern`
/// (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:1700-1760`):
/// o operador pelo tipo casado `t` (`==` para `==` e `!=`); o operando com o
/// parâmetro dele como contexto (anulável na igualdade); o operando contra o
/// parâmetro (`RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE`) e o retorno
/// contra `bool`. Devolve o tipo do operando.
pub(crate) fn padrao_relacional(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, op: ast::BinaryOp, value: ExprId, t: TypeId) -> TypeId {
    let u = inf.core.unknown;
    let (nome, lexema, igualdade) = match op {
        ast::BinaryOp::Eq => ("==", "==", true),
        ast::BinaryOp::NotEq => ("==", "!=", true),
        ast::BinaryOp::Lt => ("<", "<", false),
        ast::BinaryOp::Gt => (">", ">", false),
        ast::BinaryOp::LtEq => ("<=", "<=", false),
        ast::BinaryOp::GtEq => (">=", ">=", false),
        _ => ("", "", false),
    };
    let operador = match inf.interner.lookup(nome).map(|s| inf.buscar_membro(cx.lib, t, s, false)) {
        Some(Busca::Achado(m)) => match inf.table.get(m.tipo).clone() {
            Type::Function { positional, ret, .. } => positional.first().copied().map(|p| (p, ret)),
            _ => None,
        },
        _ => None,
    };
    let ctx = match operador {
        Some((p, _)) if igualdade => inf.anulavel(p),
        Some((p, _)) => p,
        None => u,
    };
    let tv = inferir(inf, cx, value, ctx);
    if let Some((parametro, retorno)) = operador {
        if !inf.atribuivel(tv, ctx) {
            let sp = inf.span_expr(cx.unit, value);
            let a1 = inf.table.format(tv, inf.interner, inf.program);
            let a2 = inf.table.format(parametro, inf.interner, inf.program);
            inf.aviso_com_codigo(
                dartforge_diagnostics::codigos::compile_time_error::RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE,
                sp,
                &[&a1, &a2, lexema],
            );
        }
        let bool_ = inf.core.bool_;
        if !inf.atribuivel(retorno, bool_) {
            let inicio = inf.program.unit(cx.unit).ast.pattern(p).span.start;
            let token = dartforge_diagnostics::Span { start: inicio, end: inicio + lexema.len() };
            inf.aviso_com_codigo(
                dartforge_diagnostics::codigos::compile_time_error::RELATIONAL_PATTERN_OPERATOR_RETURN_TYPE_NOT_ASSIGNABLE_TO_BOOL,
                token,
                &[],
            );
        }
    }
    tv
}
