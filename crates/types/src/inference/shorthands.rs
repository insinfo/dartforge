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
use super::body::Body;
use super::expr::resolver;
use crate::resolved::Resolved;
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::SymbolId;

/// Nó externo de uma cadeia `.id…`: registra o contexto dela para a raiz.
pub(crate) fn record_chain(inf: &BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) {
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
    if let Some(root) = a.raiz_de_atalho(e) {
        cx.shorthand_context.entry(root.0).or_insert(ctx);
    }
}

/// `e == .x` / `e != .x`: o atalho à direita usa o tipo de `e`.
pub(crate) fn record_equality(
    inf: &BodyInferrer<'_>,
    cx: &mut Body,
    right: ExprId,
    left: TypeId,
) {
    if let Some(r) = inf.program.unit(cx.unit).ast.raiz_de_atalho(right) {
        cx.shorthand_context.insert(r.0, left);
    }
}

/// O contexto sem os `FutureOr` externos (`futureOrBase`), para exibir.
fn future_or_base(inf: &BodyInferrer<'_>, mut t: TypeId) -> TypeId {
    while let Type::FutureOr { arg, .. } = inf.table.get(t) {
        t = *arg;
    }
    t
}

/// O contexto exibido na mensagem: sem `FutureOr` e sem o `?` (o
/// analyzer 3.13.4 mostra `Object` para o contexto `Object?`).
fn exibir(inf: &mut BodyInferrer<'_>, t: TypeId) -> String {
    let base = future_or_base(inf, t);
    let base = inf.non_null(base);
    // O analyzer passa o texto já pronto (`getDisplayString()`, sem alias).
    inf.table.format_sem_alias(base, inf.interner, inf.program)
}

/// A declaração `D` que o contexto denota (spec 3.10, "Declaration denoted by
/// a type scheme"): `C`/`C<…>` de classe, mixin, enum ou extension type; `S?`
/// e `FutureOr<S>` denotam o que `S` denota. `Future<S>` cai para `S` quando
/// `Future` não tem o membro (o contexto do `=>` de função `async`).
///
/// Uma declaração privada de outra biblioteca não é denotada (o analyzer
/// 3.13.4 trata o contexto como ausente).
fn declaracao(inf: &BodyInferrer<'_>, cx: &Body, ctx: TypeId, the_name: SymbolId) -> Option<ClassId> {
    let mut t = ctx;
    let d = loop {
        match inf.table.get(t) {
            Type::Interface { class, args, .. } => {
                let c = *class;
                if Some(c) == inf.core.future_class && args.len() == 1 && !has_member(inf, c, the_name)
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
    if inf.constructor_or_primary(d, k).is_some() {
        return true;
    }
    let class = inf.program.class(d);
    Some(k) == inf.sym.vazio
        && class.constructors.is_empty()
        && matches!(class.kind, ClassKind::Class | ClassKind::MixinApplication)
}

/// A chave do construtor `nome` (`new` = o sem nome).
fn constructor_key(inf: &BodyInferrer<'_>, the_name: SymbolId) -> Option<SymbolId> {
    if Some(the_name) == inf.sym.new_ {
        inf.sym.vazio
    } else {
        Some(the_name)
    }
}

/// Membro estático, constante de enum ou construtor `nome` de `d`.
fn has_member(inf: &BodyInferrer<'_>, d: ClassId, the_name: SymbolId) -> bool {
    let class = inf.program.class(d);
    class.static_members.contains_key(&the_name)
        || class
            .enum_constants
            .iter()
            .any(|v| inf.program.variable(*v).name == the_name)
        || constructor_key(inf, the_name).is_some_and(|k| tem_construtor(inf, d, k))
}

/// O tipo de `D.nome` como valor: getter/campo estático, constante de enum,
/// método estático (tear-off) ou construtor (tear-off).
fn tipo_do_membro(inf: &mut BodyInferrer<'_>, d: ClassId, the_name: SymbolId) -> Option<TypeId> {
    let class = inf.program.class(d);
    if let Some(&fid) = class.static_members.get(&the_name) {
        let f = inf.program.function(fid);
        return Some(match (f.kind, f.variable) {
            (FunctionKind::ImplicitAccessor, Some(vid)) => inf.variable_type(vid),
            (FunctionKind::Getter, _) => inf.outline.functions[fid.0 as usize].return_type,
            _ => inf.outline.functions[fid.0 as usize].signature,
        });
    }
    if class
        .enum_constants
        .iter()
        .any(|v| inf.program.variable(*v).name == the_name)
    {
        return Some(inf.table.intern(Type::Interface {
            class: d,
            args: Box::new([]),
            nullable: false,
        }));
    }
    let k = constructor_key(inf, the_name)?;
    match inf.constructor_or_primary(d, k) {
        Some(Some(fid)) => Some(inf.outline.functions[fid.0 as usize].signature),
        Some(None) => Some(inf.primary_signature(d)),
        None => tem_construtor(inf, d, k).then_some(inf.core.dynamic_),
    }
}

/// `.nome` sem chamada: getter, campo, constante de enum ou tear-off.
pub(crate) fn valor(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    the_name: SymbolId,
    ctx: TypeId,
) -> TypeId {
    let ctx = cx.shorthand_context.remove(&e.0).unwrap_or(ctx);
    let reported = cx.reported_shorthands.remove(&e.0);
    match declaracao(inf, cx, ctx, the_name) {
        Some(d) => {
            resolver(inf, cx, e, Resolved::Element(Element::Class(d)));
            // `PropertyElementResolver.resolveDotShorthand` (checkout main,
            // `property_element_resolver.dart:281-305`): o tear-off de
            // construtor gerador (declarado, ou o implícito de `.new` sem
            // construtores) de classe abstrata, no nó.
            let k = inf.program.class(d);
            if !reported && k.kind == dartforge_elements::model::ClassKind::Class && k.modifiers.abstract_ {
                let generator = match constructor_key(inf, the_name).and_then(|c| k.constructors.get(&c)) {
                    Some(&f) => !inf.program.function(f).factory,
                    None => Some(the_name) == inf.sym.new_ && k.constructors.is_empty(),
                };
                if generator {
                    let span = inf.program.unit(cx.unit).ast.expr(e).span;
                    inf.warning_with_code(
                        dartforge_diagnostics::codigos::compile_time_error::TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS,
                        span,
                        &[],
                    );
                }
            }
            match tipo_do_membro(inf, d, the_name) {
                Some(t) => t,
                None => {
                    if !reported {
                        let (text, span) = name_and_span(inf, cx, e);
                        let ty = exibir(inf, ctx);
                        relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_GETTER", span, &[&text, &ty]);
                    }
                    inf.core.dynamic_
                }
            }
        }
        None => {
            if !reported {
                let span = inf.program.unit(cx.unit).ast.expr(e).span;
                relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT", span, &[]);
            }
            inf.core.dynamic_
        }
    }
}

/// Relata pelo nome único do código (os três são do suplemento 3.13.4 da
/// tabela de `dartforge_diagnostics`).
fn relatar(inf: &mut BodyInferrer<'_>, unique: &str, span: dartforge_diagnostics::Span, args: &[&str]) {
    if let Some(code) = dartforge_diagnostics::Codigo::por_unico(unique) {
        inf.warning_with_code(code, span, args);
    }
}

/// O texto e o intervalo do nome de um `DotShorthand`.
fn name_and_span(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> (String, dartforge_diagnostics::Span) {
    match &inf.program.unit(cx.unit).ast.expr(e).kind {
        ExprKind::DotShorthand { name, .. } => (inf.interner.resolve(name.sym).to_string(), name.span),
        _ => (String::new(), inf.program.unit(cx.unit).ast.expr(e).span),
    }
}

/// `.nome(args)` / `const .nome(args)` que é construção: o construtor de `D`,
/// com os argumentos de tipo do contexto da chamada. `None` quando não é
/// construção (método estático fica para o caminho comum da chamada, que
/// tipa o alvo por [`valor`]).
pub(crate) fn construction(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    ctx: TypeId,
) -> Option<TypeId> {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, .. } = &a.expr(e).kind else {
        return None;
    };
    let tgt = *target;
    let ExprKind::DotShorthand { name, const_ } = &a.expr(tgt).kind else {
        return None;
    };
    let (the_name, const_) = (name.sym, *const_);
    let ctx_chain = cx.shorthand_context.get(&tgt.0).copied().unwrap_or(ctx);
    let Some(d) = declaracao(inf, cx, ctx_chain, the_name) else {
        // Sem declaração no contexto: a criação `const .id(…)` não tem
        // contexto; a invocação `.id(…)` não acha o membro em `_`.
        if const_ {
            let span = inf.program.unit(cx.unit).ast.expr(e).span;
            relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT", span, &[]);
        } else {
            let (text, span) = name_and_span(inf, cx, tgt);
            let ty = exibir(inf, ctx_chain);
            relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION", span, &[&text, &ty]);
        }
        cx.reported_shorthands.insert(tgt.0);
        return None;
    };
    let class = inf.program.class(d);
    if class.static_members.contains_key(&the_name) {
        return None;
    }
    // `ConstructorInvocationResolver.resolveDotShorthand` (checkout main,
    // `constructor_invocation_resolver.dart:60-95`): `const .id(…)` sem
    // construtor `id` acessível na declaração do contexto é
    // `CONST_WITH_UNDEFINED_CONSTRUCTOR` no nome, com o nome da classe.
    let accessible = |inf: &BodyInferrer<'_>, f: dartforge_elements::model::FunctionElementId| {
        let fe = inf.program.function(f);
        !inf.interner.resolve(fe.name).starts_with('_') || fe.library == cx.lib
    };
    let finding = constructor_key(inf, the_name).and_then(|k| class.constructors.get(&k)).copied();
    let finding = if const_ { finding.filter(|&f| accessible(inf, f)) } else { finding };
    let Some(fid) = finding else {
        // `.new()` de classe abstrata sem construtor declarado: o construtor
        // implícito é gerador (`resolveDotShorthand`, `:107-112`), no nó.
        let k = inf.program.class(d);
        if !const_
            && Some(the_name) == inf.sym.new_
            && k.kind == dartforge_elements::model::ClassKind::Class
            && k.modifiers.abstract_
            && k.constructors.is_empty()
        {
            let span = inf.program.unit(cx.unit).ast.expr(e).span;
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
            // O alvo segue pelo caminho comum (`valor`), que não é tear-off.
            cx.reported_shorthands.insert(tgt.0);
        }
        if const_ {
            let (text, span) = name_and_span(inf, cx, tgt);
            let cls = inf.interner.resolve(inf.program.class(d).name).to_string();
            relatar(inf, "CompileTimeErrorCode.CONST_WITH_UNDEFINED_CONSTRUCTOR", span, &[&cls, &text]);
            cx.reported_shorthands.insert(tgt.0);
        } else if !has_member(inf, d, the_name) {
            let (text, span) = name_and_span(inf, cx, tgt);
            let ty = exibir(inf, ctx_chain);
            relatar(inf, "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION", span, &[&text, &ty]);
            cx.reported_shorthands.insert(tgt.0);
        }
        return None;
    };
    // `resolveDotShorthand` (checkout main,
    // `constructor_invocation_resolver.dart:96-117`): a classe abstrata com
    // construtor que não é fábrica é `INSTANTIATE_ABSTRACT_CLASS` no nó
    // inteiro; senão os argumentos de tipo escritos são
    // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR` na lista.
    {
        let program = inf.program;
        let ExprKind::Call { arguments, .. } = &program.unit(cx.unit).ast.expr(e).kind else {
            return None;
        };
        let k = inf.program.class(d);
        if k.kind == dartforge_elements::model::ClassKind::Class && k.modifiers.abstract_ && !inf.program.function(fid).factory {
            let span = program.unit(cx.unit).ast.expr(e).span;
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
        } else if let Some(sp) = super::calls::type_list_range(inf, cx.unit, &arguments.type_args) {
            let (text, _) = name_and_span(inf, cx, tgt);
            let cls = inf.interner.resolve(k.name).to_string();
            relatar(inf, "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR", sp, &[&cls, &text]);
        }
    }
    // A invocação do construtor pelo caminho comum da criação
    // (`InstanceCreationExpression` com o tipo do contexto): os argumentos
    // de tipo pelo contexto e a conferência dos argumentos (aridade, tipos).
    cx.shorthand_context.remove(&tgt.0);
    resolver(inf, cx, tgt, Resolved::Element(Element::Class(d)));
    let program = inf.program;
    let ExprKind::Call { arguments, .. } = &program.unit(cx.unit).ast.expr(e).kind else {
        return None;
    };
    // A entidade e o nome dos erros de aridade: o nome do construtor
    // (`'new'`, `'named'`).
    let (text, span) = name_and_span(inf, cx, tgt);
    super::calls::define_target(inf, Some(text), span);
    let r = super::calls::construir(inf, cx, Some(e), d, Some(fid), None, arguments, ctx);
    inf.arity_target = None;
    Some(r)
}
