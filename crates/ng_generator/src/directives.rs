//! Attribute directives that the generator instantiates on the node: the model
//! of what the official compiler knows about each one (the
//! `CompileDirectiveMetadata`, in the part the emitter uses) and the resolution
//! of a node's providers.
//!
//! The metadata is **not** written here: `metadata.rs` reads it from the loaded
//! program, as `find_components.dart` reads it from the analyzer — selector,
//! `@Input`/`@Output`/`@HostListener` also inherited from superclasses,
//! interfaces and mixins, providers, visibility and the constructor
//! dependencies. A directive whose reading does not close, or that has
//! something the emitter does not write yet, is refused with the reason
//! ([`Directive::blocker`]).
//!
//! The resolution of the node's providers follows `provider_parser.dart`
//! (`_ProviderResolver.resolve`, `_getOrCreateLocalProvider`) and
//! `ProviderResolver.addDirectiveProviders`: the order of the providers is that
//! of the depth-first search through the dependencies, the field's `uniqueId`
//! is the size of the instance table at that moment (five element built-ins
//! before everything), and an `ExistingProvider` of a provider of the node
//! itself becomes an alias, without a field.
use crate::component::Hooks;
use std::sync::Arc;

/// `package:ngdart/src/meta/di_tokens.dart`, where `MultiToken` is.
pub const DI_TOKENS: &str = "package:ngdart/src/meta/di_tokens.dart";

/// The `T` of a `MultiToken<T>`/`OpaqueToken<T>`: the class and how many type
/// arguments it has. With empty `args`, all `dynamic`, as `fromDartType` writes
/// them; otherwise, the concrete arguments (`List<RelativePosition>`), each
/// with the same shape (`dynamic` is the `dynamic` class without a URI).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TokenType {
    pub uri: String,
    pub class: String,
    pub generics: usize,
    pub args: Vec<TokenType>,
}

impl TokenType {
    /// `Object` of `dart:core`, the `T` of `ngValidators`.
    pub fn e_object(&self) -> bool {
        self.uri == "dart:core" && self.class == "Object" && self.generics == 0
    }

    /// `dynamic` as a type argument.
    pub fn dynamic() -> Self {
        TokenType {
            class: "dynamic".into(),
            ..Default::default()
        }
    }

    pub fn is_dynamic(&self) -> bool {
        self.uri.is_empty() && self.class == "dynamic"
    }
}

/// The library of ngdart's `Injector`.
const INJECTOR: &str = "package:ngdart/src/di/injector.dart";

/// An injection token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// A class, by the library that declares it.
    Class { uri: String, class: String },
    /// `const MultiToken<T>('name')`.
    Multi { name: String, ty: TokenType },
    /// `const OpaqueToken<T>('name')`.
    Opaque { name: String, ty: TokenType },
    /// `HtmlElement`/`Element`: the node itself (element built-in).
    Element,
    /// `ChangeDetectorRef`: in a directive, the view itself (`o.thisExpr`).
    Detector,
}

impl Token {
    /// The name that goes in the field (`_NgModel_3_9`, `_NgValidators_3_6`):
    /// the `CompileTokenMetadata.name`, which for a string token replaces with
    /// `_` whatever is not a letter, digit or `_`.
    pub fn name(&self) -> String {
        match self {
            Token::Class { class, .. } => class.clone(),
            Token::Multi { name, .. } | Token::Opaque { name, .. } => name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect(),
            Token::Element => "HtmlElement".into(),
            Token::Detector => "ChangeDetectorRef".into(),
        }
    }

    /// One of the element built-ins (`ElementRef`, `Injector`,
    /// `ViewContainerRef`…), which a service does not receive like the others.
    pub fn builtin(&self) -> bool {
        match self {
            Token::Element | Token::Detector => true,
            Token::Class { uri, class } => {
                uri.starts_with("package:ngdart/")
                    && matches!(
                        class.as_str(),
                        "ElementRef"
                            | "Injector"
                            | "ViewContainerRef"
                            | "TemplateRef"
                            | "ComponentLoader"
                            | "NgContentRef"
                            | "ChangeDetectorRef"
                    )
            }
            _ => false,
        }
    }
}

/// A constant `useValue:` value in the part the emitter writes
/// (`_useValueExpression`): string, integer, boolean, and constant object with
/// arguments of those kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Str(String),
    Integer(i64),
    Boolean(bool),
    /// `const C(..)`/`const C.name(..)`, of a class without type parameters.
    Object {
        uri: String,
        class: String,
        constructor: Option<String>,
        positional: Vec<ConstValue>,
        named: Vec<(String, ConstValue)>,
    },
}

/// Where the value of a provider comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provides {
    /// `useExisting:` — another token.
    Existing(Token),
    /// `useClass:` (or the token itself), with the constructor dependencies.
    Class {
        uri: String,
        class: String,
        deps: Vec<Dependency>,
    },
    /// `useValue:`.
    Value(ConstValue),
    /// `useFactory:` of a top-level function, with `deps:` (or the parameters).
    Factory {
        uri: String,
        name: String,
        deps: Vec<Dependency>,
    },
}

/// An item of `providers:`, already flattened (`ModuleReader`,
/// `_normalizeProviders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub token: Token,
    pub source: Provides,
    pub multi: bool,
    /// The `T` of `Provider<T>` (`inferProviderType`): the field type when it
    /// is not that of the expression itself. `None` when inference finds
    /// nothing (and the field takes the type of the value, or `dynamic`).
    pub ty: Option<TokenType>,
}

impl Provider {
    /// `ExistingProvider(token, target)` without an inferred type — the only
    /// shape a template node knows how to create.
    pub fn alias(token: Token, target: Token, multi: bool) -> Self {
        Provider {
            token,
            source: Provides::Existing(target),
            multi,
            ty: None,
        }
    }
}

/// A positional constructor parameter (`_getCompileDiDependencyMetadata` skips
/// the named ones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub token: Token,
    /// `@Optional()` or an optional positional parameter.
    pub optional: bool,
    /// `@Self()`.
    pub self_only: bool,
    /// `@Host()`.
    pub host: bool,
    /// `@SkipSelf()`.
    pub skip_self: bool,
    /// `@Attribute('name')`: the value of the element's static attribute, or
    /// `null` (`_getLocalDependency`).
    pub attribute: Option<String>,
}

/// An `@Input`: name in the template, member, and whether the type is `bool`
/// (an attribute without a value becomes `true`, `visitEmptyExpr`); `None` when
/// the type is not known from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub name: String,
    pub member: String,
    pub boolean: Option<bool>,
}

/// A `@HostListener`: the event, the method and the arguments as
/// `_addHostListener` writes them. Empty `args`: a method without parameters;
/// `$event`: one parameter; other text: a complex handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub event: String,
    pub method: String,
    pub args: String,
}

/// What the generator knows about a `@Directive` (or `@Component`), read from
/// the program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Directive {
    pub class: String,
    pub uri: String,
    pub selector: String,
    pub is_component: bool,
    pub export_as: Option<String>,
    /// `visibility: Visibility.all`: injectable by whoever is below.
    pub visible: bool,
    pub providers: Vec<Provider>,
    pub dependencies: Vec<Dependency>,
    /// In the order of the `inputs` map (`_SortInputsVisitor`).
    pub inputs: Vec<Input>,
    /// (name in the template, member), in the order of the `outputs` map.
    pub outputs: Vec<(String, String)>,
    /// In the order of the `hostListeners` map.
    pub listeners: Vec<Listener>,
    /// `@HostBinding`: (binding name, member).
    pub host_bindings: Vec<(String, String)>,
    /// Some `@HostBinding` on a static member (outside
    /// `host_bindings`): the official compiler writes it once, in the
    /// view constructor — for a component, through its reader (case j96); here,
    /// no case.
    pub static_host: bool,
    /// The static members among the `host_bindings` (`StaticRead`,
    /// immutable, `dynamic` for the `_TypeResolver`).
    pub host_statics: Vec<String>,
    /// `hostAttributes` of a directive: `@HostBinding` on an immutable static
    /// outside `class.x`/`style.x` (name without `attr.`, member). Outside the
    /// `XNgCd`; the directive's user writes them on the element.
    pub host_attributes: Vec<(String, String)>,
    pub hooks: Hooks,
    /// Some `@ViewChild(ren)` (a directive has no view).
    pub queries: bool,
    /// The `@ContentChild(ren)`, with the types resolved in the directive's
    /// library: setters, then fields, each group in declaration order.
    pub content_queries: Vec<crate::view::ChildQuery>,
    /// What the reading could not understand: with anything here, the metadata
    /// is incomplete and the directive is not used.
    pub unsupported: Vec<String>,
    /// The `viewProviders:` of a component (`ProviderAstType.privateService` on
    /// its element), in order.
    pub view_providers: Vec<Provider>,
    /// The constructor dependencies were read completely (`unsupported` may have
    /// something else, like an unreadable `viewProviders:`, which does not
    /// affect them).
    pub dependencies_read: bool,
}

impl Directive {
    pub fn token(&self) -> Token {
        Token::Class {
            uri: self.uri.clone(),
            class: self.class.clone(),
        }
    }

    /// All `providers:` are `ExistingProvider`s of a class token or
    /// `MultiToken` — what a template node knows how to create. (The others are
    /// only written by the component's own host view.)
    pub fn only_aliases(&self) -> bool {
        self.providers.iter().all(|p| {
            matches!(&p.source, Provides::Existing(Token::Class { .. }))
                && matches!(p.token, Token::Class { .. } | Token::Multi { .. })
        })
    }

    pub fn input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|e| e.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&str> {
        self.outputs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m.as_str())
    }

    /// Why the emitter does not instantiate this directive on an HTML element
    /// yet, or `None` if it does. Each item is a shape without a case in the
    /// corpus: generating while ignoring it would give wrong output.
    pub fn blocker(&self) -> Option<String> {
        if let Some(f) = self.unsupported.first() {
            return Some(f.clone());
        }
        if !self.host_attributes.is_empty() {
            return Some("directive hostAttributes (merged into the element)".into());
        }
        if self.is_component {
            return Some("component as a directive".into());
        }

        if self.queries {
            return Some("content or view query".into());
        }
        if self
            .listeners
            .iter()
            .any(|o| !crate::view::native_event(&o.event))
        {
            return Some("@HostListener of a non-native event".into());
        }
        for d in &self.dependencies {
            if let Token::Multi { ty, .. } = &d.token
                && !ty.e_object()
                && ty.generics == 0
            {
                return Some("MultiToken of a non-generic type".into());
            }
        }
        // `OpaqueToken<T>` with a generic `T`: the shape of `T` in the token
        // expression (`createDiTokenExpression`) has no case yet.
        if self
            .dependencies
            .iter()
            .any(|d| matches!(&d.token, Token::Opaque { ty, .. } if ty.generics > 0))
        {
            return Some("dependency on an OpaqueToken of a generic type".into());
        }
        for p in &self.providers {
            if let Token::Multi { ty, .. } = &p.token
                && !ty.e_object()
                && ty.generics == 0
            {
                return Some("MultiToken of a non-generic type".into());
            }
        }
        None
    }
}

/// An argument of a directive's constructor, already resolved on the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argument {
    /// The node (`_el_3` or `this._el_3`).
    Element,
    /// The view (`this`).
    Detector,
    /// `@Optional() @Self()` without a provider on the node.
    Null,
    /// The field of another provider of the node.
    Field(String),
    /// A provider of an element above (`_getDependency` walks up through the
    /// parents): the expression that reads it from this view.
    Above(String),
    /// `ViewContainerRef`: the node's `ViewContainer` (`this._appEl_n`).
    Container,
    /// `@Attribute('name')`: the literal of the element's static attribute, or
    /// `null`.
    Attribute(String),
    /// `Injector`: the element's own injector, `this.injector(n)` (one of the
    /// built-ins of `CompileElement`, like `ElementRef`).
    Injector(u32),
    /// No element of the chain provides it: the injector from outside the view
    /// (`injectFromViewParentInjector`), `injectorGetOptional` with
    /// `@Optional()`.
    FromOutside { token: Token, optional: bool },
}

/// An injectable provider of an element above the node, in its view or in an
/// ancestor view (the `parentView` chain up to it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAbove {
    pub token: Token,
    /// The expression that reads it from the view the node is in.
    pub read: String,
    /// A lazy provider of the owning element: requested by a node below, the
    /// official compiler transforms it during the child's visit, before the
    /// owner's `afterElement` (`provider_parser.dart:318-366`), which changes
    /// its index and shape there — not modeled yet (gap L1).
    pub lazy: bool,
    /// Seen from another view (it crossed a `<template>`): `_getDependency`
    /// asks with `eager: false` (`_isViewRoot`), the provider stays lazy and is
    /// read through the field — when the owner modeled that request
    /// ([`resolve_child_node`]).
    pub lazy_read: bool,
}

/// What there is above the node for the dependencies it does not satisfy.
#[derive(Debug, Clone, Copy)]
pub struct Above<'a> {
    /// From the closest to the farthest.
    pub providers: &'a [ProviderAbove],
    /// Some element above has a provider the emitter does not model (a
    /// component): not finding it does not prove there is none.
    pub uncertain: bool,
    /// The tokens of the `viewProviders:` of this view's component: an
    /// `@Host()` that asks for one of them goes to the outside injector
    /// (`provider_parser.dart:347-351`).
    pub of_view: &'a [Token],
}

/// How a provider field is created in `build()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Creation {
    /// `Class(args)`.
    Directive {
        directive: Arc<Directive>,
        args: Vec<Argument>,
    },
    /// `[a, b]`: the fields the multi-provider joins.
    List(Vec<String>),
    /// Only in the host view: a `providers:` provider that is neither a
    /// directive nor a local alias (`ClassProviderSource`,
    /// `FactoryProviderSource`, `ExpressionProviderSource` and the
    /// `injectorGet` of an outside alias).
    Expression(Expr),
    /// Only in the host view: the multi-provider with items of any shape.
    Multi(Vec<Expr>),
}

/// The value of a host view provider, as `ProviderSource.build` writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// `this.field` — another provider of the node (`component` is the
    /// component).
    Field(String),
    /// A provider of an element above, in a template node: the expression that
    /// reads it from this view (`_getDependency`).
    Read(String),
    /// `this.injectorGet(token, this.parentIndex)` (or `injectorGetOptional`):
    /// what the node does not provide comes from the outside injector.
    Injector {
        token: Token,
        optional: bool,
    },
    /// `Class(args)`.
    Class {
        uri: String,
        class: String,
        args: Vec<Expr>,
    },
    /// `function(args)`.
    Factory {
        uri: String,
        name: String,
        args: Vec<Expr>,
    },
    Value(ConstValue),
}

impl Expr {
    /// Some dependency comes from the injector (`hasDynamicDependencies`): the
    /// creation comes out wrapped in `debugInjectorWrap`.
    pub fn dynamic(&self) -> bool {
        match self {
            Expr::Class { args, .. } | Expr::Factory { args, .. } => {
                args.iter().any(|a| matches!(a, Expr::Injector { .. }))
            }
            _ => false,
        }
    }
}

/// A provider of the node that becomes a field of the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub token: Token,
    pub field: String,
    pub creation: Creation,
    /// The tokens through which it is injectable below (`injectorGetInternal`):
    /// its own, if visible, and the aliases.
    pub injectable_by: Vec<Token>,
    /// The node's `ExistingProvider`s that point to this instance, visible or
    /// not: a query also finds it through them.
    pub aliases: Vec<Token>,
    /// No eager provider (directive, component) depends on it: the official
    /// compiler creates it in `afterElement` with `eager: false`, as a `late`
    /// field with an initializer (`late List<T> _X_n_m = [..];`), and not in
    /// `build()`.
    pub lazy: bool,
    /// How the instance is read: the field, or `field.instance` when the
    /// directive has `@HostBinding` and the field holds its `XNgCd`.
    pub read: String,
    /// The inferred `T` of the provider (the `typeArgument`), which types the
    /// field.
    pub ty: Option<TokenType>,
}

/// The directive providers of a node, resolved.
#[derive(Debug, Clone, Default)]
pub struct ResolvedNode {
    /// The fields, in creation order.
    pub instances: Vec<Instance>,
    /// The directives, in the order the official compiler binds them
    /// (`transformedDirectiveAsts`: that of the providers), with how each
    /// instance is read (the field, or `field.instance` with `XNgCd`).
    pub directives: Vec<(Arc<Directive>, String)>,
    /// The node has a `ViewContainer` (`requiresViewContainer`): a directive or
    /// its component injects `ViewContainerRef` (cases j47–j49).
    pub container: bool,
}

#[derive(Debug)]
enum Source {
    Directive(Arc<Directive>),
    Existing(Token),
    /// `useClass`/`useValue`/`useFactory` of `providers:`.
    Provider(Provides),
}

#[derive(Debug)]
struct Resolved {
    token: Token,
    sources: Vec<Source>,
    multi: bool,
    eager: bool,
    visible: bool,
    ty: Option<TokenType>,
    /// It came from `viewProviders:` (`ProviderAstType.privateService`): only
    /// the component and another private provider of the node find it locally
    /// (`_getOrCreateLocalProvider`); directives and public services, of the
    /// node or from below, go up to the injector.
    is_private: bool,
}

/// Resolves the providers of the `matched` directives (in `directives:` order)
/// on node `n`. A dependency the node itself does not satisfy and that is not
/// `@Optional() @Self()` is refused: it would come from another node or from
/// the injector, shapes without a case yet.
pub fn resolver(
    matched: &[Arc<Directive>],
    n: u32,
    above: Option<Above>,
) -> Result<ResolvedNode, &'static str> {
    resolve_in(matched, n, above, false)
}

/// [`resolver`] with the tokens that the view queries read from the node
/// (`read:`), which the official compiler creates eagerly (`queriedTokens`).
pub fn resolve_queried(
    matched: &[Arc<Directive>],
    n: u32,
    above: Option<Above>,
    queried: &[Token],
) -> Result<ResolvedNode, &'static str> {
    resolve_with(matched, n, above, false, None, false, None, queried, &[])
}

/// The providers of a written `<template>` (`EmbeddedTemplateAst`): the
/// built-ins (`ElementRef`, `Element`, `HtmlElement`, `Injector`,
/// `ViewContainer`; `ViewContainerRef` if some directive asks for it;
/// `ChangeDetectorRef`, `ComponentLoader`), the `TemplateRef` — the first of
/// the resolved ones (`setEmbeddedView`), at the next index — and the
/// directives after it. `template_ref` gives how the node's `TemplateRef` is
/// read through its index, which is returned too (case j85).
pub fn resolve_template(
    matched: &[Arc<Directive>],
    n: u32,
    above: Option<Above>,
    // A query reads a `#ref` of the node as `ViewContainerRef`
    // (`_requiresViewContainer`, `provider_parser.dart:95-104`).
    force_container: bool,
    template_ref: &dyn Fn(u32) -> String,
) -> Result<(ResolvedNode, u32), &'static str> {
    let container = force_container || matched.iter().any(|d| requests_container(d));
    let index = crate::dialect::extra_builtins() + if container { 8 } else { 7 };
    let read = template_ref(index);
    let r = resolve_with(
        matched,
        n,
        above,
        false,
        None,
        container,
        Some((&read, index)),
        &[],
        &[],
    )?;
    Ok((r, index))
}

/// O `TemplateRef` do ngdart.
pub fn e_template_ref(t: &Token) -> bool {
    matches!(t, Token::Class { uri, class }
        if uri == "package:ngdart/src/core/linker/template_ref.dart" && class == "TemplateRef")
}

/// The providers of the host view's node (`_ViewXHost0`, node 0): the component
/// and its `providers:`. What the node does not provide comes from the outside
/// injector (`injectFromViewParentInjector`), and the component instance is the
/// `component` field of the `HostView` (`hostViewComponentFieldName`).
pub fn resolve_host(component: Arc<Directive>) -> Result<ResolvedNode, &'static str> {
    resolve_in(&[component], 0, None, true)
}

/// The providers of the node of a child component in a template: the child
/// (`matched[0]`, without the constructor dependencies, which are resolved
/// separately) with its `providers:` of any shape, and the node's directives
/// (`matched[1..]`), which still only have `ExistingProvider`. The child's
/// providers are written as in the host view (`ProviderSource.build`), but a
/// dependency the node does not satisfy would go to the elements above or to
/// the outside injector (`parentView.injectorGet`): with no case yet, it is
/// refused.
///
/// `container`: the child injects `ViewContainerRef` and the node gets a
/// `ViewContainer`, with three more built-ins (`ViewContainer`,
/// `ViewContainerRef`, `ComponentLoader`) before the providers (case j47).
///
/// `requests`: the tokens the content nodes request from this node during their
/// visit (`_getDependency` walks up and finds them here), in order, which the
/// official compiler transforms eagerly before this node's `afterElement` (case
/// i76).
pub fn resolve_child_node(
    matched: &[Arc<Directive>],
    child: usize,
    n: u32,
    above: Option<Above>,
    container: bool,
    requests: &[(Token, bool)],
) -> Result<ResolvedNode, &'static str> {
    resolve_with(
        matched,
        n,
        above,
        false,
        Some(child),
        container,
        None,
        &[],
        requests,
    )
}

fn resolve_in(
    matched: &[Arc<Directive>],
    n: u32,
    above: Option<Above>,
    host: bool,
) -> Result<ResolvedNode, &'static str> {
    resolve_with(matched, n, above, host, None, false, None, &[], &[])
}

/// `component`: the index of the child component in `matched` (on a child's
/// node), which has its `providers:` of any shape written, like all of them in
/// the host view; the others only with `ExistingProvider`.
#[allow(clippy::too_many_arguments)]
fn resolve_with(
    matched: &[Arc<Directive>],
    n: u32,
    above: Option<Above>,
    host: bool,
    component: Option<usize>,
    container: bool,
    hand_template: Option<(&str, u32)>,
    queried: &[Token],
    requests: &[(Token, bool)],
) -> Result<ResolvedNode, &'static str> {
    let complete = |k: usize| host || component == Some(k);
    // `requiresViewContainer`: with a `ViewContainer` the node gets three
    // built-ins (`ViewContainer`, `ViewContainerRef`, `ComponentLoader`) before
    // the providers, and the fields start at 8.
    let container = container || matched.iter().any(|d| requests_container(d));
    // `_ProviderResolver.resolve`: the directives (eager), then the
    // `providers:` of each; the same multi token accumulates.
    let mut all: Vec<Resolved> = Vec::new();
    for d in matched {
        all.push(Resolved {
            token: d.token(),
            sources: vec![Source::Directive(d.clone())],
            multi: false,
            eager: true,
            visible: d.visible,
            ty: None,
            is_private: false,
        });
    }
    // The `providers:` with the component before the directives ("directives
    // are able to overwrite providers of a component").
    let in_order = component
        .into_iter()
        .chain((0..matched.len()).filter(|k| Some(*k) != component));
    for k in in_order {
        let d = &matched[k];
        // `viewProviders:` only for a component (the node of a child with child
        // nodes, which would put them in a separate `ProviderNode`, is refused
        // earlier).
        if !d.view_providers.is_empty() && !complete(k) {
            return Err("directive viewProviders");
        }
        let public = d.providers.iter().map(|p| (p, false));
        let private = d.view_providers.iter().map(|p| (p, true));
        for (p, is_private) in public.chain(private) {
            let source = match &p.source {
                Provides::Existing(t) => Source::Existing(t.clone()),
                f => Source::Provider(f.clone()),
            };
            match all.iter_mut().find(|r| r.token == p.token) {
                Some(r) => {
                    if r.is_private != is_private {
                        // The `providerType` of the one that stays: no case
                        // yet.
                        return Err("public and private provider on the same token");
                    }
                    if r.multi != p.multi {
                        return Err("multi and non-multi provider on the same token");
                    }
                    if r.eager && (complete(k) || matches!(source, Source::Provider(_))) {
                        // The component's token overwritten by `providers:`: no
                        // case yet.
                        return Err("provider with the component's token");
                    }
                    if !p.multi {
                        r.sources.clear();
                        r.ty = p.ty.clone();
                    }
                    r.sources.push(source);
                }
                None => all.push(Resolved {
                    token: p.token.clone(),
                    sources: vec![source],
                    multi: p.multi,
                    eager: false,
                    visible: true,
                    ty: p.ty.clone(),
                    is_private,
                }),
            }
        }
    }
    // What a query reads from the node (`queriedTokens`: the `read:` of each
    // query that matches it) is eager too.
    for r in &mut all {
        if queried.contains(&r.token) {
            r.eager = true;
        }
    }
    // `_getOrCreateLocalProvider`: depth-first, the dependencies first. Only
    // the dependencies that `_getDependency` looks for on the node itself
    // become edges: `@SkipSelf` skips the node (`provider_parser.dart:325`),
    // and `_getLocalDependency` returns `@Attribute`, `Injector` and the
    // element built-ins without creating a provider (`:270-301`). Without this,
    // the `@Optional() @SkipSelf()` of the node's own token looked like a
    // cycle.
    fn local(dep: &Dependency) -> bool {
        !dep.skip_self && dep.attribute.is_none() && !dep.token.builtin()
    }
    /// Whoever asks as `component` or `privateService` finds the private ones.
    fn sees_private(r: &Resolved) -> bool {
        r.is_private || matches!(r.sources.as_slice(), [Source::Directive(d)] if d.is_component)
    }
    fn create(
        all: &[Resolved],
        i: usize,
        order: &mut Vec<usize>,
        seen: &mut Vec<usize>,
        host: bool,
    ) -> Result<(), &'static str> {
        if order.contains(&i) {
            return Ok(());
        }
        if seen.contains(&i) {
            return Err("cyclic dependency between directives");
        }
        seen.push(i);
        let is_private = sees_private(&all[i]);
        let request = |t: &Token, order: &mut Vec<usize>, seen: &mut Vec<usize>| match all
            .iter()
            .position(|r| r.token == *t && (is_private || !r.is_private))
        {
            Some(j) => create(all, j, order, seen, host),
            None => Ok(()),
        };
        for f in &all[i].sources {
            match f {
                Source::Existing(t) => {
                    // An alias from outside the node reads the injector (host
                    // view) or the element above / the outside injector
                    // (template node).
                    if let Some(j) = all
                        .iter()
                        .position(|r| r.token == *t && (is_private || !r.is_private))
                    {
                        create(all, j, order, seen, host)?;
                    }
                }
                Source::Directive(d) => {
                    for dep in d.dependencies.iter().filter(|d| local(d)) {
                        request(&dep.token, order, seen)?;
                    }
                }
                Source::Provider(Provides::Class { deps, .. } | Provides::Factory { deps, .. }) => {
                    for dep in deps.iter().filter(|d| local(d)) {
                        request(&dep.token, order, seen)?;
                    }
                }
                Source::Provider(_) => {}
            }
        }
        order.push(i);
        Ok(())
    }
    let mut order = Vec::new();
    let mut seen = Vec::new();
    for i in 0..all.len() {
        if all[i].eager {
            create(&all, i, &mut order, &mut seen, host)?;
        }
    }
    // The eager ones so far; the following requests mark those they create.
    let mut eager_idx: Vec<usize> = order.clone();
    // The requests of the nodes below, during their visit (before this one's
    // `afterElement`): each one transformed right away, with the dependencies
    // first (`_getLocalDependency` with the `eager` of the requester); one that
    // crossed a `<template>` asks with `eager: false` and stays lazy, but in
    // the position of the visit. Once transformed, it stays as it is.
    for (t, eager) in requests {
        if let Some(i) = all.iter().position(|r| r.token == *t) {
            let before = order.len();
            create(&all, i, &mut order, &mut seen, host)?;
            if *eager {
                eager_idx.extend_from_slice(&order[before..]);
            }
        }
    }
    // `afterElement`: what is left (the aliases, in general).
    for i in 0..all.len() {
        create(&all, i, &mut order, &mut seen, host)?;
    }

    // `addDirectiveProviders`: the `uniqueId` is the size of the table, which
    // already has the five element built-ins. In a `<template>`, the directives
    // come after the `TemplateRef`.
    let mut size = match hand_template {
        Some((_, index)) => index + 1,
        None if container => 8 + crate::dialect::extra_builtins(),
        None => 5 + crate::dialect::extra_builtins(),
    };
    let mut fields: Vec<(Token, String)> = Vec::new();
    let mut aliases: Vec<(Token, Token)> = Vec::new();
    let mut output = ResolvedNode {
        container,
        ..ResolvedNode::default()
    };
    for (position, &i) in order.iter().enumerate() {
        let r = &all[i];
        let _ = position;
        let lazy = !eager_idx.contains(&i);
        if let (false, [Source::Existing(target)]) = (r.multi, r.sources.as_slice())
            && let Some(real) = fields
                .iter()
                .find(|(t, _)| t == target)
                .map(|_| target.clone())
                .or_else(|| {
                    aliases
                        .iter()
                        .find(|(t, _)| t == target)
                        .map(|(_, a)| a.clone())
                })
        {
            aliases.push((r.token.clone(), real));
            size += 1;
            continue;
        }
        let host_component =
            host && matches!(r.sources.as_slice(), [Source::Directive(d)] if d.is_component);
        let field = if host_component {
            "component".to_string()
        } else {
            format!("_{}_{n}_{size}", r.token.name())
        };
        // Directive with `@HostBinding`: the field is the `XNgCd` that wraps it
        // (`createProvider`, `providerHasChangeDetector`). Not for a component:
        // its `@HostBinding` is the `detectHostChanges` of its view, and the
        // field is the instance (case j30).
        let read = match r.sources.as_slice() {
            [Source::Directive(d)]
                if !d.host_bindings.is_empty() && !d.is_component && !host_component =>
            {
                format!("{field}.instance")
            }
            _ => field.clone(),
        };
        let field_of = |t: &Token| -> Option<String> {
            // A private provider is only seen by another private one.
            if !sees_private(r) && all.iter().any(|x| x.is_private && x.token == *t) {
                return None;
            }
            let t = aliases
                .iter()
                .find(|(a, _)| a == t)
                .map(|(_, real)| real)
                .unwrap_or(t);
            fields.iter().find(|(x, _)| x == t).map(|(_, c)| c.clone())
        };
        // A service dependency (`_getDependency` in the host view): the local
        // field, or the outside injector.
        // What the node does not provide: in the host view, the injector; in a
        // template node, the element above that provides it (`_getDependency`)
        // or, without one, the injector from outside the view (case j71).
        let from_outside = |t: &Token, optional: bool| -> Result<Expr, &'static str> {
            if !host && let Some(a) = above {
                if let Some(p) = a.providers.iter().find(|p| p.token == *t) {
                    if p.lazy {
                        return Err("lazy provider of an element above requested below (L1)");
                    }
                    return Ok(Expr::Read(p.read.clone()));
                }
                if a.uncertain {
                    return Err("provider dependency under a component without metadata");
                }
            }
            Ok(Expr::Injector {
                token: t.clone(),
                optional,
            })
        };
        let dependency = |dep: &Dependency| -> Result<Expr, &'static str> {
            if dep.self_only || dep.host {
                return Err("@Self/@Host dependency of a provider");
            }
            if dep.attribute.is_some() {
                return Err("@Attribute dependency of a provider");
            }
            if dep.token.builtin() {
                return Err("provider that depends on an element built-in");
            }
            // `@SkipSelf`: the node is not consulted
            // (`provider_resolver.dart:189`); it walks up through the elements
            // above and, without them, the outside injector.
            if dep.skip_self {
                return from_outside(&dep.token, dep.optional);
            }
            match field_of(&dep.token) {
                Some(c) => Ok(Expr::Field(c)),
                None => from_outside(&dep.token, dep.optional),
            }
        };
        let expr_of = |f: &Provides| -> Result<Expr, &'static str> {
            Ok(match f {
                Provides::Existing(t) => match field_of(t) {
                    Some(c) => Expr::Field(c),
                    None if t.builtin() => return Err("alias of an element built-in"),
                    None => from_outside(t, false)?,
                },
                Provides::Class { uri, class, deps } => Expr::Class {
                    uri: uri.clone(),
                    class: class.clone(),
                    args: deps.iter().map(dependency).collect::<Result<_, _>>()?,
                },
                Provides::Factory { uri, name, deps } => Expr::Factory {
                    uri: uri.clone(),
                    name: name.clone(),
                    args: deps.iter().map(dependency).collect::<Result<_, _>>()?,
                },
                Provides::Value(v) => Expr::Value(v.clone()),
            })
        };
        let creation = match r.sources.as_slice() {
            [Source::Directive(d)] => {
                let mut args = Vec::new();
                for dep in &d.dependencies {
                    if let Some(name) = &dep.attribute {
                        args.push(Argument::Attribute(name.clone()));
                        continue;
                    }
                    args.push(match &dep.token {
                        Token::Element | Token::Detector if dep.skip_self => {
                            return Err("@SkipSelf dependency on an element built-in");
                        }
                        Token::Element => Argument::Element,
                        Token::Detector => Argument::Detector,
                        t if e_view_container_ref(t) && !dep.skip_self => Argument::Container,
                        // The `TemplateRef` of the `<template>` itself.
                        t if e_template_ref(t) && !dep.skip_self && hand_template.is_some() => {
                            Argument::Above(
                                hand_template
                                    .map(|(l, _)| l.to_string())
                                    .unwrap_or_default(),
                            )
                        }
                        Token::Class { uri, class } if uri == INJECTOR && class == "Injector" => {
                            // `@SkipSelf()` would read the injector of the
                            // element above (`injector(parent)`): no case yet.
                            if dep.skip_self {
                                return Err("@SkipSelf dependency on Injector");
                            }
                            Argument::Injector(n)
                        }
                        // `@SkipSelf()`: `_getDependency` starts at the parent
                        // (the `ControlContainer` of `NgControlName` is the
                        // `NgForm` of the `<form>` above).
                        t => match field_of(t).filter(|_| !dep.skip_self) {
                            Some(c) => Argument::Field(c),
                            None => outside_node(dep, above)?,
                        },
                    });
                }
                if !host_component {
                    output.directives.push((d.clone(), read.clone()));
                }
                Creation::Directive {
                    directive: d.clone(),
                    args,
                }
            }
            sources
                if r.multi
                    && (host || sources.iter().any(|f| matches!(f, Source::Provider(_)))) =>
            {
                let mut items = Vec::new();
                for f in sources {
                    items.push(match f {
                        Source::Existing(t) => expr_of(&Provides::Existing(t.clone()))?,
                        Source::Provider(p) => expr_of(p)?,
                        Source::Directive(_) => return Err("directive multi-provider"),
                    });
                }
                Creation::Multi(items)
            }
            sources if r.multi => {
                let mut items = Vec::new();
                for f in sources {
                    let Source::Existing(t) = f else {
                        return Err("multi-provider that is not an alias");
                    };
                    items.push(
                        field_of(t).ok_or("multi-provider of a token from outside the node")?,
                    );
                }
                Creation::List(items)
            }
            // An alias of a token the node does not provide: the injector (host
            // view) or the element above / the outside injector (template
            // node).
            [Source::Existing(t)] => Creation::Expression(expr_of(&Provides::Existing(t.clone()))?),
            [Source::Provider(p)] => Creation::Expression(expr_of(p)?),
            _ => return Err("alias provider of a token from outside the node"),
        };
        output.instances.push(Instance {
            token: r.token.clone(),
            field: field.clone(),
            creation,
            injectable_by: if r.visible {
                vec![r.token.clone()]
            } else {
                Vec::new()
            },
            aliases: Vec::new(),
            lazy,
            read: read.clone(),
            ty: r.ty.clone(),
        });
        fields.push((r.token.clone(), read));
        size += 1;
    }
    for (alias, real) in aliases {
        if let Some(inst) = output.instances.iter_mut().find(|x| x.token == real) {
            inst.aliases.push(alias.clone());
            inst.injectable_by.push(alias);
        }
    }
    Ok(output)
}

/// ngdart's `ViewContainerRef`, or `ComponentLoader`, which is an alias of the
/// same `ViewContainer` and also turns on `_requiresViewContainer`
/// (`provider_parser.dart:290-295`, `compile_element.dart:204-206`).
pub fn e_view_container_ref(t: &Token) -> bool {
    matches!(t, Token::Class { uri, class }
        if uri.starts_with("package:ngdart/")
            && matches!(class.as_str(), "ViewContainerRef" | "ComponentLoader"))
}

/// The directive injects `ViewContainerRef` and its node gets a `ViewContainer`
/// (`_requiresViewContainer`, in `provider_parser.dart`).
pub fn requests_container(d: &Directive) -> bool {
    d.dependencies
        .iter()
        .any(|dep| e_view_container_ref(&dep.token))
}

/// The built-in providers of an element besides `Element`/`HtmlElement`,
/// `Injector` and `ChangeDetectorRef` (`CompileElement`): `ElementRef`, and
/// with a `ViewContainer` also `ViewContainer`, `ViewContainerRef`,
/// `ComponentLoader` and `TemplateRef`.
pub fn element_builtin(t: &Token) -> bool {
    matches!(t, Token::Class { uri, class }
    if uri.starts_with("package:ngdart/")
        && matches!(
            class.as_str(),
            "ElementRef" | "ViewContainerRef" | "ViewContainer" | "ComponentLoader" | "TemplateRef"
        ))
}

/// A dependency the node itself does not satisfy (`_getDependency`): `@Self`
/// stops at the node; otherwise it walks up through the elements above (`@Host`
/// up to the host, which here is the root of the component's view). Without a
/// result and with `@Optional`, `null` — unless there is something above that
/// the emitter does not model. Without `@Self` or `@Host`, after the elements
/// comes the injector from outside the view (case j40).
fn outside_node(dep: &Dependency, above: Option<Above>) -> Result<Argument, &'static str> {
    // Without context (only the order of the imports matters): the value does
    // not matter.
    let Some(above) = above else {
        return Ok(Argument::Null);
    };
    if !dep.self_only
        && let Some(p) = above.providers.iter().find(|p| p.token == dep.token)
    {
        if p.lazy && !p.lazy_read {
            return Err("lazy provider of an element above requested below (L1)");
        }
        return Ok(Argument::Above(p.read.clone()));
    }
    // An element built-in (`ElementRef`, `ViewContainerRef`, `TemplateRef`…) is
    // never null and never comes from the outside injector: the official
    // compiler finds it on the node itself or on one above (`CompileElement`),
    // a shape without a case yet.
    if element_builtin(&dep.token) {
        return Err("dependency on an element built-in (ElementRef/ViewContainerRef/TemplateRef)");
    }
    match (dep.optional, dep.self_only, dep.host) {
        (true, true, _) => Ok(Argument::Null),
        (_, false, true) if !above.uncertain && above.of_view.contains(&dep.token) => {
            Ok(Argument::FromOutside {
                token: dep.token.clone(),
                optional: dep.optional,
            })
        }
        (true, false, true) if !above.uncertain => Ok(Argument::Null),
        (_, false, true) => Err("@Host dependency of a directive without a provider above"),
        // Some element above has a provider the emitter does not model: it
        // could be what the official compiler finds before the outside
        // injector.
        (_, false, false) if !above.uncertain => Ok(Argument::FromOutside {
            token: dep.token.clone(),
            optional: dep.optional,
        }),
        _ => Err("dependency on a directive from outside the node"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(uri: &str, c: &str) -> Token {
        Token::Class {
            uri: uri.into(),
            class: c.into(),
        }
    }

    fn validators() -> Token {
        Token::Multi {
            name: "NgValidators".into(),
            ty: TokenType {
                uri: "dart:core".into(),
                class: "Object".into(),
                generics: 0,
                args: Vec::new(),
            },
        }
    }

    fn accessors() -> Token {
        Token::Multi {
            name: "NgValueAccessor".into(),
            ty: TokenType {
                uri: "cva".into(),
                class: "ControlValueAccessor".into(),
                generics: 1,
                args: Vec::new(),
            },
        }
    }

    fn dep(token: Token, optional: bool, self_only: bool) -> Dependency {
        Dependency {
            token,
            optional,
            self_only,
            host: false,
            skip_self: false,
            attribute: None,
        }
    }

    /// Test directives with the shape that `metadata.rs` reads from `ngforms`.
    fn ng_model() -> Arc<Directive> {
        Arc::new(Directive {
            class: "NgModel".into(),
            uri: "m".into(),
            visible: true,
            providers: vec![Provider::alias(
                class("c", "NgControl"),
                class("m", "NgModel"),
                false,
            )],
            dependencies: vec![dep(validators(), true, true), dep(accessors(), true, true)],
            ..Default::default()
        })
    }

    fn accessor() -> Arc<Directive> {
        Arc::new(Directive {
            class: "DefaultValueAccessor".into(),
            uri: "d".into(),
            providers: vec![Provider::alias(
                accessors(),
                class("d", "DefaultValueAccessor"),
                true,
            )],
            dependencies: vec![dep(Token::Element, false, false)],
            ..Default::default()
        })
    }

    fn required() -> Arc<Directive> {
        Arc::new(Directive {
            class: "RequiredValidator".into(),
            uri: "v".into(),
            providers: vec![Provider::alias(
                validators(),
                class("v", "RequiredValidator"),
                true,
            )],
            ..Default::default()
        })
    }

    /// The order and numbering of `alterar_senha_page.template.dart`: the
    /// `RequiredValidator` before the multi that joins it, the accessor before
    /// its multi, `NgModel` last; `NgControl` is an alias.
    #[test]
    fn input_with_ng_model_and_required() {
        let model = ng_model();
        let matched = [model.clone(), accessor(), required()];
        let r = resolver(&matched, 33, None).unwrap();
        let fields: Vec<&str> = r.instances.iter().map(|i| i.field.as_str()).collect();
        assert_eq!(
            fields,
            [
                "_RequiredValidator_33_5",
                "_NgValidators_33_6",
                "_DefaultValueAccessor_33_7",
                "_NgValueAccessor_33_8",
                "_NgModel_33_9"
            ]
        );
        let ng_model = &r.instances[4];
        assert_eq!(
            ng_model.creation,
            Creation::Directive {
                directive: model,
                args: vec![
                    Argument::Field("_NgValidators_33_6".into()),
                    Argument::Field("_NgValueAccessor_33_8".into())
                ]
            }
        );
        assert_eq!(ng_model.injectable_by.len(), 2);
        assert!(r.instances[0].injectable_by.is_empty());
        assert_eq!(r.instances[1].injectable_by, vec![validators()]);
    }

    #[test]
    fn form_with_ng_form() {
        let form = Arc::new(Directive {
            class: "NgForm".into(),
            uri: "f".into(),
            visible: true,
            providers: vec![Provider::alias(
                class("cc", "ControlContainer"),
                class("f", "NgForm"),
                false,
            )],
            dependencies: vec![
                dep(validators(), true, true),
                dep(Token::Detector, false, false),
            ],
            ..Default::default()
        });
        let r = resolver(std::slice::from_ref(&form), 19, None).unwrap();
        assert_eq!(r.instances.len(), 1);
        assert_eq!(r.instances[0].field, "_NgForm_19_5");
        assert_eq!(
            r.instances[0].creation,
            Creation::Directive {
                directive: form,
                args: vec![Argument::Null, Argument::Detector]
            }
        );
        assert_eq!(r.instances[0].injectable_by.len(), 2);
    }

    fn service(c: &str, deps: &[&str]) -> Provider {
        Provider {
            token: class("s", c),
            source: Provides::Class {
                uri: "s".into(),
                class: c.into(),
                deps: deps
                    .iter()
                    .map(|d| dep(class("s", d), false, false))
                    .collect(),
            },
            multi: false,
            ty: None,
        }
    }

    /// The numbering of `i66_provider_dependencias.template.dart`: what the
    /// component asks for (and what that asks for) comes before it, eager; the
    /// rest stays lazy, afterwards, in `providers:` order.
    #[test]
    fn host_with_eager_and_lazy_providers() {
        let comp = Arc::new(Directive {
            class: "Comp".into(),
            uri: "s".into(),
            is_component: true,
            providers: vec![
                service("Cache", &["Repo"]),
                service("Solto", &[]),
                service("Repo", &["Api"]),
                service("Api", &[]),
                Provider::alias(class("s", "Base"), class("s", "Comp"), false),
            ],
            dependencies: vec![dep(class("s", "Repo"), false, false)],
            ..Default::default()
        });
        let r = resolve_host(comp).unwrap();
        let fields: Vec<(&str, bool)> = r
            .instances
            .iter()
            .map(|i| (i.field.as_str(), i.lazy))
            .collect();
        assert_eq!(
            fields,
            [
                ("_Api_0_5", false),
                ("_Repo_0_6", false),
                ("component", false),
                ("_Cache_0_8", true),
                ("_Solto_0_9", true),
            ]
        );
        // The component's alias: without a field, injectable through
        // `this.component`.
        assert_eq!(r.instances[2].injectable_by, vec![class("s", "Base")]);
        assert_eq!(
            r.instances[3].creation,
            Creation::Expression(Expr::Class {
                uri: "s".into(),
                class: "Cache".into(),
                args: vec![Expr::Field("_Repo_0_6".into())],
            })
        );
        assert!(r.directives.is_empty());
    }

    /// In a template node, the `providers:` of a directive of any shape (case
    /// j71): what it injects comes before it, eager; the rest stays lazy; the
    /// dependency the node does not provide comes from an element above or,
    /// without one, from the outside injector.
    #[test]
    fn template_node_with_class_providers() {
        let dir = Arc::new(Directive {
            class: "Espiao".into(),
            uri: "s".into(),
            dependencies: vec![dep(class("s", "Servico"), false, false)],
            providers: vec![
                service("Servico", &["Grupo", "Config"]),
                service("Solto", &[]),
            ],
            ..Default::default()
        });
        let above = [ProviderAbove {
            token: class("s", "Grupo"),
            read: "this._Grupo_0_5".into(),
            lazy: false,
            lazy_read: false,
        }];
        let r = resolver(
            &[dir],
            3,
            Some(Above {
                providers: &above,
                uncertain: false,
                of_view: &[],
            }),
        )
        .unwrap();
        let fields: Vec<(&str, bool)> = r
            .instances
            .iter()
            .map(|i| (i.field.as_str(), i.lazy))
            .collect();
        assert_eq!(
            fields,
            [
                ("_Servico_3_5", false),
                ("_Espiao_3_6", false),
                ("_Solto_3_7", true)
            ]
        );
        let Creation::Expression(Expr::Class { args, .. }) = &r.instances[0].creation else {
            panic!("service without creation by class");
        };
        assert_eq!(
            args,
            &[
                Expr::Read("this._Grupo_0_5".into()),
                Expr::Injector {
                    token: class("s", "Config"),
                    optional: false
                }
            ]
        );
        // With a component without metadata above, not finding proves nothing.
        let dir = Arc::new(Directive {
            class: "Espiao".into(),
            uri: "s".into(),
            providers: vec![service("Servico", &["Config"])],
            ..Default::default()
        });
        assert!(
            resolver(
                &[dir],
                3,
                Some(Above {
                    providers: &[],
                    uncertain: true,
                    of_view: &[],
                }),
            )
            .is_err()
        );
    }
}
