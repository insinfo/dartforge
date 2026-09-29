//! Atalhos de ponto (Dart 3.10, `docs/VERSOES-LINGUAGEM.md` §4.4): `.id`,
//! `.id(args)`, `.new(args)` e `const .id(args)` resolvidos na declaração
//! que o **contexto** da cadeia de seletores denota.
//!
//! O contexto é o da cadeia inteira: o nó mais externo de uma cadeia cuja
//! raiz é `.id` registra o contexto dele para a raiz ([`registrar_cadeia`]);
//! `e == .x` registra o tipo de `e` antes ([`registrar_igualdade`]). Só o
//! primeiro registro vale.
//!
//! Como o analyzer 3.13.4, os atalhos são resolvidos mesmo numa biblioteca
//! sem o recurso (o parser já relatou `experiment_not_enabled`), e os erros
//! de resolução são os do `ResolverVisitor`: sem declaração no contexto, um
//! acesso `.id` (ou uma criação `const .id(…)`) é
//! `DOT_SHORTHAND_MISSING_CONTEXT` no nó; uma invocação `.id(…)` é
//! `DOT_SHORTHAND_UNDEFINED_INVOCATION` no nome, com o contexto exibido
//! (`_` quando não há); com a declaração e sem o membro, os
//! `DOT_SHORTHAND_UNDEFINED_GETTER`/`_INVOCATION` no nome.

use super::BodyInferrer;
use super::corpo::Corpo;
use super::expr::{inferir, resolver};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_intern::SymbolId;

/// Nó externo de uma cadeia `.id…`: registra o contexto dela para a raiz.
pub(crate) fn registrar_cadeia(inf: &BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) {
    let a = &inf.program.unit(cx.unit).ast;
    if !matches!(
        a.expr(e).kind,
        ExprKind::Property { .. }
            | ExprKind::Call { .. }
            | ExprKind::Index { .. }
            | ExprKind::TypeArguments { .. }
            | ExprKind::Unary { .. }
    ) {
        return;
    }
    if let Some(raiz) = a.raiz_de_atalho(e) {
        cx.contexto_atalho.entry(raiz.0).or_insert(ctx);
    }
}

/// `e == .x` / `e != .x`: o atalho à direita usa o tipo de `e`.
pub(crate) fn registrar_igualdade(
    inf: &BodyInferrer<'_>,
    cx: &mut Corpo,
    direita: ExprId,
    esquerda: TypeId,
) {
    if let Some(r) = inf.program.unit(cx.unit).ast.raiz_de_atalho(direita) {
        cx.contexto_atalho.insert(r.0, esquerda);
    }
}

/// O contexto sem os `FutureOr` externos (`futureOrBase`), para exibir.
fn base_de_future_or(inf: &BodyInferrer<'_>, mut t: TypeId) -> TypeId {
    while let Type::FutureOr { arg, .. } = inf.table.get(t) {
        t = *arg;
    }
    t
}

/// O contexto exibido na mensagem: sem `FutureOr` e sem o `?` (o
/// analyzer 3.13.4 mostra `Object` para o contexto `Object?`).
fn exibir(inf: &mut BodyInferrer<'_>, t: TypeId) -> String {
    let base = base_de_future_or(inf, t);
    let base = inf.nao_nulo(base);
    inf.table.format(base, inf.interner, inf.program)
}

/// A declaração `D` que o contexto denota (spec 3.10, "Declaration denoted by
/// a type scheme"): `C`/`C<…>` de classe, mixin, enum ou extension type; `S?`
/// e `FutureOr<S>` denotam o que `S` denota. `Future<S>` cai para `S` quando
/// `Future` não tem o membro (o contexto do `=>` de função `async`).
///
/// Uma declaração privada de outra biblioteca não é denotada (o analyzer
/// 3.13.4 trata o contexto como ausente).
fn declaracao(inf: &BodyInferrer<'_>, cx: &Corpo, ctx: TypeId, nome: SymbolId) -> Option<ClassId> {
    let mut t = ctx;
    let d = loop {
        match inf.table.get(t) {
            Type::Interface { class, args, .. } => {
                let c = *class;
                if Some(c) == inf.core.future_class && args.len() == 1 && !tem_membro(inf, c, nome)
                {
                    t = args[0];
                    continue;
                }
                break c;
            }
            Type::ExtensionType { decl, .. } => break *decl,
            Type::FutureOr { arg, .. } => t = *arg,
            _ => return None,
        }
    };
    let class = inf.program.class(d);
    if class.library != cx.lib && inf.interner.resolve(class.name).starts_with('_') {
        return None;
    }
    Some(d)
}

/// `D` tem o construtor da chave `k`: declarado, o primário de um tipo de
/// extensão, ou o sem nome implícito de uma classe sem construtores.
fn tem_construtor(inf: &BodyInferrer<'_>, d: ClassId, k: SymbolId) -> bool {
    if inf.construtor_ou_primario(d, k).is_some() {
        return true;
    }
    let class = inf.program.class(d);
    Some(k) == inf.sym.vazio
        && class.constructors.is_empty()
        && matches!(class.kind, ClassKind::Class | ClassKind::MixinApplication)
}

/// A chave do construtor `nome` (`new` = o sem nome).
fn chave_de_construtor(inf: &BodyInferrer<'_>, nome: SymbolId) -> Option<SymbolId> {
    if Some(nome) == inf.sym.new_ {
        inf.sym.vazio
    } else {
        Some(nome)
    }
}

/// Membro estático, constante de enum ou construtor `nome` de `d`.
fn tem_membro(inf: &BodyInferrer<'_>, d: ClassId, nome: SymbolId) -> bool {
    let class = inf.program.class(d);
    class.static_members.contains_key(&nome)
        || class
            .enum_constants
            .iter()
            .any(|v| inf.program.variable(*v).name == nome)
        || chave_de_construtor(inf, nome).is_some_and(|k| tem_construtor(inf, d, k))
}

/// O tipo de `D.nome` como valor: getter/campo estático, constante de enum,
/// método estático (tear-off) ou construtor (tear-off).
fn tipo_do_membro(inf: &mut BodyInferrer<'_>, d: ClassId, nome: SymbolId) -> Option<TypeId> {
    let class = inf.program.class(d);
    if let Some(&fid) = class.static_members.get(&nome) {
        let f = inf.program.function(fid);
        return Some(match (f.kind, f.variable) {
            (FunctionKind::ImplicitAccessor, Some(vid)) => inf.tipo_variavel(vid),
            (FunctionKind::Getter, _) => inf.outline.functions[fid.0 as usize].return_type,
            _ => inf.outline.functions[fid.0 as usize].signature,
        });
    }
    if class
        .enum_constants
        .iter()
        .any(|v| inf.program.variable(*v).name == nome)
    {
        return Some(inf.table.intern(Type::Interface {
            class: d,
            args: Box::new([]),
            nullable: false,
        }));
    }
    let k = chave_de_construtor(inf, nome)?;
    match inf.construtor_ou_primario(d, k) {
        Some(Some(fid)) => Some(inf.outline.functions[fid.0 as usize].signature),
        Some(None) => Some(inf.assinatura_primario(d)),
        None => tem_construtor(inf, d, k).then_some(inf.core.dynamic_),
    }
}

/// `.nome` sem chamada: getter, campo, constante de enum ou tear-off.
pub(crate) fn valor(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    e: ExprId,
    nome: SymbolId,
    ctx: TypeId,
) -> TypeId {
    let ctx = cx.contexto_atalho.remove(&e.0).unwrap_or(ctx);
    let relatado = cx.atalhos_relatados.remove(&e.0);
    match declaracao(inf, cx, ctx, nome) {
        Some(d) => {
            resolver(inf, cx, e, Resolved::Element(Element::Class(d)));
            match tipo_do_membro(inf, d, nome) {
                Some(t) => t,
                None => {
                    if !relatado {
                        let (texto, span) = nome_e_span(inf, cx, e);
                        let tipo = exibir(inf, ctx);
                        relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_GETTER", span, &[&texto, &tipo]);
                    }
                    inf.core.dynamic_
                }
            }
        }
        None => {
            if !relatado {
                let span = inf.program.unit(cx.unit).ast.expr(e).span;
                relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT", span, &[]);
            }
            inf.core.dynamic_
        }
    }
}

/// Relata pelo nome único do código (os três são do suplemento 3.13.4 da
/// tabela de `dartforge_diagnostics`).
fn relatar(inf: &mut BodyInferrer<'_>, unico: &str, span: dartforge_diagnostics::Span, args: &[&str]) {
    if let Some(codigo) = dartforge_diagnostics::Codigo::por_unico(unico) {
        inf.aviso_com_codigo(codigo, span, args);
    }
}

/// O texto e o intervalo do nome de um `DotShorthand`.
fn nome_e_span(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> (String, dartforge_diagnostics::Span) {
    match &inf.program.unit(cx.unit).ast.expr(e).kind {
        ExprKind::DotShorthand { name, .. } => (inf.interner.resolve(name.sym).to_string(), name.span),
        _ => (String::new(), inf.program.unit(cx.unit).ast.expr(e).span),
    }
}

/// `.nome(args)` / `const .nome(args)` que é construção: o construtor de `D`,
/// com os argumentos de tipo do contexto da chamada. `None` quando não é
/// construção (método estático fica para o caminho comum da chamada, que
/// tipa o alvo por [`valor`]).
pub(crate) fn construcao(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    e: ExprId,
    ctx: TypeId,
) -> Option<TypeId> {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, arguments } = &a.expr(e).kind else {
        return None;
    };
    let alvo = *target;
    let ExprKind::DotShorthand { name, const_ } = &a.expr(alvo).kind else {
        return None;
    };
    let (nome, const_) = (name.sym, *const_);
    let ctx_cadeia = cx.contexto_atalho.get(&alvo.0).copied().unwrap_or(ctx);
    let Some(d) = declaracao(inf, cx, ctx_cadeia, nome) else {
        // Sem declaração no contexto: a criação `const .id(…)` não tem
        // contexto; a invocação `.id(…)` não acha o membro em `_`.
        if const_ {
            let span = inf.program.unit(cx.unit).ast.expr(e).span;
            relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT", span, &[]);
        } else {
            let (texto, span) = nome_e_span(inf, cx, alvo);
            let tipo = exibir(inf, ctx_cadeia);
            relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION", span, &[&texto, &tipo]);
        }
        cx.atalhos_relatados.insert(alvo.0);
        return None;
    };
    let class = inf.program.class(d);
    if class.static_members.contains_key(&nome) {
        return None;
    }
    let Some(fid) = chave_de_construtor(inf, nome).and_then(|k| class.constructors.get(&k)).copied() else {
        // A criação `const .id(…)` sem o construtor é o
        // `CONST_WITH_UNDEFINED_CONSTRUCTOR` do caminho comum.
        if const_ {
            cx.atalhos_relatados.insert(alvo.0);
        } else if !tem_membro(inf, d, nome) {
            let (texto, span) = nome_e_span(inf, cx, alvo);
            let tipo = exibir(inf, ctx_cadeia);
            relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION", span, &[&texto, &tipo]);
            cx.atalhos_relatados.insert(alvo.0);
        }
        return None;
    };
    let args_ast: Vec<(bool, ExprId)> = arguments
        .args
        .iter()
        .map(|x: &ast::Argument| (x.name.is_none(), x.value))
        .collect();
    cx.contexto_atalho.remove(&alvo.0);
    resolver(inf, cx, alvo, Resolved::Element(Element::Class(d)));
    resolver(inf, cx, e, Resolved::Constructor(fid));
    let params = inf.outline.classes[d.0 as usize].type_params.clone();
    let args: Vec<TypeId> = match inf.table.get(ctx).clone() {
        Type::Interface {
            class: cc, args, ..
        } if cc == d && args.len() == params.len() => args.to_vec(),
        _ => params.iter().map(|_| inf.core.dynamic_).collect(),
    };
    let esperados: Vec<TypeId> = inf.outline.functions[fid.0 as usize]
        .parameters
        .iter()
        .map(|p| p.ty)
        .collect();
    let u = inf.core.unknown;
    for (i, (posicional, valor)) in args_ast.into_iter().enumerate() {
        let esperado = if posicional {
            esperados.get(i).copied().unwrap_or(u)
        } else {
            u
        };
        inferir(inf, cx, valor, esperado);
    }
    let e_ext = inf.program.class(d).kind == ClassKind::ExtensionType;
    Some(if e_ext {
        inf.table.intern(Type::ExtensionType {
            decl: d,
            args: args.into_boxed_slice(),
            nullable: false,
        })
    } else {
        inf.table.intern(Type::Interface {
            class: d,
            args: args.into_boxed_slice(),
            nullable: false,
        })
    })
}
