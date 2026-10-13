//! Padrões (Dart 3): esquema de tipo de um padrão (contexto do valor
//! casado), tipagem dos subpadrões pelo tipo casado, variáveis de padrão,
//! `switch` como expressão, `if-case`, declarações e atribuições por padrão.

use super::body::{Body, Local, Nome};
use super::expr::{self, declarar_local, inferir, infer_free};
use super::flow::Fluxo;
use super::members::Busca;
use super::BodyInferrer;
use crate::constraints::GenericInferrer;
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, Element};
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};

mod matching;
pub(crate) use matching::{declared_variables, Matching};

/// O fluxo de padrões do analyzer (T6) está em uso neste corpo.
pub(crate) fn pattern_flow_enabled(inf: &BodyInferrer<'_>) -> bool {
    matching::ligado(inf)
}

/// Esquema de tipo de um padrão (`_` onde nada restringe).
pub(crate) fn schema(inf: &mut BodyInferrer<'_>, cx: &Body, p: PatternId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let u = inf.core.unknown;
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty } | PatternKind::Variable { ty, .. } => match ty {
            Some(t) => inf.type_from_annotation(cx, *t),
            None => u,
        },
        PatternKind::Constant(_) | PatternKind::Relational { .. } | PatternKind::Cast { .. } | PatternKind::Or(..) => u,
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            let a1 = schema(inf, cx, x);
            let a2 = schema(inf, cx, y);
            if inf.the_is_unknown(a1) {
                a2
            } else {
                a1
            }
        }
        PatternKind::NullCheck(x) => {
            let s = schema(inf, cx, *x);
            if inf.the_is_unknown(s) {
                s
            } else {
                inf.anulavel(s)
            }
        }
        PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => schema(inf, cx, *x),
        PatternKind::List { type_args, elements } => {
            let el = if let Some(t) = type_args.first() {
                inf.type_argument_type(cx, *t)
            } else {
                let mut acc = u;
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) = e {
                        let s = schema(inf, cx, *x);
                        acc = inf.down(acc, s);
                    }
                }
                acc
            };
            inf.lista(el)
        }
        PatternKind::Map { type_args, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.type_argument_type(cx, type_args[0]), inf.type_argument_type(cx, type_args[1]))
            } else {
                (u, u)
            };
            inf.map_type(k, v)
        }
        PatternKind::Record { fields } => {
            let mut pos = Vec::new();
            let mut nm = Vec::new();
            let fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            for (n, x) in fields {
                let s = schema(inf, cx, x);
                match n {
                    Some(n) => nm.push((n.sym, s)),
                    None => match nome_implicito(inf, cx, x) {
                        Some(sym) if implicit_named_field(inf, cx, p, x) => nm.push((sym, s)),
                        _ => pos.push(s),
                    },
                }
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        PatternKind::Object { ty, .. } => {
            let t = inf.type_from_annotation(cx, *ty);
            t
        }
    }
}

/// `:x` num campo nomeado de record/objeto usa o nome da variável.
fn nome_implicito(inf: &BodyInferrer<'_>, cx: &Body, p: PatternId) -> Option<dartforge_intern::SymbolId> {
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
fn implicit_named_field(inf: &BodyInferrer<'_>, cx: &Body, parent: PatternId, child: PatternId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    let fields = match &a.pattern(parent).kind {
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields,
        _ => return false,
    };
    fields.iter().any(|f| f.pattern == child && f.name.is_none() && inf.program.unit(cx.unit).source.as_bytes().get(f.span.start) == Some(&b':'))
}

/// Tipa o padrão `p` contra o tipo casado `t`, declarando variáveis.
/// `decl`: contexto de declaração (variáveis sem tipo são declaradas com o
/// tipo casado); em atribuição, os identificadores são variáveis existentes.
fn type_check(inf: &mut BodyInferrer<'_>, cx: &mut Body, p: PatternId, t: TypeId, final_: bool, assignment: bool) {
    let a = &inf.program.unit(cx.unit).ast;
    // O tipo casado de cada padrão, lido pelo verificador de constantes
    // (`UnitBodyTypes::tipos_casados`); ainda sem a promoção do fluxo de
    // padrões (T6 da especificação).
    inf.body_types.units[cx.unit.0 as usize].tipos_casados.insert(p, t);
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            let r = inf.type_from_annotation(cx, x);
            let inv = invalid_annotation(inf, cx, x, r);
            record_pattern_type(inf, cx, p, r, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(x).span;
            never_matches(inf, cx, t, r, sp);
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
            if assignment && !*var_ && !f2 && ty.is_none() {
                if let Some(Nome::Local(id)) = cx.buscar(name.sym) {
                    super::expr::write_in_primary(inf, cx, id, name.span);
                    if inf.registrar_locais {
                        let offset = cx.local(id).offset;
                        inf.body_types.units[cx.unit.0 as usize].declaracoes_de_padroes.insert(p, offset);
                    }
                    let decl = cx.local(id).tipo;
                    let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                    let reason = super::flow::NonPromotionReason::Escrita { nome: name.sym, span: name.span };
                    inf.assign_flow(&mut f, id, decl, t, Some(reason));
                    cx.flow = f;
                }
                return;
            }
            let typ = match ty {
                Some(x) => {
                    let r = inf.type_from_annotation(cx, x);
                    never_matches(inf, cx, t, r, inf.program.unit(cx.unit).ast.ty(x).span);
                    r
                }
                None => t,
            };
            let inv = ty.is_some_and(|x| invalid_annotation(inf, cx, x, typ));
            record_pattern_type(inf, cx, p, typ, inv);
            declarar_local(inf, cx, Local { nome: name.sym, tipo: typ, final_: final_ || f2, late: false, const_: false, offset: name.span.start, funcao_local: false }, true);
        }
        PatternKind::Constant(e) => {
            let e = *e;
            inferir(inf, cx, e, t);
        }
        PatternKind::Relational { op, value } => {
            let (op, value) = (*op, *value);
            relational_pattern(inf, cx, p, op, value, t);
        }
        PatternKind::Or(x, y) => {
            let (x, y) = (*x, *y);
            type_check(inf, cx, x, t, final_, assignment);
            type_check(inf, cx, y, t, final_, assignment);
        }
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            type_check(inf, cx, x, t, final_, assignment);
            type_check(inf, cx, y, t, final_, assignment);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) => {
            let x = *x;
            let nn = inf.non_null(t);
            type_check(inf, cx, x, nn, final_, assignment);
        }
        PatternKind::Parenthesized(x) => {
            let x = *x;
            type_check(inf, cx, x, t, final_, assignment);
        }
        PatternKind::Cast { pattern, ty } => {
            let (pattern, ty) = (*pattern, *ty);
            let c = inf.type_from_annotation(cx, ty);
            let inv = invalid_annotation(inf, cx, ty, c);
            record_pattern_type(inf, cx, p, c, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
            never_matches(inf, cx, t, c, sp);
            type_check(inf, cx, pattern, c, final_, assignment);
        }
        PatternKind::List { type_args, elements } => {
            let el = if let Some(x) = type_args.first() {
                inf.type_argument_type(cx, *x)
            } else if inf.e_dynamic(t) {
                inf.core.dynamic_
            } else {
                inf.as_instance_of(t, inf.core.list_class).map(|a| a[0]).unwrap_or(inf.core.object_nullable)
            };
            {
                let the_required = inf.lista(el);
                let inv = type_args.first().is_some_and(|&x| invalid_annotation(inf, cx, x, el));
                record_pattern_type(inf, cx, p, the_required, inv);
                let sp = inf.program.unit(cx.unit).ast.pattern(p).span;
                never_matches(inf, cx, t, the_required, sp);
            }
            let els: Vec<ListPatternElement> = elements.iter().map(|e| match e {
                ListPatternElement::Pattern(x) => ListPatternElement::Pattern(*x),
                ListPatternElement::Rest(x) => ListPatternElement::Rest(*x),
            }).collect();
            for e in els {
                match e {
                    ListPatternElement::Pattern(x) => type_check(inf, cx, x, el, final_, assignment),
                    ListPatternElement::Rest(Some(x)) => {
                        let l = inf.lista(el);
                        type_check(inf, cx, x, l, final_, assignment);
                    }
                    ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { type_args, entries, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.type_argument_type(cx, type_args[0]), inf.type_argument_type(cx, type_args[1]))
            } else if inf.e_dynamic(t) {
                (inf.core.dynamic_, inf.core.dynamic_)
            } else {
                match inf.as_instance_of(t, inf.core.map_class) {
                    Some(a) => (a[0], a[1]),
                    None => (inf.core.object_nullable, inf.core.object_nullable),
                }
            };
            if let Some(mc) = inf.core.map_class {
                let the_required = inf.table.intern(Type::Interface { class: mc, args: vec![k, v].into_boxed_slice(), nullable: false });
                let inv = type_args.len() == 2 && (invalid_annotation(inf, cx, type_args[0], k) || invalid_annotation(inf, cx, type_args[1], v));
                record_pattern_type(inf, cx, p, the_required, inv);
            }
            let es: Vec<(ExprId, PatternId)> = entries.iter().map(|e| (e.key, e.value)).collect();
            for (key, val) in es {
                inferir(inf, cx, key, k);
                type_check(inf, cx, val, v, final_, assignment);
            }
        }
        PatternKind::Record { fields } => {
            let fields: Vec<(Option<ast::Name>, PatternId, dartforge_diagnostics::Span)> = fields.iter().map(|f| (f.name, f.pattern, f.span)).collect();
            let tnn = inf.non_null(t);
            let rec = match inf.table.get(tnn).clone() {
                Type::Record { positional, named, .. } => Some((positional, named)),
                _ => None,
            };
            let mut ipos = 0;
            for (n, x, _) in fields {
                let the_name = n.map(|n| n.sym).or_else(|| if implicit_named_field(inf, cx, p, x) { nome_implicito(inf, cx, x) } else { None });
                let ft = match (&rec, the_name) {
                    (Some((_, named)), Some(nm)) => named.iter().find(|(s, _)| *s == nm).map(|(_, t)| *t),
                    (Some((pos, _)), None) => {
                        let r = pos.get(ipos).copied();
                        ipos += 1;
                        r
                    }
                    _ => None,
                };
                let ft = ft.unwrap_or(if inf.e_dynamic(t) { inf.core.dynamic_ } else { inf.core.object_nullable });
                type_check(inf, cx, x, ft, final_, assignment);
            }
        }
        PatternKind::Object { ty, fields } => {
            let ty = *ty;
            let fields: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            let obj = object_pattern_type(inf, cx, ty, t);
            let inv = invalid_annotation(inf, cx, ty, obj);
            record_pattern_type(inf, cx, p, obj, inv);
            {
                let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
                never_matches(inf, cx, t, obj, sp);
            }
            for (n, x) in fields {
                let the_name = n.map(|n| n.sym).or_else(|| nome_implicito(inf, cx, x));
                let ft = match the_name {
                    Some(nm) => match inf.lookup_member(cx.lib, obj, nm, false) {
                        Busca::Achado(m) => {
                            // Membro de extensão ou de tipo de extensão: a
                            // exaustividade o lê como `ExtensionKey` (o tipo
                            // do objeto é apagado).
                            let of_extension_type = matches!(
                                &m.resolved,
                                crate::resolved::Resolved::Member { class, .. }
                                    if inf.program.class(*class).kind == dartforge_elements::model::ClassKind::ExtensionType
                            );
                            if m.of_extension || of_extension_type {
                                inf.body_types.units[cx.unit.0 as usize].extension_fields.insert(x, m.tipo);
                            }
                            m.tipo
                        }
                        Busca::NeverType => inf.core.never,
                        _ => inf.core.dynamic_,
                    },
                    None => inf.core.dynamic_,
                };
                type_check(inf, cx, x, ft, final_, assignment);
            }
        }
    }
}

/// Guarda o tipo do padrão para a exaustividade (ver
/// [`crate::resolved::UnitBodyTypes::tipos_de_padroes`]); um tipo escrito
/// que não resolve (fica `dynamic` sem ser `dynamic`) marca o padrão como
/// inválido, como o `InvalidType` do analyzer.
fn record_pattern_type(inf: &mut BodyInferrer<'_>, cx: &Body, p: PatternId, t: TypeId, invalid: bool) {
    let b = &mut inf.body_types.units[cx.unit.0 as usize];
    b.tipos_de_padroes.insert(p, t);
    if invalid {
        b.invalid_patterns.insert(p);
    }
}

/// A anotação `x`, resolvida para `r`, não resolve: `dynamic` sem ter sido
/// escrita `dynamic`.
fn invalid_annotation(inf: &BodyInferrer<'_>, cx: &Body, x: ast::TypeId, r: TypeId) -> bool {
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
fn never_matches(inf: &mut BodyInferrer<'_>, cx: &Body, matched: TypeId, the_required: TypeId, span: dartforge_diagnostics::Span) {
    if !cx.padrao_refutavel || span.start == span.end {
        return;
    }
    if can_be_subtype(inf, matched, the_required, 0) {
        return;
    }
    inf.warning_with_args(
        dartforge_diagnostics::codigos::warning::PATTERN_NEVER_MATCHES_VALUE_TYPE,
        span,
        &[crate::exibicao::Arg::Tipo(matched), crate::exibicao::Arg::Tipo(the_required)],
    );
}

/// `TypeSystemImpl.canBeSubtypeOf` (`an611:src/dart/element/type_system.dart:
/// 147-315`): pode existir um valor de `left` que seja de `right`? Pelo lado
/// otimista: só diz "não" quando sabe (Null, funções contra interfaces,
/// enums, classes `final`/`sealed` com todos os subtipos na biblioteca,
/// registros).
pub(crate) fn can_be_subtype(inf: &mut BodyInferrer<'_>, left: TypeId, right: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return true;
    }
    let left = inf.erase_extension(left);
    let right = inf.erase_extension(right);
    if matches!(inf.table.get(left), Type::Dynamic | Type::Void) || matches!(inf.table.get(right), Type::Dynamic | Type::Void) {
        return true;
    }
    let left_nullable = !inf.is_non_nullable(left);
    let right_nullable = !inf.is_non_nullable(right);
    if matches!(inf.table.get(left), Type::Null) {
        return right_nullable;
    }
    if matches!(inf.table.get(right), Type::Null) {
        return left_nullable;
    }
    if left_nullable && right_nullable {
        return true;
    }
    let left = inf.non_null(left);
    let right = inf.non_null(right);
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
        return can_be_subtype(inf, arg, right, prof + 1) || can_be_subtype(inf, fut, right, prof + 1);
    }
    if let Type::FutureOr { arg, .. } = inf.table.get(right).clone() {
        let fut = inf.futuro(arg);
        return can_be_subtype(inf, left, arg, prof + 1) || can_be_subtype(inf, left, fut, prof + 1);
    }
    if let (Type::Interface { class: lc, args: la, .. }, Type::Interface { class: rc, args: ra, .. }) =
        (inf.table.get(left).clone(), inf.table.get(right).clone())
    {
        let int = inf.core.int_class;
        let double = inf.core.double_class;
        if (Some(lc) == int && Some(rc) == double) || (Some(lc) == double && Some(rc) == int) {
            return true;
        }
        // Enum: os tipos de todas as instâncias são conhecidos; alguma
        // constante (`v1<String>()` é `A<String>`) é subtipo do da direita
        // (3.6.2 `type_system.dart:221-229`).
        if inf.program.class(lc).kind == dartforge_elements::model::ClassKind::Enum {
            let constantes = inf.program.class(lc).enum_constants.clone();
            return constantes.into_iter().any(|v| {
                let t = inf.variable_type(v);
                inf.sub(t, right)
            });
        }
        if lc == rc {
            return la.iter().zip(ra.iter()).all(|(&a, &b)| can_be_subtype(inf, a, b, prof + 1));
        }
        if let Some(subtypes) = all_subtypes(inf, lc) {
            for cand in std::iter::once(lc).chain(subtypes) {
                let tc = inf.class_this_type(cand);
                if let Some(args) = inf.as_instance_of(tc, Some(rc)) {
                    // `_canBeEqualArguments`: só classes diferentes impedem.
                    let equal = args.iter().zip(ra.iter()).all(|(&a, &b)| {
                        match (inf.table.get(a), inf.table.get(b)) {
                            (Type::Interface { class: x, .. }, Type::Interface { class: y, .. }) => x == y,
                            _ => true,
                        }
                    });
                    if equal {
                        return true;
                    }
                }
            }
            return false;
        }
        if let Some(subtypes) = all_subtypes(inf, rc) {
            for cand in std::iter::once(rc).chain(subtypes) {
                let tc = if cand == rc { right } else { inf.class_this_type(cand) };
                if let Some(args) = inf.as_instance_of(tc, Some(lc)) {
                    if la.iter().zip(args.iter()).all(|(&a, &b)| can_be_subtype(inf, a, b, prof + 1)) {
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
                if !can_be_subtype(inf, a, b, prof + 1) {
                    return false;
                }
            }
            for ((at, a), (nb, b)) in ln.iter().zip(rn.iter()) {
                if at != nb || !can_be_subtype(inf, *a, *b, prof + 1) {
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
fn all_subtypes(inf: &mut BodyInferrer<'_>, c: ClassId) -> Option<Vec<ClassId>> {
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
        let tc = inf.class_this_type(cand);
        if inf.as_instance_of(tc, Some(c)).is_none() {
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
fn object_pattern_type(inf: &mut BodyInferrer<'_>, cx: &Body, ty: ast::TypeId, matched: TypeId) -> TypeId {
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
                if !params.is_empty() && !inf.e_dynamic(matched) {
                    let this = inf.class_this_type(c);
                    let mut gi = GenericInferrer::new(&params);
                    let mut env = inf.env();
                    gi.constrain_return(this, matched, &mut env);
                    let args = gi.choose_final(&mut env);
                    drop(env);
                    let map = inf.mapa(&params, &args);
                    let t = inf.subst(this, &map);
                    return if node.nullable { inf.anulavel(t) } else { t };
                }
            }
        }
    }
    inf.type_from_annotation(cx, ty)
}

/// Tipo que o valor casado passa a ter quando `p` casa com um valor de
/// tipo `t` (`promoteForPattern`, R-FLU-14): tipo escrito de variável ou
/// curinga, `p?`/`p!`/`!= null` não nulo, cast, padrão objeto; `None` se o
/// padrão não estreita.
fn tipo_casado(inf: &mut BodyInferrer<'_>, cx: &Body, p: PatternId, t: TypeId) -> Option<TypeId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.pattern(p).kind {
        PatternKind::Variable { ty: Some(x), .. } | PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            Some(inf.type_from_annotation(cx, x))
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) => {
            let x = *x;
            let nn = inf.non_null(t);
            Some(tipo_casado(inf, cx, x, nn).unwrap_or(nn))
        }
        PatternKind::Relational { op: ast::BinaryOp::NotEq, value } if matches!(a.expr(*value).kind, ast::ExprKind::Null) => Some(inf.non_null(t)),
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
            Some(inf.type_from_annotation(cx, ty))
        }
        PatternKind::Object { ty, .. } => {
            let ty = *ty;
            Some(object_pattern_type(inf, cx, ty, t))
        }
        _ => None,
    }
}

/// `case p when g`: tipa o padrão e a guarda, devolve `(casou, não casou)`.
/// `escrutinio`: a expressão casada; variável promovível é promovida ao
/// tipo casado no ramo que casa, antes da guarda (R-FLU-14).
pub(crate) fn caso(inf: &mut BodyInferrer<'_>, cx: &mut Body, p: PatternId, t: TypeId, guard: Option<ExprId>, scrutinee: Option<ExprId>) -> (Fluxo, Fluxo) {
    if matching::ligado(inf) {
        return matching::caso(inf, cx, p, t, guard, scrutinee);
    }
    let before = cx.flow.clone();
    // O escrutínio é resolvido no escopo de fora, antes de o padrão declarar
    // as suas variáveis: em `switch (e) { Neg(:final e) => … }` o `e` do
    // padrão sombreia o escrutinado, e é o escrutinado (não a variável nova,
    // do tipo do campo) que o caso promove.
    let target = scrutinee.and_then(|e| expr::promotion_target(inf, cx, e));
    let refutable_before = std::mem::replace(&mut cx.padrao_refutavel, true);
    type_check(inf, cx, p, t, false, false);
    cx.padrao_refutavel = refutable_before;
    let mut yes = cx.flow.clone();
    if let Some(id) = target
        && let Some(tc) = tipo_casado(inf, cx, p, t)
    {
        let decl = cx.local(id).tipo;
        inf.promover(&mut yes, id, decl, tc);
        cx.flow = yes.clone();
    }
    // Padrão que casa com qualquer valor do tipo (`case _:`, `case var x:`,
    // `case Object o:` sobre `Object`): a falha do casamento é inalcançável,
    // como na análise de fluxo do analyzer.
    let mut not_ = if irrefutable(inf, cx, p, t) { before.unreachable() } else { before };
    if let Some(g) = guard {
        let (gv, gf) = expr::checked_condition(inf, cx, g);
        yes = gv;
        not_ = inf.juntar(&not_, &gf);
    }
    cx.flow = yes.clone();
    (yes, not_)
}

/// O padrão (num `case`) casa com todo valor de tipo `t`? Só as formas
/// simples: curinga e variável (com tipo que `t` satisfaz), `as`, `!`,
/// parênteses, `&&` e `||`. O identificador solto de um `case` é constante,
/// não variável.
fn irrefutable(inf: &mut BodyInferrer<'_>, cx: &Body, p: PatternId, t: TypeId) -> bool {
    let kind = &inf.program.unit(cx.unit).ast.pattern(p).kind;
    match kind {
        PatternKind::Wildcard { ty: None } => true,
        PatternKind::Variable { ty: None, var_, final_, .. } => *var_ || *final_,
        PatternKind::Wildcard { ty: Some(a) } | PatternKind::Variable { ty: Some(a), .. } => {
            let a = *a;
            let tt = inf.type_from_annotation(cx, a);
            inf.sub(t, tt)
        }
        PatternKind::Cast { .. } => true,
        PatternKind::Parenthesized(q) => {
            let q = *q;
            irrefutable(inf, cx, q, t)
        }
        PatternKind::NullAssert(q) => {
            let q = *q;
            let nn = inf.non_null(t);
            irrefutable(inf, cx, q, nn)
        }
        PatternKind::And(a, b) => {
            let (a, b) = (*a, *b);
            irrefutable(inf, cx, a, t) && irrefutable(inf, cx, b, t)
        }
        PatternKind::Or(a, b) => {
            let (a, b) = (*a, *b);
            irrefutable(inf, cx, a, t) || irrefutable(inf, cx, b, t)
        }
        _ => false,
    }
}

/// `var (a, b) = e;` / `final [x] = e;`
pub(crate) fn declaracao(inf: &mut BodyInferrer<'_>, cx: &mut Body, final_: bool, p: PatternId, the_value: ExprId) {
    let s = schema(inf, cx, p);
    let t = inferir(inf, cx, the_value, s);
    if matching::ligado(inf) {
        matching::irrefutable(inf, cx, p, t, final_, false, Some(the_value));
        return;
    }
    type_check(inf, cx, p, t, final_, false);
}

/// Declara as variáveis de um padrão casado com `t` (`for (var (a, b) in …)`).
pub(crate) fn declare_by_type(inf: &mut BodyInferrer<'_>, cx: &mut Body, final_: bool, p: PatternId, t: TypeId) {
    if matching::ligado(inf) {
        matching::irrefutable(inf, cx, p, t, final_, false, None);
        return;
    }
    type_check(inf, cx, p, t, final_, false);
}

/// `(a, b) = e` — atribuição por padrão a variáveis existentes.
pub(crate) fn pattern_assignment(inf: &mut BodyInferrer<'_>, cx: &mut Body, p: PatternId, the_value: ExprId) -> TypeId {
    let s = assignment_schema(inf, cx, p);
    // `patternAssignment_afterRhs` empurra o escrutínio com a informação do
    // lado direito (`_pushScrutinee`, `addPreviousInfo`); a variável no nível
    // de cima (só parênteses em volta) a escreve com ela
    // (`assignedVariablePattern` → `_write`): `(b) = x != null` guarda a
    // condição em `b`. Num subpadrão o valor casado é outro, sem a condição.
    let mut q = p;
    while let PatternKind::Parenthesized(i) = &inf.program.unit(cx.unit).ast.pattern(q).kind {
        q = *i;
    }
    let local = match &inf.program.unit(cx.unit).ast.pattern(q).kind {
        PatternKind::Variable { name, ty: None, .. } => match cx.buscar(name.sym) {
            Some(Nome::Local(id)) => Some(id),
            _ => None,
        },
        _ => None,
    };
    let (t, saved_condition) = if local.is_some() && expr::is_condition_form(inf, cx, the_value) {
        let (yes, not_) = expr::condicao(inf, cx, the_value);
        cx.flow = inf.juntar(&yes, &not_);
        let t = inf.body_types.units[cx.unit.0 as usize].get_type(the_value).unwrap_or(inf.core.bool_);
        (t, Some((yes, not_)))
    } else {
        (inferir(inf, cx, the_value, s), None)
    };
    if matching::ligado(inf) {
        matching::irrefutable(inf, cx, p, t, false, true, None);
    } else {
        type_check(inf, cx, p, t, false, true);
    }
    if let (Some(id), Some((yes, not_))) = (local, saved_condition)
        && let Some(version) = cx.flow.versao(id)
    {
        cx.conditions.insert(id, (yes, not_, version));
    }
    t
}

/// Esquema de um padrão de atribuição: variáveis contribuem com o tipo declarado.
fn assignment_schema(inf: &mut BodyInferrer<'_>, cx: &Body, p: PatternId) -> TypeId {
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
                let s = assignment_schema(inf, cx, x);
                match n {
                    Some(n) => nm.push((n.sym, s)),
                    None => pos.push(s),
                }
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        _ => schema(inf, cx, p),
    }
}

/// `switch (v) { p => e, ... }` como expressão.
pub(crate) fn switch_expression(inf: &mut BodyInferrer<'_>, cx: &mut Body, the_value: ExprId, cases: &[ast::SwitchExprCase], ctx: TypeId) -> TypeId {
    let t = infer_free(inf, cx, the_value);
    expr::void_use(inf, cx, the_value, t);
    let before = cx.flow.clone();
    let mut not_matched = before.clone();
    let mut types: Vec<TypeId> = Vec::new();
    let mut exits: Vec<Fluxo> = Vec::new();
    let outer_scrutinee = cx.switch_scrutinee.replace(None);
    for c in cases {
        cx.flow = not_matched.clone();
        cx.push_scope();
        // O caso é um bloco básico (`flowEnd` em `resolver.dart:874`). Um
        // caso a que nenhum valor chega é código morto inteiro; um caso cujo
        // padrão nunca casa, do corpo ao fim (Apêndice B do §B).
        let new_ = matching::ligado(inf);
        if new_ {
            super::statements::enter_flow(cx, c.span.end);
            if !cx.flow.alcancavel && cx.dead_code.is_none() {
                inf.aviso(crate::codes::DEAD_CODE.template.to_string(), c.span);
                cx.dead_code = Some(cx.flow_ends.len());
            }
        }
        let (yes, not_) = caso(inf, cx, c.pattern, t, c.guard, Some(the_value));
        cx.flow = yes;
        if new_ && !cx.flow.alcancavel && cx.dead_code.is_none() {
            let start = inf.span_expr(cx.unit, c.body).start;
            inf.aviso(crate::codes::DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: start, end: c.span.end });
            cx.dead_code = Some(cx.flow_ends.len());
        }
        let tc = inferir(inf, cx, c.body, ctx);
        if new_ {
            super::statements::exit_flow(cx);
        }
        types.push(tc);
        exits.push(cx.flow.clone());
        cx.pop_scope();
        not_matched = not_;
    }
    cx.switch_scrutinee = outer_scrutinee;
    cx.flow = inf.join_all(&before, &exits);
    if types.is_empty() {
        return inf.core.never;
    }
    let mut acc = types[0];
    for &x in &types[1..] {
        acc = inf.up(acc, x);
    }
    if !inf.the_is_unknown(ctx) {
        let s = inf.greatest_closure_of(ctx);
        if !inf.sub(acc, s) && types.iter().all(|&x| inf.sub(x, s)) {
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
pub(crate) fn relational_pattern(inf: &mut BodyInferrer<'_>, cx: &mut Body, p: PatternId, op: ast::BinaryOp, value: ExprId, t: TypeId) -> TypeId {
    let u = inf.core.unknown;
    let (the_name, lexeme, equality) = match op {
        ast::BinaryOp::Eq => ("==", "==", true),
        ast::BinaryOp::NotEq => ("==", "!=", true),
        ast::BinaryOp::Lt => ("<", "<", false),
        ast::BinaryOp::Gt => (">", ">", false),
        ast::BinaryOp::LtEq => ("<=", "<=", false),
        ast::BinaryOp::GtEq => (">=", ">=", false),
        _ => ("", "", false),
    };
    // `resolveRelationalPatternOperator`: fora da igualdade, o
    // `TypePropertyResolver` com o tipo casado; potencialmente anulável é
    // `UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE` no operador (o pai é
    // `RelationalPattern`, `type_property_resolver.dart:127-130`).
    if !equality
        && let Some(s) = inf.interner.lookup(the_name)
        && inf.requires_null_check(cx.lib, t, s, false)
    {
        let start = inf.program.unit(cx.unit).ast.pattern(p).span.start;
        let token = dartforge_diagnostics::Span { start: start, end: start + lexeme.len() };
        inf.null_warning(t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE, token, &[the_name]);
    }
    let operator = match inf.interner.lookup(the_name).map(|s| inf.lookup_member(cx.lib, t, s, false)) {
        Some(Busca::Achado(m)) => match {
            inf.body_types.units[cx.unit.0 as usize].relational_operators.insert(p, m.resolved.clone());
            inf.table.get(m.tipo).clone()
        } {
            Type::Function { positional, ret, .. } => positional.first().copied().map(|p| (p, ret)),
            _ => None,
        },
        _ => None,
    };
    let ctx = match operator {
        Some((p, _)) if equality => inf.anulavel(p),
        Some((p, _)) => p,
        None => u,
    };
    let tv = inferir(inf, cx, value, ctx);
    if let Some((parameter, return_)) = operator {
        if !inf.atribuivel(tv, ctx) {
            let sp = inf.span_expr(cx.unit, value);
            let a1 = inf.table.format(tv, inf.interner, inf.program);
            let a2 = inf.table.format(parameter, inf.interner, inf.program);
            inf.warning_with_code(
                dartforge_diagnostics::codigos::compile_time_error::RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE,
                sp,
                &[&a1, &a2, lexeme],
            );
        }
        let bool_ = inf.core.bool_;
        if !inf.atribuivel(return_, bool_) {
            let start = inf.program.unit(cx.unit).ast.pattern(p).span.start;
            let token = dartforge_diagnostics::Span { start: start, end: start + lexeme.len() };
            inf.warning_with_code(
                dartforge_diagnostics::codigos::compile_time_error::RELATIONAL_PATTERN_OPERATOR_RETURN_TYPE_NOT_ASSIGNABLE_TO_BOOL,
                token,
                &[],
            );
        }
    }
    tv
}
