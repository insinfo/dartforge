//! Corpos de funções declaradas, construtores (inicializadores), expressões
//! de função (com inferência do retorno, `inference.md` "Function literal
//! return type inference"), funções locais, variáveis de topo sem tipo e
//! metadados.

use super::body::{Body, FunctionCtx, Local};
use super::expr::{self, declarar_local, inferir, infer_free};
use super::statements;
use super::BodyInferrer;
use crate::codes::*;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, FunctionBody};
use std::collections::HashMap;

impl<'a> BodyInferrer<'a> {
    /// Contexto das expressões de `return` para um retorno declarado `r`
    /// (`BodyInferenceContext._contextTypeForImposed`, 3.6.2
    /// `body_inference_context.dart:168-214`). O retorno `dynamic` (escrito
    /// ou implícito) não é imposto (`resolver.dart:2325`, `:3185`,
    /// `function_expression_resolver.dart:46`): o contexto é `_`. Gerador
    /// cujo retorno não é `Iterable<S>`/`Stream<S>` cai, como o assíncrono,
    /// em `FutureOr<futureValueTypeSchema(r)>`.
    pub(crate) fn declared_return_context(&mut self, r: TypeId, m: AsyncModifier) -> TypeId {
        if matches!(self.table.get(r), Type::Dynamic) || self.the_is_unknown(r) {
            return self.core.unknown;
        }
        let element = match m {
            AsyncModifier::None => return r,
            AsyncModifier::Async => None,
            AsyncModifier::SyncStar => self.as_instance_of(r, self.core.iterable_class).map(|a| a[0]),
            AsyncModifier::AsyncStar => self.as_instance_of(r, self.core.stream_class).map(|a| a[0]),
        };
        if let Some(e) = element {
            return e;
        }
        let fv = self.future_value_type_schema(r);
        self.future_or(fv)
    }
}

/// `ErrorVerifier.visitDefaultFormalParameter` (3.6.2,
/// `error_verifier.dart:640-651`): o valor padrão contra o tipo do parâmetro
/// (`checkForAssignableExpressionAtType`, `INVALID_ASSIGNMENT`; o `void` do
/// valor é `USE_OF_VOID_RESULT`).
fn assignable_default_value(inf: &mut BodyInferrer<'_>, cx: &Body, d: ast::ExprId, td: TypeId, t: TypeId) {
    if matches!(inf.table.get(t), Type::Void) {
        return;
    }
    expr::verify_assignable_expr(inf, cx, d, td, t, crate::codes::INVALID_ASSIGNMENT.template);
}

/// Declara os parâmetros de uma lista escrita, com os tipos dados; infere
/// os valores padrão. `so_inicializadores` marca `this.x`/`super.x`.
fn declarar_parametros(inf: &mut BodyInferrer<'_>, cx: &mut Body, params: &[ast::Parameter], types: &[TypeId], skip_initializers: bool) {
    for (i, p) in params.iter().enumerate() {
        let t = types.get(i).copied().unwrap_or(inf.core.dynamic_);
        if let Some(d) = p.default_value {
            let td = inferir(inf, cx, d, t);
            assignable_default_value(inf, cx, d, td, t);
        }
        if skip_initializers && (p.this_ || p.super_) {
            continue;
        }
        if let Some(n) = &p.name {
            // Tipo escrito que não resolve: `dynamic` sem ser `dynamic`.
            if matches!(inf.table.get(t), Type::Dynamic) {
                if let Some(x) = p.ty {
                    if let ast::TypeKind::Named { name, .. } = &inf.program.unit(cx.unit).ast.ty(x).kind {
                        if !name.last().is_some_and(|k| inf.interner.resolve(k.sym) == "dynamic") {
                            inf.invalid_locals.insert((cx.unit, n.span.start));
                        }
                    }
                }
            }
            // `this.x` e `super.x` introduzem locais finais na lista de
            // inicializadores (`FieldFormalParameterElementImpl.isFinal`,
            // `SuperFormalParameterElementImpl.isFinal`, 3.6.2
            // `element.dart:3992-3995`, `:9282-9285`).
            let final_ = p.final_ || p.this_ || p.super_;
            declarar_local(
                inf,
                cx,
                Local { nome: n.sym, tipo: t, final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                true,
            );
        }
    }
}

/// Infere o corpo de uma função/método/construtor declarado.
pub(crate) fn infer_declared_function(inf: &mut BodyInferrer<'_>, f: FunctionElementId) {
    let fe = inf.program.function(f);
    match fe.node {
        FunctionRef::Function { unit, function } => {
            let af = inf.program.unit(unit).ast.function(function);
            let data = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Body::to_function(inf, f, unit);
            cx.raiz = super::body::Raiz::Funcao(function);
            for &p in data.type_params.iter() {
                let the_name = inf.table.param(p).name;
                cx.declare_type_param(the_name, p);
            }
            cx.push_scope();
            if let Some(ps) = &af.parameters {
                let types: Vec<TypeId> = data.parameters.iter().map(|p| p.ty).collect();
                declarar_parametros(inf, &mut cx, ps, &types, false);
            }
            let ret = data.return_type;
            let ctx_ret = inf.declared_return_context(ret, af.modifier);
            let executable = inf.declared_executable(f);
            let legal_return = legal_return_type(inf, unit, af.return_type, af.modifier, ret);
            cx.funcoes.push(FunctionCtx { closure_imposed: None, modificador: af.modifier, retorno: Some(ret), return_context: ctx_ret, returned: Vec::new(), empty_return: false, valueless_returns: Vec::new(), returned_expressions: Vec::new(), executavel: executable, legal_return });
            function_body(inf, &mut cx, &af.body, af.modifier, ret, None);
            // `checkForBodyMayCompleteNormally` no nome da função/método.
            // O setter não confere: o elemento tem retorno `void` (o tipo
            // escrito é só `NON_VOID_RETURN_FOR_SETTER`).
            if matches!(af.body, FunctionBody::Block(_))
                && cx.flow.alcancavel
                && af.kind != ast::FunctionKind::Setter
                && let Some(n) = af.name
            {
                body_completes_normally(inf, Some(ret), Some(ctx_ret), af.modifier, n.span);
            }
            cx.funcoes.pop();
            // `FunctionDeclaration` (de topo, não método) com retorno escrito.
            let fe = inf.program.function(f);
            if fe.class.is_none() && fe.extension.is_none() && af.return_type.is_some() {
                unnecessary_set(inf, unit, function, ret);
            }
        }
        FunctionRef::Constructor { unit, member } => {
            let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return };
            let data = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Body::to_function(inf, f, unit);
            cx.raiz = super::body::Raiz::Construtor(member);
            if !fe.factory {
                cx.estatico = false;
                if let Some(c) = fe.class {
                    cx.tipo_this = Some(inf.class_this_type(c));
                }
            }
            let types: Vec<TypeId> = data.parameters.iter().map(|p| p.ty).collect();
            // Escopo dos inicializadores: todos os parâmetros.
            cx.push_scope();
            // O cabeçalho do primário (3.13) fica fora do corpo da classe: o
            // `this` de um valor padrão ali não tem o tipo da classe
            // (`dynamic`; o oráculo 3.13.4 não relata `invalid_assignment`).
            let primary = is_primary_constructor(inf.program, f);
            let header_this = if primary { cx.tipo_this.take() } else { None };
            cx.untyped_this = primary;
            declarar_parametros(inf, &mut cx, &ctor.parameters, &types, false);
            cx.untyped_this = false;
            if primary {
                cx.tipo_this = header_this;
            }
            // A parte `this` repetida (ou a do tipo de extensão) vê os
            // parâmetros do primário.
            let of_primary = if ctor.parte_primaria { the_primary_parameters(inf, f) } else { Vec::new() };
            for &(n, t, final_, _) in &of_primary {
                super::expr::declarar_local(inf, &mut cx, Local { nome: n.sym, tipo: t, final_, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
            }
            // Os parâmetros do primário, protegidos nos inicializadores (o
            // corpo declara locais novos).
            if (is_primary_constructor(inf.program, f) || !of_primary.is_empty())
                && let Some(scope) = cx.escopos.last()
            {
                let ids: Vec<_> = scope.iter().filter_map(|(_, n)| match n {
                    super::body::Nome::Local(id) => Some(*id),
                    _ => None,
                }).collect();
                cx.primary_parameters.extend(ids);
            }
            // Inicializadores não veem `this` (salvo o acesso ao campo inicializado).
            let saved_this = cx.tipo_this.take();
            let saved_static = cx.estatico;
            cx.estatico = true;
            cx.untyped_this = true;
            for init in ctor.initializers.iter() {
                inicializador(inf, &mut cx, fe.class, init, &ctor.parameters, fe.const_, f);
            }
            cx.untyped_this = false;
            // `_checkForRecursiveConstructorRedirect` (`error_verifier.dart:5047-5066`):
            // no primeiro `this(...)` de um construtor gerador em ciclo.
            if !fe.factory
                && !is_primary_constructor(inf.program, f)
                && let Some(sp) = ctor.initializers.iter().find_map(|i| match i {
                    ast::Initializer::Redirect { span, .. } => Some(*span),
                    _ => None,
                })
                && redirects_in_cycle(inf, f)
            {
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::RECURSIVE_CONSTRUCTOR_REDIRECT, sp, &[]);
            }
            if let Some(red) = &ctor.redirect {
                redirecting_factory_target(inf, &mut cx, red, f, ctor);
            }
            if let Some(c) = fe.class {
                implicit_super(inf, c, ctor, unit, member);
                super_parameters(inf, c, ctor, &types);
            }
            // `_checkForValidField` (`error_verifier.dart:5702-5760`): o tipo
            // escrito de `this.x` contra o do campo (subtipo, não atribuível).
            if let Some(c) = fe.class {
                for (i, p) in ctor.parameters.iter().enumerate() {
                    if !p.this_ || p.super_ || p.ty.is_none() {
                        continue;
                    }
                    let Some(n) = p.name else { continue };
                    let field = inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == n.sym);
                    let Some(v) = field else { continue };
                    if inf.program.variable(v).static_ {
                        continue;
                    }
                    let (Some(&pt), ft) = (types.get(i), inf.variable_type(v)) else { continue };
                    if !inf.sub(pt, ft) {
                        let start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                        let sp = dartforge_diagnostics::Span { start: start, end: n.span.end };
                        inf.warning_with_args(
                            dartforge_diagnostics::codigos::compile_time_error::FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE,
                            sp,
                            &[crate::exibicao::Arg::Tipo(pt), crate::exibicao::Arg::Tipo(ft)],
                        );
                    }
                }
            }
            cx.tipo_this = saved_this;
            cx.estatico = saved_static;
            cx.pop_scope();
            // Escopo do corpo: parâmetros sem `this.`/`super.`.
            cx.push_scope();
            let mut cx2 = cx;
            for (i, p) in ctor.parameters.iter().enumerate() {
                if p.this_ || p.super_ {
                    continue;
                }
                if let Some(n) = &p.name {
                    let t = types.get(i).copied().unwrap_or(inf.core.dynamic_);
                    let id = cx2.declarar(Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false });
                    cx2.flow.inicializar(id);
                }
            }
            for &(n, t, final_, initiator) in &of_primary {
                if !initiator {
                    let id = cx2.declarar(Local { nome: n.sym, tipo: t, final_, late: false, const_: false, offset: n.span.start, funcao_local: false });
                    cx2.flow.inicializar(id);
                }
            }
            let v = inf.core.void_;
            let ret = if fe.factory { data.return_type } else { v };
            let ctx_ret = ret;
            // Construtor gerador: `return e;` é `return_in_generative_constructor`.
            let executable = if fe.factory { inf.declared_executable(f) } else { None };
            cx2.funcoes.push(FunctionCtx { closure_imposed: None, modificador: AsyncModifier::None, retorno: Some(ret), return_context: ctx_ret, returned: Vec::new(), empty_return: false, valueless_returns: Vec::new(), returned_expressions: Vec::new(), executavel: executable, legal_return: true });
            // `flowEnd(ConstructorDeclaration)`: o analyzer não apara o
            // construtor na última instrução; o trecho vai até o fim dele.
            let end = inf.program.unit(unit).ast.member(member).span.end;
            function_body(inf, &mut cx2, &ctor.body, AsyncModifier::None, ret, Some(end));
            // Construtor gerador com `=> e`: o `verifyExpressionFunctionBody`
            // (3.6.2 `return_type_verifier.dart:45-52`, `:134-200`) confere `e`
            // contra o `returnType` do construtor, o tipo da classe nos
            // próprios parâmetros de tipo (`RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR`).
            // O corpo `this => e` do construtor primário é outro erro
            // (`PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY`), sem a conferência.
            // (A parte `this` sem elaboração, no tipo de extensão, guarda a
            // marca `parte_primaria`; na classe e no enum é o k2 elaborado.)
            let primary = ctor.parte_primaria
                || fe.class.and_then(|c| inf.program.class(c).decl).is_some_and(|d| match &inf.program.unit(d.unit).ast.decl(d.decl).kind {
                    ast::DeclKind::Class(k) => k.primary_constructor == Some(member),
                    ast::DeclKind::Enum(k) => k.primary_constructor == Some(member),
                    _ => false,
                });
            if !fe.factory
                && !primary
                && let FunctionBody::Expression(x) = ctor.body
                && let Some(c) = fe.class
                && let Some(tx) = inf.body_types.units[unit.0 as usize].get_type(x)
            {
                let tc = inf.class_this_type(c);
                let fc = FunctionCtx {
                    closure_imposed: None,
                    modificador: AsyncModifier::None,
                    retorno: Some(tc),
                    return_context: tc,
                    returned: Vec::new(),
                    empty_return: false, valueless_returns: Vec::new(),
                    returned_expressions: Vec::new(),
                    executavel: inf.declared_executable(f),
                    legal_return: true,
                };
                verify_expression_return(inf, &cx2, &fc, x, tx);
            }
            // Factory: `atConstructorDeclaration`, do tipo de retorno ao fim do
            // nome.
            // A factory com modificador (`NON_SYNC_FACTORY`) não confere: o
            // gerador sai cedo e o `async` tem tipo imposto ilegal
            // (`checkForBodyMayCompleteNormally`, 3.6.2 `resolver.dart:542-563`).
            let with_modifier = match ctor.body {
                FunctionBody::Block(b) => {
                    let source = inf.program.unit(unit).source.as_bytes();
                    let mut i = inf.program.unit(unit).ast.stmt(b).span.start;
                    while i > 0 && source[i - 1].is_ascii_whitespace() {
                        i -= 1;
                    }
                    i > 0 && (source[i - 1] == b'*' || source[..i].ends_with(b"async"))
                }
                _ => false,
            };
            if fe.factory && matches!(ctor.body, FunctionBody::Block(_)) && cx2.flow.alcancavel && !with_modifier {
                let where_ = dartforge_diagnostics::Span { start: ctor.class_name.span.start, end: ctor.name.map_or(ctor.class_name.span.end, |n| n.span.end) };
                body_completes_normally(inf, Some(ret), Some(ret), AsyncModifier::None, where_);
            }
        }
        FunctionRef::None => {}
    }
}

/// Um inicializador de construtor.
fn inicializador(inf: &mut BodyInferrer<'_>, cx: &mut Body, cls: Option<ClassId>, init: &ast::Initializer, params: &[ast::Parameter], const_constructor: bool, current: FunctionElementId) {
    let u = inf.core.unknown;
    match init {
        ast::Initializer::Field { name, value, .. } => {
            let field = cls.and_then(|c| inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == name.sym));
            let t = field.map(|v| inf.variable_type(v)).unwrap_or(u);
            let tv = inferir(inf, cx, *value, t);
            // `checkForFieldInitializerNotAssignable`
            // (`error_detection_helpers.dart:168-205`): na expressão; num
            // construtor `const`, a variante `CONST_`.
            if field.is_some() {
                if inf.atribuivel(tv, t) {
                    if !matches!(inf.table.get(t), Type::Void) {
                        expr::void_use(inf, cx, *value, tv);
                    }
                } else {
                    use dartforge_diagnostics::codigos::compile_time_error as c;
                    let code = if const_constructor { c::CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE } else { c::FIELD_INITIALIZER_NOT_ASSIGNABLE };
                    let sp = inf.span_expr(cx.unit, *value);
                    let since = inf.diagnostics.len();
                    inf.warning_with_args(code, sp, &[crate::exibicao::Arg::Tipo(tv), crate::exibicao::Arg::Tipo(t)]);
                    inf.attach_non_promotion(since, cx, Some(*value), sp);
                }
            }
        }
        ast::Initializer::Super { span, constructor, arguments } => {
            super_initializer(inf, cx, cls, *span, *constructor, arguments, params);
        }
        ast::Initializer::Redirect { span, constructor, arguments } => {
            redirecting_initializer(inf, cls, *span, *constructor, current);
            let tgt = cls.map(|c| {
                let ps = inf.outline.classes[c.0 as usize].type_params.clone();
                let args: Vec<TypeId> = ps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
                (c, args.into_boxed_slice())
            });
            call_constructor_of(inf, cx, tgt, *constructor, arguments, &[], *span);
        }
        ast::Initializer::Assert { condition, message, .. } => {
            expr::assert_condition(inf, cx, *condition);
            if let Some(m) = message {
                infer_free(inf, cx, *m);
            }
        }
    }
}

/// O que o nome do tipo de um alvo de redirecionamento (`= X`, `= p.X`,
/// `= X.n`, `= X<T>.n`) designa, como o `NamedTypeResolver` o resolve com
/// `redirectedConstructor_namedType`
/// (`an611:src/dart/resolver/named_type_resolver.dart:84-133`, `:230-312`).
enum TargetType {
    /// Classe, enum, tipo de extensão ou alias que expande a um deles.
    Classe(ClassId),
    /// Um tipo sem construtores (parâmetro de tipo, `Never`, alias de tipo de
    /// função): o `staticElement` fica nulo e o tipo é o do `{1}` de
    /// `redirect_to_missing_constructor`.
    NoConstructors(TypeId),
    /// `dynamic` (ou alias dele): nada a relatar.
    Dinamico,
    /// Alias que expande a um parâmetro de tipo (`_verifyTypeAliasForContext`).
    AliasDeParametro,
    /// `reportNullOrNonTypeElement`: `redirect_to_non_class` com o nome.
    NaoClasse(dartforge_intern::SymbolId),
    /// `p.X` com `p` que não é prefixo nem tipo.
    ShadowedPrefix,
    /// Indefinido mas calado (`shouldIgnoreUndefinedNamedType`) ou sintaxe
    /// que não chega a ser alvo.
    Nada,
}

/// O alvo escrito de uma factory redirecionadora, já resolvido.
struct WrittenTarget {
    tipo: TargetType,
    /// O nome do construtor (`None` = sem nome; `new` já vira sem nome).
    construtor: Option<dartforge_intern::SymbolId>,
    /// `NamedType.qualifiedName` com o `.nome` do construtor, como escrito
    /// (o `{0}` de `redirect_to_missing_constructor`).
    cited_name: String,
    /// `_getErrorRange`: do prefixo ao nome do tipo, sem argumentos de tipo.
    type_range: dartforge_diagnostics::Span,
    /// O token do prefixo (para `prefix_shadowed_by_local_declaration`).
    prefixo: Option<ast::Name>,
}

/// O nome `nome` no escopo do construtor de `classe` (os parâmetros dele, os
/// parâmetros de tipo da classe, os membros dela e o escopo da unidade), como
/// o `nameScope` do `ResolutionVisitor.visitConstructorDeclaration`
/// (`an611:src/dart/resolver/resolution_visitor.dart:356-382`: os
/// parâmetros são definidos antes de `_resolveRedirectedConstructor`).
enum InScope {
    Elemento(Element),
    TypeParam(TypeParamId),
    Prefixo,
    Embutido,
    /// Parâmetro, membro ou constante de enum: não é tipo.
    Outro,
    Nenhum,
}

fn name_in_constructor_scope(inf: &BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, params: &[ast::Parameter], the_name: dartforge_intern::SymbolId) -> InScope {
    if params.iter().any(|p| p.name.is_some_and(|n| n.sym == the_name)) {
        return InScope::Outro;
    }
    if let Some(c) = cls {
        if let Some(&p) = inf.outline.classes[c.0 as usize].type_params.iter().find(|&&p| inf.table.param(p).name == the_name) {
            return InScope::TypeParam(p);
        }
        let setter_key = inf.setter_key(the_name);
        if inf.lexical_declared_member(Some(c), None, the_name, setter_key).is_some() {
            return InScope::Outro;
        }
        let ce = inf.program.class(c);
        if ce.enum_constants.iter().chain(ce.fields.iter()).any(|&v| inf.program.variable(v).name == the_name) {
            return InScope::Outro;
        }
    }
    if let Some(b) = inf.program.lookup_na_unidade(unit, the_name)
        && let Some(el) = b.getter.or(b.setter)
    {
        if let Element::Prefix(..) = el {
            return InScope::Prefixo;
        }
        return InScope::Elemento(el);
    }
    if inf.program.prefixos_na_unidade(unit).contains_key(&the_name) {
        return InScope::Prefixo;
    }
    if matches!(inf.interner.resolve(the_name), "dynamic" | "Never") {
        return InScope::Embutido;
    }
    InScope::Nenhum
}

/// O tipo que um elemento de tipo designa (`_resolveToElement`).
fn tipo_do_elemento(inf: &mut BodyInferrer<'_>, el: Element, the_name: dartforge_intern::SymbolId) -> TargetType {
    match el {
        Element::Class(c) => TargetType::Classe(c),
        Element::Typedef(td) => {
            let tgt = inf.outline.typedefs[td.0 as usize].target_type;
            match inf.table.get(tgt).clone() {
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => TargetType::Classe(class),
                Type::TypeParameter { .. } => TargetType::AliasDeParametro,
                _ if inf.e_dynamic(tgt) => TargetType::Dinamico,
                _ if inf.table.e_invalido(tgt) => TargetType::Nada,
                _ => TargetType::NoConstructors(tgt),
            }
        }
        _ => TargetType::NaoClasse(the_name),
    }
}

/// Resolve o alvo escrito de `= …` no escopo do construtor.
fn resolve_written_target(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, params: &[ast::Parameter], red: &ast::RedirectTarget) -> Option<WrittenTarget> {
    let (names, written): (Vec<ast::Name>, Option<ast::Name>) = {
        let node = inf.program.unit(unit).ast.ty(red.ty);
        let ast::TypeKind::Named { name, .. } = &node.kind else { return None };
        (name.to_vec(), red.constructor)
    };
    let without_new = |inf: &BodyInferrer<'_>, s: Option<dartforge_intern::SymbolId>| match s {
        Some(x) if Some(x) == inf.sym.new_ => None,
        x => x,
    };
    // (prefixo, nome do tipo, nome do construtor)
    let (prefix, typ, ctor): (Option<ast::Name>, ast::Name, Option<ast::Name>) = match (names.as_slice(), written) {
        ([a], c) => (None, *a, c),
        ([a, b], None) => {
            // `A.b`: o parser não distingue `prefixo.Tipo` de
            // `Tipo.construtor`; o resolvedor reescreve o segundo quando o
            // primeiro designa uma classe ou alias.
            match name_in_constructor_scope(inf, unit, cls, params, a.sym) {
                InScope::Elemento(Element::Class(_) | Element::Typedef(_)) => (None, *a, Some(*b)),
                _ => (Some(*a), *b, None),
            }
        }
        ([a, b], Some(c)) => (Some(*a), *b, Some(c)),
        ([a, b, c], None) => (Some(*a), *b, Some(*c)),
        _ => return None,
    };
    let the_constructor = without_new(inf, ctor.map(|n| n.sym));
    let mut cited_name: String = match prefix {
        Some(p) => format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(typ.sym)),
        None => inf.interner.resolve(typ.sym).to_string(),
    };
    if let Some(c) = ctor {
        cited_name.push('.');
        cited_name.push_str(inf.interner.resolve(c.sym));
    }
    let type_range = dartforge_diagnostics::Span { start: prefix.map_or(typ.span.start, |p| p.span.start), end: typ.span.end };
    let target_type = match prefix {
        None => match name_in_constructor_scope(inf, unit, cls, params, typ.sym) {
            InScope::Elemento(el) => tipo_do_elemento(inf, el, typ.sym),
            InScope::TypeParam(p) => TargetType::NoConstructors(inf.table.intern(Type::TypeParameter { param: p, nullable: false })),
            InScope::Embutido if inf.interner.resolve(typ.sym) == "dynamic" => TargetType::Dinamico,
            InScope::Embutido => TargetType::NoConstructors(inf.core.never),
            InScope::Prefixo | InScope::Outro => TargetType::NaoClasse(typ.sym),
            InScope::Nenhum => {
                if crate::scope::should_ignore_undefined(inf.program, inf.interner, unit, None, typ.sym) {
                    TargetType::Nada
                } else {
                    TargetType::NaoClasse(typ.sym)
                }
            }
        },
        Some(p) => match name_in_constructor_scope(inf, unit, cls, params, p.sym) {
            InScope::Prefixo => match inf.program.lookup_prefixed_na_unidade(unit, p.sym, typ.sym).and_then(|b| b.getter) {
                Some(el) => tipo_do_elemento(inf, el, typ.sym),
                None if crate::scope::should_ignore_undefined(inf.program, inf.interner, unit, Some(p.sym), typ.sym) => TargetType::Nada,
                None => TargetType::NaoClasse(typ.sym),
            },
            InScope::Nenhum => {
                if crate::scope::should_ignore_undefined(inf.program, inf.interner, unit, Some(p.sym), typ.sym) {
                    TargetType::Nada
                } else {
                    TargetType::NaoClasse(typ.sym)
                }
            }
            // `p.X.n` com `p` classe: sintaxe que não chega a alvo.
            InScope::Elemento(Element::Class(_) | Element::Typedef(_)) => TargetType::Nada,
            _ => TargetType::ShadowedPrefix,
        },
    };
    Some(WrittenTarget { tipo: target_type, construtor: the_constructor, cited_name, type_range, prefixo: prefix })
}

/// O construtor que um alvo designa (`lookUpConstructor`: com o teste de
/// acessibilidade do privado).
#[derive(Clone, Copy)]
enum Construtor {
    Declarado(FunctionElementId),
    /// O sintético sem nome de uma classe abstrata ou enum sem construtor
    /// declarado (o modelo só cria o das classes concretas), ou o primário de
    /// um tipo de extensão.
    Implicito,
}

fn find_constructor(inf: &BodyInferrer<'_>, tgt: ClassId, the_name: Option<dartforge_intern::SymbolId>, lib: dartforge_elements::model::LibraryId) -> Option<Construtor> {
    let key = the_name.or(inf.sym.vazio)?;
    if inf.interner.resolve(key).starts_with('_') && inf.program.class(tgt).library != lib {
        return None;
    }
    match inf.constructor_or_primary(tgt, key) {
        Some(Some(f)) => Some(Construtor::Declarado(f)),
        Some(None) => Some(Construtor::Implicito),
        None => {
            if the_name.is_some() {
                return None;
            }
            // O sintético que o modelo não cria, também repassado por uma
            // aplicação de mixin.
            let mut c = tgt;
            loop {
                let ce = inf.program.class(c);
                match ce.kind {
                    ClassKind::MixinApplication => c = ce.supertype_class?,
                    ClassKind::Class | ClassKind::Enum if ce.constructors.is_empty() => return Some(Construtor::Implicito),
                    _ => return None,
                }
            }
        }
    }
}

/// O sintético sem nome que o modelo não cria (classe abstrata sem
/// construtor declarado, também através de aplicações de mixin).
pub(crate) fn without_implicit_name(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let mut current = c;
    for _ in 0..16 {
        let ce = inf.program.class(current);
        match ce.kind {
            ClassKind::MixinApplication => match ce.supertype_class {
                Some(s) => current = s,
                None => return false,
            },
            ClassKind::Class | ClassKind::Enum => return ce.constructors.is_empty(),
            _ => return false,
        }
    }
    false
}

/// `isFactory`/`isConst` de um construtor achado.
fn factory_and_const(inf: &BodyInferrer<'_>, tgt: ClassId, c: Construtor) -> (bool, bool) {
    match c {
        Construtor::Declarado(f) => {
            let fe = inf.program.function(f);
            (fe.factory, fe.const_)
        }
        // O sintético de enum é `const`; o de classe não. O primário de um
        // tipo de extensão é `const` com `extension type const`.
        Construtor::Implicito => {
            let ce = inf.program.class(tgt);
            match ce.kind {
                ClassKind::Enum => (false, true),
                ClassKind::ExtensionType => {
                    let const_ = ce.decl.is_some_and(|d| matches!(&inf.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::ExtensionType(et) if et.const_));
                    (false, const_)
                }
                _ => (false, false),
            }
        }
    }
}

/// O tipo de função de um construtor, com o retorno trocado pelo tipo
/// `this` da classe (o do construtor repassado de uma aplicação de mixin é
/// o da aplicação).
fn constructor_type(inf: &mut BodyInferrer<'_>, tgt: ClassId, c: Construtor) -> TypeId {
    let this = inf.class_this_type(tgt);
    let sig = match c {
        Construtor::Declarado(f) if !matches!(inf.program.function(f).node, FunctionRef::None) => inf.outline.functions[f.0 as usize].signature,
        Construtor::Implicito if inf.program.class(tgt).kind == ClassKind::ExtensionType => inf.primary_signature(tgt),
        _ => inf.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: this,
            positional: Box::new([]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        }),
    };
    // Os parâmetros de tipo da classe ficam livres (o `ConstructorElement.type`
    // não tem parâmetros de tipo próprios).
    match inf.table.get(sig).clone() {
        Type::Function { positional, optional, named, nullable, .. } => {
            inf.table.intern(Type::Function { type_params: Box::new([]), ret: this, positional, optional, named, nullable })
        }
        _ => sig,
    }
}

/// `redirectedConstructor?.declaration` de um construtor declarado: o alvo
/// de `= …` ou do `this(...)` (o primeiro), quando é um construtor
/// declarado.
pub(crate) fn redirected_constructor(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> Option<FunctionElementId> {
    let fe = inf.program.function(f);
    let FunctionRef::Constructor { unit, member } = fe.node else { return None };
    let cls = fe.class;
    let lib = fe.library;
    let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return None };
    if let Some(red) = &ctor.redirect {
        let tgt = resolve_written_target(inf, unit, cls, &ctor.parameters, red)?;
        let TargetType::Classe(c) = tgt.tipo else { return None };
        return match find_constructor(inf, c, tgt.construtor, lib)? {
            Construtor::Declarado(g) => Some(g),
            Construtor::Implicito => None,
        };
    }
    if ctor.factory || is_primary_constructor(inf.program, f) {
        return None;
    }
    let the_name = ctor.initializers.iter().find_map(|i| match i {
        ast::Initializer::Redirect { constructor, .. } => Some(*constructor),
        _ => None,
    })?;
    let c = cls?;
    let key = match the_name.map(|n| n.sym) {
        Some(s) if Some(s) == inf.sym.new_ => inf.sym.vazio,
        Some(s) => Some(s),
        None => inf.sym.vazio,
    }?;
    inf.construtor_de(c, key)
}

/// Os parâmetros do construtor primário da declaração de `f`, quando `f` é
/// uma parte `this` que não virou o k2: a segunda parte em diante de classe e
/// enum, e a parte do tipo de extensão (que não é elaborada). No 3.13.4 toda
/// `PrimaryConstructorBody` resolve no escopo do primário
/// (`scope_context.dart:406-430`: `ConstructorInitializerScope` nos
/// inicializadores, `PrimaryParameterScope` no corpo), com os parâmetros
/// visíveis. Sai (nome, tipo, final, `this.`/`super.`/declarante).
fn the_primary_parameters(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> Vec<(ast::Name, TypeId, bool, bool)> {
    let program = inf.program;
    let Some(c) = program.function(f).class else { return Vec::new() };
    let Some(d) = program.class(c).decl else { return Vec::new() };
    let ast = &program.unit(d.unit).ast;
    let k2 = match &ast.decl(d.decl).kind {
        ast::DeclKind::Class(x) => x.primary_constructor,
        ast::DeclKind::Enum(x) => x.primary_constructor,
        ast::DeclKind::ExtensionType(x) => {
            let n = x.representation_name;
            let field = program.class(c).fields.iter().copied().find(|&v| program.variable(v).name == n.sym && !program.variable(v).static_);
            let t = match field {
                Some(v) => inf.variable_type(v),
                None => inf.core.dynamic_,
            };
            return vec![(n, t, true, true)];
        }
        _ => None,
    };
    let Some(k2) = k2 else { return Vec::new() };
    let Some(g) = program
        .class(c)
        .constructors
        .values()
        .copied()
        .find(|&g| matches!(program.function(g).node, FunctionRef::Constructor { unit, member } if unit == d.unit && member == k2))
    else {
        return Vec::new();
    };
    if g == f {
        return Vec::new();
    }
    let ast::MemberKind::Constructor(kc) = &ast.member(k2).kind else { return Vec::new() };
    let types: Vec<TypeId> = inf.outline.functions[g.0 as usize].parameters.iter().map(|p| p.ty).collect();
    kc.parameters
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let n = p.name?;
            let t = types.get(i).copied().unwrap_or(inf.core.dynamic_);
            Some((n, t, p.final_ || p.this_ || p.super_, p.this_ || p.super_))
        })
        .collect()
}

/// `f` é o construtor primário (elaborado) de uma classe ou enum: o
/// redirecionamento dele é `PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT` e não entra
/// na cadeia de `RECURSIVE_CONSTRUCTOR_REDIRECT`.
fn is_primary_constructor(program: &dartforge_elements::model::Program, f: FunctionElementId) -> bool {
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return false };
    program.unit(unit).ast.decls.iter().any(|d| match &d.kind {
        ast::DeclKind::Class(x) => x.primary_constructor == Some(member),
        ast::DeclKind::Enum(x) => x.primary_constructor == Some(member),
        _ => false,
    })
}

/// `_hasRedirectingFactoryConstructorCycle`
/// (`an611:src/generated/error_verifier.dart:6327-6338`): `f` volta a si
/// mesmo pela cadeia de redirecionamentos (um ciclo adiante dele não conta).
pub(crate) fn redirects_in_cycle(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(f);
    while let Some(g) = current {
        if !seen.insert(g) {
            return g == f;
        }
        current = redirected_constructor(inf, g);
    }
    false
}

/// As regras de `visitConstructorDeclaration` para a factory
/// redirecionadora `= alvo` (`an611:src/generated/error_verifier.dart:589-605`),
/// na ordem do analyzer:
///
/// 1. `_checkForRedirectingConstructorErrorCodes` (`:5094-5137`):
///    `redirect_to_non_const_constructor`,
///    `redirect_to_abstract_class_constructor` e
///    `invalid_reference_to_generative_enum_constructor`;
/// 2. `_checkForRecursiveFactoryRedirect` (`:5073-5089`) ou, sem ciclo,
///    `_checkForAllRedirectConstructorErrorCodes` (`:2025-2075`):
///    `redirect_to_missing_constructor`, `redirect_to_invalid_return_type` e
///    `redirect_to_invalid_function_type`.
///
/// Antes, os da resolução do tipo do alvo (`redirect_to_non_class`,
/// `redirect_to_type_alias_expands_to_type_parameter`,
/// `prefix_shadowed_by_local_declaration`).
fn redirecting_factory_target(inf: &mut BodyInferrer<'_>, cx: &mut Body, red: &ast::RedirectTarget, f_current: FunctionElementId, ctor: &ast::Constructor) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let fe = inf.program.function(f_current);
    let (owner, lib, current_const) = (fe.class, fe.library, fe.const_);
    let Some(tgt) = resolve_written_target(inf, cx.unit, owner, &ctor.parameters, red) else { return };
    let cls = match tgt.tipo {
        TargetType::Classe(c) => c,
        TargetType::NaoClasse(the_name) => {
            let text = inf.interner.resolve(the_name).to_string();
            inf.warning_with_code(ce::REDIRECT_TO_NON_CLASS, tgt.type_range, &[&text]);
            return;
        }
        TargetType::AliasDeParametro => {
            inf.warning_with_code(ce::REDIRECT_TO_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, tgt.type_range, &[]);
            return;
        }
        TargetType::ShadowedPrefix => {
            if let Some(p) = tgt.prefixo {
                let text = inf.interner.resolve(p.sym).to_string();
                inf.warning_with_code(ce::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, p.span, &[&text]);
            }
            return;
        }
        TargetType::NoConstructors(t) => {
            if !redirects_in_cycle(inf, f_current) {
                let displayed = inf.table.format(t, inf.interner, inf.program);
                inf.warning_with_code(ce::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&tgt.cited_name, &displayed]);
            }
            return;
        }
        TargetType::Dinamico | TargetType::Nada => return,
    };
    // Os argumentos de tipo escritos do alvo (`= B<T>.nome`), no escopo do
    // construtor: o `NamedType.type` que o `TypeArgumentsVerifier` lê.
    let written_ones: Vec<ast::TypeId> = match &inf.program.unit(cx.unit).ast.ty(red.ty).kind {
        ast::TypeKind::Named { args, .. } => args.to_vec(),
        _ => Vec::new(),
    };
    let written_args: Vec<TypeId> = written_ones.iter().map(|&x| inf.type_argument_type(cx, x)).collect();
    let n_params = inf.outline.classes[cls.0 as usize].type_params.len();
    if !written_ones.is_empty() && written_ones.len() != n_params {
        let text = inf.interner.resolve(inf.program.class(cls).name).to_string();
        let sp = inf.program.unit(cx.unit).ast.ty(red.ty).span;
        inf.report_type_arguments(&text, n_params, written_ones.len(), sp);
    }
    let finding = find_constructor(inf, cls, tgt.construtor, lib);
    // 1. `_checkForRedirectingConstructorErrorCodes`.
    if let Some(c) = finding {
        let (factory, const_target) = factory_and_const(inf, cls, c);
        if current_const && !const_target {
            inf.warning_with_code(ce::REDIRECT_TO_NON_CONST_CONSTRUCTOR, red.span, &[]);
        }
        let ce_target = inf.program.class(cls);
        if matches!(ce_target.kind, ClassKind::Class | ClassKind::MixinApplication) && ce_target.modifiers.abstract_ && !factory {
            let mut the_name = owner.map(|d| inf.interner.resolve(inf.program.class(d).name).to_string()).unwrap_or_default();
            if let Some(n) = ctor.name {
                the_name.push('.');
                the_name.push_str(inf.interner.resolve(n.sym));
            }
            let target_class = inf.interner.resolve(ce_target.name).to_string();
            inf.warning_with_code(ce::REDIRECT_TO_ABSTRACT_CLASS_CONSTRUCTOR, red.span, &[&the_name, &target_class]);
        }
        // `_checkForInvalidGenerativeConstructorReference`.
        if inf.program.class(cls).kind == ClassKind::Enum && !factory {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), red.span);
        }
    }
    // 2. Ciclo, ou as regras do alvo.
    if finding.is_some_and(|c| matches!(c, Construtor::Declarado(_))) && redirects_in_cycle(inf, f_current) {
        inf.warning_with_code(ce::RECURSIVE_FACTORY_REDIRECT, red.span, &[]);
        return;
    }
    let Some(c) = finding else {
        // `{1}`: o tipo do alvo como `DartType` (o da classe instanciada com
        // os próprios parâmetros, que é o que a inferência dá sem
        // argumentos escritos).
        let typ = inf.class_this_type(cls);
        let displayed = inf.table.format(typ, inf.interner, inf.program);
        inf.warning_with_code(ce::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&tgt.cited_name, &displayed]);
        return;
    };
    let Some(owner) = owner else { return };
    // O tipo do alvo (`NamedTypeResolver._instantiateElement`, 3.6.2
    // `named_type_resolver.dart:202-262`): com argumentos escritos, a classe
    // instanciada com eles (`dynamic` em todos se a contagem erra); sem eles,
    // `_inferRedirectedConstructor` (`:172-200`): a própria classe ou a sem
    // parâmetros de tipo vale o `thisType`; senão os argumentos saem da
    // inferência de `C<X…> <: Dona<T…>` (o `thisType` da dona como contexto),
    // pelos tipos finais. O alias escrito (`= Alias.nome`) não infere: é a
    // instanciação do alias (com os argumentos escritos, ou pelos limites,
    // `instantiateTypeAliasToBounds`), o tipo da própria anotação (os relatos
    // dela já saíram na resolução do alvo).
    let target_params = inf.outline.classes[cls.0 as usize].type_params.clone();
    let written_via_alias = match &inf.program.unit(cx.unit).ast.ty(red.ty).kind {
        ast::TypeKind::Named { name, .. } => {
            let binding = if name.len() == 2 {
                inf.program.lookup_prefixed_na_unidade(cx.unit, name[0].sym, name[1].sym)
            } else {
                name.first().and_then(|n| inf.program.lookup_na_unidade(cx.unit, n.sym))
            };
            matches!(binding.and_then(|b| b.getter), Some(Element::Typedef(_)))
        }
        _ => false,
    };
    let the_target_args: Vec<TypeId> = if written_via_alias && !target_params.is_empty() {
        let (n_diag, n_units) = (inf.diagnostics.len(), inf.warning_units.len());
        let t = inf.type_from_annotation(cx, red.ty);
        inf.diagnostics.truncate(n_diag);
        inf.warning_units.truncate(n_units);
        match inf.table.get(t).clone() {
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } if class == cls => args.to_vec(),
            _ => return,
        }
    } else if target_params.is_empty() || (written_ones.is_empty() && cls == owner) {
        target_params.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect()
    } else if !written_ones.is_empty() {
        if written_args.len() == target_params.len() {
            written_args
        } else {
            vec![inf.core.dynamic_; target_params.len()]
        }
    } else {
        let this_target = inf.class_this_type(cls);
        let owner_this = inf.class_this_type(owner);
        let mut gi = crate::constraints::GenericInferrer::new(&target_params);
        let mut env = inf.env();
        gi.constrain_return(this_target, owner_this, &mut env);
        let r = gi.choose_final(&mut env);
        drop(env);
        r
    };
    let target_map = inf.mapa(&target_params, &the_target_args);
    let of = constructor_type(inf, cls, c);
    let of = inf.subst(of, &target_map);
    let for_ = constructor_type(inf, owner, Construtor::Declarado(f_current));
    let (Type::Function { ret: ret_of, .. }, Type::Function { ret: ret_for, .. }) = (inf.table.get(of).clone(), inf.table.get(for_).clone()) else { return };
    if !inf.atribuivel(ret_of, ret_for) {
        let (a, b) = (inf.table.format(ret_of, inf.interner, inf.program), inf.table.format(ret_for, inf.interner, inf.program));
        inf.warning_with_code(ce::REDIRECT_TO_INVALID_RETURN_TYPE, red.span, &[&a, &b]);
    } else if !inf.sub(of, for_) {
        let (a, b) = (inf.table.format(of, inf.interner, inf.program), inf.table.format(for_, inf.interner, inf.program));
        inf.warning_with_code(ce::REDIRECT_TO_INVALID_FUNCTION_TYPE, red.span, &[&a, &b]);
    }
}

/// `enclosingClass.supertype` do analyzer para uma classe
/// (`an611:src/summary2/types_builder.dart:117-127` e
/// `library_builder.dart:732-743`): o `extends` escrito quando é uma classe
/// (não enum, tipo de extensão, mixin, `Function`, `Null` nem anulável);
/// senão `Object`. Nada para o próprio `Object`, para enums (o supertipo é
/// `Enum`, cujo sintético sem nome nunca falta) e para o que não é classe.
/// Com mixins o supertipo continua o do `extends` (o analyzer não
/// desaçucara a aplicação).
fn analyzer_supertype(inf: &mut BodyInferrer<'_>, c: ClassId) -> Option<(ClassId, TypeId)> {
    if inf.program.class(c).kind != ClassKind::Class {
        return None;
    }
    let object = inf.core.object_class?;
    if c == object {
        return None;
    }
    if let Some(t) = inf.outline.classes[c.0 as usize].supertype
        && let Type::Interface { class: s, nullable: false, .. } = inf.table.get(t).clone()
        && matches!(inf.program.class(s).kind, ClassKind::Class | ClassKind::MixinApplication)
        && Some(s) != inf.core.function_class
        && Some(s) != inf.core.null_class
    {
        return Some((s, t));
    }
    Some((object, inf.core.object))
}

/// `shouldIgnoreUndefinedNamedType` do `extends` escrito de `c`.
fn ignored_extends(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let Some((u, t)) = inf.program.class(c).supertype else { return false };
    let ast::TypeKind::Named { name, .. } = &inf.program.unit(u).ast.ty(t).kind else { return false };
    let (prefix, the_name) = match name.as_ref() {
        [a] => (None, a.sym),
        [p, a] => (Some(p.sym), a.sym),
        _ => return false,
    };
    crate::scope::should_ignore_undefined(inf.program, inf.interner, u, prefix, the_name)
}

/// `constructors.every((c) => c.isFactory)` da classe `s` (o sintético é
/// gerador; os de uma aplicação de mixin são os geradores repassados da
/// base, e a lista vazia passa).
fn only_factories(inf: &BodyInferrer<'_>, s: ClassId) -> bool {
    let ce = inf.program.class(s);
    if ce.kind == ClassKind::MixinApplication {
        let mut base = ce.supertype_class;
        while let Some(b) = base {
            let be = inf.program.class(b);
            if be.kind != ClassKind::MixinApplication {
                return !(be.constructors.is_empty() || be.constructors.values().any(|&f| !inf.program.function(f).factory));
            }
            base = be.supertype_class;
        }
        return true;
    }
    !ce.constructors.is_empty() && ce.constructors.values().all(|&f| inf.program.function(f).factory)
}

/// Os parâmetros de um construtor achado: o nó do AST (com a unidade) e os
/// tipos do outline, na mesma ordem.
fn parametros_do_construtor<'a>(inf: &BodyInferrer<'a>, k: Construtor) -> Option<(UnitId, &'a ast::Constructor, Vec<crate::resolve::ParameterTypeData>)> {
    let Construtor::Declarado(f) = k else { return None };
    let the_program: &'a dartforge_elements::model::Program = inf.program;
    let FunctionRef::Constructor { unit, member } = the_program.function(f).node else { return None };
    let ast::MemberKind::Constructor(ctor) = &the_program.unit(unit).ast.member(member).kind else { return None };
    Some((unit, ctor, inf.outline.functions[f.0 as usize].parameters.to_vec()))
}

/// Os posicionais obrigatórios e os nomes dos nomeados `required`.
fn constructor_required(inf: &BodyInferrer<'_>, k: Construtor) -> (usize, Vec<dartforge_intern::SymbolId>) {
    let Some((_, ctor, _)) = parametros_do_construtor(inf, k) else { return (0, Vec::new()) };
    let positional_args = ctor.parameters.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
    let named_args = ctor.parameters.iter().filter(|p| p.kind == ast::ParameterKind::Named && p.required).filter_map(|p| p.nome_externo().map(|n| n.sym)).collect();
    (positional_args, named_args)
}

/// `ConstructorElement.getDisplayString()`
/// (`an611:src/dart/element/display_string_builder.dart:78-93`, `:405-461`,
/// `:531-553`): `Tipo Classe.nome(int a, [int b = 0], {required int c})`,
/// com o tipo e os parâmetros do construtor visto pelo tipo `tipo` (o
/// `ConstructorMember` substituído).
/// `sem_retorno`: a exibição do elemento no 3.13.4 não traz o tipo de retorno
/// (`'A.named()'`, não `'A A.named()'`).
fn display_constructor(inf: &mut BodyInferrer<'_>, tgt: ClassId, k: Construtor, typ: TypeId, without_return: bool) -> String {
    let mut s = String::new();
    if !without_return {
        s.push_str(&inf.table.format(typ, inf.interner, inf.program));
        s.push(' ');
    }
    s.push_str(inf.interner.resolve(inf.program.class(tgt).name));
    if let Construtor::Declarado(f) = k {
        let the_name = inf.program.function(f).name;
        if Some(the_name) != inf.sym.vazio {
            s.push('.');
            s.push_str(inf.interner.resolve(the_name));
        }
    }
    s.push('(');
    let map = match inf.table.get(typ).clone() {
        Type::Interface { args, .. } => {
            let ps = inf.outline.classes[tgt.0 as usize].type_params.clone();
            (ps.len() == args.len()).then(|| inf.mapa(&ps, &args))
        }
        _ => None,
    };
    if let Some((unit, ctor, types)) = parametros_do_construtor(inf, k) {
        let the_program: &dartforge_elements::model::Program = inf.program;
        let source = &the_program.unit(unit).source;
        let mut pieces: Vec<(ast::ParameterKind, String)> = Vec::new();
        for (i, p) in ctor.parameters.iter().enumerate() {
            let mut t = types.get(i).map_or(inf.core.dynamic_, |d| d.ty);
            if let Some(m) = &map {
                t = inf.subst(t, m);
            }
            let mut text = String::new();
            if p.kind == ast::ParameterKind::Named && p.required {
                text.push_str("required ");
            }
            text.push_str(&inf.table.format(t, inf.interner, inf.program));
            text.push(' ');
            if let Some(n) = p.nome_externo() {
                text.push_str(inf.interner.resolve(n.sym));
            }
            if let Some(d) = p.default_value {
                let sp = the_program.unit(unit).ast.expr(d).span;
                text.push_str(" = ");
                text.push_str(&source[sp.start..sp.end]);
            }
            pieces.push((p.kind, text));
        }
        let mut last: Option<ast::ParameterKind> = None;
        let mut closure = "";
        for (i, (type_p, text)) in pieces.iter().enumerate() {
            if i != 0 {
                s.push_str(", ");
            }
            if last != Some(*type_p) {
                s.push_str(closure);
                let (open, closes) = match type_p {
                    ast::ParameterKind::Required => ("", ""),
                    ast::ParameterKind::Optional => ("[", "]"),
                    ast::ParameterKind::Named => ("{", "}"),
                };
                s.push_str(open);
                closure = closes;
                last = Some(*type_p);
            }
            s.push_str(text);
        }
        s.push_str(closure);
    }
    s.push(')');
    s
}

/// `super(...)`/`super.n(...)`: `ElementResolver.visitSuperConstructorInvocation`
/// (`an611:src/generated/element_resolver.dart:338-404`):
/// `undefined_constructor_in_initializer(_default)` sem o construtor (ou
/// privado de outra biblioteca) e `non_generative_constructor` com uma
/// factory numa superclasse que tem algum gerador; depois os argumentos
/// contra o construtor (salvo com o `extends` calado por
/// `shouldIgnoreUndefinedNamedType`).
fn super_initializer(inf: &mut BodyInferrer<'_>, cx: &mut Body, cls: Option<ClassId>, span: dartforge_diagnostics::Span, constructor: Option<ast::Name>, arguments: &ast::Arguments, params: &[ast::Parameter]) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let sup = cls.and_then(|c| analyzer_supertype(inf, c).map(|s| (c, s)));
    let Some((c, (s, st))) = sup else {
        for a in arguments.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return;
    };
    let key = match constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => None,
        x => x,
    };
    let lib = inf.program.class(c).library;
    let Some(k) = find_constructor(inf, s, key, lib) else {
        let displayed = inf.table.format(st, inf.interner, inf.program);
        match constructor {
            Some(n) => {
                let the_name = inf.interner.resolve(n.sym).to_string();
                inf.warning_with_code(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER, span, &[&displayed, &the_name]);
            }
            None => inf.warning_with_code(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT, span, &[&displayed]),
        }
        for a in arguments.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return;
    };
    if factory_and_const(inf, s, k).0 && !only_factories(inf, s) {
        let v313 = inf.program.referencia(cx.unit) == dartforge_diagnostics::Referencia::V3_13;
        let displayed = display_constructor(inf, s, k, st, v313);
        inf.warning_with_code(ce::NON_GENERATIVE_CONSTRUCTOR, span, &[&displayed]);
    }
    if ignored_extends(inf, c) {
        for a in arguments.args.iter() {
            infer_free(inf, cx, a.value);
        }
        return;
    }
    let tgt = match inf.table.get(st).clone() {
        Type::Interface { class, args, .. } => Some((class, args)),
        _ => None,
    };
    call_constructor_of(inf, cx, tgt, constructor, arguments, params, span);
}

/// `this(...)`/`this.n(...)` (`_checkForConflictingInitializerErrorCodes`,
/// `an611:src/generated/error_verifier.dart:2705-2733`): fora de `factory`,
/// `redirect_generative_to_missing_constructor` sem o alvo (procurado só na
/// própria classe) e `redirect_generative_to_non_generative_constructor`
/// com uma factory; e `redirect_to_non_const_constructor` no nome (ou no
/// `this`) quando o atual é `const` e o alvo não.
///
/// No corpo do construtor primário, o 3.13.4 só relata
/// `PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT` (o parser) e sai
/// (`_checkForConflictingPrimaryConstructorInitializers`,
/// `error_verifier.dart:3984-3999`): nenhum destes.
fn redirecting_initializer(inf: &mut BodyInferrer<'_>, cls: Option<ClassId>, span: dartforge_diagnostics::Span, constructor: Option<ast::Name>, current: FunctionElementId) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let Some(c) = cls else { return };
    if is_primary_constructor(inf.program, current) {
        return;
    }
    let key = match constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => inf.sym.vazio,
        Some(x) => Some(x),
        None => inf.sym.vazio,
    };
    let finding = key.and_then(|k| inf.constructor_or_primary(c, k)).map(|f| match f {
        Some(f) => Construtor::Declarado(f),
        None => Construtor::Implicito,
    });
    let fe = inf.program.function(current);
    let (current_factory, current_const) = (fe.factory, fe.const_);
    if !current_factory {
        match finding {
            None => {
                let class_name = inf.interner.resolve(inf.program.class(c).name).to_string();
                let mut the_name = class_name.clone();
                if let Some(n) = constructor {
                    the_name.push('.');
                    the_name.push_str(inf.interner.resolve(n.sym));
                }
                inf.warning_with_code(ce::REDIRECT_GENERATIVE_TO_MISSING_CONSTRUCTOR, span, &[&the_name, &class_name]);
            }
            Some(k) if factory_and_const(inf, c, k).0 => {
                inf.warning_with_code(ce::REDIRECT_GENERATIVE_TO_NON_GENERATIVE_CONSTRUCTOR, span, &[]);
            }
            Some(_) => {}
        }
    }
    if let Some(k) = finding
        && current_const
        && !factory_and_const(inf, c, k).1
    {
        let sp = constructor.map_or(dartforge_diagnostics::Span { start: span.start, end: span.start + 4 }, |n| n.span);
        inf.warning_with_code(ce::REDIRECT_TO_NON_CONST_CONSTRUCTOR, sp, &[]);
    }
}

/// `SuperConstructorResolver._constructor`
/// (`an611:src/summary2/super_constructor_resolver.dart:28-62`): o
/// `superConstructor` é o construtor do supertipo com o nome do `super(...)`
/// (ou o sem nome), comparado ao pé da letra (`super.new()` não acha), sem
/// teste de acessibilidade e factory incluída; só para classes (enum fica
/// sem).
/// A cadeia de superclasses de `c` volta a `c` (ciclo de herança).
fn in_superclass_cycle(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let mut current = inf.program.class(c).supertype_class;
    for _ in 0..64 {
        match current {
            Some(k) if k == c => return true,
            Some(k) => current = inf.program.class(k).supertype_class,
            None => return false,
        }
    }
    false
}

fn super_constructor(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor) -> Option<(ClassId, TypeId, Construtor)> {
    let the_name = ctor
        .initializers
        .iter()
        .find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        })
        .flatten();
    let (s, st) = analyzer_supertype(inf, c)?;
    let key = the_name.or(inf.sym.vazio)?;
    if Some(key) == inf.sym.new_ {
        return None;
    }
    let mut current = s;
    let mut passed_on = false;
    for _ in 0..16 {
        let ce = inf.program.class(current);
        if ce.kind == ClassKind::MixinApplication {
            current = ce.supertype_class?;
            passed_on = true;
            continue;
        }
        if let Some(&f) = ce.constructors.get(&key) {
            if passed_on && inf.program.function(f).factory {
                return None;
            }
            if matches!(inf.program.function(f).node, FunctionRef::None) {
                return Some((s, st, Construtor::Implicito));
            }
            return Some((s, st, Construtor::Declarado(f)));
        }
        return (Some(key) == inf.sym.vazio && ce.constructors.is_empty() && ce.kind == ClassKind::Class).then_some((s, st, Construtor::Implicito));
    }
    None
}

/// `ErrorVerifier.visitSuperFormalParameter`
/// (`an611:src/generated/error_verifier.dart:1457-1503`) nos `super.x` de um
/// construtor gerador não redirecionador e não `external` (os outros lugares
/// são `invalid_super_formal_parameter_location`, em `analise`; num tipo de
/// extensão, `extension_type_constructor_with_super_formal_parameter`): sem
/// o parâmetro associado (o posicional pelo índice entre os `super.`, o
/// nomeado pelo nome) `super_formal_parameter_without_associated_*`; com
/// ele e o tipo escrito fora do dele,
/// `super_formal_parameter_type_is_not_subtype_of_associated`. No nome.
fn super_parameters(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor, types: &[TypeId]) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    if ctor.factory || ctor.external || ctor.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. })) {
        return;
    }
    if inf.program.class(c).kind == ClassKind::ExtensionType || !ctor.parameters.iter().any(|p| p.super_) {
        return;
    }
    // A superclasse num ciclo de herança é cortada (fica `Object`), e o enum
    // não chega a um construtor com parâmetros: nos dois, o construtor
    // associado não tem posicionais, e cada `super.x` posicional relata.
    let cycle = in_superclass_cycle(inf, c);
    let without_parameters = cycle || inf.program.class(c).kind == ClassKind::Enum;
    let associated = if cycle { None } else { super_constructor(inf, c, ctor) };
    // No 3.13.4 (`error_verifier.dart:2391-2408` do checkout main), sem o
    // `superConstructor` a verificação para; no 3.6.2 cada `super.x` relata.
    let unit_id = inf.program.class(c).decl.map(|d| d.unit);
    let v313 = unit_id.is_some_and(|u| inf.program.referencia(u) == dartforge_diagnostics::Referencia::V3_13);
    if associated.is_none() && v313 && !without_parameters {
        return;
    }
    // Os parâmetros do construtor associado, vistos pelo supertipo.
    let (positional_args, named_args): (Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId)>) = match associated {
        Some((s, st, k)) => {
            let map = match inf.table.get(st).clone() {
                Type::Interface { args, .. } => {
                    let ps = inf.outline.classes[s.0 as usize].type_params.clone();
                    (ps.len() == args.len()).then(|| inf.mapa(&ps, &args))
                }
                _ => None,
            };
            let mut pos = Vec::new();
            let mut the_named = Vec::new();
            if let Some((_, k_ast, data)) = parametros_do_construtor(inf, k) {
                for (i, p) in k_ast.parameters.iter().enumerate() {
                    let mut t = data.get(i).map_or(inf.core.dynamic_, |d| d.ty);
                    if let Some(m) = &map {
                        t = inf.subst(t, m);
                    }
                    match p.kind {
                        ast::ParameterKind::Named => {
                            if let Some(n) = p.nome_externo() {
                                the_named.push((n.sym, t));
                            }
                        }
                        _ => pos.push(t),
                    }
                }
            }
            (pos, the_named)
        }
        None => (Vec::new(), Vec::new()),
    };
    // O índice do posicional é o `indexIn` do elemento (`element.dart:9318-9323`):
    // no 3.6.2, dois `super._` homônimos são o mesmo elemento pela localização,
    // e o segundo fica com o índice do primeiro.
    let super_names: Vec<Option<dartforge_intern::SymbolId>> =
        ctor.parameters.iter().filter(|p| p.super_).map(|p| p.name.map(|n| n.sym)).collect();
    let mut idx = 0usize;
    for (i, p) in ctor.parameters.iter().enumerate() {
        if !p.super_ {
            continue;
        }
        let element_index = match p.name {
            Some(n) if !v313 => super_names.iter().position(|s| *s == Some(n.sym)).unwrap_or(idx),
            _ => idx,
        };
        let Some(n) = p.name else {
            idx += 1;
            continue;
        };
        let tgt = if p.kind == ast::ParameterKind::Named {
            let external = p.nome_externo().map_or(n.sym, |x| x.sym);
            named_args.iter().find(|(m, _)| *m == external).map(|(_, t)| *t)
        } else {
            positional_args.get(element_index).copied()
        };
        idx += 1;
        let Some(associated_type) = tgt else {
            let code = if p.kind == ast::ParameterKind::Named { ce::SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_NAMED } else { ce::SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_POSITIONAL };
            inf.warning_with_code(code, n.span, &[]);
            continue;
        };
        // Sem tipo escrito o tipo é o herdado: nunca dispara.
        if p.ty.is_none() && p.function_parameters.is_none() {
            continue;
        }
        let Some(&own) = types.get(i) else { continue };
        if !inf.sub(own, associated_type) {
            inf.warning_with_args(
                ce::SUPER_FORMAL_PARAMETER_TYPE_IS_NOT_SUBTYPE_OF_ASSOCIATED,
                n.span,
                &[crate::exibicao::Arg::Tipo(own), crate::exibicao::Arg::Tipo(associated_type)],
            );
        }
    }
}

/// O `errorRange` de `_checkForUndefinedConstructorInInitializerImplicit`. No
/// 3.6.2, o nome da classe (`returnType`). No 3.13.4 (`error_verifier.dart:2124-2129`,
/// `:7997-8002` do checkout main): o construtor comum vai do nome do tipo (ou do
/// `new`) ao nome dele (`ConstructorDeclaration.errorRange`); o primário, o
/// `this` da parte de corpo quando há uma, senão do primeiro token (o `const`,
/// se escrito) ao nome do construtor (`PrimaryConstructorDeclaration.errorRange`).
fn implicit_super_range(inf: &BodyInferrer<'_>, ctor: &ast::Constructor, unit: UnitId, member: ast::MemberId) -> dartforge_diagnostics::Span {
    use dartforge_diagnostics::Span;
    if inf.program.referencia(unit) != dartforge_diagnostics::Referencia::V3_13 {
        return ctor.class_name.span;
    }
    let u = inf.program.unit(unit);
    let primary = u.ast.decls.iter().any(|d| match &d.kind {
        ast::DeclKind::Class(x) => x.primary_constructor == Some(member),
        ast::DeclKind::Enum(x) => x.primary_constructor == Some(member),
        _ => false,
    });
    if !primary {
        let end = ctor.name.map_or(ctor.class_name.span.end, |n| n.span.end);
        return Span { start: ctor.class_name.span.start, end: end };
    }
    let ms = u.ast.member(member).span;
    if ms.start != ctor.class_name.span.start && u.source[ms.start..].starts_with("this") {
        return Span { start: ms.start, end: ms.start + 4 };
    }
    let start = if ctor.const_ {
        let before = u.source[..ctor.class_name.span.start].trim_end();
        if before.ends_with("const") { before.len() - "const".len() } else { ctor.class_name.span.start }
    } else {
        ctor.class_name.span.start
    };
    let end = match ctor.name {
        Some(n) => n.span.end,
        None if start < ctor.class_name.span.start => start + "const".len(),
        None => ctor.class_name.span.end,
    };
    Span { start: start, end: end }
}

/// `_checkForUndefinedConstructorInInitializerImplicit`
/// (`an611:src/generated/error_verifier.dart:5444-5551`): construtor
/// gerador, não `external`, sem `super(...)` nem `this(...)`, numa classe
/// cuja superclasse tem algum gerador — o sem nome dela falta
/// (`undefined_constructor_in_initializer_default`), é factory
/// (`non_generative_constructor`) ou exige mais argumentos do que os
/// `super.x` dão (`implicit_super_initializer_missing_arguments`; sem
/// `super-parameters`, `no_default_super_constructor`). No nome da classe
/// (até o nome do construtor nos dois últimos).
fn implicit_super(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor, unit: UnitId, member: ast::MemberId) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    if ctor.factory || ctor.external {
        return;
    }
    if ctor.initializers.iter().any(|i| matches!(i, ast::Initializer::Super { .. } | ast::Initializer::Redirect { .. })) {
        return;
    }
    let Some((s, st)) = analyzer_supertype(inf, c) else { return };
    if only_factories(inf, s) {
        return;
    }
    let typ = implicit_super_range(inf, ctor, unit, member);
    let lib = inf.program.class(c).library;
    // `superElement.unnamedConstructor`: sem o teste de acessibilidade.
    let Some(k) = find_constructor(inf, s, None, inf.program.class(s).library) else {
        let the_name = inf.interner.resolve(inf.program.class(s).name).to_string();
        inf.warning_with_code(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT, typ, &[&the_name]);
        return;
    };
    if factory_and_const(inf, s, k).0 {
        // `{0}` = o elemento: `superElement.unnamedConstructor`, sem
        // substituição (o tipo é o da classe com os próprios parâmetros).
        let own = inf.class_this_type(s);
        let v313 = inf.program.referencia(unit) == dartforge_diagnostics::Referencia::V3_13;
        let displayed = display_constructor(inf, s, k, own, v313);
        inf.warning_with_code(ce::NON_GENERATIVE_CONSTRUCTOR, typ, &[&displayed]);
        return;
    }
    let (positional_args, mut named_args) = constructor_required(inf, k);
    let range = dartforge_diagnostics::Span { start: typ.start, end: ctor.name.map_or(typ.end, |n| n.span.end) };
    let displayed = inf.table.format(st, inf.interner, inf.program);
    let super_parameters_of = inf.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(2, 17);
    if !super_parameters_of {
        if positional_args != 0 || !named_args.is_empty() {
            inf.warning_with_code(ce::NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT, range, &[&displayed]);
        }
        return;
    }
    let super_positional_args = ctor.parameters.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named).count();
    let super_named_args: Vec<dartforge_intern::SymbolId> = ctor.parameters.iter().filter(|p| p.super_ && p.kind == ast::ParameterKind::Named).filter_map(|p| p.nome_externo().map(|n| n.sym)).collect();
    named_args.retain(|n| !super_named_args.contains(n));
    if positional_args > super_positional_args || !named_args.is_empty() {
        inf.warning_with_code(ce::IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS, range, &[&displayed]);
    }
}

/// `super(...)`/`this(...)`: invoca o construtor da classe instanciada.
/// Os parâmetros `super.x` do construtor corrente são argumentos
/// implícitos (posicionais depois dos explícitos; nomeados pelo nome).
fn call_constructor_of(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: Option<(ClassId, Box<[TypeId]>)>, the_name: Option<ast::Name>, args: &ast::Arguments, params: &[ast::Parameter], span: dartforge_diagnostics::Span) {
    let u = inf.core.unknown;
    let key = the_name.map(|n| n.sym).or(inf.sym.vazio);
    let f = tgt.as_ref().and_then(|(c, _)| key.and_then(|k| inf.program.class(*c).constructors.get(&k).copied()));
    match (tgt, f) {
        (Some((c, targs)), Some(f)) => {
            let sig = inf.outline.functions[f.0 as usize].signature;
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            let sig = if ps.len() == targs.len() {
                let map = inf.mapa(&ps, &targs);
                inf.subst(sig, &map)
            } else {
                sig
            };
            // Aridade: os `super.x` contam como argumentos implícitos
            // (`verifySuperFormalParameters`); o nome citado é o do
            // construtor, ou `<tipo de retorno>.new`; a entidade do nomeado
            // obrigatório é a invocação inteira.
            let cited_name = match the_name {
                Some(n) => inf.interner.resolve(n.sym).to_string(),
                None => {
                    let ret = match inf.table.get(sig) {
                        Type::Function { ret, .. } => *ret,
                        _ => sig,
                    };
                    format!("{}.new", inf.table.format(ret, inf.interner, inf.program))
                }
            };
            let super_positional_args = params.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named).count();
            // `POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT`
            // (`verifySuperFormalParameters`,
            // an611:src/error/super_formal_parameters_verifier.dart:22-33): no
            // nome de cada `super.x` posicional, se o `super(...)` tem
            // argumento posicional.
            if args.args.iter().any(|a| a.name.is_none()) {
                for p in params.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named) {
                    if let Some(n) = p.name {
                        inf.warning_with_code(
                            dartforge_diagnostics::codigos::compile_time_error::POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT,
                            n.span,
                            &[],
                        );
                    }
                }
            }
            let super_named_args: Vec<_> =
                params.iter().filter(|p| p.super_ && p.kind == ast::ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
            inf.arity_target = Some(super::calls::ArityTarget {
                nome: Some(cited_name),
                entidade: Some(span),
                super_positional_args,
                super_named_args,
            });
            super::calls::invoke(inf, cx, sig, args, u, None);
            inf.arity_target = None;
        }
        _ => {
            for a in args.args.iter() {
                infer_free(inf, cx, a.value);
            }
        }
    }
}

/// Infere um corpo (bloco ou expressão) com o retorno já no contexto.
/// `fim`: onde termina o trecho morto do corpo (o padrão é a última
/// instrução do bloco).
fn function_body(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    body: &FunctionBody,
    m: AsyncModifier,
    ret: TypeId,
    end: Option<usize>,
) {
    let _ = (m, ret);
    match body {
        FunctionBody::Expression(e) => {
            let ctx = cx.funcoes.last().map(|f| f.return_context).unwrap_or(inf.core.unknown);
            let t = inferir(inf, cx, *e, ctx);
            if let Some(fc) = cx.funcoes.last().cloned() {
                verify_expression_return(inf, cx, &fc, *e, t);
            }
        }
        FunctionBody::Block(s) => {
            let end = end.unwrap_or_else(|| statements::flow_end(inf, cx, *s));
            statements::enter_flow(cx, end);
            statements::infer_statement(inf, cx, *s);
            statements::exit_flow(cx);
        }
        _ => {}
    }
}

/// `ReturnTypeVerifier.verifyReturnType` (`an611:src/error/return_type_verifier.dart:79-130`,
/// `_isLegalReturnType` `:297-322`): com o retorno escrito, `async` exige
/// `Future<Never>` subtipo dele, `async*` `Stream<Never>`, `sync*`
/// `Iterable<Never>`, e um gerador não pode ser `void`; o relato vai no tipo
/// escrito. Devolve `hasLegalReturnType`.
fn legal_return_type(inf: &mut BodyInferrer<'_>, unit: UnitId, written: Option<ast::TypeId>, m: AsyncModifier, ret: TypeId) -> bool {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let Some(written) = written else { return true };
    let (cls, code) = match m {
        AsyncModifier::None => return true,
        AsyncModifier::Async => (inf.core.future_class, ce::ILLEGAL_ASYNC_RETURN_TYPE),
        AsyncModifier::AsyncStar => (inf.core.stream_class, ce::ILLEGAL_ASYNC_GENERATOR_RETURN_TYPE),
        AsyncModifier::SyncStar => (inf.core.iterable_class, ce::ILLEGAL_SYNC_GENERATOR_RETURN_TYPE),
    };
    let generator = matches!(m, AsyncModifier::AsyncStar | AsyncModifier::SyncStar);
    let illegal = if generator && matches!(inf.table.get(ret), Type::Void) {
        true
    } else {
        let n = inf.core.never;
        let piso = inf.iface(cls, vec![n]);
        !inf.sub(piso, ret)
    };
    if illegal {
        let sp = inf.program.unit(unit).ast.ty(written).span;
        inf.warning_with_code(code, sp, &[]);
    }
    !illegal
}

/// `ResolverVisitor.checkForBodyMayCompleteNormally`
/// (`an611:src/generated/resolver.dart:522-603`), com o fim do corpo de bloco
/// alcançável: sem contexto de retorno, nada (o `catchError` é outra regra);
/// gerador, nada; `async` com o imposto fora de `Future<Never>`, nada; o
/// contexto potencialmente não anulável dá `BODY_MIGHT_COMPLETE_NORMALLY`; o
/// anulável cuja base (`futureOrBase`) não é `dynamic`, `_`, `void` nem
/// `Null`, `BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE`.
fn body_completes_normally(inf: &mut BodyInferrer<'_>, imposed: Option<TypeId>, context: Option<TypeId>, m: AsyncModifier, where_: dartforge_diagnostics::Span) {
    use dartforge_diagnostics::codigos::{compile_time_error as ce, warning as w};
    let Some(rt) = context else { return };
    if matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) || inf.the_is_unknown(rt) {
        return;
    }
    if m == AsyncModifier::Async
        && let Some(i) = imposed
    {
        let n = inf.core.never;
        let piso = inf.futuro(n);
        if !inf.sub(piso, i) {
            return;
        }
    }
    // `isPotentiallyNonNullable` é `!isNullable` (type_system.dart:1242-1284):
    // `dynamic`, inválido, o desconhecido `_`, `void` e `Null` são anuláveis,
    // e `FutureOr<T>` é anulável se `T` for (`FutureOr<_>`, o contexto de uma
    // closure passada a `Future.delayed`, é anulável).
    fn anulavel(inf: &mut BodyInferrer<'_>, t: TypeId, bottom: u32) -> bool {
        if bottom > 32 {
            return false;
        }
        if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.the_is_unknown(t) || matches!(inf.table.get(t), Type::Void | Type::Null) {
            return true;
        }
        match inf.table.get(t).clone() {
            Type::FutureOr { arg, nullable } => nullable || anulavel(inf, arg, bottom + 1),
            Type::Intersection { bound, .. } => anulavel(inf, bound, bottom + 1),
            _ => inf.the_is_nullable(t),
        }
    }
    if !anulavel(inf, rt, 0) {
        inf.warning_with_args(ce::BODY_MIGHT_COMPLETE_NORMALLY, where_, &[crate::exibicao::Arg::Tipo(rt)]);
        return;
    }
    let mut base = rt;
    while let Type::FutureOr { arg, .. } = inf.table.get(base) {
        base = *arg;
    }
    if inf.e_dynamic(base) || inf.table.e_invalido(base) || inf.the_is_unknown(base) || matches!(inf.table.get(base), Type::Void | Type::Null) {
        return;
    }
    inf.warning_with_args(w::BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE, where_, &[crate::exibicao::Arg::Tipo(rt)]);
}

/// `_checkForFutureCatchErrorOnError` (`an611:src/generated/resolver.dart:3971-4006`):
/// a expressão de função sem contexto de retorno, argumento posicional direto
/// de `x.catchError(...)` com `x` um `Future<T>`: o `{` relata
/// `BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR` com a base de `FutureOr<T>`.
fn catch_error_without_return(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, where_: dartforge_diagnostics::Span) {
    let dartforge_frontend::pais::Pai::Expr(call) = inf.parent_of(cx.unit, e) else { return };
    let a = &inf.program.unit(cx.unit).ast;
    let ast::ExprKind::Call { target, arguments } = &a.expr(call).kind else { return };
    if !arguments.args.iter().any(|x| x.value == e && x.name.is_none()) {
        return;
    }
    let ast::ExprKind::Property { target: tgt, name, .. } = &a.expr(*target).kind else { return };
    if inf.interner.resolve(name.sym) != "catchError" {
        return;
    }
    let Some(recv) = inf.body_types.units[cx.unit.0 as usize].static_types.get(tgt.0 as usize).copied() else { return };
    if !matches!(inf.table.get(recv), Type::Interface { .. }) {
        return;
    }
    let Some(args) = inf.as_instance_of(recv, inf.core.future_class) else { return };
    let Some(&t) = args.first() else { return };
    let mut base = t;
    while let Type::FutureOr { arg, .. } = inf.table.get(base) {
        base = *arg;
    }
    if inf.e_dynamic(base) || inf.the_is_unknown(base) || matches!(inf.table.get(base), Type::Void | Type::Null) {
        return;
    }
    inf.warning_with_args(dartforge_diagnostics::codigos::warning::BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR, where_, &[crate::exibicao::Arg::Tipo(base)]);
}

/// Tipo de uma variável de topo/campo sem tipo: sobreposição (campo que
/// sobrepõe um getter) ou inicializador (`Null` vira `dynamic`).
pub(crate) fn infer_untyped_variable_type(inf: &mut BodyInferrer<'_>, vid: VariableId) -> TypeId {
    let v = inf.program.variable(vid);
    // Sobreposição de membro herdado (campos de instância).
    if !v.static_ {
        if let Some(c) = v.class {
            if let Some(t) = overlay_type(inf, c, vid) {
                if let Some((unit, init)) = inf.inicializador(vid) {
                    let mut cx = Body::to_variable(inf, vid, unit);
                    inferir(inf, &mut cx, init, t);
                }
                return t;
            }
        }
    }
    match inf.inicializador(vid) {
        Some((unit, init)) => {
            let mut cx = Body::to_variable(inf, vid, unit);
            let u = inf.core.unknown;
            let t = inferir(inf, &mut cx, init, u);
            if matches!(inf.table.get(t), Type::Null) {
                inf.core.dynamic_
            } else {
                t
            }
        }
        None => match v.node {
            // O tipo da constante vem do outline (a instanciação escrita ou a
            // dos limites); o tipo `this` da classe (`E<T>`) vazaria o `T`.
            VariableRef::EnumConstant { unit, decl, index } => {
                if let Some(t) = enum_constant_type(inf, vid, unit, decl, index) {
                    return t;
                }
                let d = &inf.outline.variables[vid.0 as usize];
                match d.declared_type.or(d.inferred) {
                    Some(t) => t,
                    None => v.class.map(|c| inf.class_this_type(c)).unwrap_or(inf.core.dynamic_),
                }
            }
            _ => inf.core.dynamic_,
        },
    }
}

/// O enum `c` tem o construtor sem nome sintético: nenhum declarado é
/// gerador nem sem nome (`buildEnumSyntheticConstructors`).
fn enum_has_synthetic(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let k = inf.program.class(c);
    !k.constructors.iter().any(|(the_name, &f)| !inf.program.function(f).factory || inf.interner.resolve(*the_name).is_empty())
}

/// O tipo implícito de uma constante de enum genérica sem argumentos de tipo
/// escritos: o da criação `E.nome(args)` inferida sem contexto (a
/// inicialização sintética do `ElementBuilder`, tipada como um campo sem
/// tipo). A inferência é especulativa: os avisos dela saem na visita da
/// declaração do enum.
fn enum_constant_type(inf: &mut BodyInferrer<'_>, vid: VariableId, unit: UnitId, decl: ast::DeclId, index: usize) -> Option<TypeId> {
    let c = inf.program.variable(vid).class?;
    let program = inf.program;
    let ast::DeclKind::Enum(en) = &program.unit(unit).ast.decl(decl).kind else { return None };
    let k = en.constants.get(index)?;
    let args = k.arguments.as_ref()?;
    let key = match k.constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => inf.sym.vazio,
        Some(x) => Some(x),
        None => inf.sym.vazio,
    };
    let f = key.and_then(|ch| program.class(c).constructors.get(&ch).copied())?;
    let before = inf.diagnostics.len();
    let saved_target = inf.arity_target.take();
    let mut cx = Body::novo(inf, unit, Some(c), None, true);
    let u = inf.core.unknown;
    let t = super::calls::construir(inf, &mut cx, None, c, Some(f), None, args, u);
    inf.arity_target = saved_target;
    inf.diagnostics.truncate(before);
    inf.warning_units.truncate(before);
    match inf.table.get(t) {
        Type::Interface { class, .. } if *class == c => Some(t),
        _ => None,
    }
}

/// O tipo de um campo de instância sem tipo pela inferência de sobrescrita
/// (`_inferAccessorOrField` com `field`): os getters e os setters que ele
/// sobrescreve nas superinterfaces diretas (`getOverridden2`), pela
/// assinatura combinada. Só getters: o retorno do getter combinado; só
/// setters: o parâmetro do setter combinado; os dois: o do getter num campo
/// `final`, e num não `final` o tipo só quando os dois coincidem. Sem nada
/// (ou sem coincidência), o inicializador decide.
fn overlay_type(inf: &mut BodyInferrer<'_>, c: ClassId, vid: VariableId) -> Option<TypeId> {
    let before = inf.declared_untyped.replace(c);
    let r = overlay_type_at(inf, c, vid);
    inf.declared_untyped = before;
    if inf.provisional_inheritance && before.is_none() {
        inf.heranca = crate::heranca::Heranca::default();
        inf.provisional_inheritance = false;
    }
    r
}

fn overlay_type_at(inf: &mut BodyInferrer<'_>, c: ClassId, vid: VariableId) -> Option<TypeId> {
    let v = inf.program.variable(vid);
    let final_ = v.final_ || v.const_;
    let (getters, setters, n_getter, n_setter) = inf.overridden_getters_and_setters(c, v.library, v.name);
    match (getters.is_empty(), setters.is_empty(), n_setter) {
        (false, true, _) => Some(inf.combined_getter_type(c, &getters, n_getter)),
        (true, false, Some(ns)) => Some(inf.combined_setter_type(c, &setters, ns)),
        (false, false, Some(ns)) => {
            let g = inf.combined_getter_type(c, &getters, n_getter);
            if final_ {
                return Some(g);
            }
            let s = inf.combined_setter_type(c, &setters, ns);
            (inf.table.canonico(g) == inf.table.canonico(s)).then_some(g)
        }
        _ => None,
    }
}

/// Extrai de um contexto o tipo de função (através de `?` e `FutureOr`).
fn context_function(inf: &mut BodyInferrer<'_>, ctx: TypeId) -> Option<TypeId> {
    if inf.the_is_unknown(ctx) {
        return None;
    }
    let t = inf.non_null(ctx);
    match inf.table.get(t).clone() {
        Type::Function { .. } => Some(t),
        Type::FutureOr { arg, .. } => context_function(inf, arg),
        _ => None,
    }
}

/// Expressão de função (closure) no contexto `ctx`.
pub(crate) fn expressao_de_funcao(inf: &mut BodyInferrer<'_>, cx: &mut Body, fid: ast::FunctionId, ctx: TypeId, e: ExprId) -> TypeId {
    // `wasFunctionTypeSupplied` (`function_expression_resolver.dart:35`): o
    // contexto, como chegou, é um tipo de função.
    if matches!(inf.table.get(ctx), Type::Function { .. }) {
        inf.body_types.units[cx.unit.0 as usize].with_function_type.insert(fid);
    }
    let (t, _) = function_literal(inf, cx, fid, ctx, None, Some(e));
    t
}

/// Infere uma função literal ou local. `local` é o id da função local
/// (declarada antes, para chamadas recursivas).
fn function_literal(inf: &mut BodyInferrer<'_>, cx: &mut Body, fid: ast::FunctionId, ctx: TypeId, local: Option<crate::resolved::LocalId>, expression: Option<ExprId>) -> (TypeId, ()) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let u = inf.core.unknown;
    let ctx_fn = context_function(inf, ctx);
    // Captura de escrita: locais externos escritos dentro da closure perdem
    // as promoções (aqui e depois dela).
    let written_ones = statements::names_written_in_function(inf, cx.unit, af);
    let mut captured: Vec<crate::resolved::LocalId> = Vec::new();
    for n in &written_ones {
        if let Some(id) = statements::write_local(cx, *n) {
            captured.push(id);
        }
    }
    let flow_before = cx.flow.clone();
    let mut flow_inside = cx.flow.clone();
    for &id in &captured {
        flow_inside.capture(id);
    }
    for e in cx.written_in_closure.clone().iter() {
        if let Some(id) = statements::write_local(cx, *e) {
            flow_inside.capture(id);
        }
    }
    if cx.written_in_body.is_none() {
        let a = &inf.program.unit(cx.unit).ast;
        cx.written_in_body = Some(match cx.raiz {
            // O membro inteiro, inclusive escritas nos próprios parâmetros
            // (R-FLU-07 regra 2; `base ??= const {}` e depois a closure).
            super::body::Raiz::Funcao(f) => statements::separately_written_names(inf, cx.unit, &a.function(f).body),
            super::body::Raiz::Construtor(m) => match &a.member(m).kind {
                ast::MemberKind::Constructor(c) => statements::separately_written_names(inf, cx.unit, &c.body),
                _ => Default::default(),
            },
            super::body::Raiz::Nada => Default::default(),
        });
    }
    // `functionExpression_begin`: `conservativeJoin(anywhere.written,
    // anywhere.captured)` — escrita em qualquer lugar do membro perde as
    // promoções dentro do literal mas continua promovível; escrita dentro de
    // algum literal do membro (antes ou depois deste) é capturada e não
    // promove (R-FLU-07 regra 2).
    let (outside, inside) = cx.written_in_body.clone().unwrap_or_default();
    for e in outside.iter() {
        if let Some(id) = statements::write_local(cx, *e) {
            flow_inside.conservative_join(&[id], &[]);
        }
    }
    for e in inside.iter() {
        if let Some(id) = statements::write_local(cx, *e) {
            flow_inside.conservative_join(&[], &[id]);
        }
    }
    // Só a escrita de variável de fora da função é captura: a dos próprios
    // parâmetros e locais (declaração achada na varredura dela) não.
    cx.written_in_closure.extend(written_ones.iter().copied().filter(|e| e.1.is_none()));
    cx.flow = flow_inside;
    cx.push_scope();
    // Parâmetros de tipo da função literal.
    let mut tps: Vec<TypeParamId> = Vec::new();
    for tp in af.type_params.iter() {
        let p = inf.table.alloc_type_param(tp.name.sym, crate::table::TypeParamOwner::GenericFunctionType, inf.core.object_nullable, crate::table::Variance::Unspecified);
        inf.table.without_default_type.insert(p);
        cx.declare_type_param(tp.name.sym, p);
        tps.push(p);
    }
    for (tp, &p) in af.type_params.iter().zip(tps.iter()) {
        if let Some(b) = tp.bound {
            let bt = inf.type_from_annotation(cx, b);
            inf.table.set_type_param_bound(p, bt);
        }
    }
    // Contexto de função com os parâmetros de tipo renomeados para os nossos.
    let (cpos, copt, cnamed, cret) = match ctx_fn.map(|t| inf.table.get(t).clone()) {
        Some(Type::Function { type_params, ret, positional, optional, named, .. }) => {
            let map: HashMap<TypeParamId, TypeId> = if type_params.len() == tps.len() {
                type_params.iter().copied().zip(tps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false }))).collect()
            } else {
                HashMap::new()
            };
            let s = |inf: &mut BodyInferrer<'_>, t: TypeId| inf.subst(t, &map);
            let pos: Vec<TypeId> = positional.iter().map(|&t| s(inf, t)).collect();
            let opt: Vec<TypeId> = optional.iter().map(|&t| s(inf, t)).collect();
            let named: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|(n, t, r)| (*n, s(inf, *t), *r)).collect();
            let r = s(inf, ret);
            (pos, opt, named, Some(r))
        }
        _ => (Vec::new(), Vec::new(), Vec::new(), None),
    };
    let all_ctx: Vec<TypeId> = cpos.iter().chain(copt.iter()).copied().collect();
    let mut pos = Vec::new();
    let mut opt = Vec::new();
    let mut named = Vec::new();
    let mut ipos = 0usize;
    // As anotações dos parâmetros de uma função local ou literal, no escopo
    // de fora (a validação do `AnnotationResolver` com as locais visíveis).
    if let Some(ps) = &af.parameters {
        fn anotacoes<'b>(ps: &'b [ast::Parameter], output: &mut Vec<&'b ast::Annotation>) {
            for q in ps.iter() {
                output.extend(q.metadata.iter());
                if let Some(fs) = &q.function_parameters {
                    anotacoes(fs, output);
                }
            }
        }
        let mut all = Vec::new();
        anotacoes(ps, &mut all);
        for m in all {
            super::statements::validate_local_annotation(inf, cx, m);
        }
    }
    if let Some(ps) = &af.parameters {
        for p in ps.iter() {
            // `ResolutionVisitor.visitFieldFormalParameter`/
            // `visitSuperFormalParameter` sem `ElementWalker` (função local,
            // closure; 3.6.2 `resolution_visitor.dart:614-635`, `:1263`): o
            // `this.x`/`super.x` obrigatório só é encerrado, não entra nos
            // parâmetros (nem no tipo nem no escopo); o opcional entra pelo
            // `DefaultFormalParameter` (`:440-467`).
            if p.kind == ast::ParameterKind::Required && (p.this_ || p.super_) {
                continue;
            }
            let scope = cx.visible_type_parameters();
            // Dentro de um membro estático (`_inStaticMethod` sobe pelos
            // elementos que envolvem a closure), o parâmetro de tipo da classe
            // no tipo do parâmetro é `TYPE_PARAMETER_REFERENCED_BY_STATIC`.
            let static_before = std::mem::replace(&mut inf.in_static_member, cx.static_member);
            let written = inf.written_parameter_type(cx.unit, cx.lib, p, &scope);
            inf.in_static_member = static_before;
            let do_ctx = match p.kind {
                ast::ParameterKind::Named => p.name.and_then(|n| cnamed.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t)),
                _ => {
                    let t = all_ctx.get(ipos).copied();
                    ipos += 1;
                    t
                }
            };
            let t = match written {
                Some(t) => t,
                None => match do_ctx {
                    Some(k) => {
                        let s = inf.greatest_closure_of(k);
                        let n = inf.core.null;
                        if inf.sub(s, n) {
                            inf.core.object_nullable
                        } else {
                            s
                        }
                    }
                    None => inf.core.dynamic_,
                },
            };
            if let Some(d) = p.default_value {
                let td = inferir(inf, cx, d, t);
                assignable_default_value(inf, cx, d, td, t);
            } else if p.kind != ast::ParameterKind::Required
                && !p.required
                && let Some(the_name) = p.name
                && !inf.the_is_nullable(t)
            {
                // `_checkUseOfDefaultValuesInParameters` (3.6.2,
                // `error_verifier.dart:6180-6250`) também nas expressões de
                // função e funções locais: o opcional sem padrão de tipo
                // potencialmente não anulável (o do elemento, inferido do
                // contexto se não escrito).
                use dartforge_diagnostics::codigos::compile_time_error as ce;
                let annotated = p.metadata.iter().any(|a| a.name.last().is_some_and(|n| inf.interner.resolve(n.sym) == "required"));
                if annotated {
                    inf.warning_with_code(ce::MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION, the_name.span, &[]);
                } else {
                    let code = if p.kind == ast::ParameterKind::Optional { ce::MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL } else { ce::MISSING_DEFAULT_VALUE_FOR_PARAMETER };
                    let text = inf.interner.resolve(the_name.sym).to_string();
                    inf.warning_with_code(code, the_name.span, &[&text]);
                }
            }
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
            if let Some(n) = &p.name {
                declarar_local(
                    inf,
                    cx,
                    Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                    true,
                );
            }
        }
    }
    // Retorno: escrito, ou inferido com o esquema imposto pelo contexto.
    let written_ret = af.return_type.map(|r| inf.type_from_annotation(cx, r));
    let m = af.modifier;
    // O contexto do retorno inteiro (o tipo de execução de um gerador o usa).
    let full_context = cret;
    let (ctx_ret, declared) = match written_ret {
        Some(r) => (inf.declared_return_context(r, m), Some(r)),
        None => {
            let s = cret.unwrap_or(u);
            let k = match m {
                AsyncModifier::None => s,
                AsyncModifier::AsyncStar => inf.as_schema_instance_of(s, inf.core.stream_class).unwrap_or(u),
                AsyncModifier::SyncStar => inf.as_schema_instance_of(s, inf.core.iterable_class).unwrap_or(u),
                AsyncModifier::Async => {
                    let fv = inf.future_value_type_schema(s);
                    inf.future_or(fv)
                }
            };
            (k, None)
        }
    };
    // Função local com retorno escrito: tipo disponível para recursão.
    if let (Some(id), Some(r)) = (local, declared) {
        let ft = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        cx.locais[id.0 as usize].tipo = ft;
        cx.locais[id.0 as usize].funcao_local = false;
    }
    // Função local com nome: "function 'f'"; closure: outra regra (fora).
    let executable = match (local, af.name) {
        (Some(_), Some(n)) => Some(super::body::Executavel {
            especie: super::body::ExecutableKind::Funcao,
            nome: inf.interner.resolve(n.sym).to_string(),
        }),
        _ => None,
    };
    // `verifyReturnType` da função local nomeada (`visitFunctionDeclaration`).
    let legal_return = match (local, declared) {
        (Some(_), Some(r)) => legal_return_type(inf, cx.unit, af.return_type, m, r),
        _ => true,
    };
    // O `imposedType` do literal de função: o retorno do tipo de função do
    // contexto, salvo `dynamic` e `_` (a função local com nome usa o
    // declarado).
    let closure_imposed = if local.is_none() && written_ret.is_none() {
        full_context.filter(|&r| !inf.e_dynamic(r) && !inf.the_is_unknown(r))
    } else {
        None
    };
    cx.funcoes.push(FunctionCtx { closure_imposed, modificador: m, retorno: declared, return_context: ctx_ret, returned: Vec::new(), empty_return: false, valueless_returns: Vec::new(), returned_expressions: Vec::new(), executavel: executable, legal_return });
    let saved_jumps = std::mem::take(&mut cx.saltos);
    let saved_cascades = std::mem::take(&mut cx.cascades);
    let saved_targets = std::mem::take(&mut cx.cascade_targets);
    let saved_bases = std::mem::take(&mut cx.cascade_bases);
    let (body_t, completes) = match &af.body {
        FunctionBody::Expression(e) => {
            let t = inferir(inf, cx, *e, ctx_ret);
            if let Some(fc) = cx.funcoes.last().cloned() {
                verify_expression_return(inf, cx, &fc, *e, t);
            }
            if let Some(f) = cx.funcoes.last_mut() {
                f.returned_expressions.push((*e, t));
            }
            (Some(t), false)
        }
        FunctionBody::Block(s) => {
            let end = statements::flow_end(inf, cx, *s);
            statements::enter_flow(cx, end);
            statements::infer_statement(inf, cx, *s);
            statements::exit_flow(cx);
            let reachable = cx.flow.alcancavel;
            if reachable {
                // `checkForBodyMayCompleteNormally`: a função local no nome,
                // a expressão de função no `{`; o imposto é o retorno escrito
                // ou o do contexto.
                let imposed = declared.or(cret);
                let where_ = match (local, af.name) {
                    (Some(_), Some(n)) => n.span,
                    _ => {
                        let begin = inf.program.unit(cx.unit).ast.stmt(*s).span.start;
                        dartforge_diagnostics::Span { start: begin, end: begin + 1 }
                    }
                };
                body_completes_normally(inf, imposed, imposed.map(|_| ctx_ret), m, where_);
                if imposed.is_none()
                    && local.is_none()
                    && let Some(ex) = expression
                {
                    catch_error_without_return(inf, cx, ex, where_);
                }
            }
            (None, reachable)
        }
        _ => (None, false),
    };
    cx.saltos = saved_jumps;
    cx.cascades = saved_cascades;
    cx.cascade_targets = saved_targets;
    cx.cascade_bases = saved_bases;
    let fc = cx.funcoes.pop().unwrap();
    cx.pop_scope();
    cx.flow = flow_before;
    for &id in &captured {
        cx.flow.capture(id);
    }
    let mut generator_execution = None;
    let ret = match declared {
        Some(r) => r,
        None => {
            // Tipo efetivamente retornado.
            let mut t = match body_t {
                Some(t) => t,
                None => {
                    if completes && !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                        inf.core.null
                    } else {
                        inf.core.never
                    }
                }
            };
            let generator = matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar);
            if body_t.is_none() {
                for &r in &fc.returned {
                    t = inf.up(r, t);
                }
                // `return;` contribui `Null`, também em gerador: o analyzer
                // 3.6.2 (oráculo) junta os `return` aos `yield` do gerador
                // (a regra que ignora `return;` em gerador é posterior).
                if fc.empty_return {
                    let n = inf.core.null;
                    t = inf.up(n, t);
                }
                // Gerador sem `yield` nem `return`: elemento `dynamic`
                // (analyzer 3.6.2), não `Never`.
                if generator && fc.returned.is_empty() && !fc.empty_return {
                    t = inf.core.dynamic_;
                }
            } else if m == AsyncModifier::Async {
                t = inf.flatten(t);
            }
            let r = inf.greatest_closure_of(ctx_ret);
            let wrap = |inf: &mut BodyInferrer<'_>, t: TypeId| {
                let s = if matches!(inf.table.get(r), Type::Void)
                    || (m == AsyncModifier::Async && matches!(inf.table.get(r), Type::FutureOr { arg, .. } if matches!(inf.table.get(*arg), Type::Void)))
                {
                    inf.core.void_
                } else if inf.sub(t, r) {
                    t
                } else {
                    r
                };
                match m {
                    AsyncModifier::Async => {
                        let f = inf.flatten(s);
                        inf.futuro(f)
                    }
                    AsyncModifier::AsyncStar => inf.flow_of(s),
                    AsyncModifier::SyncStar => inf.iterable(s),
                    AsyncModifier::None => s,
                }
            };
            let static_ = wrap(inf, t);
            // O `ReturnTypeVerifier` confere todo `return` da closure contra o
            // tipo de retorno dela (o inferido, já ajustado ao contexto); o
            // `=> e` com retorno achatado `void` aceita qualquer expressão
            // (`verifyExpressionFunctionBody`, `return_type_verifier.dart:45-52`).
            let flattened = if m == AsyncModifier::Async { inf.flatten(static_) } else { static_ };
            let void_arrow = matches!(af.body, FunctionBody::Expression(_)) && matches!(inf.table.get(flattened), Type::Void);
            if fc.executavel.is_none() && !generator && !void_arrow {
                closure_returns(inf, cx, &fc, static_, static_);
            }
            // `_checkReturnWithoutValue` (`return_type_verifier.dart:278-295`)
            // com o tipo de retorno da closure, o inferido: `return;` onde ele
            // (o valor futuro, em `async`) não é `void`, `dynamic` nem `Null`.
            if fc.executavel.is_none() && !generator && !fc.valueless_returns.is_empty() {
                let tv = if m == AsyncModifier::Async { inf.future_value_type(static_) } else { static_ };
                if !matches!(inf.table.get(tv), Type::Void | Type::Dynamic | Type::Null) {
                    for &sp in &fc.valueless_returns {
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::RETURN_WITHOUT_VALUE, sp, &[]);
                    }
                }
            }
            // O tipo de execução do gerador (a regra do CFE, conferida contra
            // a VM 3.6.2 e a 3.13.4): o elemento é o limite superior dos
            // `yield` (o `return;` não conta; `Null` sem nenhum), embrulhado
            // (`Iterable`/`Stream`); o CFE compara esse tipo embrulhado com o
            // ELEMENTO do contexto e, se não for subtipo dele, o retorno é o
            // tipo de retorno do contexto, inteiro (`Iterable<num>? Function()`
            // dá `Iterable<num>?`).
            if generator && body_t.is_none() {
                let mut e = if fc.returned.is_empty() { inf.core.null } else { inf.core.never };
                for &y in &fc.returned {
                    e = inf.up(y, e);
                }
                let wrapped = match m {
                    AsyncModifier::AsyncStar => inf.flow_of(e),
                    _ => inf.iterable(e),
                };
                let execution = if inf.sub(wrapped, r) {
                    wrapped
                } else {
                    let context = full_context.unwrap_or(inf.core.unknown);
                    inf.greatest_closure_of(context)
                };
                if execution != static_ {
                    generator_execution = Some(execution);
                }
            }
            static_
        }
    };
    if let Some(r) = generator_execution {
        let fe = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        inf.body_types.units[cx.unit.0 as usize].function_runtime_types.insert(fid, fe);
    }
    let ft = inf.table.intern(Type::Function {
        type_params: tps.into_boxed_slice(),
        ret,
        positional: pos.into_boxed_slice(),
        optional: opt.into_boxed_slice(),
        named: named.into_boxed_slice(),
        nullable: false,
    });
    (ft, ())
}

impl<'a> BodyInferrer<'a> {
    /// `S` é `C<S1>` (sem subtipos): `S1`.
    pub(crate) fn as_schema_instance_of(&mut self, s: TypeId, c: Option<ClassId>) -> Option<TypeId> {
        let c = c?;
        match self.table.get(s) {
            Type::Interface { class, args, .. } if *class == c && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    }
}

/// Declaração de função local.
pub(crate) fn funcao_local(inf: &mut BodyInferrer<'_>, cx: &mut Body, fid: ast::FunctionId) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let Some(the_name) = af.name else { return };
    let d = inf.core.dynamic_;
    let id = cx.declarar(Local { nome: the_name.sym, tipo: d, final_: true, late: false, const_: false, offset: the_name.span.start, funcao_local: true });
    cx.funcoes_locais.insert(id);
    cx.flow.inicializar(id);
    let u = inf.core.unknown;
    let (t, _) = function_literal(inf, cx, fid, u, Some(id), None);
    cx.locais[id.0 as usize].tipo = t;
    cx.locais[id.0 as usize].funcao_local = false;
    inf.body_types.units[cx.unit.0 as usize].set_local_type(the_name.span.start, t);
    // A função local com retorno escrito (`parent.returnType?.type`).
    if let Some(rt) = inf.program.unit(cx.unit).ast.function(fid).return_type
        && let Some(&return_) = inf.body_types.units[cx.unit.0 as usize].tipos_de_anotacoes.get(&rt)
    {
        unnecessary_set(inf, cx.unit, fid, return_);
    }
}

/// `retorno` é `void`, `Future<void>` ou `FutureOr<void>`.
fn void_return(inf: &BodyInferrer<'_>, t: TypeId) -> bool {
    match inf.table.get(t) {
        Type::Void => true,
        Type::Interface { class, args, .. } if Some(*class) == inf.core.future_class => args.len() == 1 && matches!(inf.table.get(args[0]), Type::Void),
        Type::FutureOr { arg, .. } => matches!(inf.table.get(*arg), Type::Void),
        _ => false,
    }
}

/// `UNNECESSARY_SET_LITERAL` (`_checkForUnnecessarySetLiteral`,
/// `an611:src/error/best_practices_verifier.dart:1283-1318`): o corpo `=> {…}`
/// é um literal de conjunto e o retorno (o do parâmetro que recebe a closure,
/// ou o escrito da declaração) é `void`, `Future<void>` ou `FutureOr<void>`;
/// no literal.
pub(crate) fn unnecessary_set(inf: &mut BodyInferrer<'_>, unit: UnitId, fid: ast::FunctionId, return_: TypeId) {
    let a = &inf.program.unit(unit).ast;
    let ast::FunctionBody::Expression(e) = a.function(fid).body else { return };
    if !matches!(a.expr(e).kind, ast::ExprKind::SetOrMap { .. }) || !void_return(inf, return_) {
        return;
    }
    let Some(t) = inf.body_types.units[unit.0 as usize].get_type(e) else { return };
    if matches!(inf.table.get(t), Type::Interface { class, .. } if Some(*class) == inf.core.set_class) {
        let sp = inf.span_expr(unit, e);
        inf.warning_with_code(dartforge_diagnostics::codigos::warning::UNNECESSARY_SET_LITERAL, sp, &[]);
    }
}

/// Metadados (anotações) e argumentos de constantes de enum de uma unidade.
pub(crate) fn infer_unit_metadata(inf: &mut BodyInferrer<'_>, unit: UnitId) {
    let a = &inf.program.unit(unit).ast;
    // As anotações das diretivas (`@Timeout(Duration(seconds: 9)) library;`,
    // ou antes do primeiro `import`): no escopo da biblioteca, como as das
    // declarações (`ResolutionVisitor.visitDirective`).
    for d in inf.program.unit(unit).unit.directives.iter() {
        for m in d.metadata.iter() {
            anotacao(inf, unit, None, None, m);
        }
    }
    for (di, d) in a.decls.iter().enumerate() {
        let (cls, _) = classe_da_decl(inf, unit, d);
        // A anotação da declaração é resolvida no escopo de fora dela (o da
        // biblioteca): os membros do contêiner não estão nele.
        for m in d.metadata.iter() {
            anotacao(inf, unit, None, None, m);
        }
        // As anotações dos parâmetros de tipo: no escopo dos parâmetros de
        // tipo, cujo pai é o da biblioteca (os membros do contêiner não).
        let tps: &[ast::TypeParameter] = match &d.kind {
            ast::DeclKind::Class(x) => &x.type_params,
            ast::DeclKind::Mixin(x) => &x.type_params,
            ast::DeclKind::Enum(x) => &x.type_params,
            ast::DeclKind::Extension(x) => &x.type_params,
            ast::DeclKind::ExtensionType(x) => &x.type_params,
            ast::DeclKind::Typedef(x) => &x.type_params,
            ast::DeclKind::Function(_) | ast::DeclKind::Variables(_) => &[],
        };
        // Os elementos dos parâmetros de tipo da declaração, que estão no
        // escopo das anotações deles (`@A(T) U` resolve `T` para o parâmetro
        // de tipo; avaliado, é `CONST_TYPE_PARAMETER`).
        let ids: Vec<TypeParamId> = if tps.iter().any(|tp| !tp.metadata.is_empty()) {
            let decl = ast::DeclId(di as u32);
            let same = |r: &dartforge_elements::model::DeclRef| r.unit == unit && r.decl == decl;
            match &d.kind {
                ast::DeclKind::Extension(_) => inf
                    .program
                    .extensions
                    .iter()
                    .position(|x| same(&x.decl))
                    .and_then(|i| inf.outline.extensions.get(i))
                    .map(|x| x.type_params.to_vec())
                    .unwrap_or_default(),
                ast::DeclKind::Typedef(_) => inf
                    .program
                    .typedefs
                    .iter()
                    .position(|x| same(&x.decl))
                    .and_then(|i| inf.outline.typedefs.get(i))
                    .map(|x| x.type_params.to_vec())
                    .unwrap_or_default(),
                _ => cls.and_then(|c| inf.outline.classes.get(c.0 as usize)).map(|x| x.type_params.to_vec()).unwrap_or_default(),
            }
        } else {
            Vec::new()
        };
        let scope: Vec<(dartforge_intern::SymbolId, TypeParamId)> =
            if ids.len() == tps.len() { tps.iter().map(|tp| tp.name.sym).zip(ids.iter().copied()).collect() } else { Vec::new() };
        for tp in tps.iter() {
            for m in tp.metadata.iter() {
                validate_annotation(inf, unit, None, m);
                unvalidated_annotation_with(inf, unit, None, None, m, &scope);
            }
        }
        match &d.kind {
            ast::DeclKind::Enum(en) => {
                let Some(c) = cls else { continue };
                for k in en.constants.iter() {
                    for m in k.metadata.iter() {
                        anotacao(inf, unit, Some(c), None, m);
                    }
                    // `ResolverVisitor.visitEnumConstantDeclaration`
                    // (`an611:src/generated/resolver.dart:2483-2520`), antes
                    // dos argumentos: o construtor da criação sintética é
                    // factory, ou não existe (o sintético `const` só sem
                    // construtor declarado).
                    let key = match k.constructor.map(|n| n.sym) {
                        Some(x) if Some(x) == inf.sym.new_ => inf.sym.vazio,
                        Some(x) => Some(x),
                        None => inf.sym.vazio,
                    };
                    {
                        use dartforge_diagnostics::codigos::compile_time_error as ce;
                        let ce_enum = inf.program.class(c);
                        let declared = key.and_then(|ch| ce_enum.constructors.get(&ch).copied());
                        // `buildEnumSyntheticConstructors` (`library_builder.dart:238-260`):
                        // o sem nome sintético existe quando nenhum construtor
                        // é gerador nem sem nome (só fábricas nomeadas contam
                        // como ausência).
                        let synthetic = key == inf.sym.vazio && enum_has_synthetic(inf, c);
                        match declared {
                            Some(f) if inf.program.function(f).factory => {
                                let sp = k.constructor.map_or(k.name.span, |n| n.span);
                                inf.warning_with_code(ce::ENUM_CONSTANT_INVOKES_FACTORY_CONSTRUCTOR, sp, &[]);
                            }
                            Some(_) => {}
                            None if synthetic => {}
                            None => match k.constructor {
                                Some(n) => {
                                    let the_name = inf.interner.resolve(n.sym).to_string();
                                    inf.warning_with_code(ce::UNDEFINED_ENUM_CONSTRUCTOR_NAMED, n.span, &[&the_name]);
                                }
                                None => inf.warning_with_code(ce::UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED, k.name.span, &[]),
                            },
                        }
                        // `TypeArgumentsVerifier.checkEnumConstantDeclaration`
                        // (`type_arguments_verifier.dart:93-112`): com o
                        // construtor resolvido, a lista `<…>` escrita com
                        // contagem diferente da do enum.
                        let resolved = declared.is_some() || synthetic;
                        let the_expected = inf.program.class(c).type_params.len();
                        if resolved
                            && let (Some(&first), Some(&last)) = (k.type_args.first(), k.type_args.last())
                            && k.type_args.len() != the_expected
                        {
                            let source = &inf.program.unit(unit).source;
                            let arg_start = a.ty(first).span.start;
                            let arg_end = a.ty(last).span.end;
                            let start = source[..arg_start].rfind('<').unwrap_or(arg_start);
                            let end = source[arg_end..].find('>').map_or(arg_end, |i| arg_end + i + 1);
                            let (e, d) = (the_expected.to_string(), k.type_args.len().to_string());
                            inf.warning_with_code(ce::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ENUM, dartforge_diagnostics::Span { start: start, end: end }, &[&e, &d]);
                        }
                    }
                    if let Some(args) = &k.arguments {
                        let mut cx = Body::novo(inf, unit, Some(c), None, true);
                        let f = key.and_then(|ch| inf.program.class(c).constructors.get(&ch).copied());
                        if let Some(f) = f {
                            let explicit = if k.type_args.is_empty() {
                                None
                            } else {
                                Some(k.type_args.iter().map(|&t| inf.type_argument_type(&cx, t)).collect())
                            };
                            let u = inf.core.unknown;
                            let enum_name = inf.interner.resolve(en.name.sym).to_string();
                            super::calls::define_target(inf, Some(enum_name), k.name.span);
                            super::calls::construir(inf, &mut cx, None, c, Some(f), explicit, args, u);
                            inf.arity_target = None;
                            // `checkEnumConstantDeclaration` do `TypeArgumentsVerifier`:
                            // os argumentos de tipo do construtor (escritos ou
                            // inferidos), no argumento escrito ou no nome.
                            let params = inf.outline.classes[c.0 as usize].type_params.to_vec();
                            if let Some(types) = inf.body_types.units[unit.0 as usize].instanciacao(args.span.start).map(|x| x.to_vec())
                                && types.len() == params.len()
                            {
                                let nodes = k.type_args.to_vec();
                                super::calls::check_explicit_bounds(inf, unit, &params, &types, &nodes, k.name.span, None);
                            }
                        } else if key == inf.sym.vazio && enum_has_synthetic(inf, c) {
                            // Construtor sem nome implícito `const E()`: os
                            // argumentos passam pela aridade dele (sem
                            // parâmetros).
                            let ret = inf.class_this_type(c);
                            let sig = inf.table.intern(Type::Function {
                                type_params: Box::new([]),
                                ret,
                                positional: Box::new([]),
                                optional: Box::new([]),
                                named: Box::new([]),
                                nullable: false,
                            });
                            let enum_name = inf.interner.resolve(en.name.sym).to_string();
                            super::calls::define_target(inf, Some(enum_name), k.name.span);
                            let u = inf.core.unknown;
                            super::calls::invoke(inf, &mut cx, sig, args, u, None);
                            inf.arity_target = None;
                            // `checkEnumConstantDeclaration`: os argumentos
                            // escritos do construtor implícito.
                            let params = inf.outline.classes[c.0 as usize].type_params.to_vec();
                            if !k.type_args.is_empty() && k.type_args.len() == params.len() {
                                let nodes = k.type_args.to_vec();
                                let types: Vec<TypeId> = nodes.iter().map(|&t| inf.type_argument_type(&cx, t)).collect();
                                super::calls::check_explicit_bounds(inf, unit, &params, &types, &nodes, k.name.span, None);
                            }
                        } else {
                            for x in args.args.iter() {
                                infer_free(inf, &mut cx, x.value);
                            }
                        }
                    } else if let Some(f) = inf.sym.vazio.and_then(|v| inf.program.class(c).constructors.get(&v).copied()) {
                        let enum_name = inf.interner.resolve(en.name.sym).to_string();
                        super::calls::arity_without_arguments(inf, f, enum_name, k.name.span);
                    }
                }
            }
            _ => {}
        }
    }
    // As anotações dos parâmetros de um tipo de função escrito
    // (`void Function([@A() int p])`): o `AnnotationResolver` visita toda
    // anotação da árvore.
    for ty in a.types.iter() {
        if let ast::TypeKind::Function { parameters, .. } = &ty.kind {
            for p in parameters.iter() {
                for m in p.metadata.iter() {
                    anotacao(inf, unit, None, None, m);
                }
            }
        }
    }
    for mb in a.members.iter() {
        for m in mb.metadata.iter() {
            // Classe dona desconhecida aqui: resolve no escopo da biblioteca.
            anotacao(inf, unit, None, None, m);
        }
        if let ast::MemberKind::Constructor(ctor) = &mb.kind {
            for p in ctor.parameters.iter() {
                for m in p.metadata.iter() {
                    anotacao(inf, unit, None, None, m);
                }
            }
        }
    }
    // Funções de topo e métodos (as locais e os literais veem locais que a
    // anotação pode citar: ficam sem a validação de nomes).
    let the_declared: std::collections::HashMap<u32, (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>)> = inf
        .program
        .functions
        .iter()
        .filter_map(|fe| match fe.node {
            dartforge_elements::model::FunctionRef::Function { unit: u, function } if u == unit => Some((function.0, (fe.class, fe.extension))),
            _ => None,
        })
        .collect();
    for (i, f) in a.functions.iter().enumerate() {
        let validate = the_declared.contains_key(&(i as u32));
        // Os parâmetros de tipo de um método veem o escopo do contêiner —
        // o da classe, ou o da extensão, com os membros estáticos dela
        // (`@A(foo) T` no método de uma extensão com `static foo()`).
        let (owner, owner_extension) = the_declared.get(&(i as u32)).copied().unwrap_or((None, None));
        // Os parâmetros de tipo da própria função também estão no escopo.
        let scope: Vec<(dartforge_intern::SymbolId, TypeParamId)> = if f.type_params.iter().any(|tp| !tp.metadata.is_empty()) {
            let fid = inf.program.functions.iter().position(|fe| {
                matches!(fe.node, dartforge_elements::model::FunctionRef::Function { unit: u, function } if u == unit && function.0 == i as u32)
            });
            match fid.and_then(|x| inf.outline.functions.get(x)) {
                Some(data) if data.type_params.len() == f.type_params.len() => {
                    f.type_params.iter().map(|tp| tp.name.sym).zip(data.type_params.iter().copied()).collect()
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        for tp in f.type_params.iter() {
            for m in tp.metadata.iter() {
                if validate {
                    validate_annotation_at(inf, unit, owner, owner_extension, m);
                    unvalidated_annotation_with(inf, unit, owner, owner_extension, m, &scope);
                } else {
                    unvalidated_annotation_with(inf, unit, None, None, m, &scope);
                }
            }
        }
        if let Some(ps) = &f.parameters {
            for p in ps.iter() {
                // Os parâmetros de um parâmetro de função à moda antiga
                // (`void f(void cb([@A() int p]))`) também são visitados.
                if validate {
                    fn nested(inf: &mut BodyInferrer<'_>, unit: UnitId, ps: &[ast::Parameter]) {
                        for q in ps.iter() {
                            for m in q.metadata.iter() {
                                anotacao(inf, unit, None, None, m);
                            }
                            if let Some(fs) = &q.function_parameters {
                                nested(inf, unit, fs);
                            }
                        }
                    }
                    if let Some(fs) = &p.function_parameters {
                        nested(inf, unit, fs);
                    }
                }
                for m in p.metadata.iter() {
                    if validate {
                        anotacao(inf, unit, None, None, m);
                    } else {
                        unvalidated_annotation(inf, unit, None, m);
                    }
                }
            }
        }
    }
}

fn classe_da_decl(inf: &BodyInferrer<'_>, unit: UnitId, d: &ast::Decl) -> (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>) {
    let lib = inf.program.unit(unit).library;
    let the_name = match &d.kind {
        ast::DeclKind::Class(c) => Some(c.name.sym),
        ast::DeclKind::Mixin(m) => Some(m.name.sym),
        ast::DeclKind::Enum(e) => Some(e.name.sym),
        ast::DeclKind::ExtensionType(e) => Some(e.name.sym),
        _ => None,
    };
    match the_name.and_then(|n| inf.program.library(lib).declared.get(&n)).and_then(|b| b.getter) {
        Some(Element::Class(c)) => (Some(c), None),
        _ => (None, None),
    }
}

/// O que um nome da anotação é, para o `AnnotationResolver`.
enum InAnnotation {
    Nada,
    Prefixo,
    Classe(ClassId),
    Extensao(dartforge_elements::model::ExtensionId),
    Alias(dartforge_elements::model::TypedefId),
    /// Getter: o implícito de uma variável (`Some(const)`) ou um escrito (`None`).
    Getter(Option<bool>),
    Outro,
}

fn classify_in_annotation(inf: &BodyInferrer<'_>, b: Option<dartforge_elements::model::Binding>) -> InAnnotation {
    use dartforge_elements::model::FunctionKind as K;
    match b.and_then(|b| b.getter) {
        None => InAnnotation::Nada,
        Some(Element::Prefix(..)) => InAnnotation::Prefixo,
        Some(Element::Class(c)) => InAnnotation::Classe(c),
        Some(Element::Extension(x)) => InAnnotation::Extensao(x),
        Some(Element::Typedef(td)) => InAnnotation::Alias(td),
        Some(Element::Variable(v)) => InAnnotation::Getter(Some(inf.program.variable(v).const_)),
        Some(Element::Function(f)) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (K::ImplicitAccessor, Some(v)) => InAnnotation::Getter(Some(inf.program.variable(v).const_)),
                (K::Getter, _) => InAnnotation::Getter(None),
                _ => InAnnotation::Outro,
            }
        }
    }
}

/// O getter estático `nome` de uma classe ou extensão (`getGetter`):
/// `Some(Some(const))` para o implícito de um campo (ou constante de enum,
/// ou `values`), `Some(None)` para um getter escrito.
fn static_getter(
    inf: &BodyInferrer<'_>,
    cls: Option<ClassId>,
    ext: Option<dartforge_elements::model::ExtensionId>,
    the_name: dartforge_intern::SymbolId,
) -> Option<Option<bool>> {
    use dartforge_elements::model::FunctionKind as K;
    let (statics, fields) = match (cls, ext) {
        (Some(c), _) => {
            let k = inf.program.class(c);
            if k.enum_constants.iter().any(|&v| inf.program.variable(v).name == the_name) {
                return Some(Some(true));
            }
            if k.kind == dartforge_elements::model::ClassKind::Enum && inf.interner.resolve(the_name) == "values" {
                return Some(Some(true));
            }
            (&k.static_members, k.fields.clone())
        }
        (None, Some(x)) => {
            let e = inf.program.extension(x);
            (&e.static_members, e.fields.clone())
        }
        _ => return None,
    };
    if let Some(&f) = statics.get(&the_name) {
        let fe = inf.program.function(f);
        return match (fe.kind, fe.variable) {
            (K::ImplicitAccessor, Some(v)) => Some(Some(inf.program.variable(v).const_)),
            (K::Getter, _) => Some(None),
            _ => None,
        };
    }
    fields
        .iter()
        .find(|&&v| inf.program.variable(v).name == the_name && inf.program.variable(v).static_)
        .map(|&v| Some(inf.program.variable(v).const_))
}

/// O `AnnotationResolver` (`an611:src/dart/resolver/annotation_resolver.dart:
/// 30-420`): `UNDEFINED_ANNOTATION` no nome que não resolve,
/// `INVALID_ANNOTATION` na anotação que não é referência a constante nem
/// invocação de construtor constante. Os nomes são resolvidos no escopo da
/// biblioteca (o da classe, só quando ela é conhecida).
pub(crate) fn validate_annotation(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, m: &ast::Annotation) {
    validate_annotation_at(inf, unit, cls, None, m);
}

/// [`validar_anotacao`] no escopo de uma classe ou de uma extensão (os
/// membros estáticos dela).
pub(crate) fn validate_annotation_at(
    inf: &mut BodyInferrer<'_>,
    unit: UnitId,
    cls: Option<ClassId>,
    extension: Option<dartforge_elements::model::ExtensionId>,
    m: &ast::Annotation,
) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let span = m.span;
    let Some(&n1) = m.name.first() else { return };
    let args = m.arguments.is_some();
    let n2 = m.name.get(1).copied();
    // Constantes estáticas da classe (ou da extensão) em que a anotação está.
    if m.name.len() == 1 {
        if let Some(g) = static_getter(inf, cls, extension, n1.sym) {
            if g != Some(true) || args {
                inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
            }
            return;
        }
    }
    let b1 = inf.program.lookup_na_unidade(unit, n1.sym);
    let mut e1 = classify_in_annotation(inf, b1);
    if matches!(e1, InAnnotation::Nada) && inf.program.prefixos_na_unidade(unit).contains_key(&n1.sym) {
        e1 = InAnnotation::Prefixo;
    }
    // Para o prefixo: o segundo nome resolvido nele, e o terceiro é o membro.
    let (tgt, mem, target_name) = match e1 {
        InAnnotation::Prefixo => {
            let Some(n2) = n2 else {
                inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
                return;
            };
            let b2 = inf.program.lookup_prefixed_na_unidade(unit, n1.sym, n2.sym);
            match classify_in_annotation(inf, b2) {
                InAnnotation::Prefixo | InAnnotation::Outro => {
                    inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
                    return;
                }
                x => (x, m.name.get(2).copied(), n2),
            }
        }
        x => (x, n2, n1),
    };
    let invalid = match tgt {
        InAnnotation::Nada => {
            let t = inf.interner.resolve(target_name.sym).to_string();
            inf.warning_with_code(c::UNDEFINED_ANNOTATION, span, &[&t]);
            return;
        }
        InAnnotation::Classe(k) => {
            let kind = inf.program.class(k).kind;
            let is_class = matches!(kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication);
            // `C.new` é o construtor sem nome.
            let ctor = |inf: &BodyInferrer<'_>, n: Option<ast::Name>| {
                let n = n.filter(|n| inf.interner.resolve(n.sym) != "new");
                let key = n.map(|n| n.sym).or(inf.sym.vazio);
                key.and_then(|ch| inf.program.class(k).constructors.get(&ch).copied()).is_some()
            };
            if kind == dartforge_elements::model::ClassKind::ExtensionType {
                // O construtor (primário) de um extension type: o modelo não
                // o põe na tabela pelo nome; fica sem relato.
                false
            } else if is_class && args {
                let found = ctor(inf, mem);
                // O construtor existe e não é `const`
                // (`constant_verifier.dart:103-115`): na anotação inteira.
                if found {
                    let n = mem.filter(|n| inf.interner.resolve(n.sym) != "new");
                    let key = n.map(|n| n.sym).or(inf.sym.vazio);
                    let e_const = key
                        .and_then(|ch| inf.program.class(k).constructors.get(&ch).copied())
                        .is_some_and(|f| inf.program.function(inf.program.publico(f)).const_);
                    if !e_const {
                        inf.warning_with_code(c::NON_CONSTANT_ANNOTATION_CONSTRUCTOR, span, &[]);
                    }
                }
                !found
            } else {
                // Sem lista de argumentos, a anotação que resolve a um
                // construtor `const` é `NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS`
                // (`constant_verifier.dart:103-128`), na anotação inteira; o
                // construtor que não é `const` tem o código dele.
                let const_constructor = |inf: &BodyInferrer<'_>, n: Option<ast::Name>| -> bool {
                    let n = n.filter(|n| inf.interner.resolve(n.sym) != "new");
                    let key = n.map(|n| n.sym).or(inf.sym.vazio);
                    key
                        .and_then(|ch| inf.program.class(k).constructors.get(&ch).copied())
                        .is_some_and(|f| inf.program.function(inf.program.publico(f)).const_)
                };
                let of_member = match mem {
                    Some(n) => match static_getter(inf, Some(k), None, n.sym) {
                        Some(g) => Some(g != Some(true) || args),
                        None => None,
                    },
                    None => None,
                };
                match of_member {
                    Some(invalid) => invalid,
                    None => {
                        let found = ctor(inf, mem);
                        if found && !args && is_class && const_constructor(inf, mem) {
                            inf.warning_with_code(c::NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS, span, &[]);
                        }
                        !found
                    }
                }
            }
        }
        InAnnotation::Extensao(x) => match mem.and_then(|n| static_getter(inf, None, Some(x), n.sym)) {
            Some(g) => g != Some(true) || args,
            None => true,
        },
        InAnnotation::Getter(g) => g != Some(true) || args,
        // Alias (`_typeAliasConstructorInvocation`/`_typeAliasGetter`, 3.6.2
        // `annotation_resolver.dart:274-285`, `:307-318`, `:383-413`): com
        // argumentos e alvo classe, é criação (o construtor decide, sem
        // relato aqui); senão o getter do alvo pelo segundo nome: sem ele
        // (`@V`, ou alias de função), `INVALID_ANNOTATION`.
        InAnnotation::Alias(td) => {
            let tgt = inf.outline.typedefs.get(td.0 as usize).map(|d| d.target_type);
            let target_class = tgt.and_then(|t| match inf.table.get(t) {
                Type::Interface { class, .. } => Some(*class),
                _ => None,
            });
            if args && target_class.is_some() {
                false
            } else {
                match (target_class, mem) {
                    (Some(k), Some(n)) => match static_getter(inf, Some(k), None, n.sym) {
                        Some(g) => g != Some(true) || args,
                        None => true,
                    },
                    _ => true,
                }
            }
        }
        InAnnotation::Prefixo | InAnnotation::Outro => true,
    };
    if invalid {
        inf.warning_with_code(c::INVALID_ANNOTATION, span, &[]);
    }
}

/// Uma anotação: `@x`, `@C(args)`, `@C.nome(args)`, `@p.C(args)`.
fn anotacao(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, _ext: Option<dartforge_elements::model::ExtensionId>, m: &ast::Annotation) {
    validate_annotation(inf, unit, cls, m);
    unvalidated_annotation(inf, unit, cls, m);
}

/// Os argumentos da anotação (inferência), sem a validação dos nomes.
fn unvalidated_annotation(inf: &mut BodyInferrer<'_>, unit: UnitId, cls: Option<ClassId>, m: &ast::Annotation) {
    unvalidated_annotation_with(inf, unit, cls, None, m, &[]);
}

/// Como [`anotacao_sem_validar`], com parâmetros de tipo a mais no escopo
/// (os da declaração cujo parâmetro de tipo a anotação marca).
fn unvalidated_annotation_with(
    inf: &mut BodyInferrer<'_>,
    unit: UnitId,
    cls: Option<ClassId>,
    extension: Option<dartforge_elements::model::ExtensionId>,
    m: &ast::Annotation,
    types: &[(dartforge_intern::SymbolId, TypeParamId)],
) {
    let Some(args) = &m.arguments else { return };
    let mut cx = Body::novo(inf, unit, cls, extension, true);
    for &(the_name, p) in types {
        cx.declare_type_param(the_name, p);
    }
    let names: Vec<dartforge_intern::SymbolId> = m.name.iter().map(|n| n.sym).collect();
    // Classe e construtor.
    let (c, ctor) = match names.as_slice() {
        [c] => (inf.program.lookup_na_unidade(unit, *c).and_then(|b| b.getter), None),
        [a, b] => match inf.program.lookup_na_unidade(unit, *a).and_then(|x| x.getter) {
            Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
            _ => (inf.program.lookup_prefixed_na_unidade(unit, *a, *b).and_then(|x| x.getter), None),
        },
        [p, c, n] => (inf.program.lookup_prefixed_na_unidade(unit, *p, *c).and_then(|x| x.getter), Some(*n)),
        _ => (None, None),
    };
    let u = inf.core.unknown;
    // `@B<T>()` com `B` alias de classe: os argumentos contra os parâmetros
    // de tipo do alias (o `AnnotationInferrer` com o construtor do alias).
    let c_original = c;
    let (c, params_do_alias) = match c {
        Some(Element::Typedef(td)) => {
            let tgt = inf.outline.typedefs[td.0 as usize].target_type;
            match inf.table.get(tgt).clone() {
                Type::Interface { class, .. } => (Some(Element::Class(class)), Some(inf.outline.typedefs[td.0 as usize].type_params.to_vec())),
                _ => (c, None),
            }
        }
        _ => (c, None),
    };
    if let Some(Element::Class(c)) = c {
        let key = ctor.or(inf.sym.vazio);
        if let Some(f) = key.and_then(|k| inf.program.class(c).constructors.get(&k).copied()) {
            let explicit: Option<Vec<TypeId>> = if m.type_args.is_empty() {
                None
            } else {
                Some(m.type_args.iter().map(|&t| inf.type_argument_type(&cx, t)).collect())
            };
            // `_needsTypeArgumentBoundsCheck` do `AnnotationInferrer`.
            if let Some(ex) = &explicit {
                let params = params_do_alias.clone().unwrap_or_else(|| inf.outline.classes[c.0 as usize].type_params.to_vec());
                // `_reportWrongNumberOfTypeArguments` (`invocation_inferrer.dart:316-327`)
                // com o código do `AnnotationInferrer`: `{0}` é o tipo de
                // função cru do construtor, na lista `<…>`.
                if ex.len() != params.len()
                    && let Some(sp) = super::calls::type_list_range(inf, unit, &m.type_args)
                {
                    // O tipo do construtor como função genérica nos parâmetros
                    // de tipo da classe (ou do alias, com o alvo dele).
                    let mut raw = inf.constructor_signature(c, f);
                    if params_do_alias.is_some()
                        && let Some(Element::Typedef(td)) = c_original
                        && let Type::Interface { args: target_args, .. } = inf.table.get(inf.outline.typedefs[td.0 as usize].target_type).clone()
                    {
                        let formals = inf.outline.classes[c.0 as usize].type_params.to_vec();
                        let map = inf.mapa(&formals, &target_args);
                        raw = inf.subst(raw, &map);
                    }
                    if let Type::Function { ret, positional, optional, named, .. } = inf.table.get(raw).clone() {
                        raw = inf.table.intern(Type::Function { type_params: params.clone().into_boxed_slice(), ret, positional, optional, named, nullable: false });
                    }
                    let text = inf.table.format(raw, inf.interner, inf.program);
                    let (n, d) = (params.len().to_string(), ex.len().to_string());
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS, sp, &[&text, &n, &d]);
                }
                if ex.len() == params.len() {
                    let (ex, nodes) = (ex.clone(), m.type_args.to_vec());
                    super::calls::check_explicit_bounds(inf, unit, &params, &ex, &nodes, m.span, None);
                }
            }
            if params_do_alias.is_some() {
                for x in args.args.iter() {
                    infer_free(inf, &mut cx, x.value);
                }
                return;
            }
            // `_reportNotEnoughPositionalArguments` com `Annotation`: o
            // `identifier` do nome prefixado (`@A.nome` → `nome`, `@p.A` →
            // `A`), ou `<nome>.new`; o `MISSING_REQUIRED_ARGUMENT` vai no
            // identificador do construtor, ou no da classe.
            let prefix0 = m.name.first().is_some_and(|n| inf.program.prefixos_na_unidade(unit).contains_key(&n.sym));
            let cited_name = match m.name.as_slice() {
                [a] => format!("{}.new", inf.interner.resolve(a.sym)),
                [a, b] if !m.type_args.is_empty() && !prefix0 => {
                    let _ = b;
                    format!("{}.new", inf.interner.resolve(a.sym))
                }
                [_, b, ..] => inf.interner.resolve(b.sym).to_string(),
                [] => String::new(),
            };
            let entity = match m.name.as_slice() {
                [a] => a.span,
                [_, b] => b.span,
                [_, _, c] => c.span,
                _ => m.span,
            };
            super::calls::define_target(inf, Some(cited_name), entity);
            super::calls::construir(inf, &mut cx, None, c, Some(f), explicit, args, u);
            inf.arity_target = None;
            return;
        }
    }
    for x in args.args.iter() {
        infer_free(inf, &mut cx, x.value);
    }
}

/// `_checkReturnExpression` de uma closure cujo retorno é o do contexto `r`
/// (embrulhado: `exibido`): `RETURN_OF_INVALID_TYPE_FROM_CLOSURE`
/// (`an611:src/error/return_type_verifier.dart:150-170`), com o tipo
/// retornado e o da closure.
fn closure_returns(inf: &mut BodyInferrer<'_>, cx: &Body, fc: &FunctionCtx, r: TypeId, displayed: TypeId) {
    let e_void_dyn = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic);
    for &(e, s) in &fc.returned_expressions {
        let error = match fc.modificador {
            AsyncModifier::None => {
                if matches!(inf.table.get(s), Type::Void) {
                    !e_void_dyn(inf, r)
                } else {
                    !inf.atribuivel(s, r)
                }
            }
            AsyncModifier::Async => {
                let tv = inf.flatten(r);
                let fs = inf.flatten(s);
                if matches!(inf.table.get(tv), Type::Void) {
                    !matches!(inf.table.get(fs), Type::Void | Type::Dynamic | Type::Null)
                } else if matches!(inf.table.get(fs), Type::Void) {
                    !e_void_dyn(inf, tv)
                } else {
                    !inf.atribuivel(s, tv) && !inf.sub(fs, tv)
                }
            }
            _ => false,
        };
        if error {
            let sp = inf.span_expr(cx.unit, e);
            let of = inf.table.format(s, inf.interner, inf.program);
            let for_ = inf.table.format(displayed, inf.interner, inf.program);
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::RETURN_OF_INVALID_TYPE_FROM_CLOSURE, sp, &[&of, &for_]);
        }
    }
}

/// `=> e`: o `verifyExpressionFunctionBody` do analyzer — retorno (depois do
/// `flatten` em `async`) `void` aceita qualquer expressão; o resto é a regra
/// de `return e;` ([`verificar_retorno`]).
fn verify_expression_return(inf: &mut BodyInferrer<'_>, cx: &Body, fc: &FunctionCtx, e: ExprId, t: TypeId) {
    let Some(r) = fc.retorno else { return };
    let flattened = if fc.modificador == AsyncModifier::None { r } else { inf.flatten(r) };
    if matches!(inf.table.get(flattened), Type::Void) {
        return;
    }
    verify_return(inf, cx, fc, e, t);
}

/// `_checkReturnExpression` do `ReturnTypeVerifier`: `return e;` (ou `=> e`)
/// com `e` de tipo `S` num executável de retorno declarado `T`. Em geradores,
/// closures e construtores geradores nada sai daqui.
pub(crate) fn verify_return(inf: &mut BodyInferrer<'_>, cx: &Body, fc: &FunctionCtx, e: ExprId, s: TypeId) {
    // `_checkReturnExpression`: com o retorno declarado ilegal, nada.
    if !fc.legal_return {
        return;
    }
    let (Some(t), Some(exe)) = (fc.retorno, fc.executavel.as_ref()) else { return };
    let e_void_dyn = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic);
    let e_void_dyn_null = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic | Type::Null);
    let error = match fc.modificador {
        AsyncModifier::None => {
            if matches!(inf.table.get(t), Type::Void) && !e_void_dyn_null(inf, s) {
                true
            } else if matches!(inf.table.get(s), Type::Void) {
                !e_void_dyn(inf, t)
            } else if let Some(field) = single_record_field(inf, cx, t, s, e)
                && inf.atribuivel(field, s)
            {
                // `(int,) f() => (1);`: o parêntese queria ser um registro.
                let sp = inf.span_expr(cx.unit, e);
                inf.warning_with_code(
                    dartforge_diagnostics::codigos::compile_time_error::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA,
                    sp,
                    &[],
                );
                return;
            } else {
                !inf.atribuivel(s, t)
            }
        }
        AsyncModifier::Async => {
            let tv = inf.future_value_type(t);
            let fs = inf.flatten(s);
            if matches!(inf.table.get(tv), Type::Void) && !e_void_dyn_null(inf, fs) {
                true
            } else if matches!(inf.table.get(fs), Type::Void) {
                !e_void_dyn(inf, tv)
            } else {
                !inf.atribuivel(s, tv) && !inf.sub(fs, tv)
            }
        }
        AsyncModifier::SyncStar | AsyncModifier::AsyncStar => false,
    };
    if !error {
        return;
    }
    use super::body::ExecutableKind as E;
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let code = match exe.especie {
        E::Funcao => c::RETURN_OF_INVALID_TYPE_FROM_FUNCTION,
        E::Metodo => c::RETURN_OF_INVALID_TYPE_FROM_METHOD,
        E::Construtor => c::RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR,
    };
    let sp = inf.span_expr(cx.unit, e);
    let of = inf.table.format(s, inf.interner, inf.program);
    let for_ = inf.table.format(t, inf.interner, inf.program);
    let the_name = exe.nome.clone();
    inf.warning_with_code(code, sp, &[&of, &for_, &the_name]);
}

/// `T` é um registro de um só campo posicional, `S` não é registro e `e` é
/// uma expressão entre parênteses: o tipo do campo.
fn single_record_field(inf: &BodyInferrer<'_>, cx: &Body, t: TypeId, s: TypeId, e: ExprId) -> Option<TypeId> {
    let Type::Record { positional, named, .. } = inf.table.get(t) else { return None };
    if positional.len() != 1 || !named.is_empty() || matches!(inf.table.get(s), Type::Record { .. }) {
        return None;
    }
    let field = positional[0];
    matches!(inf.program.unit(cx.unit).ast.expr(e).kind, ast::ExprKind::Parenthesized(_)).then_some(field)
}

impl<'a> BodyInferrer<'a> {
    /// **futureValueType**(`T`) de um retorno declarado: `Future<S>`,
    /// `FutureOr<S>` (anuláveis ou não) dão `S`; `void` e `dynamic` ficam;
    /// o resto é `Object?`.
    pub(crate) fn future_value_type(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Void | Type::Dynamic => t,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.core.future_class && args.len() == 1 => args[0],
            _ => self.core.object_nullable,
        }
    }

    /// Espécie e nome de exibição de um executável declarado.
    pub(crate) fn declared_executable(&self, f: FunctionElementId) -> Option<super::body::Executavel> {
        use super::body::{ExecutableKind as E, Executavel};
        let fe = self.program.function(f);
        let the_name = self.interner.resolve(fe.name);
        if matches!(fe.node, FunctionRef::Constructor { .. }) {
            let cls = self.interner.resolve(self.program.class(fe.class?).name).to_string();
            let the_name = if the_name.is_empty() || the_name == "new" { cls } else { format!("{cls}.{the_name}") };
            return Some(Executavel { especie: E::Construtor, nome: the_name });
        }
        let the_kind = match fe.kind {
            FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => E::Funcao,
            _ if fe.class.is_some() || fe.extension.is_some() => E::Metodo,
            _ => E::Funcao,
        };
        // O setter se exibe sem o `=` da chave; `operator ==` fica como está.
        let the_name = if fe.kind == FunctionKind::Setter { the_name.strip_suffix('=').unwrap_or(the_name) } else { the_name };
        Some(Executavel { especie: the_kind, nome: the_name.to_string() })
    }
}
