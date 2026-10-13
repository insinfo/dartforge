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
//! ([`Diretiva::pendencia`]).
//!
//! The resolution of the node's providers follows `provider_parser.dart`
//! (`_ProviderResolver.resolve`, `_getOrCreateLocalProvider`) and
//! `ProviderResolver.addDirectiveProviders`: the order of the providers is that
//! of the depth-first search through the dependencies, the field's `uniqueId`
//! is the size of the instance table at that moment (five element built-ins
//! before everything), and an `ExistingProvider` of a provider of the node
//! itself becomes an alias, without a field.
use crate::component::Ganchos;
use std::sync::Arc;

/// `package:ngdart/src/meta/di_tokens.dart`, where `MultiToken` is.
pub const DI_TOKENS: &str = "package:ngdart/src/meta/di_tokens.dart";

/// The `T` of a `MultiToken<T>`/`OpaqueToken<T>`: the class and how many type
/// arguments it has. With empty `args`, all `dynamic`, as `fromDartType` writes
/// them; otherwise, the concrete arguments (`List<RelativePosition>`), each
/// with the same shape (`dynamic` is the `dynamic` class without a URI).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TipoDeToken {
    pub uri: String,
    pub classe: String,
    pub genericos: usize,
    pub args: Vec<TipoDeToken>,
}

impl TipoDeToken {
    /// `Object` do `dart:core`, o `T` do `ngValidators`.
    pub fn e_object(&self) -> bool {
        self.uri == "dart:core" && self.classe == "Object" && self.genericos == 0
    }

    /// `dynamic` as a type argument.
    pub fn dinamico() -> Self {
        TipoDeToken {
            classe: "dynamic".into(),
            ..Default::default()
        }
    }

    pub fn e_dinamico(&self) -> bool {
        self.uri.is_empty() && self.classe == "dynamic"
    }
}

/// A biblioteca do `Injector` do ngdart.
const INJECTOR: &str = "package:ngdart/src/di/injector.dart";

/// An injection token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// A class, by the library that declares it.
    Classe { uri: String, classe: String },
    /// `const MultiToken<T>('nome')`.
    Multi { nome: String, tipo: TipoDeToken },
    /// `const OpaqueToken<T>('nome')`.
    Opaco { nome: String, tipo: TipoDeToken },
    /// `HtmlElement`/`Element`: the node itself (element built-in).
    Elemento,
    /// `ChangeDetectorRef`: in a directive, the view itself (`o.thisExpr`).
    Detector,
}

impl Token {
    /// The name that goes in the field (`_NgModel_3_9`, `_NgValidators_3_6`):
    /// the `CompileTokenMetadata.name`, which for a string token replaces with
    /// `_` whatever is not a letter, digit or `_`.
    pub fn nome(&self) -> String {
        match self {
            Token::Classe { classe, .. } => classe.clone(),
            Token::Multi { nome, .. } | Token::Opaco { nome, .. } => nome
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect(),
            Token::Elemento => "HtmlElement".into(),
            Token::Detector => "ChangeDetectorRef".into(),
        }
    }

    /// One of the element built-ins (`ElementRef`, `Injector`,
    /// `ViewContainerRef`…), which a service does not receive like the others.
    pub fn embutido(&self) -> bool {
        match self {
            Token::Elemento | Token::Detector => true,
            Token::Classe { uri, classe } => {
                uri.starts_with("package:ngdart/")
                    && matches!(
                        classe.as_str(),
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
pub enum ValorConst {
    Texto(String),
    Inteiro(i64),
    Booleano(bool),
    /// `const C(..)`/`const C.name(..)`, of a class without type parameters.
    Objeto {
        uri: String,
        classe: String,
        construtor: Option<String>,
        posicionais: Vec<ValorConst>,
        nomeados: Vec<(String, ValorConst)>,
    },
}

/// Where the value of a provider comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fornece {
    /// `useExisting:` — outro token.
    Existente(Token),
    /// `useClass:` (or the token itself), with the constructor dependencies.
    Classe {
        uri: String,
        classe: String,
        deps: Vec<Dependencia>,
    },
    /// `useValue:`.
    Valor(ValorConst),
    /// `useFactory:` of a top-level function, with `deps:` (or the parameters).
    Fabrica {
        uri: String,
        nome: String,
        deps: Vec<Dependencia>,
    },
}

/// An item of `providers:`, already flattened (`ModuleReader`,
/// `_normalizeProviders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provedor {
    pub token: Token,
    pub fonte: Fornece,
    pub multi: bool,
    /// The `T` of `Provider<T>` (`inferProviderType`): the field type when it
    /// is not that of the expression itself. `None` when inference finds
    /// nothing (and the field takes the type of the value, or `dynamic`).
    pub tipo: Option<TipoDeToken>,
}

impl Provedor {
    /// `ExistingProvider(token, target)` without an inferred type — the only
    /// shape a template node knows how to create.
    pub fn apelido(token: Token, alvo: Token, multi: bool) -> Self {
        Provedor {
            token,
            fonte: Fornece::Existente(alvo),
            multi,
            tipo: None,
        }
    }
}

/// A positional constructor parameter (`_getCompileDiDependencyMetadata` skips
/// the named ones).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependencia {
    pub token: Token,
    /// `@Optional()` or an optional positional parameter.
    pub opcional: bool,
    /// `@Self()`.
    pub proprio: bool,
    /// `@Host()`.
    pub hospedeiro: bool,
    /// `@SkipSelf()`.
    pub pular: bool,
    /// `@Attribute('name')`: the value of the element's static attribute, or
    /// `null` (`_getLocalDependency`).
    pub atributo: Option<String>,
}

/// An `@Input`: name in the template, member, and whether the type is `bool`
/// (an attribute without a value becomes `true`, `visitEmptyExpr`); `None` when
/// the type is not known from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub nome: String,
    pub membro: String,
    pub booleana: Option<bool>,
}

/// A `@HostListener`: the event, the method and the arguments as
/// `_addHostListener` writes them. Empty `args`: a method without parameters;
/// `$event`: one parameter; other text: a complex handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ouvinte {
    pub evento: String,
    pub metodo: String,
    pub args: String,
}

/// What the generator knows about a `@Directive` (or `@Component`), read from
/// the program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diretiva {
    pub classe: String,
    pub uri: String,
    pub seletor: String,
    pub e_componente: bool,
    pub export_as: Option<String>,
    /// `visibility: Visibility.all`: injectable by whoever is below.
    pub visivel: bool,
    pub provedores: Vec<Provedor>,
    pub dependencias: Vec<Dependencia>,
    /// Na ordem do mapa `inputs` (`_SortInputsVisitor`).
    pub entradas: Vec<Entrada>,
    /// (nome no template, membro), na ordem do mapa `outputs`.
    pub saidas: Vec<(String, String)>,
    /// Na ordem do mapa `hostListeners`.
    pub ouvintes: Vec<Ouvinte>,
    /// `@HostBinding`: (binding name, member).
    pub ligacoes_do_hospedeiro: Vec<(String, String)>,
    /// Some `@HostBinding` on a static member (outside
    /// `ligacoes_do_hospedeiro`): the official compiler writes it once, in the
    /// view constructor — for a component, through its reader (case j96); here,
    /// no case.
    pub hospedeiro_estatico: bool,
    /// The static members among the `ligacoes_do_hospedeiro` (`StaticRead`,
    /// immutable, `dynamic` for the `_TypeResolver`).
    pub estaticos_do_hospedeiro: Vec<String>,
    /// `hostAttributes` of a directive: `@HostBinding` on an immutable static
    /// outside `class.x`/`style.x` (name without `attr.`, member). Outside the
    /// `XNgCd`; the directive's user writes them on the element.
    pub atributos_do_hospedeiro: Vec<(String, String)>,
    pub ganchos: Ganchos,
    /// Some `@ViewChild(ren)` (a directive has no view).
    pub consultas: bool,
    /// The `@ContentChild(ren)`, with the types resolved in the directive's
    /// library: setters, then fields, each group in declaration order.
    pub consultas_de_conteudo: Vec<crate::view::ConsultaDoFilho>,
    /// What the reading could not understand: with anything here, the metadata
    /// is incomplete and the directive is not used.
    pub fora: Vec<String>,
    /// The `viewProviders:` of a component (`ProviderAstType.privateService` on
    /// its element), in order.
    pub provedores_de_visao: Vec<Provedor>,
    /// The constructor dependencies were read completely (`fora` may have
    /// something else, like an unreadable `viewProviders:`, which does not
    /// affect them).
    pub dependencias_lidas: bool,
}

impl Diretiva {
    pub fn token(&self) -> Token {
        Token::Classe {
            uri: self.uri.clone(),
            classe: self.classe.clone(),
        }
    }

    /// All `providers:` are `ExistingProvider`s of a class token or
    /// `MultiToken` — what a template node knows how to create. (The others are
    /// only written by the component's own host view.)
    pub fn so_apelidos(&self) -> bool {
        self.provedores.iter().all(|p| {
            matches!(&p.fonte, Fornece::Existente(Token::Classe { .. }))
                && matches!(p.token, Token::Classe { .. } | Token::Multi { .. })
        })
    }

    pub fn entrada(&self, nome: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.nome == nome)
    }

    pub fn saida(&self, nome: &str) -> Option<&str> {
        self.saidas
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, m)| m.as_str())
    }

    /// Why the emitter does not instantiate this directive on an HTML element
    /// yet, or `None` if it does. Each item is a shape without a case in the
    /// corpus: generating while ignoring it would give wrong output.
    pub fn pendencia(&self) -> Option<String> {
        if let Some(f) = self.fora.first() {
            return Some(f.clone());
        }
        if !self.atributos_do_hospedeiro.is_empty() {
            return Some("directive hostAttributes (merged into the element)".into());
        }
        if self.e_componente {
            return Some("component as a directive".into());
        }

        if self.consultas {
            return Some("content or view query".into());
        }
        if self
            .ouvintes
            .iter()
            .any(|o| !crate::view::evento_nativo(&o.evento))
        {
            return Some("@HostListener of a non-native event".into());
        }
        for d in &self.dependencias {
            if let Token::Multi { tipo, .. } = &d.token
                && !tipo.e_object()
                && tipo.genericos == 0
            {
                return Some("MultiToken of a non-generic type".into());
            }
        }
        // `OpaqueToken<T>` with a generic `T`: the shape of `T` in the token
        // expression (`createDiTokenExpression`) has no case yet.
        if self
            .dependencias
            .iter()
            .any(|d| matches!(&d.token, Token::Opaco { tipo, .. } if tipo.genericos > 0))
        {
            return Some("dependency on an OpaqueToken of a generic type".into());
        }
        for p in &self.provedores {
            if let Token::Multi { tipo, .. } = &p.token
                && !tipo.e_object()
                && tipo.genericos == 0
            {
                return Some("MultiToken of a non-generic type".into());
            }
        }
        None
    }
}

/// An argument of a directive's constructor, already resolved on the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argumento {
    /// The node (`_el_3` or `this._el_3`).
    Elemento,
    /// The view (`this`).
    Detector,
    /// `@Optional() @Self()` without a provider on the node.
    Nulo,
    /// The field of another provider of the node.
    Campo(String),
    /// A provider of an element above (`_getDependency` walks up through the
    /// parents): the expression that reads it from this view.
    Acima(String),
    /// `ViewContainerRef`: the node's `ViewContainer` (`this._appEl_n`).
    Container,
    /// `@Attribute('name')`: the literal of the element's static attribute, or
    /// `null`.
    Atributo(String),
    /// `Injector`: the element's own injector, `this.injector(n)` (one of the
    /// built-ins of `CompileElement`, like `ElementRef`).
    Injetor(u32),
    /// No element of the chain provides it: the injector from outside the view
    /// (`injectFromViewParentInjector`), `injectorGetOptional` with
    /// `@Optional()`.
    DeFora { token: Token, opcional: bool },
}

/// An injectable provider of an element above the node, in its view or in an
/// ancestor view (the `parentView` chain up to it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvedorAcima {
    pub token: Token,
    /// The expression that reads it from the view the node is in.
    pub leitura: String,
    /// A lazy provider of the owning element: requested by a node below, the
    /// official compiler transforms it during the child's visit, before the
    /// owner's `afterElement` (`provider_parser.dart:318-366`), which changes
    /// its index and shape there — not modeled yet (gap L1).
    pub preguicoso: bool,
    /// Seen from another view (it crossed a `<template>`): `_getDependency`
    /// asks with `eager: false` (`_isViewRoot`), the provider stays lazy and is
    /// read through the field — when the owner modeled that request
    /// ([`resolver_no_do_filho`]).
    pub leitura_preguicosa: bool,
}

/// What there is above the node for the dependencies it does not satisfy.
#[derive(Debug, Clone, Copy)]
pub struct Acima<'a> {
    /// From the closest to the farthest.
    pub provedores: &'a [ProvedorAcima],
    /// Some element above has a provider the emitter does not model (a
    /// component): not finding it does not prove there is none.
    pub incerto: bool,
    /// The tokens of the `viewProviders:` of this view's component: an
    /// `@Host()` that asks for one of them goes to the outside injector
    /// (`provider_parser.dart:347-351`).
    pub de_visao: &'a [Token],
}

/// How a provider field is created in `build()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Criacao {
    /// `Classe(args)`.
    Diretiva {
        diretiva: Arc<Diretiva>,
        args: Vec<Argumento>,
    },
    /// `[a, b]`: the fields the multi-provider joins.
    Lista(Vec<String>),
    /// Only in the host view: a `providers:` provider that is neither a
    /// directive nor a local alias (`ClassProviderSource`,
    /// `FactoryProviderSource`, `ExpressionProviderSource` and the
    /// `injectorGet` of an outside alias).
    Expressao(Expr),
    /// Only in the host view: the multi-provider with items of any shape.
    Multi(Vec<Expr>),
}

/// The value of a host view provider, as `ProviderSource.build` writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// `this.field` — another provider of the node (`component` is the
    /// component).
    Campo(String),
    /// A provider of an element above, in a template node: the expression that
    /// reads it from this view (`_getDependency`).
    Leitura(String),
    /// `this.injectorGet(token, this.parentIndex)` (or `injectorGetOptional`):
    /// what the node does not provide comes from the outside injector.
    Injetor {
        token: Token,
        opcional: bool,
    },
    /// `Classe(args)`.
    Classe {
        uri: String,
        classe: String,
        args: Vec<Expr>,
    },
    /// `funcao(args)`.
    Fabrica {
        uri: String,
        nome: String,
        args: Vec<Expr>,
    },
    Valor(ValorConst),
}

impl Expr {
    /// Some dependency comes from the injector (`hasDynamicDependencies`): the
    /// creation comes out wrapped in `debugInjectorWrap`.
    pub fn dinamica(&self) -> bool {
        match self {
            Expr::Classe { args, .. } | Expr::Fabrica { args, .. } => {
                args.iter().any(|a| matches!(a, Expr::Injetor { .. }))
            }
            _ => false,
        }
    }
}

/// A provider of the node that becomes a field of the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instancia {
    pub token: Token,
    pub campo: String,
    pub criacao: Criacao,
    /// The tokens through which it is injectable below (`injectorGetInternal`):
    /// its own, if visible, and the aliases.
    pub injetavel_por: Vec<Token>,
    /// The node's `ExistingProvider`s that point to this instance, visible or
    /// not: a query also finds it through them.
    pub apelidos: Vec<Token>,
    /// No eager provider (directive, component) depends on it: the official
    /// compiler creates it in `afterElement` with `eager: false`, as a `late`
    /// field with an initializer (`late List<T> _X_n_m = [..];`), and not in
    /// `build()`.
    pub preguicosa: bool,
    /// How the instance is read: the field, or `field.instance` when the
    /// directive has `@HostBinding` and the field holds its `XNgCd`.
    pub leitura: String,
    /// O `T` inferido do provedor (o `typeArgument`), que tipa o campo.
    pub tipo: Option<TipoDeToken>,
}

/// The directive providers of a node, resolved.
#[derive(Debug, Clone, Default)]
pub struct NoResolvido {
    /// The fields, in creation order.
    pub instancias: Vec<Instancia>,
    /// The directives, in the order the official compiler binds them
    /// (`transformedDirectiveAsts`: that of the providers), with how each
    /// instance is read (the field, or `field.instance` with `XNgCd`).
    pub diretivas: Vec<(Arc<Diretiva>, String)>,
    /// The node has a `ViewContainer` (`requiresViewContainer`): a directive or
    /// its component injects `ViewContainerRef` (cases j47–j49).
    pub container: bool,
}

#[derive(Debug)]
enum Fonte {
    Diretiva(Arc<Diretiva>),
    Existente(Token),
    /// `useClass`/`useValue`/`useFactory` de `providers:`.
    Provedor(Fornece),
}

#[derive(Debug)]
struct Resolvido {
    token: Token,
    fontes: Vec<Fonte>,
    multi: bool,
    eager: bool,
    visivel: bool,
    tipo: Option<TipoDeToken>,
    /// It came from `viewProviders:` (`ProviderAstType.privateService`): only
    /// the component and another private provider of the node find it locally
    /// (`_getOrCreateLocalProvider`); directives and public services, of the
    /// node or from below, go up to the injector.
    privado: bool,
}

/// Resolves the providers of the `casadas` directives (in `directives:` order)
/// on node `n`. A dependency the node itself does not satisfy and that is not
/// `@Optional() @Self()` is refused: it would come from another node or from
/// the injector, shapes without a case yet.
pub fn resolver(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
) -> Result<NoResolvido, &'static str> {
    resolver_em(casadas, n, acima, false)
}

/// [`resolver`] with the tokens that the view queries read from the node
/// (`read:`), which the official compiler creates eagerly (`queriedTokens`).
pub fn resolver_consultado(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
    consultados: &[Token],
) -> Result<NoResolvido, &'static str> {
    resolver_com(
        casadas,
        n,
        acima,
        false,
        None,
        false,
        None,
        consultados,
        &[],
    )
}

/// The providers of a written `<template>` (`EmbeddedTemplateAst`): the
/// built-ins (`ElementRef`, `Element`, `HtmlElement`, `Injector`,
/// `ViewContainer`; `ViewContainerRef` if some directive asks for it;
/// `ChangeDetectorRef`, `ComponentLoader`), the `TemplateRef` — the first of
/// the resolved ones (`setEmbeddedView`), at the next index — and the
/// directives after it. `template_ref` gives how the node's `TemplateRef` is
/// read through its index, which is returned too (case j85).
pub fn resolver_de_molde(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
    // A query reads a `#ref` of the node as `ViewContainerRef`
    // (`_requiresViewContainer`, `provider_parser.dart:95-104`).
    forcar_container: bool,
    template_ref: &dyn Fn(u32) -> String,
) -> Result<(NoResolvido, u32), &'static str> {
    let container = forcar_container || casadas.iter().any(|d| pede_container(d));
    let indice = crate::dialect::embutidos_extra() + if container { 8 } else { 7 };
    let leitura = template_ref(indice);
    let r = resolver_com(
        casadas,
        n,
        acima,
        false,
        None,
        container,
        Some((&leitura, indice)),
        &[],
        &[],
    )?;
    Ok((r, indice))
}

/// O `TemplateRef` do ngdart.
pub fn e_template_ref(t: &Token) -> bool {
    matches!(t, Token::Classe { uri, classe }
        if uri == "package:ngdart/src/core/linker/template_ref.dart" && classe == "TemplateRef")
}

/// The providers of the host view's node (`_ViewXHost0`, node 0): the component
/// and its `providers:`. What the node does not provide comes from the outside
/// injector (`injectFromViewParentInjector`), and the component instance is the
/// `component` field of the `HostView` (`hostViewComponentFieldName`).
pub fn resolver_hospedeira(componente: Arc<Diretiva>) -> Result<NoResolvido, &'static str> {
    resolver_em(&[componente], 0, None, true)
}

/// The providers of the node of a child component in a template: the child
/// (`casadas[0]`, without the constructor dependencies, which are resolved
/// separately) with its `providers:` of any shape, and the node's directives
/// (`casadas[1..]`), which still only have `ExistingProvider`. The child's
/// providers are written as in the host view (`ProviderSource.build`), but a
/// dependency the node does not satisfy would go to the elements above or to
/// the outside injector (`parentView.injectorGet`): with no case yet, it is
/// refused.
///
/// `container`: the child injects `ViewContainerRef` and the node gets a
/// `ViewContainer`, with three more built-ins (`ViewContainer`,
/// `ViewContainerRef`, `ComponentLoader`) before the providers (case j47).
///
/// `pedidos`: the tokens the content nodes request from this node during their
/// visit (`_getDependency` walks up and finds them here), in order, which the
/// official compiler transforms eagerly before this node's `afterElement` (case
/// i76).
pub fn resolver_no_do_filho(
    casadas: &[Arc<Diretiva>],
    filho: usize,
    n: u32,
    acima: Option<Acima>,
    container: bool,
    pedidos: &[(Token, bool)],
) -> Result<NoResolvido, &'static str> {
    resolver_com(
        casadas,
        n,
        acima,
        false,
        Some(filho),
        container,
        None,
        &[],
        pedidos,
    )
}

fn resolver_em(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
    hospedeira: bool,
) -> Result<NoResolvido, &'static str> {
    resolver_com(casadas, n, acima, hospedeira, None, false, None, &[], &[])
}

/// `componente`: the index of the child component in `casadas` (on a child's
/// node), which has its `providers:` of any shape written, like all of them in
/// the host view; the others only with `ExistingProvider`.
#[allow(clippy::too_many_arguments)]
fn resolver_com(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
    hospedeira: bool,
    componente: Option<usize>,
    container: bool,
    molde: Option<(&str, u32)>,
    consultados: &[Token],
    pedidos: &[(Token, bool)],
) -> Result<NoResolvido, &'static str> {
    let completa = |k: usize| hospedeira || componente == Some(k);
    // `requiresViewContainer`: with a `ViewContainer` the node gets three
    // built-ins (`ViewContainer`, `ViewContainerRef`, `ComponentLoader`) before
    // the providers, and the fields start at 8.
    let container = container || casadas.iter().any(|d| pede_container(d));
    // `_ProviderResolver.resolve`: the directives (eager), then the
    // `providers:` of each; the same multi token accumulates.
    let mut todos: Vec<Resolvido> = Vec::new();
    for d in casadas {
        todos.push(Resolvido {
            token: d.token(),
            fontes: vec![Fonte::Diretiva(d.clone())],
            multi: false,
            eager: true,
            visivel: d.visivel,
            tipo: None,
            privado: false,
        });
    }
    // The `providers:` with the component before the directives ("directives
    // are able to overwrite providers of a component").
    let em_ordem = componente
        .into_iter()
        .chain((0..casadas.len()).filter(|k| Some(*k) != componente));
    for k in em_ordem {
        let d = &casadas[k];
        // `viewProviders:` only for a component (the node of a child with child
        // nodes, which would put them in a separate `ProviderNode`, is refused
        // earlier).
        if !d.provedores_de_visao.is_empty() && !completa(k) {
            return Err("directive viewProviders");
        }
        let publicos = d.provedores.iter().map(|p| (p, false));
        let privados = d.provedores_de_visao.iter().map(|p| (p, true));
        for (p, privado) in publicos.chain(privados) {
            let fonte = match &p.fonte {
                Fornece::Existente(t) => Fonte::Existente(t.clone()),
                f => Fonte::Provedor(f.clone()),
            };
            match todos.iter_mut().find(|r| r.token == p.token) {
                Some(r) => {
                    if r.privado != privado {
                        // The `providerType` of the one that stays: no case
                        // yet.
                        return Err("public and private provider on the same token");
                    }
                    if r.multi != p.multi {
                        return Err("multi and non-multi provider on the same token");
                    }
                    if r.eager && (completa(k) || matches!(fonte, Fonte::Provedor(_))) {
                        // The component's token overwritten by `providers:`: no
                        // case yet.
                        return Err("provider with the component's token");
                    }
                    if !p.multi {
                        r.fontes.clear();
                        r.tipo = p.tipo.clone();
                    }
                    r.fontes.push(fonte);
                }
                None => todos.push(Resolvido {
                    token: p.token.clone(),
                    fontes: vec![fonte],
                    multi: p.multi,
                    eager: false,
                    visivel: true,
                    tipo: p.tipo.clone(),
                    privado,
                }),
            }
        }
    }
    // What a query reads from the node (`queriedTokens`: the `read:` of each
    // query that matches it) is eager too.
    for r in &mut todos {
        if consultados.contains(&r.token) {
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
    fn local(dep: &Dependencia) -> bool {
        !dep.pular && dep.atributo.is_none() && !dep.token.embutido()
    }
    /// Whoever asks as `component` or `privateService` finds the private ones.
    fn ve_privados(r: &Resolvido) -> bool {
        r.privado || matches!(r.fontes.as_slice(), [Fonte::Diretiva(d)] if d.e_componente)
    }
    fn criar(
        todos: &[Resolvido],
        i: usize,
        ordem: &mut Vec<usize>,
        vistos: &mut Vec<usize>,
        hospedeira: bool,
    ) -> Result<(), &'static str> {
        if ordem.contains(&i) {
            return Ok(());
        }
        if vistos.contains(&i) {
            return Err("cyclic dependency between directives");
        }
        vistos.push(i);
        let privado = ve_privados(&todos[i]);
        let pedir = |t: &Token, ordem: &mut Vec<usize>, vistos: &mut Vec<usize>| match todos
            .iter()
            .position(|r| r.token == *t && (privado || !r.privado))
        {
            Some(j) => criar(todos, j, ordem, vistos, hospedeira),
            None => Ok(()),
        };
        for f in &todos[i].fontes {
            match f {
                Fonte::Existente(t) => {
                    // An alias from outside the node reads the injector (host
                    // view) or the element above / the outside injector
                    // (template node).
                    if let Some(j) = todos
                        .iter()
                        .position(|r| r.token == *t && (privado || !r.privado))
                    {
                        criar(todos, j, ordem, vistos, hospedeira)?;
                    }
                }
                Fonte::Diretiva(d) => {
                    for dep in d.dependencias.iter().filter(|d| local(d)) {
                        pedir(&dep.token, ordem, vistos)?;
                    }
                }
                Fonte::Provedor(Fornece::Classe { deps, .. } | Fornece::Fabrica { deps, .. }) => {
                    for dep in deps.iter().filter(|d| local(d)) {
                        pedir(&dep.token, ordem, vistos)?;
                    }
                }
                Fonte::Provedor(_) => {}
            }
        }
        ordem.push(i);
        Ok(())
    }
    let mut ordem = Vec::new();
    let mut vistos = Vec::new();
    for i in 0..todos.len() {
        if todos[i].eager {
            criar(&todos, i, &mut ordem, &mut vistos, hospedeira)?;
        }
    }
    // The eager ones so far; the following requests mark those they create.
    let mut ansiosos_idx: Vec<usize> = ordem.clone();
    // The requests of the nodes below, during their visit (before this one's
    // `afterElement`): each one transformed right away, with the dependencies
    // first (`_getLocalDependency` with the `eager` of the requester); one that
    // crossed a `<template>` asks with `eager: false` and stays lazy, but in
    // the position of the visit. Once transformed, it stays as it is.
    for (t, ansioso) in pedidos {
        if let Some(i) = todos.iter().position(|r| r.token == *t) {
            let antes = ordem.len();
            criar(&todos, i, &mut ordem, &mut vistos, hospedeira)?;
            if *ansioso {
                ansiosos_idx.extend_from_slice(&ordem[antes..]);
            }
        }
    }
    // `afterElement`: what is left (the aliases, in general).
    for i in 0..todos.len() {
        criar(&todos, i, &mut ordem, &mut vistos, hospedeira)?;
    }

    // `addDirectiveProviders`: the `uniqueId` is the size of the table, which
    // already has the five element built-ins. In a `<template>`, the directives
    // come after the `TemplateRef`.
    let mut tamanho = match molde {
        Some((_, indice)) => indice + 1,
        None if container => 8 + crate::dialect::embutidos_extra(),
        None => 5 + crate::dialect::embutidos_extra(),
    };
    let mut campos: Vec<(Token, String)> = Vec::new();
    let mut apelidos: Vec<(Token, Token)> = Vec::new();
    let mut saida = NoResolvido {
        container,
        ..NoResolvido::default()
    };
    for (posicao, &i) in ordem.iter().enumerate() {
        let r = &todos[i];
        let _ = posicao;
        let preguicosa = !ansiosos_idx.contains(&i);
        if let (false, [Fonte::Existente(alvo)]) = (r.multi, r.fontes.as_slice())
            && let Some(real) = campos
                .iter()
                .find(|(t, _)| t == alvo)
                .map(|_| alvo.clone())
                .or_else(|| {
                    apelidos
                        .iter()
                        .find(|(t, _)| t == alvo)
                        .map(|(_, a)| a.clone())
                })
        {
            apelidos.push((r.token.clone(), real));
            tamanho += 1;
            continue;
        }
        let componente_da_hospedeira =
            hospedeira && matches!(r.fontes.as_slice(), [Fonte::Diretiva(d)] if d.e_componente);
        let campo = if componente_da_hospedeira {
            "component".to_string()
        } else {
            format!("_{}_{n}_{tamanho}", r.token.nome())
        };
        // Directive with `@HostBinding`: the field is the `XNgCd` that wraps it
        // (`createProvider`, `providerHasChangeDetector`). Not for a component:
        // its `@HostBinding` is the `detectHostChanges` of its view, and the
        // field is the instance (case j30).
        let leitura = match r.fontes.as_slice() {
            [Fonte::Diretiva(d)]
                if !d.ligacoes_do_hospedeiro.is_empty()
                    && !d.e_componente
                    && !componente_da_hospedeira =>
            {
                format!("{campo}.instance")
            }
            _ => campo.clone(),
        };
        let campo_de = |t: &Token| -> Option<String> {
            // A private provider is only seen by another private one.
            if !ve_privados(r) && todos.iter().any(|x| x.privado && x.token == *t) {
                return None;
            }
            let t = apelidos
                .iter()
                .find(|(a, _)| a == t)
                .map(|(_, real)| real)
                .unwrap_or(t);
            campos.iter().find(|(x, _)| x == t).map(|(_, c)| c.clone())
        };
        // A service dependency (`_getDependency` in the host view): the local
        // field, or the outside injector.
        // What the node does not provide: in the host view, the injector; in a
        // template node, the element above that provides it (`_getDependency`)
        // or, without one, the injector from outside the view (case j71).
        let de_fora = |t: &Token, opcional: bool| -> Result<Expr, &'static str> {
            if !hospedeira && let Some(a) = acima {
                if let Some(p) = a.provedores.iter().find(|p| p.token == *t) {
                    if p.preguicoso {
                        return Err("lazy provider of an element above requested below (L1)");
                    }
                    return Ok(Expr::Leitura(p.leitura.clone()));
                }
                if a.incerto {
                    return Err("provider dependency under a component without metadata");
                }
            }
            Ok(Expr::Injetor {
                token: t.clone(),
                opcional,
            })
        };
        let dependencia = |dep: &Dependencia| -> Result<Expr, &'static str> {
            if dep.proprio || dep.hospedeiro {
                return Err("@Self/@Host dependency of a provider");
            }
            if dep.atributo.is_some() {
                return Err("@Attribute dependency of a provider");
            }
            if dep.token.embutido() {
                return Err("provider that depends on an element built-in");
            }
            // `@SkipSelf`: the node is not consulted
            // (`provider_resolver.dart:189`); it walks up through the elements
            // above and, without them, the outside injector.
            if dep.pular {
                return de_fora(&dep.token, dep.opcional);
            }
            match campo_de(&dep.token) {
                Some(c) => Ok(Expr::Campo(c)),
                None => de_fora(&dep.token, dep.opcional),
            }
        };
        let expr_de = |f: &Fornece| -> Result<Expr, &'static str> {
            Ok(match f {
                Fornece::Existente(t) => match campo_de(t) {
                    Some(c) => Expr::Campo(c),
                    None if t.embutido() => return Err("alias of an element built-in"),
                    None => de_fora(t, false)?,
                },
                Fornece::Classe { uri, classe, deps } => Expr::Classe {
                    uri: uri.clone(),
                    classe: classe.clone(),
                    args: deps.iter().map(dependencia).collect::<Result<_, _>>()?,
                },
                Fornece::Fabrica { uri, nome, deps } => Expr::Fabrica {
                    uri: uri.clone(),
                    nome: nome.clone(),
                    args: deps.iter().map(dependencia).collect::<Result<_, _>>()?,
                },
                Fornece::Valor(v) => Expr::Valor(v.clone()),
            })
        };
        let criacao = match r.fontes.as_slice() {
            [Fonte::Diretiva(d)] => {
                let mut args = Vec::new();
                for dep in &d.dependencias {
                    if let Some(nome) = &dep.atributo {
                        args.push(Argumento::Atributo(nome.clone()));
                        continue;
                    }
                    args.push(match &dep.token {
                        Token::Elemento | Token::Detector if dep.pular => {
                            return Err("@SkipSelf dependency on an element built-in");
                        }
                        Token::Elemento => Argumento::Elemento,
                        Token::Detector => Argumento::Detector,
                        t if e_view_container_ref(t) && !dep.pular => Argumento::Container,
                        // The `TemplateRef` of the `<template>` itself.
                        t if e_template_ref(t) && !dep.pular && molde.is_some() => {
                            Argumento::Acima(molde.map(|(l, _)| l.to_string()).unwrap_or_default())
                        }
                        Token::Classe { uri, classe }
                            if uri == INJECTOR && classe == "Injector" =>
                        {
                            // `@SkipSelf()` would read the injector of the
                            // element above (`injector(parent)`): no case yet.
                            if dep.pular {
                                return Err("@SkipSelf dependency on Injector");
                            }
                            Argumento::Injetor(n)
                        }
                        // `@SkipSelf()`: `_getDependency` starts at the parent
                        // (the `ControlContainer` of `NgControlName` is the
                        // `NgForm` of the `<form>` above).
                        t => match campo_de(t).filter(|_| !dep.pular) {
                            Some(c) => Argumento::Campo(c),
                            None => fora_do_no(dep, acima)?,
                        },
                    });
                }
                if !componente_da_hospedeira {
                    saida.diretivas.push((d.clone(), leitura.clone()));
                }
                Criacao::Diretiva {
                    diretiva: d.clone(),
                    args,
                }
            }
            fontes
                if r.multi
                    && (hospedeira || fontes.iter().any(|f| matches!(f, Fonte::Provedor(_)))) =>
            {
                let mut itens = Vec::new();
                for f in fontes {
                    itens.push(match f {
                        Fonte::Existente(t) => expr_de(&Fornece::Existente(t.clone()))?,
                        Fonte::Provedor(p) => expr_de(p)?,
                        Fonte::Diretiva(_) => return Err("directive multi-provider"),
                    });
                }
                Criacao::Multi(itens)
            }
            fontes if r.multi => {
                let mut itens = Vec::new();
                for f in fontes {
                    let Fonte::Existente(t) = f else {
                        return Err("multi-provider that is not an alias");
                    };
                    itens.push(campo_de(t).ok_or("multi-provider of a token from outside the node")?);
                }
                Criacao::Lista(itens)
            }
            // An alias of a token the node does not provide: the injector (host
            // view) or the element above / the outside injector (template
            // node).
            [Fonte::Existente(t)] => Criacao::Expressao(expr_de(&Fornece::Existente(t.clone()))?),
            [Fonte::Provedor(p)] => Criacao::Expressao(expr_de(p)?),
            _ => return Err("alias provider of a token from outside the node"),
        };
        saida.instancias.push(Instancia {
            token: r.token.clone(),
            campo: campo.clone(),
            criacao,
            injetavel_por: if r.visivel {
                vec![r.token.clone()]
            } else {
                Vec::new()
            },
            apelidos: Vec::new(),
            preguicosa,
            leitura: leitura.clone(),
            tipo: r.tipo.clone(),
        });
        campos.push((r.token.clone(), leitura));
        tamanho += 1;
    }
    for (apelido, real) in apelidos {
        if let Some(inst) = saida.instancias.iter_mut().find(|x| x.token == real) {
            inst.apelidos.push(apelido.clone());
            inst.injetavel_por.push(apelido);
        }
    }
    Ok(saida)
}

/// ngdart's `ViewContainerRef`, or `ComponentLoader`, which is an alias of the
/// same `ViewContainer` and also turns on `_requiresViewContainer`
/// (`provider_parser.dart:290-295`, `compile_element.dart:204-206`).
pub fn e_view_container_ref(t: &Token) -> bool {
    matches!(t, Token::Classe { uri, classe }
        if uri.starts_with("package:ngdart/")
            && matches!(classe.as_str(), "ViewContainerRef" | "ComponentLoader"))
}

/// The directive injects `ViewContainerRef` and its node gets a `ViewContainer`
/// (`_requiresViewContainer`, in `provider_parser.dart`).
pub fn pede_container(d: &Diretiva) -> bool {
    d.dependencias
        .iter()
        .any(|dep| e_view_container_ref(&dep.token))
}

/// The built-in providers of an element besides `Element`/`HtmlElement`,
/// `Injector` and `ChangeDetectorRef` (`CompileElement`): `ElementRef`, and
/// with a `ViewContainer` also `ViewContainer`, `ViewContainerRef`,
/// `ComponentLoader` and `TemplateRef`.
pub fn embutido_do_elemento(t: &Token) -> bool {
    matches!(t, Token::Classe { uri, classe }
    if uri.starts_with("package:ngdart/")
        && matches!(
            classe.as_str(),
            "ElementRef" | "ViewContainerRef" | "ViewContainer" | "ComponentLoader" | "TemplateRef"
        ))
}

/// A dependency the node itself does not satisfy (`_getDependency`): `@Self`
/// stops at the node; otherwise it walks up through the elements above (`@Host`
/// up to the host, which here is the root of the component's view). Without a
/// result and with `@Optional`, `null` — unless there is something above that
/// the emitter does not model. Without `@Self` or `@Host`, after the elements
/// comes the injector from outside the view (case j40).
fn fora_do_no(dep: &Dependencia, acima: Option<Acima>) -> Result<Argumento, &'static str> {
    // Without context (only the order of the imports matters): the value does
    // not matter.
    let Some(acima) = acima else {
        return Ok(Argumento::Nulo);
    };
    if !dep.proprio
        && let Some(p) = acima.provedores.iter().find(|p| p.token == dep.token)
    {
        if p.preguicoso && !p.leitura_preguicosa {
            return Err("lazy provider of an element above requested below (L1)");
        }
        return Ok(Argumento::Acima(p.leitura.clone()));
    }
    // An element built-in (`ElementRef`, `ViewContainerRef`, `TemplateRef`…) is
    // never null and never comes from the outside injector: the official
    // compiler finds it on the node itself or on one above (`CompileElement`),
    // a shape without a case yet.
    if embutido_do_elemento(&dep.token) {
        return Err(
            "dependency on an element built-in (ElementRef/ViewContainerRef/TemplateRef)",
        );
    }
    match (dep.opcional, dep.proprio, dep.hospedeiro) {
        (true, true, _) => Ok(Argumento::Nulo),
        (_, false, true) if !acima.incerto && acima.de_visao.contains(&dep.token) => {
            Ok(Argumento::DeFora {
                token: dep.token.clone(),
                opcional: dep.opcional,
            })
        }
        (true, false, true) if !acima.incerto => Ok(Argumento::Nulo),
        (_, false, true) => Err("@Host dependency of a directive without a provider above"),
        // Some element above has a provider the emitter does not model: it
        // could be what the official compiler finds before the outside
        // injector.
        (_, false, false) if !acima.incerto => Ok(Argumento::DeFora {
            token: dep.token.clone(),
            opcional: dep.opcional,
        }),
        _ => Err("dependency on a directive from outside the node"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn classe(uri: &str, c: &str) -> Token {
        Token::Classe {
            uri: uri.into(),
            classe: c.into(),
        }
    }

    fn validadores() -> Token {
        Token::Multi {
            nome: "NgValidators".into(),
            tipo: TipoDeToken {
                uri: "dart:core".into(),
                classe: "Object".into(),
                genericos: 0,
                args: Vec::new(),
            },
        }
    }

    fn acessores() -> Token {
        Token::Multi {
            nome: "NgValueAccessor".into(),
            tipo: TipoDeToken {
                uri: "cva".into(),
                classe: "ControlValueAccessor".into(),
                genericos: 1,
                args: Vec::new(),
            },
        }
    }

    fn dep(token: Token, opcional: bool, proprio: bool) -> Dependencia {
        Dependencia {
            token,
            opcional,
            proprio,
            hospedeiro: false,
            pular: false,
            atributo: None,
        }
    }

    /// Test directives with the shape that `metadata.rs` reads from `ngforms`.
    fn ng_model() -> Arc<Diretiva> {
        Arc::new(Diretiva {
            classe: "NgModel".into(),
            uri: "m".into(),
            visivel: true,
            provedores: vec![Provedor::apelido(
                classe("c", "NgControl"),
                classe("m", "NgModel"),
                false,
            )],
            dependencias: vec![dep(validadores(), true, true), dep(acessores(), true, true)],
            ..Default::default()
        })
    }

    fn acessor() -> Arc<Diretiva> {
        Arc::new(Diretiva {
            classe: "DefaultValueAccessor".into(),
            uri: "d".into(),
            provedores: vec![Provedor::apelido(
                acessores(),
                classe("d", "DefaultValueAccessor"),
                true,
            )],
            dependencias: vec![dep(Token::Elemento, false, false)],
            ..Default::default()
        })
    }

    fn obrigatorio() -> Arc<Diretiva> {
        Arc::new(Diretiva {
            classe: "RequiredValidator".into(),
            uri: "v".into(),
            provedores: vec![Provedor::apelido(
                validadores(),
                classe("v", "RequiredValidator"),
                true,
            )],
            ..Default::default()
        })
    }

    /// The order and numbering of `alterar_senha_page.template.dart`: the
    /// `RequiredValidator` before the multi that joins it, the accessor before
    /// its multi, `NgModel` last; `NgControl` is an alias.
    #[test]
    fn input_com_ng_model_e_required() {
        let modelo = ng_model();
        let casadas = [modelo.clone(), acessor(), obrigatorio()];
        let r = resolver(&casadas, 33, None).unwrap();
        let campos: Vec<&str> = r.instancias.iter().map(|i| i.campo.as_str()).collect();
        assert_eq!(
            campos,
            [
                "_RequiredValidator_33_5",
                "_NgValidators_33_6",
                "_DefaultValueAccessor_33_7",
                "_NgValueAccessor_33_8",
                "_NgModel_33_9"
            ]
        );
        let ng_model = &r.instancias[4];
        assert_eq!(
            ng_model.criacao,
            Criacao::Diretiva {
                diretiva: modelo,
                args: vec![
                    Argumento::Campo("_NgValidators_33_6".into()),
                    Argumento::Campo("_NgValueAccessor_33_8".into())
                ]
            }
        );
        assert_eq!(ng_model.injetavel_por.len(), 2);
        assert!(r.instancias[0].injetavel_por.is_empty());
        assert_eq!(r.instancias[1].injetavel_por, vec![validadores()]);
    }

    #[test]
    fn form_com_ng_form() {
        let form = Arc::new(Diretiva {
            classe: "NgForm".into(),
            uri: "f".into(),
            visivel: true,
            provedores: vec![Provedor::apelido(
                classe("cc", "ControlContainer"),
                classe("f", "NgForm"),
                false,
            )],
            dependencias: vec![
                dep(validadores(), true, true),
                dep(Token::Detector, false, false),
            ],
            ..Default::default()
        });
        let r = resolver(std::slice::from_ref(&form), 19, None).unwrap();
        assert_eq!(r.instancias.len(), 1);
        assert_eq!(r.instancias[0].campo, "_NgForm_19_5");
        assert_eq!(
            r.instancias[0].criacao,
            Criacao::Diretiva {
                diretiva: form,
                args: vec![Argumento::Nulo, Argumento::Detector]
            }
        );
        assert_eq!(r.instancias[0].injetavel_por.len(), 2);
    }

    fn servico(c: &str, deps: &[&str]) -> Provedor {
        Provedor {
            token: classe("s", c),
            fonte: Fornece::Classe {
                uri: "s".into(),
                classe: c.into(),
                deps: deps
                    .iter()
                    .map(|d| dep(classe("s", d), false, false))
                    .collect(),
            },
            multi: false,
            tipo: None,
        }
    }

    /// The numbering of `i66_provider_dependencias.template.dart`: what the
    /// component asks for (and what that asks for) comes before it, eager; the
    /// rest stays lazy, afterwards, in `providers:` order.
    #[test]
    fn hospedeira_com_provedores_ansiosos_e_preguicosos() {
        let comp = Arc::new(Diretiva {
            classe: "Comp".into(),
            uri: "s".into(),
            e_componente: true,
            provedores: vec![
                servico("Cache", &["Repo"]),
                servico("Solto", &[]),
                servico("Repo", &["Api"]),
                servico("Api", &[]),
                Provedor::apelido(classe("s", "Base"), classe("s", "Comp"), false),
            ],
            dependencias: vec![dep(classe("s", "Repo"), false, false)],
            ..Default::default()
        });
        let r = resolver_hospedeira(comp).unwrap();
        let campos: Vec<(&str, bool)> = r
            .instancias
            .iter()
            .map(|i| (i.campo.as_str(), i.preguicosa))
            .collect();
        assert_eq!(
            campos,
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
        assert_eq!(r.instancias[2].injetavel_por, vec![classe("s", "Base")]);
        assert_eq!(
            r.instancias[3].criacao,
            Criacao::Expressao(Expr::Classe {
                uri: "s".into(),
                classe: "Cache".into(),
                args: vec![Expr::Campo("_Repo_0_6".into())],
            })
        );
        assert!(r.diretivas.is_empty());
    }

    /// In a template node, the `providers:` of a directive of any shape (case
    /// j71): what it injects comes before it, eager; the rest stays lazy; the
    /// dependency the node does not provide comes from an element above or,
    /// without one, from the outside injector.
    #[test]
    fn no_de_template_com_provedores_de_classe() {
        let dir = Arc::new(Diretiva {
            classe: "Espiao".into(),
            uri: "s".into(),
            dependencias: vec![dep(classe("s", "Servico"), false, false)],
            provedores: vec![
                servico("Servico", &["Grupo", "Config"]),
                servico("Solto", &[]),
            ],
            ..Default::default()
        });
        let acima = [ProvedorAcima {
            token: classe("s", "Grupo"),
            leitura: "this._Grupo_0_5".into(),
            preguicoso: false,
            leitura_preguicosa: false,
        }];
        let r = resolver(
            &[dir],
            3,
            Some(Acima {
                provedores: &acima,
                incerto: false,
                de_visao: &[],
            }),
        )
        .unwrap();
        let campos: Vec<(&str, bool)> = r
            .instancias
            .iter()
            .map(|i| (i.campo.as_str(), i.preguicosa))
            .collect();
        assert_eq!(
            campos,
            [
                ("_Servico_3_5", false),
                ("_Espiao_3_6", false),
                ("_Solto_3_7", true)
            ]
        );
        let Criacao::Expressao(Expr::Classe { args, .. }) = &r.instancias[0].criacao else {
            panic!("service without creation by class");
        };
        assert_eq!(
            args,
            &[
                Expr::Leitura("this._Grupo_0_5".into()),
                Expr::Injetor {
                    token: classe("s", "Config"),
                    opcional: false
                }
            ]
        );
        // With a component without metadata above, not finding proves nothing.
        let dir = Arc::new(Diretiva {
            classe: "Espiao".into(),
            uri: "s".into(),
            provedores: vec![servico("Servico", &["Config"])],
            ..Default::default()
        });
        assert!(
            resolver(
                &[dir],
                3,
                Some(Acima {
                    provedores: &[],
                    incerto: true,
                    de_visao: &[],
                }),
            )
            .is_err()
        );
    }
}
