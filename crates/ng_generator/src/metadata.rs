//! The metadata of a `@Directive`/`@Component`, read from the loaded program —
//! what the official `_ComponentVisitor`
//! (`source_gen/template_compiler/find_components.dart`) takes from the
//! analyzer:
//!
//! * the annotation evaluated as a constant: `selector`, `exportAs`,
//!   `visibility`, `providers` (lists and top-level constants flattened, like
//!   `ModuleReader.extractProviderObjects`);
//! * `@Input`/`@Output` of the class **and of its supertypes**
//!   (`_collectInheritableMetadata`: `allSupertypes` reversed, then the class
//!   itself; in each one the fields before the setters for inputs, the
//!   accessors before the fields for outputs);
//! * `@HostListener`/`@HostBinding` in the same order (`DirectiveVisitor`:
//!   accessors, methods, fields);
//! * the lifecycle hooks (`extractLifecycleHooks`: any supertype that is the
//!   interface from `lifecycle_hooks.dart`);
//! * the constructor dependencies (`_getCompileDiDependencyMetadata`), with
//!   `@Inject`, `@Optional`, `@Self`, `@Host`, `@SkipSelf`.
//!
//! Annotations are recognized by the class they designate
//! (`TypeChecker.fromUrl('package:ngdart/src/meta/...')`), not by the written
//! name. What cannot be read goes to `Directive::unsupported`, and whoever uses the
//! directive refuses.
use crate::component::Hooks;
use crate::directives::{
    ConstValue, DI_TOKENS, Dependency, Directive, Input, Listener, Provider, Provides, Token,
    TokenType,
};
use crate::resolucao::Resolvedor;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, LibraryId, UnitId, VariableRef,
};
use dartforge_frontend::ast;

const DIRECTIVES: &str = "package:ngdart/src/meta/directives.dart";
const DI_ARGUMENTS: &str = "package:ngdart/src/meta/di_arguments.dart";
const DI_PROVIDERS: &str = "package:ngdart/src/meta/di_providers.dart";
const LIFECYCLE_HOOKS: &str = "package:ngdart/src/meta/lifecycle_hooks.dart";
const VISIBILITY: &str = "package:ngdart/src/meta/visibility.dart";

/// Maximum depth of constants that cite constants.
const DEPTH: u32 = 16;

/// An already resolved class.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Class {
    uri: String,
    name: String,
    id: ClassId,
}

impl Class {
    fn e(&self, uri: &str, name: &str) -> bool {
        self.uri == uri && self.name == name
    }
}

/// A written type, resolved: the class and the arguments; `None` for the class
/// is `dynamic`.
#[derive(Debug, Clone)]
struct Type {
    class: Option<Class>,
    args: Vec<Type>,
}

/// The constant value of an expression, in the part the metadata uses.
#[derive(Debug, Clone)]
enum Value {
    Str(String),
    Integer(i64),
    Boolean(bool),
    Null,
    /// A top-level function (`useFactory:`), by the library that declares it.
    Function {
        uri: String,
        name: String,
        id: FunctionElementId,
    },
    List(Vec<Value>),
    /// A type literal (`NgModel` in a provider list).
    Type(Class),
    /// `C<T>.ctor(a, b, name: c)`.
    Object {
        class: Class,
        constructor: Option<String>,
        types: Vec<Type>,
        positional: Vec<Value>,
        named: Vec<(String, Value)>,
    },
    /// `C.name`: enum constant or static field.
    Member {
        class: Class,
        name: String,
    },
}

/// Reads the metadata of class `id`, if it has `@Directive` or `@Component`.
pub fn ler(r: &Resolvedor, id: ClassId) -> Option<Directive> {
    Leitor { r }.directive(id)
}

struct Leitor<'r, 'a> {
    r: &'r Resolvedor<'a>,
}

/// Where a class is declared: the unit, the library, the members and the names
/// of the type parameters.
struct Declaration<'a> {
    /// The declared class (`None` for the `useFactory:` function).
    id: Option<ClassId>,
    unit: UnitId,
    lib: LibraryId,
    ast: &'a ast::Ast,
    metadata: &'a [ast::Annotation],
    members: &'a [ast::MemberId],
    type_params: Vec<String>,
    is_abstract: bool,
}

impl<'r, 'a> Leitor<'r, 'a> {
    fn name(&self, n: &ast::Name) -> &'a str {
        self.r.interner().resolve(n.sym)
    }

    fn declaration(&self, id: ClassId) -> Option<Declaration<'a>> {
        let p = self.r.program();
        let c = p.class(id);
        let d = c.decl?;
        let u = p.unit(d.unit);
        let decl = u.ast.decl(d.decl);
        let (members, tps, is_abstract): (&[ast::MemberId], &[ast::TypeParameter], bool) =
            match &decl.kind {
                ast::DeclKind::Class(k) => {
                    (&k.members[..], &k.type_params[..], k.modifiers.abstract_)
                }
                ast::DeclKind::Mixin(m) => (&m.members[..], &m.type_params[..], true),
                _ => return None,
            };
        Some(Declaration {
            id: Some(id),
            unit: d.unit,
            lib: u.library,
            ast: &u.ast,
            metadata: &decl.metadata,
            members,
            type_params: tps.iter().map(|t| self.name(&t.name).to_string()).collect(),
            is_abstract,
        })
    }

    fn class(&self, id: ClassId) -> Class {
        let p = self.r.program();
        let c = p.class(id);
        Class {
            uri: crate::dialect::canonical_string(p.library(c.library).uri.clone()),
            name: self.r.interner().resolve(c.name).to_string(),
            id,
        }
    }

    /// The analyzer's `allSupertypes` (`ClassHierarchy._getHierarchy`): for the
    /// supertype, the `on` constraints, the interfaces and the mixins, in that
    /// order, the type itself and then its supertypes, without repetition.
    fn supertypes(&self, id: ClassId, depth: u32) -> Vec<ClassId> {
        let mut output: Vec<ClassId> = Vec::new();
        if depth > 64 {
            return output;
        }
        let c = self.r.program().class(id);
        let mut direct: Vec<ClassId> = Vec::new();
        direct.extend(c.supertype_class);
        if c.kind == ClassKind::Mixin {
            direct.extend(c.on_classes.iter().copied());
        }
        direct.extend(c.interface_classes.iter().copied());
        direct.extend(c.mixin_classes.iter().copied());
        for t in direct {
            if !output.contains(&t) {
                output.push(t);
            }
            for x in self.supertypes(t, depth + 1) {
                if !output.contains(&x) {
                    output.push(x);
                }
            }
        }
        output
    }

    /// The class an annotation name designates (`Input`, `ng.Input`).
    fn annotation_class(&self, lib: LibraryId, a: &ast::Annotation) -> Option<Class> {
        let parts: Vec<&str> = a.name.iter().map(|n| self.name(n)).collect();
        let el = match parts.as_slice() {
            [n] => self.r.element_at(lib, None, n),
            [p, n] | [p, n, _] if self.r.is_prefix(lib, p) => self.r.element_at(lib, Some(p), n),
            [n, _] => self.r.element_at(lib, None, n),
            _ => None,
        }?;
        match el {
            Element::Class(id) => Some(self.class(id)),
            _ => None,
        }
    }

    /// If the annotation is one of those of ngdart's `directives.dart`, its
    /// name. An annotation with one of those names that does not resolve there
    /// is recorded in `unsupported`: the analyzer would have resolved it, and silently
    /// losing it would give wrong metadata.
    fn ngdart_annotation(
        &self,
        lib: LibraryId,
        a: &ast::Annotation,
        unsupported: &mut Vec<String>,
    ) -> Option<&'static str> {
        const NAMES: &[&str] = &[
            "Input",
            "Output",
            "HostListener",
            "HostBinding",
            "ContentChild",
            "ContentChildren",
            "ViewChild",
            "ViewChildren",
        ];
        let written = crate::annotation_name(a, self.r.interner());
        let known = NAMES.iter().copied().find(|n| *n == written);
        match self.annotation_class(lib, a) {
            Some(c) if c.uri == DIRECTIVES => NAMES.iter().copied().find(|n| *n == c.name),
            Some(_) => None,
            None => {
                if let Some(n) = known {
                    unsupported.push(format!("@{n} not resolved"));
                }
                None
            }
        }
    }

    fn directive(&self, id: ClassId) -> Option<Directive> {
        let decl = self.declaration(id)?;
        let mut annotation = None;
        for a in decl.metadata {
            if let Some(c) = self.annotation_class(decl.lib, a)
                && c.uri == DIRECTIVES
                && (c.name == "Directive" || c.name == "Component")
            {
                annotation = Some((a, c.name == "Component"));
                break;
            }
        }
        let (annotation, is_component) = annotation?;
        let eu = self.class(id);
        let mut d = Directive {
            class: eu.name.clone(),
            uri: eu.uri.clone(),
            is_component,
            ..Default::default()
        };
        self.arguments(&decl, annotation, &mut d);
        // `_collectInheritableMetadata` e `DirectiveVisitor.visitDirective`.
        let mut order: Vec<ClassId> = self.supertypes(id, 0);
        order.reverse();
        order.push(id);
        let mut inputs: Vec<(String, Input)> = Vec::new();
        for &c in &order {
            if self
                .r
                .program()
                .library(self.r.program().class(c).library)
                .is_sdk
            {
                continue;
            }
            let Some(dc) = self.declaration(c) else {
                continue;
            };
            self.members(&dc, c == id, &mut d, &mut inputs);
        }
        d.inputs = inputs.into_iter().map(|(_, e)| e).collect();
        // Generic directive with `@HostBinding`: the `XNgCd<T>` takes type
        // arguments (`lookupTypeArgumentsOf`) — no case yet.
        if !d.host_bindings.is_empty() && !decl.type_params.is_empty() && !is_component {
            d.unsupported
                .push("generic directive with @HostBinding".into());
        }
        d.hooks = self.hooks(&order);
        self.dependencies(&decl, &mut d);
        Some(d)
    }

    /// The arguments of the annotation.
    fn arguments(&self, decl: &Declaration, a: &ast::Annotation, d: &mut Directive) {
        let Some(args) = &a.arguments else {
            return;
        };
        for arg in args.args.iter() {
            let Some(name) = arg.name.as_ref().map(|n| self.name(n)) else {
                continue;
            };
            let value = || self.value(decl.unit, arg.value, 0);
            match name {
                "selector" => match value() {
                    Ok(Value::Str(s)) => d.selector = s,
                    _ => d
                        .unsupported
                        .push("selector: that is not a constant string".into()),
                },
                "exportAs" => match value() {
                    Ok(Value::Str(s)) => d.export_as = Some(s),
                    Ok(Value::Null) => {}
                    _ => d
                        .unsupported
                        .push("exportAs: that is not a constant string".into()),
                },
                "visibility" => match value() {
                    Ok(Value::Member { class, name }) if class.e(VISIBILITY, "Visibility") => {
                        d.visible = name == "all";
                    }
                    _ => d.unsupported.push("unreadable visibility:".into()),
                },
                "providers" => match value() {
                    Ok(v) => self.providers(v, d),
                    Err(f) => d.unsupported.push(format!("providers: {f}")),
                },
                // Read like the `providers:` and kept apart (private).
                "viewProviders" => match value() {
                    Ok(v) => {
                        let public = std::mem::take(&mut d.providers);
                        self.providers(v, d);
                        d.view_providers = std::mem::replace(&mut d.providers, public);
                    }
                    Err(f) => d.unsupported.push(format!("viewProviders: {f}")),
                },
                _ => {}
            }
        }
    }

    /// `ModuleReader.extractProviderObjects`: lists flattened; each item a
    /// provider (`createProviderMetadata`). What cannot be read goes to `unsupported`.
    fn providers(&self, v: Value, d: &mut Directive) {
        match v {
            Value::List(items) => {
                for i in items {
                    self.providers(i, d);
                }
            }
            Value::Null => {}
            other => match self.provider(other) {
                Ok(p) => d.providers.push(p),
                Err(f) => d.unsupported.push(f),
            },
        }
    }

    /// A provider: a type literal (`useClass` of the type itself) or a
    /// `Provider`/`ClassProvider`/`ExistingProvider`/`FactoryProvider`/
    /// `ValueProvider` (and the `.forToken` of each).
    fn provider(&self, v: Value) -> Result<Provider, String> {
        let (class, constructor, types, positional, named) = match v {
            Value::Type(c) => {
                return Ok(Provider {
                    token: Token::Class {
                        uri: c.uri.clone(),
                        class: c.name.clone(),
                    },
                    source: self.class_source(&c)?,
                    multi: false,
                    ty: None,
                });
            }
            Value::Object {
                class,
                constructor,
                types,
                positional,
                named,
            } if class.uri == DI_PROVIDERS => (class, constructor, types, positional, named),
            Value::Object { class, .. } => return Err(format!("provider {}", class.name)),
            _ => return Err("unreadable provider".into()),
        };
        let provider_type = class.name.as_str();
        let by_token = match constructor.as_deref() {
            None => false,
            Some("forToken") if provider_type != "Provider" => true,
            Some(c) => return Err(format!("{provider_type}.{c}")),
        };
        // The fields of the `Provider`, through the constructor used.
        let mut pos = positional.into_iter();
        let token_v = pos.next().ok_or("provider without a token")?;
        let mut use_class = None;
        let mut use_value = None;
        let mut use_existing = None;
        let mut use_factory = None;
        let mut deps = None;
        match provider_type {
            "Provider" | "ClassProvider" => {}
            "ExistingProvider" => use_existing = pos.next(),
            "FactoryProvider" => use_factory = pos.next(),
            "ValueProvider" => use_value = pos.next(),
            other => return Err(format!("provider {other}")),
        }
        if pos.next().is_some() {
            return Err(format!("{provider_type} with too many arguments"));
        }
        for (n, x) in named {
            let target = match (provider_type, n.as_str()) {
                ("Provider" | "ClassProvider", "useClass") => &mut use_class,
                ("Provider", "useValue") => &mut use_value,
                ("Provider", "useExisting") => &mut use_existing,
                ("Provider", "useFactory") => &mut use_factory,
                ("Provider" | "FactoryProvider", "deps") => &mut deps,
                (_, other) => return Err(format!("{provider_type}(.., {other}:)")),
            };
            *target = Some(x);
        }
        let token = self.value_token(&token_v)?;
        let multi = matches!(token, Token::Multi { .. });
        // `useValue: null` cannot be told from its absence from here.
        if matches!(use_value, Some(Value::Null)) {
            return Err("useValue: null".into());
        }
        let how_many = [&use_class, &use_value, &use_existing, &use_factory]
            .iter()
            .filter(|x| x.is_some())
            .count();
        if how_many > 1 {
            return Err("provider with more than one use*".into());
        }
        if deps.is_some() && use_factory.is_none() {
            return Err("deps: without useFactory".into());
        }
        let source = if let Some(e) = use_existing {
            Provides::Existing(self.value_token(&e)?)
        } else if let Some(f) = use_factory {
            self.factory_source(f, deps)?
        } else if let Some(v) = use_value.clone() {
            Provides::Value(self.const_value(v, 0)?)
        } else {
            let c = match use_class.unwrap_or(token_v) {
                Value::Type(c) => c,
                _ => return Err("useClass of something that is not a class".into()),
            };
            self.class_source(&c)?
        };
        // `inferProviderType`: the `T` of the `MultiToken`; otherwise the `T`
        // of the provider (written or inferred by the analyzer), unless it is
        // `dynamic` or `Object`.
        let ty = if let Token::Multi { ty, .. } = &token {
            Some(ty.clone())
        } else if let Some(t) = types.first() {
            let Some(c) = &t.class else {
                return Err("provider with a dynamic type argument".into());
            };
            if c.e("dart:core", "Object") {
                None
            } else {
                Some(self.type_without_arguments(c, t)?)
            }
        } else if by_token {
            match &token {
                Token::Opaque { ty, .. } if !ty.e_object() => Some(ty.clone()),
                _ => None,
            }
        } else if provider_type == "ValueProvider" {
            // `ValueProvider(Type, value)`: the `T` comes from the value.
            match use_value {
                Some(Value::Str(_)) => Some(Self::from_core("String")),
                Some(Value::Integer(_)) => Some(Self::from_core("int")),
                Some(Value::Boolean(_)) => Some(Self::from_core("bool")),
                Some(Value::Object { class, .. }) => Some(TokenType {
                    uri: class.uri.clone(),
                    class: class.name.clone(),
                    generics: 0,
                    args: Vec::new(),
                }),
                _ => return Err("ValueProvider of a value without a known type".into()),
            }
        } else {
            None
        };
        Ok(Provider {
            token,
            source,
            multi,
            ty,
        })
    }

    /// The `T` of a token as `fromDartType` writes it: the class and the
    /// arguments, recursively; arguments that are all `dynamic` stay in the
    /// short form (empty `args`).
    fn token_type(&self, t: &Type, depth: u32) -> Result<TokenType, String> {
        let Some(c) = &t.class else {
            return Ok(TokenType::dynamic());
        };
        if depth > 16 {
            return Err("token type nested too deeply".into());
        }
        let params = self.r.program().class(c.id).type_params.len();
        if t.args.len() != params {
            return Err("token type without all the arguments".into());
        }
        let args: Vec<TokenType> = t
            .args
            .iter()
            .map(|a| self.token_type(a, depth + 1))
            .collect::<Result<_, _>>()?;
        Ok(TokenType {
            uri: c.uri.clone(),
            class: c.name.clone(),
            generics: params,
            args: if args.iter().all(TokenType::is_dynamic) {
                Vec::new()
            } else {
                args
            },
        })
    }

    fn from_core(name: &str) -> TokenType {
        TokenType {
            uri: "dart:core".into(),
            class: name.into(),
            generics: 0,
            args: Vec::new(),
        }
    }

    /// A type without arguments (or only with `dynamic`), as `fromDartType`
    /// writes it.
    fn type_without_arguments(&self, c: &Class, t: &Type) -> Result<TokenType, String> {
        let params = self.r.program().class(c.id).type_params.len();
        if !t.args.is_empty()
            && (t.args.len() != params || t.args.iter().any(|a| a.class.is_some()))
        {
            return Err("provider type with arguments".into());
        }
        Ok(TokenType {
            uri: c.uri.clone(),
            class: c.name.clone(),
            generics: params,
            args: Vec::new(),
        })
    }

    /// `useClass:` (`_getUseClass`, `enforceClassCanBeCreated`): the class and
    /// the dependencies of the unnamed constructor.
    fn class_source(&self, c: &Class) -> Result<Provides, String> {
        let decl = self
            .declaration(c.id)
            .ok_or_else(|| format!("provider class without a declaration ({})", c.name))?;
        if !decl.type_params.is_empty() {
            return Err("provider of a generic class".into());
        }
        if decl.is_abstract {
            return Err("provider of an abstract class".into());
        }
        let mut d = Directive::default();
        self.dependencies(&decl, &mut d);
        if let Some(f) = d.unsupported.into_iter().next() {
            return Err(f);
        }
        Ok(Provides::Class {
            uri: c.uri.clone(),
            class: c.name.clone(),
            deps: d.dependencies,
        })
    }

    /// `useFactory:` of a top-level function, with `deps:` (if not empty) or
    /// its parameters (`_factoryForFunction`).
    fn factory_source(&self, f: Value, deps: Option<Value>) -> Result<Provides, String> {
        let Value::Function { uri, name, id } = f else {
            return Err("useFactory that is not a top-level function".into());
        };
        let list = match deps {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::List(l)) => l,
            Some(_) => return Err("unreadable deps:".into()),
        };
        let mut output = Vec::new();
        if !list.is_empty() {
            for item in list {
                output.push(self.deps_dependency(item)?);
            }
        } else {
            let p = self.r.program();
            let fe = p.function(id);
            let dartforge_elements::model::FunctionRef::Function { unit, function } = fe.node
            else {
                return Err("useFactory without a declaration".into());
            };
            let u = p.unit(unit);
            let decl = Declaration {
                id: None,
                unit,
                lib: u.library,
                ast: &u.ast,
                metadata: &[],
                members: &[],
                type_params: Vec::new(),
                is_abstract: false,
            };
            let function = u.ast.function(function);
            if !function.type_params.is_empty() {
                return Err("generic useFactory".into());
            }
            for param in function.parameters.iter().flat_map(|ps| ps.iter()) {
                if matches!(param.kind, ast::ParameterKind::Named) {
                    continue;
                }
                output.push(self.dependency(&decl, param, None)?);
            }
        }
        Ok(Provides::Factory {
            uri,
            name,
            deps: output,
        })
    }

    /// An item of `deps:` (`_factoryDiDep`): a type, a token, or the list
    /// `[token, Optional(), ..]`.
    fn deps_dependency(&self, v: Value) -> Result<Dependency, String> {
        let mut dep = Dependency {
            token: Token::Element,
            optional: false,
            self_only: false,
            host: false,
            skip_self: false,
            attribute: None,
        };
        match v {
            Value::List(items) => {
                let mut it = items.into_iter();
                let first = it.next().ok_or("deps: with an empty list")?;
                dep.token = self.dependency_token(&first)?;
                for m in it {
                    match m {
                        Value::Object { class, .. } if class.uri == DI_ARGUMENTS => {
                            match class.name.as_str() {
                                "Optional" => dep.optional = true,
                                "Self" => dep.self_only = true,
                                "Host" => dep.host = true,
                                "SkipSelf" => dep.skip_self = true,
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            other => dep.token = self.dependency_token(&other)?,
        }
        Ok(dep)
    }

    /// The token of a dependency written as a value: `HtmlElement` and
    /// `ChangeDetectorRef` become the built-ins, as in the parameter.
    fn dependency_token(&self, v: &Value) -> Result<Token, String> {
        match v {
            Value::Type(c) if crate::dialect::embedded_node(&c.uri, &c.name).is_some() => {
                Ok(Token::Element)
            }
            Value::Type(c)
                if c.uri.starts_with("package:ngdart/") && c.name == "ChangeDetectorRef" =>
            {
                Ok(Token::Detector)
            }
            other => self.value_token(other),
        }
    }

    /// The `_useValueExpression` in the part the emitter writes: string,
    /// integer, boolean and constant object of a non-generic class with
    /// arguments of those kinds.
    fn const_value(&self, v: Value, depth: u32) -> Result<ConstValue, String> {
        Ok(match v {
            Value::Str(s) => ConstValue::Str(s),
            Value::Integer(i) => ConstValue::Integer(i),
            Value::Boolean(b) => ConstValue::Boolean(b),
            Value::Object {
                class,
                constructor,
                types,
                positional,
                named,
            } if depth == 0 => {
                if !types.is_empty() || !self.r.program().class(class.id).type_params.is_empty() {
                    return Err("useValue of a generic class".into());
                }
                if constructor.as_deref().is_some_and(|c| c.starts_with('_')) {
                    return Err("useValue with a private constructor".into());
                }
                ConstValue::Object {
                    uri: class.uri.clone(),
                    class: class.name.clone(),
                    constructor,
                    positional: positional
                        .into_iter()
                        .map(|x| self.const_value(x, depth + 1))
                        .collect::<Result<_, _>>()?,
                    named: named
                        .into_iter()
                        .map(|(n, x)| Ok((n, self.const_value(x, depth + 1)?)))
                        .collect::<Result<_, String>>()?,
                }
            }
            _ => {
                return Err(
                    "useValue other than a string, integer, boolean or simple object".into(),
                );
            }
        })
    }

    /// The token of a constant value: a class, a `MultiToken` or an
    /// `OpaqueToken`.
    fn value_token(&self, v: &Value) -> Result<Token, String> {
        self.token_lido(v, false)
    }

    /// The token of a value; `unnamed`: `const OpaqueToken<T>()` is accepted
    /// (in the generated injector it becomes `const OpaqueToken<T>()`; in the
    /// views there is no case yet).
    fn token_lido(&self, v: &Value, unnamed: bool) -> Result<Token, String> {
        match v {
            Value::Type(c) => Ok(Token::Class {
                uri: c.uri.clone(),
                class: c.name.clone(),
            }),
            Value::Object {
                class,
                constructor: None,
                types,
                positional,
                ..
            } if class.e(DI_TOKENS, "MultiToken") || class.e(DI_TOKENS, "OpaqueToken") => {
                let multi = class.name == "MultiToken";
                let name = match positional.first() {
                    Some(Value::Str(s)) => s.clone(),
                    None if multi || unnamed => String::new(),
                    None => return Err("OpaqueToken without a name".into()),
                    _ => return Err("token with an unreadable name".into()),
                };
                // Without a type argument, written or from the context,
                // inference gives the bound of `T` (`T extends Object`):
                // `OpaqueToken<Object>`.
                let object;
                let t = match types.as_slice() {
                    [t] => t,
                    [] => {
                        let id = self
                            .r
                            .class_by_uri("dart:core", "Object")
                            .ok_or("Object of dart:core not found")?;
                        object = Type {
                            class: Some(self.class(id)),
                            args: Vec::new(),
                        };
                        &object
                    }
                    _ => return Err(format!("{} with too many type arguments", class.name)),
                };
                if t.class.is_none() {
                    return Err(format!("{}<dynamic>", class.name));
                }
                let ty = self.token_type(t, 0)?;
                Ok(if multi {
                    Token::Multi { name, ty }
                } else {
                    Token::Opaque { name, ty }
                })
            }
            Value::Object { class, .. } => Err(format!("token {}", class.name)),
            _ => Err("unreadable token".into()),
        }
    }

    /// The annotated members of a class of the hierarchy; `own` when it is
    /// the directive's class (and not a supertype).
    fn members(
        &self,
        dc: &Declaration,
        own: bool,
        d: &mut Directive,
        inputs: &mut Vec<(String, Input)>,
    ) {
        let mut from_field: Vec<(String, Input)> = Vec::new();
        let mut from_setter: Vec<(String, Input)> = Vec::new();
        let mut accessor_outputs: Vec<(String, String)> = Vec::new();
        let mut field_outputs: Vec<(String, String)> = Vec::new();
        let mut accessor_bindings = Vec::new();
        let mut method_bindings = Vec::new();
        let mut field_bindings = Vec::new();
        let mut setter_queries = Vec::new();
        let mut field_queries = Vec::new();
        for &m in dc.members {
            let member = dc.ast.member(m);
            for a in member.metadata.iter() {
                let Some(which) = self.ngdart_annotation(dc.lib, a, &mut d.unsupported) else {
                    continue;
                };
                let alias = self.first_text(dc, a);
                match (which, &member.kind) {
                    ("Input", ast::MemberKind::Field(l)) if !l.static_ => {
                        if l.final_ || l.const_ {
                            // Without a setter: the official compiler warns and
                            // ignores it.
                            continue;
                        }
                        for v in l.variables.iter() {
                            let name = self.name(&v.name).to_string();
                            if name.starts_with('_') {
                                continue;
                            }
                            let boolean = match l.ty {
                                Some(t) => self.boolean(dc, t),
                                None => match v.initializer.map(|e| &dc.ast.expr(e).kind) {
                                    Some(ast::ExprKind::Bool(_)) => Some(true),
                                    None | Some(ast::ExprKind::Null) => Some(false),
                                    _ => None,
                                },
                            };
                            from_field.push((
                                name.clone(),
                                Input {
                                    name: alias.clone().unwrap_or_else(|| name.clone()),
                                    member: name,
                                    boolean,
                                },
                            ));
                        }
                    }
                    ("Input", ast::MemberKind::Method(f)) => {
                        let function = dc.ast.function(*f);
                        let (ast::FunctionKind::Setter, Some(n), false) =
                            (function.kind, function.name, function.static_)
                        else {
                            continue;
                        };
                        let name = self.name(&n).to_string();
                        if name.starts_with('_') {
                            continue;
                        }
                        let boolean = function
                            .parameters
                            .as_deref()
                            .and_then(|ps| ps.first())
                            .and_then(|p| p.ty)
                            .and_then(|t| self.boolean(dc, t));
                        from_setter.push((
                            name.clone(),
                            Input {
                                name: alias.clone().unwrap_or_else(|| name.clone()),
                                member: name,
                                boolean,
                            },
                        ));
                    }
                    ("Output", ast::MemberKind::Field(l)) if !l.static_ => {
                        for v in l.variables.iter() {
                            let name = self.name(&v.name).to_string();
                            if name.starts_with('_') {
                                continue;
                            }
                            field_outputs
                                .push((alias.clone().unwrap_or_else(|| name.clone()), name));
                        }
                    }
                    ("Output", ast::MemberKind::Method(f)) => {
                        let function = dc.ast.function(*f);
                        if let (ast::FunctionKind::Getter, Some(n), false) =
                            (function.kind, function.name, function.static_)
                        {
                            let name = self.name(&n).to_string();
                            if !name.starts_with('_') {
                                accessor_outputs
                                    .push((alias.clone().unwrap_or_else(|| name.clone()), name));
                            }
                        }
                    }
                    ("HostListener", ast::MemberKind::Method(f)) => {
                        match self.listener(dc, *f, a) {
                            Ok(o) => match d.listeners.iter_mut().find(|x| x.event == o.event) {
                                // The official map: the last one wins, in the
                                // position of the first.
                                Some(x) => *x = o,
                                None => d.listeners.push(o),
                            },
                            Err(e) => d.unsupported.push(e),
                        }
                    }
                    ("HostListener", _) => {
                        d.unsupported.push("@HostListener outside a method".into())
                    }
                    ("HostBinding", k) => {
                        // Static member (`_addHostBinding`): a supertype's one
                        // is not inherited; the class's own is read from it
                        // (`StaticRead`) and the official compiler writes it
                        // once, in the view constructor (case j96). In a
                        // directive it is not written yet
                        // (`static_host`).
                        let is_static = match k {
                            ast::MemberKind::Field(l) => l.static_,
                            ast::MemberKind::Method(f) => dc.ast.function(*f).static_,
                            _ => false,
                        };
                        if is_static && !own {
                            continue;
                        }
                        let member_name = match k {
                            ast::MemberKind::Field(l) => {
                                l.variables.first().map(|v| self.name(&v.name).to_string())
                            }
                            ast::MemberKind::Method(f) => {
                                dc.ast.function(*f).name.map(|n| self.name(&n).to_string())
                            }
                            _ => None,
                        }
                        .unwrap_or_default();
                        let binding = (
                            alias.clone().unwrap_or_else(|| member_name.clone()),
                            member_name,
                        );
                        // `_computeHostBindingImmutability`
                        // (`compile_metadata.dart:570-600`): an immutable
                        // static outside `class.x`/`style.x` is a
                        // `hostAttribute` — it does not go into a directive's
                        // `XNgCd`; its user writes it on the element
                        // (`mergeHtmlAndDirectiveAttributes`, still refused
                        // there). In a component it stays in the list (case
                        // j96).
                        let immutable =
                            matches!(k, ast::MemberKind::Field(l) if l.final_ || l.const_);
                        let of_class_or_style =
                            binding.0.starts_with("class.") || binding.0.starts_with("style.");
                        if is_static && immutable && !of_class_or_style && !d.is_component {
                            let name = binding.0.strip_prefix("attr.").unwrap_or(&binding.0);
                            d.host_attributes
                                .push((name.to_string(), binding.1.clone()));
                            continue;
                        }
                        d.static_host |= is_static;
                        if is_static {
                            d.host_statics.push(binding.1.clone());
                        }
                        match k {
                            ast::MemberKind::Field(_) => field_bindings.push(binding),
                            ast::MemberKind::Method(f)
                                if matches!(
                                    dc.ast.function(*f).kind,
                                    ast::FunctionKind::Getter | ast::FunctionKind::Setter
                                ) =>
                            {
                                accessor_bindings.push(binding)
                            }
                            _ => method_bindings.push(binding),
                        }
                    }
                    ("ContentChild" | "ContentChildren", k) => {
                        let name = match k {
                            ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                                Some((self.name(&l.variables[0].name).to_string(), false))
                            }
                            ast::MemberKind::Method(f) => {
                                let function = dc.ast.function(*f);
                                match (function.kind, function.name, function.static_) {
                                    (ast::FunctionKind::Setter, Some(n), false) => {
                                        Some((self.name(&n).to_string(), true))
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        };
                        let Some((field, setter)) = name else {
                            d.unsupported
                                .push("@ContentChild outside a field or setter".into());
                            continue;
                        };
                        match self.content_query(dc, a, which == "ContentChildren", field) {
                            Ok(q) if setter => setter_queries.push(q),
                            Ok(q) => field_queries.push(q),
                            Err(e) => d.unsupported.push(e),
                        }
                    }
                    ("ViewChild" | "ViewChildren", _) => {
                        d.queries = true;
                    }
                    _ => {}
                }
            }
        }
        // Like the official `_queries`: setters, then fields, accumulated over
        // each class of the hierarchy (supertypes first) — the list grows, it
        // is not replaced (`find_components.dart:374`).
        d.content_queries.extend(setter_queries);
        d.content_queries.extend(field_queries);
        // `_inputs..addAll(_fieldInputs)..addAll(_setterInputs)`: the key is
        // the member; repeating the key replaces the value and keeps the
        // position.
        for (key, e) in from_field.into_iter().chain(from_setter) {
            match inputs.iter_mut().find(|(k, _)| *k == key) {
                Some(x) => x.1 = e,
                None => inputs.push((key, e)),
            }
        }
        for (name, member) in accessor_outputs.into_iter().chain(field_outputs) {
            match d.outputs.iter_mut().find(|(_, m)| *m == member) {
                Some(x) => x.0 = name,
                None => d.outputs.push((name, member)),
            }
        }
        for (name, member) in accessor_bindings
            .into_iter()
            .chain(method_bindings)
            .chain(field_bindings)
        {
            match d.host_bindings.iter_mut().find(|(n, _)| *n == name) {
                Some(x) => x.1 = member,
                None => d.host_bindings.push((name, member)),
            }
        }
    }

    /// A `@ContentChild`/`@ContentChildren`: the target (`#ref` as a string or
    /// a type, resolved in the directive's library), `descendants:` (the list
    /// one is `true` by default in ngdart 8; the single one, always) and
    /// `read:`.
    fn content_query(
        &self,
        dc: &Declaration,
        a: &ast::Annotation,
        list: bool,
        field: String,
    ) -> Result<crate::view::ChildQuery, String> {
        use crate::view::{ChildQuery, QueryRead, QueryTarget};
        let args = a
            .arguments
            .as_ref()
            .ok_or("@ContentChild without arguments")?;
        let first = args
            .args
            .iter()
            .find(|x| x.name.is_none())
            .ok_or("@ContentChild without a target")?;
        let of_package =
            |c: &Class| !c.uri.starts_with("package:ngdart/") && !c.uri.starts_with("dart:");
        let target = match self.value(dc.unit, first.value, 0)? {
            Value::Str(r) => QueryTarget::Reference(r),
            Value::Type(c) if of_package(&c) => QueryTarget::Class(c.uri, c.name),
            _ => return Err("@ContentChild of an ngdart type or of unknown shape".into()),
        };
        let mut descendants = true;
        let mut read = None;
        for x in args.args.iter() {
            match x.name.as_ref().map(|n| self.name(n)) {
                None => {}
                Some("descendants") if list => match self.value(dc.unit, x.value, 0)? {
                    Value::Boolean(b) => descendants = b,
                    _ => return Err("@ContentChildren(descendants:) that is not a literal".into()),
                },
                Some("read") => {
                    read = Some(match self.value(dc.unit, x.value, 0)? {
                        Value::Type(c)
                            if crate::dialect::embedded_node(&c.uri, &c.name).is_some() =>
                        {
                            QueryRead::Element
                        }
                        Value::Type(c) if of_package(&c) => QueryRead::Class(c.uri, c.name),
                        _ => return Err("@ContentChild(.., read:) of an ngdart type".into()),
                    })
                }
                _ => return Err("@ContentChild with an unknown argument".into()),
            }
        }
        Ok(ChildQuery {
            field,
            list,
            target,
            descendants,
            read,
        })
    }

    /// The first positional argument of an annotation, if it is a string.
    fn first_text(&self, dc: &Declaration, a: &ast::Annotation) -> Option<String> {
        let args = a.arguments.as_ref()?;
        let x = args.args.iter().find(|x| x.name.is_none())?;
        match self.value(dc.unit, x.value, 0) {
            Ok(Value::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// Is the type of an input `bool`? (`_isBoolType` of the type name, which
    /// ignores the `?`). A type parameter of the class: `None` (the official
    /// compiler uses the bound, which cannot be read from here).
    fn boolean(&self, dc: &Declaration, t: ast::TypeId) -> Option<bool> {
        match &dc.ast.ty(t).kind {
            ast::TypeKind::Named { name, .. } => {
                let parts: Vec<&str> = name.iter().map(|n| self.name(n)).collect();
                let simple = *parts.last()?;
                if parts.len() == 1 && dc.type_params.iter().any(|p| p == simple) {
                    return None;
                }
                let ty = self.ty(dc.lib, t, dc.ast);
                Some(
                    ty.and_then(|t| t.class)
                        .is_some_and(|c| c.e("dart:core", "bool")),
                )
            }
            _ => Some(false),
        }
    }

    /// A `@HostListener`, as `_addHostListener` builds it.
    fn listener(
        &self,
        dc: &Declaration,
        f: ast::FunctionId,
        a: &ast::Annotation,
    ) -> Result<Listener, String> {
        let function = dc.ast.function(f);
        if function.static_ || !matches!(function.kind, ast::FunctionKind::Function) {
            return Err("@HostListener outside an instance method".into());
        }
        let method = function
            .name
            .map(|n| self.name(&n).to_string())
            .ok_or("@HostListener without a name")?;
        let args = a
            .arguments
            .as_ref()
            .ok_or("@HostListener without arguments")?;
        let mut event = None;
        let mut list = None;
        let mut position = 0;
        for x in args.args.iter() {
            let field = match x.name.as_ref().map(|n| self.name(n)) {
                Some(n) => n,
                None => {
                    position += 1;
                    if position == 1 { "eventName" } else { "args" }
                }
            };
            let v = self.value(dc.unit, x.value, 0)?;
            match (field, v) {
                ("eventName", Value::Str(s)) => event = Some(s),
                ("args", Value::List(items)) => {
                    let mut texts = Vec::new();
                    for i in items {
                        let Value::Str(s) = i else {
                            return Err("@HostListener with args that are not strings".into());
                        };
                        texts.push(s);
                    }
                    list = Some(texts);
                }
                _ => return Err("unreadable @HostListener".into()),
            }
        }
        let event = event.ok_or("@HostListener without an event")?;
        let mut arguments = list.unwrap_or_default();
        let params = function.parameters.as_ref().map_or(0, |p| p.len());
        if arguments.is_empty() && params == 1 {
            arguments.push("$event".into());
        }
        Ok(Listener {
            event,
            method,
            args: arguments.join(", "),
        })
    }

    /// `extractLifecycleHooks`: the class is assignable to the interface.
    fn hooks(&self, classes: &[ClassId]) -> Hooks {
        let mut g = Hooks::default();
        for &c in classes {
            let k = self.class(c);
            if k.uri != LIFECYCLE_HOOKS {
                continue;
            }
            match k.name.as_str() {
                "OnInit" => g.on_init = true,
                "OnDestroy" => g.on_destroy = true,
                "DoCheck" => g.do_check = true,
                "AfterChanges" => g.after_changes = true,
                "AfterContentInit" => g.after_content_init = true,
                "AfterContentChecked" => g.after_content_checked = true,
                "AfterViewInit" => g.after_view_init = true,
                "AfterViewChecked" => g.after_view_checked = true,
                _ => {}
            }
        }
        g
    }

    /// The dependencies of the unnamed constructor (or the first one), like
    /// `CompileTypeMetadataVisitor.unnamedConstructor`.
    fn dependencies(&self, decl: &Declaration<'a>, d: &mut Directive) {
        if decl.is_abstract {
            return;
        }
        let mut constructors = Vec::new();
        for &m in decl.members {
            if let ast::MemberKind::Constructor(c) = &decl.ast.member(m).kind {
                constructors.push(c);
            }
        }
        let Some(ctor) = constructors
            .iter()
            .find(|c| c.name.is_none())
            .or(constructors.first())
        else {
            return;
        };
        if ctor.name.is_some_and(|n| self.name(&n).starts_with('_')) {
            d.unsupported.push("private constructor".into());
            return;
        }
        for p in ctor.parameters.iter() {
            if matches!(p.kind, ast::ParameterKind::Named) {
                continue;
            }
            // `super.x` without a type: that of the parameter it forwards to.
            let super_ty = (p.super_ && p.ty.is_none())
                .then(|| self.super_type(decl, ctor, p, 0))
                .flatten();
            match self.dependency(decl, p, super_ty) {
                Ok(x) => d.dependencies.push(x),
                Err(f) => {
                    d.unsupported.push(f);
                    return;
                }
            }
        }
        d.dependencies_read = true;
    }

    fn dependency(
        &self,
        decl: &Declaration<'a>,
        p: &ast::Parameter,
        super_ty: Option<(ast::TypeId, &'a ast::Ast, LibraryId)>,
    ) -> Result<Dependency, String> {
        let mut dep = Dependency {
            token: Token::Element,
            optional: matches!(p.kind, ast::ParameterKind::Optional),
            self_only: false,
            host: false,
            skip_self: false,
            attribute: None,
        };
        let mut injected = None;
        for a in p.metadata.iter() {
            match self.annotation_class(decl.lib, a) {
                Some(c) if c.uri == DI_ARGUMENTS => match c.name.as_str() {
                    "Optional" => dep.optional = true,
                    "Self" => dep.self_only = true,
                    "Host" => dep.host = true,
                    "SkipSelf" => dep.skip_self = true,
                    "Inject" => {
                        let x = a
                            .arguments
                            .as_ref()
                            .and_then(|args| args.args.first())
                            .ok_or("@Inject without a token")?;
                        let v = self.value(decl.unit, x.value, 0)?;
                        injected = Some(self.value_token(&v)?);
                    }
                    other => return Err(format!("@{other} in the directive constructor")),
                },
                Some(c) if c.uri == DIRECTIVES && c.name == "Attribute" => {
                    let x = a
                        .arguments
                        .as_ref()
                        .and_then(|args| args.args.first())
                        .ok_or("@Attribute without a name")?;
                    match self.value(decl.unit, x.value, 0)? {
                        Value::Str(name) => dep.attribute = Some(name),
                        _ => return Err("@Attribute with a name that is not a string".into()),
                    }
                }
                Some(_) => {}
                None => {
                    // A token used as an annotation (`@ngValidators`).
                    let parts: Vec<&str> = a.name.iter().map(|n| self.name(n)).collect();
                    if let [n] = parts.as_slice()
                        && let Some(Element::Variable(v)) = self.r.element_at(decl.lib, None, n)
                    {
                        let v = self.variable(v, 0)?;
                        injected = Some(self.value_token(&v)?);
                    } else {
                        return Err("unresolved annotation in the directive constructor".into());
                    }
                }
            }
        }
        dep.token = match injected {
            Some(t) => t,
            None => {
                let t = match p.ty {
                    Some(t) => Some((t, decl.ast, decl.lib)),
                    None if p.this_ => self.field_type(decl, p).map(|(t, a)| (t, a, decl.lib)),
                    None if p.super_ => super_ty,
                    None => None,
                }
                .ok_or("untyped parameter in the directive constructor")?;
                // The token is the class (`_idFor`): the type arguments do not
                // count, and a `T` of the generic directive does not get in the
                // way.
                let c = self
                    .type_class(t.2, t.0, t.1)
                    .ok_or("unresolved type in the directive constructor")?
                    .ok_or("dynamic parameter in the directive constructor")?;
                if crate::dialect::embedded_node(&c.uri, &c.name).is_some() {
                    Token::Element
                } else if c.uri.starts_with("package:ngdart/") && c.name == "ChangeDetectorRef" {
                    Token::Detector
                } else {
                    Token::Class {
                        uri: c.uri,
                        class: c.name,
                    }
                }
            }
        };
        Ok(dep)
    }

    /// The type of a `super.x` parameter without a written type
    /// (`SuperFormalParameterElement`): that of the parameter it forwards to in
    /// the superclass constructor called — that of the initializer's
    /// `super.name(..)`, or the unnamed one —, by position among the positional
    /// `super.` parameters or by name; there, the written type, that of the
    /// field (`this.x`) or, for another `super.x`, that of the next superclass.
    /// With the library where the type is written.
    fn super_type(
        &self,
        decl: &Declaration<'a>,
        ctor: &ast::Constructor,
        p: &ast::Parameter,
        depth: u32,
    ) -> Option<(ast::TypeId, &'a ast::Ast, LibraryId)> {
        if depth > 16 {
            return None;
        }
        let sup = self.r.program().class(decl.id?).supertype_class?;
        let sdecl = self.declaration(sup)?;
        let called = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => {
                Some(constructor.as_ref().map(|n| self.name(n)))
            }
            _ => None,
        });
        let called: Option<&str> = called.flatten();
        let sctor = sdecl
            .members
            .iter()
            .find_map(|&m| match &sdecl.ast.member(m).kind {
                ast::MemberKind::Constructor(c)
                    if !c.factory && c.name.as_ref().map(|n| self.name(n)) == called =>
                {
                    Some(c)
                }
                _ => None,
            })?;
        let named = |q: &ast::Parameter| matches!(q.kind, ast::ParameterKind::Named);
        let target = if named(p) {
            let name = self.name(p.name.as_ref()?);
            sctor
                .parameters
                .iter()
                .find(|q| named(q) && q.name.as_ref().map(|n| self.name(n)) == Some(name))?
        } else {
            let k = ctor
                .parameters
                .iter()
                .filter(|q| q.super_ && !named(q))
                .position(|q| std::ptr::eq(q, p))?;
            sctor.parameters.iter().filter(|q| !named(q)).nth(k)?
        };
        match target.ty {
            Some(t) => Some((t, sdecl.ast, sdecl.lib)),
            None if target.this_ => self
                .field_type(&sdecl, target)
                .map(|(t, a)| (t, a, sdecl.lib)),
            None if target.super_ => self.super_type(&sdecl, sctor, target, depth + 1),
            None => None,
        }
    }

    /// The written type of the field of a `this.x` parameter.
    fn field_type(
        &self,
        decl: &Declaration<'a>,
        p: &ast::Parameter,
    ) -> Option<(ast::TypeId, &'a ast::Ast)> {
        let target = self.name(p.name.as_ref()?);
        for &m in decl.members {
            if let ast::MemberKind::Field(l) = &decl.ast.member(m).kind
                && l.variables.iter().any(|v| self.name(&v.name) == target)
            {
                return l.ty.map(|t| (t, decl.ast));
            }
        }
        None
    }

    /// Resolves a written type in the scope of `lib`.
    /// The class of a written type, without looking at the arguments;
    /// `Some(None)` for `dynamic`.
    fn type_class(&self, lib: LibraryId, t: ast::TypeId, tree: &ast::Ast) -> Option<Option<Class>> {
        let ast::TypeKind::Named { name, .. } = &tree.ty(t).kind else {
            return None;
        };
        let parts: Vec<&str> = name.iter().map(|n| self.name(n)).collect();
        let el = match parts.as_slice() {
            ["dynamic"] => return Some(None),
            [n] => self.r.element_at(lib, None, n),
            [p, n] => self.r.element_at(lib, Some(p), n),
            _ => None,
        }?;
        let Element::Class(id) = el else {
            return None;
        };
        Some(Some(self.class(id)))
    }

    fn ty(&self, lib: LibraryId, t: ast::TypeId, tree: &ast::Ast) -> Option<Type> {
        let ast::TypeKind::Named { name, args } = &tree.ty(t).kind else {
            return None;
        };
        let parts: Vec<&str> = name.iter().map(|n| self.name(n)).collect();
        let el = match parts.as_slice() {
            ["dynamic"] => {
                return Some(Type {
                    class: None,
                    args: Vec::new(),
                });
            }
            [n] => self.r.element_at(lib, None, n),
            [p, n] => self.r.element_at(lib, Some(p), n),
            _ => None,
        }?;
        let Element::Class(id) = el else {
            return None;
        };
        let mut arguments = Vec::new();
        for a in args.iter() {
            arguments.push(self.ty(lib, *a, tree)?);
        }
        Some(Type {
            class: Some(self.class(id)),
            args: arguments,
        })
    }

    /// The value of a top-level variable or of a `const` static field.
    fn variable(
        &self,
        v: dartforge_elements::model::VariableId,
        depth: u32,
    ) -> Result<Value, String> {
        let p = self.r.program();
        let var = p.variable(v);
        if !var.const_ {
            return Err("variable that is not const".into());
        }
        let (unit, l, index) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => {
                let ast::DeclKind::Variables(l) = &p.unit(unit).ast.decl(decl).kind else {
                    return Err("unreadable constant".into());
                };
                (unit, l, index)
            }
            // A `static const` field (the `useValue:` of `C.field`).
            VariableRef::Field {
                unit,
                member,
                index,
            } if var.static_ => {
                let ast::MemberKind::Field(l) = &p.unit(unit).ast.member(member).kind else {
                    return Err("unreadable constant".into());
                };
                (unit, l, index)
            }
            _ => return Err("constant that is not top-level".into()),
        };
        let e = l
            .variables
            .get(index)
            .and_then(|x| x.initializer)
            .ok_or("constant without a value")?;
        let mut v = self.value(unit, e, depth + 1)?;
        // `const OpaqueToken<T> t = OpaqueToken('x')`: without a written type
        // argument, inference takes them from the declared type.
        if let (Some(ty), Value::Object { class, types, .. }) = (l.ty, &mut v)
            && types.is_empty()
        {
            let u = p.unit(unit);
            if let Some(declared) = self.ty(u.library, ty, &u.ast)
                && declared.class.as_ref().is_some_and(|c| c.id == class.id)
            {
                *types = declared.args;
            }
        }
        Ok(v)
    }

    /// Evaluates a constant expression written in unit `unit`.
    fn value(&self, unit: UnitId, e: ast::ExprId, depth: u32) -> Result<Value, String> {
        if depth > DEPTH {
            return Err("constante profunda demais".into());
        }
        let p = self.r.program();
        let u = p.unit(unit);
        let lib = u.library;
        let tree = &u.ast;
        let element = |el: Element| -> Result<Value, String> {
            match el {
                Element::Class(id) => Ok(Value::Type(self.class(id))),
                Element::Variable(v) => self.variable(v, depth),
                Element::Function(f) => {
                    let fe = p.function(f);
                    if fe.class.is_some()
                        || fe.extension.is_some()
                        || !matches!(
                            fe.node,
                            dartforge_elements::model::FunctionRef::Function { .. }
                        )
                    {
                        return Err("function that is not top-level".into());
                    }
                    Ok(Value::Function {
                        uri: crate::dialect::canonical_string(p.library(fe.library).uri.clone()),
                        name: self.r.interner().resolve(fe.name).to_string(),
                        id: f,
                    })
                }
                _ => Err("name that is neither a class nor a constant".into()),
            }
        };
        match &tree.expr(e).kind {
            ast::ExprKind::String(s) => s
                .constant_value()
                .map(|t| Value::Str(t.to_string_lossy()))
                .ok_or_else(|| "string with interpolation".into()),
            ast::ExprKind::Null => Ok(Value::Null),
            ast::ExprKind::Bool(b) => Ok(Value::Boolean(*b)),
            ast::ExprKind::Int(span) => {
                let text = u
                    .source
                    .get(span.start..span.end)
                    .unwrap_or("")
                    .replace('_', "");
                let n = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                    Some(h) => i64::from_str_radix(h, 16).ok(),
                    None => text.parse::<i64>().ok(),
                };
                n.map(Value::Integer)
                    .ok_or_else(|| "unreadable integer".into())
            }
            ast::ExprKind::Parenthesized(x) => self.value(unit, *x, depth),
            ast::ExprKind::List { elements, .. } => {
                let mut items = Vec::new();
                for el in elements.iter() {
                    let ast::CollectionElement::Expression(x) = el else {
                        return Err("list with a spread or `if`".into());
                    };
                    items.push(self.value(unit, *x, depth + 1)?);
                }
                Ok(Value::List(items))
            }
            ast::ExprKind::Identifier(n) => {
                let el = self
                    .r
                    .element_at(lib, None, self.name(n))
                    .ok_or("unresolved name")?;
                element(el)
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => {
                let ast::ExprKind::Identifier(target) = &tree.expr(*target).kind else {
                    return Err("property of an expression".into());
                };
                let target = self.name(target);
                if self.r.is_prefix(lib, target) {
                    let el = self
                        .r
                        .element_at(lib, Some(target), self.name(name))
                        .ok_or("unresolved name")?;
                    return element(el);
                }
                match self.r.element_at(lib, None, target) {
                    Some(Element::Class(id)) => Ok(Value::Member {
                        class: self.class(id),
                        name: self.name(name).to_string(),
                    }),
                    _ => Err("property that is not of a class".into()),
                }
            }
            ast::ExprKind::Call { target, arguments } => {
                let (class, constructor) = self.constructor(lib, tree, *target)?;
                let types = arguments
                    .type_args
                    .iter()
                    .map(|t| self.ty(lib, *t, tree).ok_or("unresolved type"))
                    .collect::<Result<Vec<_>, _>>()?;
                self.object(unit, class, constructor, types, arguments, depth)
            }
            ast::ExprKind::InstanceCreation {
                ty,
                constructor,
                arguments,
                ..
            } => {
                let t = self.ty(lib, *ty, tree).ok_or("unresolved type")?;
                let class = t.class.ok_or("instance of dynamic")?;
                self.object(
                    unit,
                    class,
                    constructor.map(|n| self.name(&n).to_string()),
                    t.args,
                    arguments,
                    depth,
                )
            }
            _ => Err("expression that is not a known constant".into()),
        }
    }

    /// The class and the constructor of `C(..)`, `C.name(..)`, `p.C(..)` and
    /// `p.C.name(..)`.
    fn constructor(
        &self,
        lib: LibraryId,
        tree: &ast::Ast,
        target: ast::ExprId,
    ) -> Result<(Class, Option<String>), String> {
        let class = |el: Option<Element>| match el {
            Some(Element::Class(id)) => Ok(self.class(id)),
            _ => Err("call that is not a constructor".to_string()),
        };
        match &tree.expr(target).kind {
            ast::ExprKind::Identifier(n) => {
                Ok((class(self.r.element_at(lib, None, self.name(n)))?, None))
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => match &tree.expr(*target).kind {
                ast::ExprKind::Identifier(a) if self.r.is_prefix(lib, self.name(a)) => Ok((
                    class(self.r.element_at(lib, Some(self.name(a)), self.name(name)))?,
                    None,
                )),
                ast::ExprKind::Identifier(a) => Ok((
                    class(self.r.element_at(lib, None, self.name(a)))?,
                    Some(self.name(name).to_string()),
                )),
                ast::ExprKind::Property {
                    target: p,
                    name: c,
                    null_aware: false,
                } => match &tree.expr(*p).kind {
                    ast::ExprKind::Identifier(pre) if self.r.is_prefix(lib, self.name(pre)) => {
                        Ok((
                            class(self.r.element_at(lib, Some(self.name(pre)), self.name(c)))?,
                            Some(self.name(name).to_string()),
                        ))
                    }
                    _ => Err("call that is not a constructor".into()),
                },
                _ => Err("call that is not a constructor".into()),
            },
            _ => Err("call that is not a constructor".into()),
        }
    }

    fn object(
        &self,
        unit: UnitId,
        class: Class,
        constructor: Option<String>,
        types: Vec<Type>,
        arguments: &ast::Arguments,
        depth: u32,
    ) -> Result<Value, String> {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for a in arguments.args.iter() {
            let v = self.value(unit, a.value, depth + 1)?;
            match a.name {
                Some(n) => named.push((self.name(&n).to_string(), v)),
                None => positional.push(v),
            }
        }
        Ok(Value::Object {
            class,
            constructor,
            types,
            positional,
            named,
        })
    }
}

// ---------------------------------------------------------------------------
// `@GenerateInjector`
// ---------------------------------------------------------------------------

const DI_MODULES: &str = "package:ngdart/src/meta/di_modules.dart";
const DI_GENERATE_INJECTOR: &str = "package:ngdart/src/meta/di_generate_injector.dart";

/// A type as `linkTypeOf` writes it: the class, by the library that declares
/// it, and the arguments. An empty `uri` is `dynamic`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenType {
    pub uri: String,
    pub symbol: String,
    pub args: Vec<WrittenType>,
}

impl WrittenType {
    fn of_class(uri: &str, symbol: &str) -> Self {
        WrittenType {
            uri: uri.into(),
            symbol: symbol.into(),
            args: Vec::new(),
        }
    }

    fn object() -> Self {
        Self::of_class("dart:core", "Object")
    }
}

/// A `useValue:` value as the `_reviveAny` of `InjectorReader` rebuilds it
/// (`ConstantReader.revive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revived {
    Null,
    Str(String),
    Integer(i64),
    Boolean(bool),
    List(Vec<Revived>),
    /// `C.field`, `Enum.value` or a top-level constant, by library.
    Access {
        uri: String,
        name: String,
    },
    /// `const C(..)`/`const C.name(..)`.
    Object {
        uri: String,
        class: String,
        constructor: Option<String>,
        positional: Vec<Revived>,
        named: Vec<(String, Revived)>,
    },
}

/// Where a provider of the generated injector takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectorSource {
    /// The class, the constructor (`findConstructor`: `None` is the unnamed
    /// one) and its dependencies.
    Class {
        uri: String,
        class: String,
        constructor: Option<String>,
        deps: Vec<Dependency>,
    },
    Existing(Token),
    Factory {
        uri: String,
        name: String,
        deps: Vec<Dependency>,
    },
    Value(Revived),
}

/// A provider of the injector (`ProviderElement`), with the type the emitter
/// writes (`providerType`, or the `useClass` class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectorProvider {
    pub token: Token,
    pub multi: bool,
    pub ty: WrittenType,
    pub source: InjectorSource,
}

/// A `@GenerateInjector` read (`InjectorReader`): the variable name and the
/// providers already flattened and without repetition (`deduplicateProviders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injector {
    pub name: String,
    pub providers: Vec<InjectorProvider>,
}

/// The positional and named arguments of a `const` call.
type ConstArguments = (Vec<Value>, Vec<(String, Value)>);

/// A module (`ModuleElement`): the direct providers and the included ones.
struct Module {
    provide: Vec<InjectorProvider>,
    include: Vec<Module>,
}

impl Module {
    /// `ModuleElement.flatten`: the included ones before the direct ones.
    fn flatten(self, output: &mut Vec<InjectorProvider>) {
        for m in self.include {
            m.flatten(output);
        }
        output.extend(self.provide);
    }
}

/// The `@GenerateInjector`s of the main unit of library `uri`
/// (`InjectorReader.findInjectors`), in the order of the top-level variables.
pub fn read_injectors(r: &Resolvedor, uri: &str) -> Vec<(String, Result<Injector, String>)> {
    let p = r.program();
    let Some(i) = p
        .libraries
        .iter()
        .position(|l| crate::dialect::canonical(&l.uri) == uri)
    else {
        return Vec::new();
    };
    let lib = LibraryId(i as u32);
    let Some(&unit) = p.library(lib).units.first() else {
        return Vec::new();
    };
    let reader = Leitor { r };
    let u = p.unit(unit);
    let mut output = Vec::new();
    for &d in &u.unit.declarations {
        let decl = u.ast.decl(d);
        let ast::DeclKind::Variables(l) = &decl.kind else {
            continue;
        };
        let annotation = decl.metadata.iter().find(|a| {
            reader
                .annotation_class(lib, a)
                .is_some_and(|c| c.e(DI_GENERATE_INJECTOR, "GenerateInjector"))
        });
        let Some(a) = annotation else {
            continue;
        };
        let lido = reader.injector(unit, a);
        for v in l.variables.iter() {
            let name = reader.name(&v.name).to_string();
            output.push((
                name.clone(),
                lido.clone().map(|providers| Injector { name, providers }),
            ));
        }
    }
    output
}

impl<'r, 'a> Leitor<'r, 'a> {
    /// `_computeProviders`: the annotation argument as a module, flattened,
    /// without the global services and without repetition.
    fn injector(&self, unit: UnitId, a: &ast::Annotation) -> Result<Vec<InjectorProvider>, String> {
        let arg = a
            .arguments
            .as_ref()
            .and_then(|args| args.args.first())
            .ok_or("@GenerateInjector without providers")?;
        let v = self.value(unit, arg.value, 0)?;
        let mut all = Vec::new();
        self.module(v, 0)?.flatten(&mut all);
        for p in &all {
            if let Token::Class { uri, class } = &p.token
                && uri.starts_with("package:ngdart/")
                && matches!(
                    class.as_str(),
                    "ApplicationRef" | "AppViewUtils" | "NgZone" | "Testability"
                )
            {
                return Err(format!("global service {class} in the injector"));
            }
        }
        // `deduplicateProviders`: of the non-multi ones, the last of each
        // token, in reverse order; then the multi ones, in order.
        let mut unique: Vec<InjectorProvider> = Vec::new();
        for p in all.iter().rev().filter(|p| !p.multi) {
            if !unique.iter().any(|u| u.token == p.token) {
                unique.push(p.clone());
            }
        }
        unique.extend(all.into_iter().filter(|p| p.multi));
        Ok(unique)
    }

    /// `ModuleReader.parseModule`.
    fn module(&self, v: Value, depth: u32) -> Result<Module, String> {
        if depth > DEPTH {
            return Err("module nested too deeply".into());
        }
        match v {
            // `_parseList`: lists and included `Module`s, the rest providers.
            Value::List(items) => {
                let mut include = Vec::new();
                let mut provide = Vec::new();
                for i in items {
                    if Self::is_module(&i) {
                        include.push(self.module(i, depth + 1)?);
                    } else {
                        provide.push(self.injector_provider(i)?);
                    }
                }
                Ok(Module { provide, include })
            }
            Value::Object {
                class,
                constructor: None,
                positional,
                named,
                ..
            } if class.e(DI_MODULES, "Module") && positional.is_empty() => {
                let mut include = Vec::new();
                let mut provide = Vec::new();
                for (n, x) in named {
                    let Value::List(items) = x else {
                        return Err(format!("Module({n}:) that is not a list"));
                    };
                    match n.as_str() {
                        "include" => {
                            for i in items {
                                include.push(self.module(i, depth + 1)?);
                            }
                        }
                        "provide" => {
                            for i in items {
                                provide.push(self.injector_provider(i)?);
                            }
                        }
                        other => return Err(format!("Module({other}:)")),
                    }
                }
                Ok(Module { provide, include })
            }
            _ => Err("module that is neither a list nor a Module".into()),
        }
    }

    fn is_module(v: &Value) -> bool {
        match v {
            Value::List(_) => true,
            Value::Object { class, .. } => class.e(DI_MODULES, "Module"),
            _ => false,
        }
    }

    /// `ProviderReader.parseProvider`, with the fields of the `Provider` taken
    /// from the written constructor.
    fn injector_provider(&self, v: Value) -> Result<InjectorProvider, String> {
        let (class, constructor, types, positional, named) = match v {
            // `Foo` is `ClassProvider(Foo)`
            // (`_parseTypeAsImplicitClassProvider`).
            Value::Type(c) => {
                return Ok(InjectorProvider {
                    token: Token::Class {
                        uri: c.uri.clone(),
                        class: c.name.clone(),
                    },
                    multi: false,
                    ty: WrittenType::of_class(&c.uri, &c.name),
                    source: self.injector_class(&c)?,
                });
            }
            Value::Object {
                class,
                constructor,
                types,
                positional,
                named,
            } if class.uri == DI_PROVIDERS => (class, constructor, types, positional, named),
            Value::Object { class, .. } => {
                return Err(format!("provider {} in the injector", class.name));
            }
            _ => return Err("unreadable provider in the injector".into()),
        };
        let exact = class.name == "Provider";
        match constructor.as_deref() {
            None => {}
            Some("forToken") if !exact => {}
            Some(c) => return Err(format!("{}.{c}", class.name)),
        }
        let mut pos = positional.into_iter();
        let token_v = pos.next().ok_or("provider without a token")?;
        let (mut use_class, mut use_value, mut use_existing, mut use_factory, mut deps) =
            (None, None, None, None, None);
        match class.name.as_str() {
            "Provider" | "ClassProvider" => {}
            "ExistingProvider" => use_existing = pos.next(),
            "FactoryProvider" => use_factory = pos.next(),
            "ValueProvider" => use_value = pos.next(),
            other => return Err(format!("provider {other}")),
        }
        if pos.next().is_some() {
            return Err(format!("{} with too many arguments", class.name));
        }
        for (n, x) in named {
            let target = match (class.name.as_str(), n.as_str()) {
                ("Provider" | "ClassProvider", "useClass") => &mut use_class,
                ("Provider", "useValue") => &mut use_value,
                ("Provider", "useExisting") => &mut use_existing,
                ("Provider", "useFactory") => &mut use_factory,
                ("Provider" | "FactoryProvider", "deps") => &mut deps,
                (_, other) => return Err(format!("{}(.., {other}:)", class.name)),
            };
            *target = Some(x);
        }
        // `ClassProvider._`: `useClass ?? token`.
        if class.name == "ClassProvider" && use_class.is_none() {
            use_class = Some(token_v.clone());
        }
        let token = self.token_lido(&token_v, true)?;
        let multi = matches!(token, Token::Multi { .. });
        // `_actualProviderType`: outside the exact `Provider`, the `T` of the
        // opaque token; otherwise the `T` of the provider — written, or
        // inferred (that of the value in `ValueProvider`; `Object`, the bound,
        // in the rest).
        let provider_type = |use_value: Option<&Value>| -> Result<WrittenType, String> {
            if !exact && let Token::Multi { ty, .. } | Token::Opaque { ty, .. } = &token {
                return Ok(Self::token_ty(ty));
            }
            if let Some(t) = types.first() {
                return self.written_type(t);
            }
            if class.name == "ValueProvider" {
                return self.value_type(use_value.ok_or("ValueProvider without a value")?);
            }
            Ok(WrittenType::object())
        };
        if let Some(c) = use_class {
            let Value::Type(c) = c else {
                return Err("useClass of something that is not a class".into());
            };
            return Ok(InjectorProvider {
                ty: WrittenType::of_class(&c.uri, &c.name),
                source: self.injector_class(&c)?,
                token,
                multi,
            });
        }
        if let Some(f) = use_factory {
            let ty = provider_type(None)?;
            return Ok(InjectorProvider {
                source: self.injector_factory(f, deps)?,
                token,
                multi,
                ty,
            });
        }
        if let Some(v) = use_value {
            let ty = provider_type(Some(&v))?;
            return Ok(InjectorProvider {
                source: InjectorSource::Value(self.revive(v, 0)?),
                token,
                multi,
                ty,
            });
        }
        if let Some(e) = use_existing {
            let ty = provider_type(None)?;
            return Ok(InjectorProvider {
                source: InjectorSource::Existing(self.token_lido(&e, true)?),
                token,
                multi,
                ty,
            });
        }
        // `Provider(Foo)` with nothing else: `useClass: Foo`.
        match (&token_v, exact) {
            (Value::Type(c), true) => Ok(InjectorProvider {
                ty: WrittenType::of_class(&c.uri, &c.name),
                source: self.injector_class(c)?,
                token,
                multi,
            }),
            _ => Err("provider without use*".into()),
        }
    }

    fn token_ty(t: &TokenType) -> WrittenType {
        written_token_type(t)
    }

    /// A written type argument, as `linkTypeOf` writes it.
    fn written_type(&self, t: &Type) -> Result<WrittenType, String> {
        let Some(c) = &t.class else {
            return Ok(WrittenType::of_class("", "dynamic"));
        };
        let params = self.r.program().class(c.id).type_params.len();
        let args = if t.args.is_empty() {
            (0..params)
                .map(|_| WrittenType::of_class("", "dynamic"))
                .collect()
        } else {
            t.args
                .iter()
                .map(|a| self.written_type(a))
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(WrittenType {
            uri: c.uri.clone(),
            symbol: c.name.clone(),
            args,
        })
    }

    /// The static type of a `ValueProvider(Type, value)` value, which infers
    /// the `T`.
    fn value_type(&self, v: &Value) -> Result<WrittenType, String> {
        Ok(match v {
            Value::Str(_) => WrittenType::of_class("dart:core", "String"),
            Value::Integer(_) => WrittenType::of_class("dart:core", "int"),
            Value::Boolean(_) => WrittenType::of_class("dart:core", "bool"),
            Value::Object { class, types, .. }
                if types.is_empty() && self.r.program().class(class.id).type_params.is_empty() =>
            {
                WrittenType::of_class(&class.uri, &class.name)
            }
            Value::Member { class, .. }
                if self.r.program().class(class.id).kind == ClassKind::Enum =>
            {
                WrittenType::of_class(&class.uri, &class.name)
            }
            _ => return Err("ValueProvider of a value without a known type".into()),
        })
    }

    /// `DependencyReader.findConstructor` and `_parseDependencies` of a class:
    /// the unnamed constructor (if the class is not abstract), otherwise the
    /// first public one of a concrete class or a `factory`.
    fn injector_class(&self, c: &Class) -> Result<InjectorSource, String> {
        let decl = self
            .declaration(c.id)
            .ok_or_else(|| format!("provider class without a declaration ({})", c.name))?;
        if !decl.type_params.is_empty() {
            return Err("provider of a generic class in the injector".into());
        }
        let mut constructors = Vec::new();
        for &m in decl.members {
            if let ast::MemberKind::Constructor(k) = &decl.ast.member(m).kind {
                constructors.push(k);
            }
        }
        let unnamed = constructors.iter().find(|k| k.name.is_none()).copied();
        let chosen = if !decl.is_abstract && (unnamed.is_some() || constructors.is_empty()) {
            unnamed
        } else {
            match constructors.iter().find(|k| {
                let public = k.name.is_none_or(|n| !self.name(&n).starts_with('_'));
                public && !decl.is_abstract || k.factory
            }) {
                Some(k) => Some(*k),
                None => return Err(format!("{} without a constructor for the injector", c.name)),
            }
        };
        let mut deps = Vec::new();
        if let Some(k) = chosen {
            for param in k.parameters.iter() {
                if let Some(d) = self.injector_dependency(&decl, param)? {
                    deps.push(d);
                }
            }
        }
        Ok(InjectorSource::Class {
            uri: c.uri.clone(),
            class: c.name.clone(),
            constructor: chosen.and_then(|k| k.name.map(|n| self.name(&n).to_string())),
            deps,
        })
    }

    /// A parameter as an injector dependency (`_parseDependencies`): the
    /// optional named and the optional positional ones without `@Optional` or a
    /// token are left out; optional is only `@Optional()`.
    fn injector_dependency(
        &self,
        decl: &Declaration<'a>,
        p: &ast::Parameter,
    ) -> Result<Option<Dependency>, String> {
        let mut optional = false;
        let mut with_token = false;
        for a in p.metadata.iter() {
            match self.annotation_class(decl.lib, a) {
                Some(c) if c.e(DI_ARGUMENTS, "Optional") => optional = true,
                Some(c) if c.e(DI_ARGUMENTS, "Inject") => with_token = true,
                Some(_) => {}
                None => with_token = true,
            }
        }
        match p.kind {
            ast::ParameterKind::Named if !p.required => return Ok(None),
            ast::ParameterKind::Named => {
                return Err("required named parameter in the injector".into());
            }
            ast::ParameterKind::Optional if !optional && !with_token => return Ok(None),
            _ => {}
        }
        let super_ty = (p.super_ && p.ty.is_none())
            .then(|| {
                let ctor = decl
                    .members
                    .iter()
                    .find_map(|&m| match &decl.ast.member(m).kind {
                        ast::MemberKind::Constructor(k)
                            if k.parameters.iter().any(|q| std::ptr::eq(q, p)) =>
                        {
                            Some(k)
                        }
                        _ => None,
                    })?;
                self.super_type(decl, ctor, p, 0)
            })
            .flatten();
        let mut d = self.dependency(decl, p, super_ty)?;
        if d.attribute.is_some() {
            return Err("@Attribute in the injector".into());
        }
        if matches!(d.token, Token::Element | Token::Detector) {
            return Err("HtmlElement/ChangeDetectorRef in the injector".into());
        }
        d.optional = optional;
        Ok(Some(d))
    }

    /// `useFactory:` with `deps:` written (`parseDependenciesList`) or the
    /// function's parameters.
    fn injector_factory(&self, f: Value, deps: Option<Value>) -> Result<InjectorSource, String> {
        let Value::Function { uri, name, id } = f else {
            return Err("useFactory that is not a top-level function".into());
        };
        let mut output = Vec::new();
        match deps {
            Some(Value::List(items)) => {
                for item in items {
                    let d = self.deps_dependency(item)?;
                    if matches!(d.token, Token::Element | Token::Detector) {
                        return Err("HtmlElement/ChangeDetectorRef in the injector".into());
                    }
                    output.push(d);
                }
            }
            None | Some(Value::Null) => {
                let p = self.r.program();
                let dartforge_elements::model::FunctionRef::Function { unit, function } =
                    p.function(id).node
                else {
                    return Err("useFactory without a declaration".into());
                };
                let u = p.unit(unit);
                let decl = Declaration {
                    id: None,
                    unit,
                    lib: u.library,
                    ast: &u.ast,
                    metadata: &[],
                    members: &[],
                    type_params: Vec::new(),
                    is_abstract: false,
                };
                let function = u.ast.function(function);
                if !function.type_params.is_empty() {
                    return Err("generic useFactory".into());
                }
                for param in function.parameters.iter().flat_map(|ps| ps.iter()) {
                    if let Some(d) = self.injector_dependency(&decl, param)? {
                        output.push(d);
                    }
                }
            }
            Some(_) => return Err("unreadable deps:".into()),
        }
        Ok(InjectorSource::Factory {
            uri,
            name,
            deps: output,
        })
    }

    /// `_reviveAny`: the constant value as `ConstantReader.revive` rebuilds it.
    fn revive(&self, v: Value, depth: u32) -> Result<Revived, String> {
        if depth > DEPTH {
            return Err("value nested too deeply".into());
        }
        Ok(match v {
            Value::Null => Revived::Null,
            Value::Str(s) => Revived::Str(s),
            Value::Integer(i) => Revived::Integer(i),
            Value::Boolean(b) => Revived::Boolean(b),
            Value::List(items) => Revived::List(
                items
                    .into_iter()
                    .map(|i| self.revive(i, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            Value::Member { class, name } => {
                if self.r.program().class(class.id).kind != ClassKind::Enum {
                    // The value of the static field, revived through its class.
                    let v = self.static_field(class.id, &name, depth)?;
                    return self.revive(v, depth + 1);
                }
                Revived::Access {
                    uri: class.uri.clone(),
                    name: format!("{}.{name}", class.name),
                }
            }
            object @ Value::Object { .. } => self.revive_object(object, depth)?,
            Value::Type(_) => return Err("type value in the injector (Type)".into()),
            Value::Function { .. } => return Err("function value in the injector".into()),
        })
    }

    /// `reviveInstance`: a public `const` field of the class itself with the
    /// same value; otherwise a `const` field of a class of its library;
    /// otherwise the constructor call; otherwise a top-level constant of the
    /// library.
    fn revive_object(&self, v: Value, depth: u32) -> Result<Revived, String> {
        let Value::Object {
            class,
            constructor,
            types,
            positional,
            named,
        } = &v
        else {
            unreachable!()
        };
        if !types.is_empty() || !self.r.program().class(class.id).type_params.is_empty() {
            return Err("constant value of a generic class in the injector".into());
        }
        let p = self.r.program();
        let public = |n: &str| !n.starts_with('_');
        // 1. The public `const` fields of the class itself.
        for &f in &p.class(class.id).fields {
            let var = p.variable(f);
            let name = self.r.interner().resolve(var.name);
            if var.const_
                && var.static_
                && public(name)
                && let Ok(x) = self.variable(f, depth)
                && self.same_object(&x, &v)
            {
                return Ok(Revived::Access {
                    uri: class.uri.clone(),
                    name: format!("{}.{name}", class.name),
                });
            }
        }
        // 2. The `const` fields of the classes of the library's main unit.
        let lib = p.class(class.id).library;
        let mut candidates: Vec<Revived> = Vec::new();
        let unit = *p
            .library(lib)
            .units
            .first()
            .ok_or("library without a unit")?;
        let u = p.unit(unit);
        for &d in &u.unit.declarations {
            let ast::DeclKind::Class(k) = &u.ast.decl(d).kind else {
                continue;
            };
            let owner = self.name(&k.name);
            let Some(Element::Class(cid)) = self.r.element_at(lib, None, owner) else {
                continue;
            };
            for &f in &p.class(cid).fields {
                let var = p.variable(f);
                if !(var.const_ && var.static_) {
                    continue;
                }
                let name = self.r.interner().resolve(var.name);
                if let Ok(x) = self.variable(f, depth)
                    && self.same_object(&x, &v)
                {
                    let r = Revived::Access {
                        uri: class.uri.clone(),
                        name: format!("{owner}.{name}"),
                    };
                    if public(name) {
                        return Ok(r);
                    }
                    candidates.push(r);
                }
            }
        }
        // 3. A chamada do construtor.
        let is_private = !public(&class.name) || constructor.as_deref().is_some_and(|c| !public(c));
        if !is_private {
            return Ok(Revived::Object {
                uri: class.uri.clone(),
                class: class.name.clone(),
                constructor: constructor.clone(),
                positional: positional
                    .iter()
                    .map(|x| self.revive(x.clone(), depth + 1))
                    .collect::<Result<_, _>>()?,
                named: named
                    .iter()
                    .map(|(n, x)| Ok((n.clone(), self.revive(x.clone(), depth + 1)?)))
                    .collect::<Result<_, String>>()?,
            });
        }
        // 4. The top-level constants of the library.
        for &d in &u.unit.declarations {
            let ast::DeclKind::Variables(l) = &u.ast.decl(d).kind else {
                continue;
            };
            if !l.const_ {
                continue;
            }
            for x in l.variables.iter() {
                let name = self.name(&x.name);
                if let Some(e) = x.initializer
                    && let Ok(val) = self.value(unit, e, depth + 1)
                    && self.same_object(&val, &v)
                    && public(name)
                {
                    return Ok(Revived::Access {
                        uri: crate::dialect::canonical_string(p.library(lib).uri.clone()),
                        name: name.to_string(),
                    });
                }
            }
        }
        candidates
            .into_iter()
            .next()
            .ok_or_else(|| format!("value of {} without public access", class.name))
    }

    /// The value of a `const` static field of the class.
    fn static_field(&self, c: ClassId, name: &str, depth: u32) -> Result<Value, String> {
        let p = self.r.program();
        for &f in &p.class(c).fields {
            let var = p.variable(f);
            if var.static_ && self.r.interner().resolve(var.name) == name {
                return self.variable(f, depth);
            }
        }
        Err(format!("static field {name} not found"))
    }

    /// Two constant values are the same object (`DartObject`'s `==`):
    /// primitives by value; objects by class, constructor and arguments with
    /// the defaults filled in.
    fn same_object(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Null, Value::Null) => true,
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Integer(x), Value::Integer(y)) => x == y,
            (Value::Boolean(x), Value::Boolean(y)) => x == y,
            (Value::Type(x), Value::Type(y)) => x.id == y.id,
            (Value::Function { id: x, .. }, Value::Function { id: y, .. }) => x == y,
            (
                Value::Member {
                    class: c1,
                    name: n1,
                },
                Value::Member {
                    class: c2,
                    name: n2,
                },
            ) => c1.id == c2.id && n1 == n2,
            (Value::List(x), Value::List(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(i, j)| self.same_object(i, j))
            }
            (
                Value::Object {
                    class: c1,
                    constructor: k1,
                    ..
                },
                Value::Object {
                    class: c2,
                    constructor: k2,
                    ..
                },
            ) => {
                if c1.id != c2.id || k1 != k2 {
                    return false;
                }
                match (self.with_defaults(a), self.with_defaults(b)) {
                    (Some((p1, n1)), Some((p2, n2))) => {
                        p1.len() == p2.len()
                            && p1.iter().zip(&p2).all(|(i, j)| self.same_object(i, j))
                            && n1.len() == n2.len()
                            && n1.iter().all(|(n, x)| {
                                n2.iter().any(|(m, y)| m == n && self.same_object(x, y))
                            })
                    }
                    _ => false,
                }
            }
            // An enum static field against the same field read.
            _ => false,
        }
    }

    /// The arguments of a `const` call with the constructor's defaults filled
    /// in (what is missing and has no default is `null`).
    fn with_defaults(&self, v: &Value) -> Option<ConstArguments> {
        let Value::Object {
            class,
            constructor,
            positional,
            named,
            ..
        } = v
        else {
            return None;
        };
        let decl = self.declaration(class.id)?;
        let k = decl
            .members
            .iter()
            .find_map(|&m| match &decl.ast.member(m).kind {
                ast::MemberKind::Constructor(k)
                    if k.name.map(|n| self.name(&n).to_string()) == *constructor =>
                {
                    Some(k)
                }
                _ => None,
            })?;
        let default = |param: &ast::Parameter| -> Option<Value> {
            match param.default_value {
                Some(e) => self.value(decl.unit, e, 1).ok(),
                None => Some(Value::Null),
            }
        };
        let mut pos = Vec::new();
        let mut nom = Vec::new();
        let mut i = 0;
        for param in k.parameters.iter() {
            match param.kind {
                ast::ParameterKind::Named => {
                    let n = self.name(param.name.as_ref()?).to_string();
                    let x = match named.iter().find(|(m, _)| *m == n) {
                        Some((_, x)) => x.clone(),
                        None => default(param)?,
                    };
                    nom.push((n, x));
                }
                _ => {
                    let x = match positional.get(i) {
                        Some(x) => x.clone(),
                        None => default(param)?,
                    };
                    pos.push(x);
                    i += 1;
                }
            }
        }
        Some((pos, nom))
    }
}

/// The [`WrittenType`] of the `T` of a token, with the concrete arguments or,
/// without them, `dynamic` for each parameter.
pub(crate) fn written_token_type(t: &TokenType) -> WrittenType {
    if t.is_dynamic() {
        return WrittenType::of_class("", "dynamic");
    }
    WrittenType {
        uri: t.uri.clone(),
        symbol: t.class.clone(),
        args: if t.args.is_empty() {
            (0..t.generics)
                .map(|_| WrittenType::of_class("", "dynamic"))
                .collect()
        } else {
            t.args.iter().map(written_token_type).collect()
        },
    }
}
