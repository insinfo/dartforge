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
//! name. What cannot be read goes to `Diretiva::fora`, and whoever uses the
//! directive refuses.
use crate::component::Ganchos;
use crate::directives::{
    DI_TOKENS, Dependencia, Diretiva, Entrada, Fornece, Ouvinte, Provedor, TipoDeToken, Token,
    ValorConst,
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
const PROFUNDIDADE: u32 = 16;

/// An already resolved class.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Classe {
    uri: String,
    nome: String,
    id: ClassId,
}

impl Classe {
    fn e(&self, uri: &str, nome: &str) -> bool {
        self.uri == uri && self.nome == nome
    }
}

/// A written type, resolved: the class and the arguments; `None` for the class
/// is `dynamic`.
#[derive(Debug, Clone)]
struct Tipo {
    classe: Option<Classe>,
    args: Vec<Tipo>,
}

/// The constant value of an expression, in the part the metadata uses.
#[derive(Debug, Clone)]
enum Valor {
    Texto(String),
    Inteiro(i64),
    Booleano(bool),
    Nulo,
    /// A top-level function (`useFactory:`), by the library that declares it.
    Funcao {
        uri: String,
        nome: String,
        id: FunctionElementId,
    },
    Lista(Vec<Valor>),
    /// A type literal (`NgModel` in a provider list).
    Tipo(Classe),
    /// `C<T>.ctor(a, b, nome: c)`.
    Objeto {
        classe: Classe,
        construtor: Option<String>,
        tipos: Vec<Tipo>,
        posicionais: Vec<Valor>,
        nomeados: Vec<(String, Valor)>,
    },
    /// `C.name`: enum constant or static field.
    Membro {
        classe: Classe,
        nome: String,
    },
}

/// Reads the metadata of class `id`, if it has `@Directive` or `@Component`.
pub fn ler(r: &Resolvedor, id: ClassId) -> Option<Diretiva> {
    Leitor { r }.diretiva(id)
}

struct Leitor<'r, 'a> {
    r: &'r Resolvedor<'a>,
}

/// Where a class is declared: the unit, the library, the members and the names
/// of the type parameters.
struct Declaracao<'a> {
    /// The declared class (`None` for the `useFactory:` function).
    id: Option<ClassId>,
    unidade: UnitId,
    lib: LibraryId,
    ast: &'a ast::Ast,
    metadados: &'a [ast::Annotation],
    membros: &'a [ast::MemberId],
    parametros_de_tipo: Vec<String>,
    abstrata: bool,
}

impl<'r, 'a> Leitor<'r, 'a> {
    fn nome(&self, n: &ast::Name) -> &'a str {
        self.r.interner().resolve(n.sym)
    }

    fn declaracao(&self, id: ClassId) -> Option<Declaracao<'a>> {
        let p = self.r.programa();
        let c = p.class(id);
        let d = c.decl?;
        let u = p.unit(d.unit);
        let decl = u.ast.decl(d.decl);
        let (membros, tps, abstrata): (&[ast::MemberId], &[ast::TypeParameter], bool) = match &decl
            .kind
        {
            ast::DeclKind::Class(k) => (&k.members[..], &k.type_params[..], k.modifiers.abstract_),
            ast::DeclKind::Mixin(m) => (&m.members[..], &m.type_params[..], true),
            _ => return None,
        };
        Some(Declaracao {
            id: Some(id),
            unidade: d.unit,
            lib: u.library,
            ast: &u.ast,
            metadados: &decl.metadata,
            membros,
            parametros_de_tipo: tps.iter().map(|t| self.nome(&t.name).to_string()).collect(),
            abstrata,
        })
    }

    fn classe(&self, id: ClassId) -> Classe {
        let p = self.r.programa();
        let c = p.class(id);
        Classe {
            uri: crate::dialect::canonica_string(p.library(c.library).uri.clone()),
            nome: self.r.interner().resolve(c.name).to_string(),
            id,
        }
    }

    /// The analyzer's `allSupertypes` (`ClassHierarchy._getHierarchy`): for the
    /// supertype, the `on` constraints, the interfaces and the mixins, in that
    /// order, the type itself and then its supertypes, without repetition.
    fn supertipos(&self, id: ClassId, profundidade: u32) -> Vec<ClassId> {
        let mut saida: Vec<ClassId> = Vec::new();
        if profundidade > 64 {
            return saida;
        }
        let c = self.r.programa().class(id);
        let mut diretos: Vec<ClassId> = Vec::new();
        diretos.extend(c.supertype_class);
        if c.kind == ClassKind::Mixin {
            diretos.extend(c.on_classes.iter().copied());
        }
        diretos.extend(c.interface_classes.iter().copied());
        diretos.extend(c.mixin_classes.iter().copied());
        for t in diretos {
            if !saida.contains(&t) {
                saida.push(t);
            }
            for x in self.supertipos(t, profundidade + 1) {
                if !saida.contains(&x) {
                    saida.push(x);
                }
            }
        }
        saida
    }

    /// The class an annotation name designates (`Input`, `ng.Input`).
    fn classe_da_anotacao(&self, lib: LibraryId, a: &ast::Annotation) -> Option<Classe> {
        let partes: Vec<&str> = a.name.iter().map(|n| self.nome(n)).collect();
        let el = match partes.as_slice() {
            [n] => self.r.elemento_em(lib, None, n),
            [p, n] | [p, n, _] if self.r.e_prefixo(lib, p) => self.r.elemento_em(lib, Some(p), n),
            [n, _] => self.r.elemento_em(lib, None, n),
            _ => None,
        }?;
        match el {
            Element::Class(id) => Some(self.classe(id)),
            _ => None,
        }
    }

    /// If the annotation is one of those of ngdart's `directives.dart`, its
    /// name. An annotation with one of those names that does not resolve there
    /// is recorded in `fora`: the analyzer would have resolved it, and silently
    /// losing it would give wrong metadata.
    fn anotacao_do_ngdart(
        &self,
        lib: LibraryId,
        a: &ast::Annotation,
        fora: &mut Vec<String>,
    ) -> Option<&'static str> {
        const NOMES: &[&str] = &[
            "Input",
            "Output",
            "HostListener",
            "HostBinding",
            "ContentChild",
            "ContentChildren",
            "ViewChild",
            "ViewChildren",
        ];
        let escrito = crate::nome_da_anotacao(a, self.r.interner());
        let conhecido = NOMES.iter().copied().find(|n| *n == escrito);
        match self.classe_da_anotacao(lib, a) {
            Some(c) if c.uri == DIRECTIVES => NOMES.iter().copied().find(|n| *n == c.nome),
            Some(_) => None,
            None => {
                if let Some(n) = conhecido {
                    fora.push(format!("@{n} not resolved"));
                }
                None
            }
        }
    }

    fn diretiva(&self, id: ClassId) -> Option<Diretiva> {
        let decl = self.declaracao(id)?;
        let mut anotacao = None;
        for a in decl.metadados {
            if let Some(c) = self.classe_da_anotacao(decl.lib, a)
                && c.uri == DIRECTIVES
                && (c.nome == "Directive" || c.nome == "Component")
            {
                anotacao = Some((a, c.nome == "Component"));
                break;
            }
        }
        let (anotacao, e_componente) = anotacao?;
        let eu = self.classe(id);
        let mut d = Diretiva {
            classe: eu.nome.clone(),
            uri: eu.uri.clone(),
            e_componente,
            ..Default::default()
        };
        self.argumentos(&decl, anotacao, &mut d);
        // `_collectInheritableMetadata` e `DirectiveVisitor.visitDirective`.
        let mut ordem: Vec<ClassId> = self.supertipos(id, 0);
        ordem.reverse();
        ordem.push(id);
        let mut entradas: Vec<(String, Entrada)> = Vec::new();
        for &c in &ordem {
            if self
                .r
                .programa()
                .library(self.r.programa().class(c).library)
                .is_sdk
            {
                continue;
            }
            let Some(dc) = self.declaracao(c) else {
                continue;
            };
            self.membros(&dc, c == id, &mut d, &mut entradas);
        }
        d.entradas = entradas.into_iter().map(|(_, e)| e).collect();
        // Generic directive with `@HostBinding`: the `XNgCd<T>` takes type
        // arguments (`lookupTypeArgumentsOf`) — no case yet.
        if !d.ligacoes_do_hospedeiro.is_empty()
            && !decl.parametros_de_tipo.is_empty()
            && !e_componente
        {
            d.fora.push("generic directive with @HostBinding".into());
        }
        d.ganchos = self.ganchos(&ordem);
        self.dependencias(&decl, &mut d);
        Some(d)
    }

    /// The arguments of the annotation.
    fn argumentos(&self, decl: &Declaracao, a: &ast::Annotation, d: &mut Diretiva) {
        let Some(args) = &a.arguments else {
            return;
        };
        for arg in args.args.iter() {
            let Some(nome) = arg.name.as_ref().map(|n| self.nome(n)) else {
                continue;
            };
            let valor = || self.valor(decl.unidade, arg.value, 0);
            match nome {
                "selector" => match valor() {
                    Ok(Valor::Texto(s)) => d.seletor = s,
                    _ => d.fora.push("selector: that is not a constant string".into()),
                },
                "exportAs" => match valor() {
                    Ok(Valor::Texto(s)) => d.export_as = Some(s),
                    Ok(Valor::Nulo) => {}
                    _ => d.fora.push("exportAs: that is not a constant string".into()),
                },
                "visibility" => match valor() {
                    Ok(Valor::Membro { classe, nome }) if classe.e(VISIBILITY, "Visibility") => {
                        d.visivel = nome == "all";
                    }
                    _ => d.fora.push("unreadable visibility:".into()),
                },
                "providers" => match valor() {
                    Ok(v) => self.provedores(v, d),
                    Err(f) => d.fora.push(format!("providers: {f}")),
                },
                // Read like the `providers:` and kept apart (private).
                "viewProviders" => match valor() {
                    Ok(v) => {
                        let publicos = std::mem::take(&mut d.provedores);
                        self.provedores(v, d);
                        d.provedores_de_visao = std::mem::replace(&mut d.provedores, publicos);
                    }
                    Err(f) => d.fora.push(format!("viewProviders: {f}")),
                },
                _ => {}
            }
        }
    }

    /// `ModuleReader.extractProviderObjects`: lists flattened; each item a
    /// provider (`createProviderMetadata`). What cannot be read goes to `fora`.
    fn provedores(&self, v: Valor, d: &mut Diretiva) {
        match v {
            Valor::Lista(itens) => {
                for i in itens {
                    self.provedores(i, d);
                }
            }
            Valor::Nulo => {}
            outro => match self.provedor(outro) {
                Ok(p) => d.provedores.push(p),
                Err(f) => d.fora.push(f),
            },
        }
    }

    /// A provider: a type literal (`useClass` of the type itself) or a
    /// `Provider`/`ClassProvider`/`ExistingProvider`/`FactoryProvider`/
    /// `ValueProvider` (and the `.forToken` of each).
    fn provedor(&self, v: Valor) -> Result<Provedor, String> {
        let (classe, construtor, tipos, posicionais, nomeados) = match v {
            Valor::Tipo(c) => {
                return Ok(Provedor {
                    token: Token::Classe {
                        uri: c.uri.clone(),
                        classe: c.nome.clone(),
                    },
                    fonte: self.fonte_de_classe(&c)?,
                    multi: false,
                    tipo: None,
                });
            }
            Valor::Objeto {
                classe,
                construtor,
                tipos,
                posicionais,
                nomeados,
            } if classe.uri == DI_PROVIDERS => (classe, construtor, tipos, posicionais, nomeados),
            Valor::Objeto { classe, .. } => return Err(format!("provedor {}", classe.nome)),
            _ => return Err("unreadable provider".into()),
        };
        let tipo_do_provedor = classe.nome.as_str();
        let por_token = match construtor.as_deref() {
            None => false,
            Some("forToken") if tipo_do_provedor != "Provider" => true,
            Some(c) => return Err(format!("{tipo_do_provedor}.{c}")),
        };
        // The fields of the `Provider`, through the constructor used.
        let mut pos = posicionais.into_iter();
        let token_v = pos.next().ok_or("provedor sem token")?;
        let mut use_class = None;
        let mut use_value = None;
        let mut use_existing = None;
        let mut use_factory = None;
        let mut deps = None;
        match tipo_do_provedor {
            "Provider" | "ClassProvider" => {}
            "ExistingProvider" => use_existing = pos.next(),
            "FactoryProvider" => use_factory = pos.next(),
            "ValueProvider" => use_value = pos.next(),
            outro => return Err(format!("provedor {outro}")),
        }
        if pos.next().is_some() {
            return Err(format!("{tipo_do_provedor} com argumentos demais"));
        }
        for (n, x) in nomeados {
            let alvo = match (tipo_do_provedor, n.as_str()) {
                ("Provider" | "ClassProvider", "useClass") => &mut use_class,
                ("Provider", "useValue") => &mut use_value,
                ("Provider", "useExisting") => &mut use_existing,
                ("Provider", "useFactory") => &mut use_factory,
                ("Provider" | "FactoryProvider", "deps") => &mut deps,
                (_, outro) => return Err(format!("{tipo_do_provedor}(.., {outro}:)")),
            };
            *alvo = Some(x);
        }
        let token = self.token_do_valor(&token_v)?;
        let multi = matches!(token, Token::Multi { .. });
        // `useValue: null` cannot be told from its absence from here.
        if matches!(use_value, Some(Valor::Nulo)) {
            return Err("useValue: null".into());
        }
        let quantos = [&use_class, &use_value, &use_existing, &use_factory]
            .iter()
            .filter(|x| x.is_some())
            .count();
        if quantos > 1 {
            return Err("provider with more than one use*".into());
        }
        if deps.is_some() && use_factory.is_none() {
            return Err("deps: sem useFactory".into());
        }
        let fonte = if let Some(e) = use_existing {
            Fornece::Existente(self.token_do_valor(&e)?)
        } else if let Some(f) = use_factory {
            self.fonte_de_fabrica(f, deps)?
        } else if let Some(v) = use_value.clone() {
            Fornece::Valor(self.valor_const(v, 0)?)
        } else {
            let c = match use_class.unwrap_or(token_v) {
                Valor::Tipo(c) => c,
                _ => return Err("useClass of something that is not a class".into()),
            };
            self.fonte_de_classe(&c)?
        };
        // `inferProviderType`: the `T` of the `MultiToken`; otherwise the `T`
        // of the provider (written or inferred by the analyzer), unless it is
        // `dynamic` or `Object`.
        let tipo = if let Token::Multi { tipo, .. } = &token {
            Some(tipo.clone())
        } else if let Some(t) = tipos.first() {
            let Some(c) = &t.classe else {
                return Err("provider with a dynamic type argument".into());
            };
            if c.e("dart:core", "Object") {
                None
            } else {
                Some(self.tipo_sem_argumentos(c, t)?)
            }
        } else if por_token {
            match &token {
                Token::Opaco { tipo, .. } if !tipo.e_object() => Some(tipo.clone()),
                _ => None,
            }
        } else if tipo_do_provedor == "ValueProvider" {
            // `ValueProvider(Tipo, valor)`: o `T` sai do valor.
            match use_value {
                Some(Valor::Texto(_)) => Some(Self::do_core("String")),
                Some(Valor::Inteiro(_)) => Some(Self::do_core("int")),
                Some(Valor::Booleano(_)) => Some(Self::do_core("bool")),
                Some(Valor::Objeto { classe, .. }) => Some(TipoDeToken {
                    uri: classe.uri.clone(),
                    classe: classe.nome.clone(),
                    genericos: 0,
                    args: Vec::new(),
                }),
                _ => return Err("ValueProvider of a value without a known type".into()),
            }
        } else {
            None
        };
        Ok(Provedor {
            token,
            fonte,
            multi,
            tipo,
        })
    }

    /// The `T` of a token as `fromDartType` writes it: the class and the
    /// arguments, recursively; arguments that are all `dynamic` stay in the
    /// short form (empty `args`).
    fn tipo_de_token(&self, t: &Tipo, profundidade: u32) -> Result<TipoDeToken, String> {
        let Some(c) = &t.classe else {
            return Ok(TipoDeToken::dinamico());
        };
        if profundidade > 16 {
            return Err("token type nested too deeply".into());
        }
        let parametros = self.r.programa().class(c.id).type_params.len();
        if t.args.len() != parametros {
            return Err("token type without all the arguments".into());
        }
        let args: Vec<TipoDeToken> = t
            .args
            .iter()
            .map(|a| self.tipo_de_token(a, profundidade + 1))
            .collect::<Result<_, _>>()?;
        Ok(TipoDeToken {
            uri: c.uri.clone(),
            classe: c.nome.clone(),
            genericos: parametros,
            args: if args.iter().all(TipoDeToken::e_dinamico) {
                Vec::new()
            } else {
                args
            },
        })
    }

    fn do_core(nome: &str) -> TipoDeToken {
        TipoDeToken {
            uri: "dart:core".into(),
            classe: nome.into(),
            genericos: 0,
            args: Vec::new(),
        }
    }

    /// A type without arguments (or only with `dynamic`), as `fromDartType`
    /// writes it.
    fn tipo_sem_argumentos(&self, c: &Classe, t: &Tipo) -> Result<TipoDeToken, String> {
        let parametros = self.r.programa().class(c.id).type_params.len();
        if !t.args.is_empty()
            && (t.args.len() != parametros || t.args.iter().any(|a| a.classe.is_some()))
        {
            return Err("provider type with arguments".into());
        }
        Ok(TipoDeToken {
            uri: c.uri.clone(),
            classe: c.nome.clone(),
            genericos: parametros,
            args: Vec::new(),
        })
    }

    /// `useClass:` (`_getUseClass`, `enforceClassCanBeCreated`): the class and
    /// the dependencies of the unnamed constructor.
    fn fonte_de_classe(&self, c: &Classe) -> Result<Fornece, String> {
        let decl = self
            .declaracao(c.id)
            .ok_or_else(|| format!("provider class without a declaration ({})", c.nome))?;
        if !decl.parametros_de_tipo.is_empty() {
            return Err("provider of a generic class".into());
        }
        if decl.abstrata {
            return Err("provedor de classe abstrata".into());
        }
        let mut d = Diretiva::default();
        self.dependencias(&decl, &mut d);
        if let Some(f) = d.fora.into_iter().next() {
            return Err(f);
        }
        Ok(Fornece::Classe {
            uri: c.uri.clone(),
            classe: c.nome.clone(),
            deps: d.dependencias,
        })
    }

    /// `useFactory:` of a top-level function, with `deps:` (if not empty) or
    /// its parameters (`_factoryForFunction`).
    fn fonte_de_fabrica(&self, f: Valor, deps: Option<Valor>) -> Result<Fornece, String> {
        let Valor::Funcao { uri, nome, id } = f else {
            return Err("useFactory that is not a top-level function".into());
        };
        let lista = match deps {
            None | Some(Valor::Nulo) => Vec::new(),
            Some(Valor::Lista(l)) => l,
            Some(_) => return Err("unreadable deps:".into()),
        };
        let mut saida = Vec::new();
        if !lista.is_empty() {
            for item in lista {
                saida.push(self.dependencia_de_deps(item)?);
            }
        } else {
            let p = self.r.programa();
            let fe = p.function(id);
            let dartforge_elements::model::FunctionRef::Function { unit, function } = fe.node
            else {
                return Err("useFactory without a declaration".into());
            };
            let u = p.unit(unit);
            let decl = Declaracao {
                id: None,
                unidade: unit,
                lib: u.library,
                ast: &u.ast,
                metadados: &[],
                membros: &[],
                parametros_de_tipo: Vec::new(),
                abstrata: false,
            };
            let funcao = u.ast.function(function);
            if !funcao.type_params.is_empty() {
                return Err("generic useFactory".into());
            }
            for par in funcao.parameters.iter().flat_map(|ps| ps.iter()) {
                if matches!(par.kind, ast::ParameterKind::Named) {
                    continue;
                }
                saida.push(self.dependencia(&decl, par, None)?);
            }
        }
        Ok(Fornece::Fabrica {
            uri,
            nome,
            deps: saida,
        })
    }

    /// An item of `deps:` (`_factoryDiDep`): a type, a token, or the list
    /// `[token, Optional(), ..]`.
    fn dependencia_de_deps(&self, v: Valor) -> Result<Dependencia, String> {
        let mut dep = Dependencia {
            token: Token::Elemento,
            opcional: false,
            proprio: false,
            hospedeiro: false,
            pular: false,
            atributo: None,
        };
        match v {
            Valor::Lista(itens) => {
                let mut it = itens.into_iter();
                let primeiro = it.next().ok_or("deps: with an empty list")?;
                dep.token = self.token_de_dependencia(&primeiro)?;
                for m in it {
                    match m {
                        Valor::Objeto { classe, .. } if classe.uri == DI_ARGUMENTS => {
                            match classe.nome.as_str() {
                                "Optional" => dep.opcional = true,
                                "Self" => dep.proprio = true,
                                "Host" => dep.hospedeiro = true,
                                "SkipSelf" => dep.pular = true,
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            outro => dep.token = self.token_de_dependencia(&outro)?,
        }
        Ok(dep)
    }

    /// The token of a dependency written as a value: `HtmlElement` and
    /// `ChangeDetectorRef` become the built-ins, as in the parameter.
    fn token_de_dependencia(&self, v: &Valor) -> Result<Token, String> {
        match v {
            Valor::Tipo(c)
                if crate::dialect::no_embutido(&c.uri, &c.nome).is_some() =>
            {
                Ok(Token::Elemento)
            }
            Valor::Tipo(c)
                if c.uri.starts_with("package:ngdart/") && c.nome == "ChangeDetectorRef" =>
            {
                Ok(Token::Detector)
            }
            outro => self.token_do_valor(outro),
        }
    }

    /// The `_useValueExpression` in the part the emitter writes: string,
    /// integer, boolean and constant object of a non-generic class with
    /// arguments of those kinds.
    fn valor_const(&self, v: Valor, profundidade: u32) -> Result<ValorConst, String> {
        Ok(match v {
            Valor::Texto(s) => ValorConst::Texto(s),
            Valor::Inteiro(i) => ValorConst::Inteiro(i),
            Valor::Booleano(b) => ValorConst::Booleano(b),
            Valor::Objeto {
                classe,
                construtor,
                tipos,
                posicionais,
                nomeados,
            } if profundidade == 0 => {
                if !tipos.is_empty() || !self.r.programa().class(classe.id).type_params.is_empty() {
                    return Err("useValue of a generic class".into());
                }
                if construtor.as_deref().is_some_and(|c| c.starts_with('_')) {
                    return Err("useValue with a private constructor".into());
                }
                ValorConst::Objeto {
                    uri: classe.uri.clone(),
                    classe: classe.nome.clone(),
                    construtor,
                    posicionais: posicionais
                        .into_iter()
                        .map(|x| self.valor_const(x, profundidade + 1))
                        .collect::<Result<_, _>>()?,
                    nomeados: nomeados
                        .into_iter()
                        .map(|(n, x)| Ok((n, self.valor_const(x, profundidade + 1)?)))
                        .collect::<Result<_, String>>()?,
                }
            }
            _ => return Err("useValue other than a string, integer, boolean or simple object".into()),
        })
    }

    /// The token of a constant value: a class, a `MultiToken` or an
    /// `OpaqueToken`.
    fn token_do_valor(&self, v: &Valor) -> Result<Token, String> {
        self.token_lido(v, false)
    }

    /// The token of a value; `sem_nome`: `const OpaqueToken<T>()` is accepted
    /// (in the generated injector it becomes `const OpaqueToken<T>()`; in the
    /// views there is no case yet).
    fn token_lido(&self, v: &Valor, sem_nome: bool) -> Result<Token, String> {
        match v {
            Valor::Tipo(c) => Ok(Token::Classe {
                uri: c.uri.clone(),
                classe: c.nome.clone(),
            }),
            Valor::Objeto {
                classe,
                construtor: None,
                tipos,
                posicionais,
                ..
            } if classe.e(DI_TOKENS, "MultiToken") || classe.e(DI_TOKENS, "OpaqueToken") => {
                let multi = classe.nome == "MultiToken";
                let nome = match posicionais.first() {
                    Some(Valor::Texto(s)) => s.clone(),
                    None if multi || sem_nome => String::new(),
                    None => return Err("OpaqueToken without a name".into()),
                    _ => return Err("token with an unreadable name".into()),
                };
                // Without a type argument, written or from the context,
                // inference gives the bound of `T` (`T extends Object`):
                // `OpaqueToken<Object>`.
                let objeto;
                let t = match tipos.as_slice() {
                    [t] => t,
                    [] => {
                        let id = self
                            .r
                            .classe_por_uri("dart:core", "Object")
                            .ok_or("Object of dart:core not found")?;
                        objeto = Tipo {
                            classe: Some(self.classe(id)),
                            args: Vec::new(),
                        };
                        &objeto
                    }
                    _ => return Err(format!("{} with too many type arguments", classe.nome)),
                };
                if t.classe.is_none() {
                    return Err(format!("{}<dynamic>", classe.nome));
                }
                let tipo = self.tipo_de_token(t, 0)?;
                Ok(if multi {
                    Token::Multi { nome, tipo }
                } else {
                    Token::Opaco { nome, tipo }
                })
            }
            Valor::Objeto { classe, .. } => Err(format!("token {}", classe.nome)),
            _ => Err("unreadable token".into()),
        }
    }

    /// The annotated members of a class of the hierarchy; `propria` when it is
    /// the directive's class (and not a supertype).
    fn membros(
        &self,
        dc: &Declaracao,
        propria: bool,
        d: &mut Diretiva,
        entradas: &mut Vec<(String, Entrada)>,
    ) {
        let mut de_campo: Vec<(String, Entrada)> = Vec::new();
        let mut de_setter: Vec<(String, Entrada)> = Vec::new();
        let mut saidas_acessor: Vec<(String, String)> = Vec::new();
        let mut saidas_campo: Vec<(String, String)> = Vec::new();
        let mut ligacoes_acessor = Vec::new();
        let mut ligacoes_metodo = Vec::new();
        let mut ligacoes_campo = Vec::new();
        let mut consultas_setter = Vec::new();
        let mut consultas_campo = Vec::new();
        for &m in dc.membros {
            let membro = dc.ast.member(m);
            for a in membro.metadata.iter() {
                let Some(qual) = self.anotacao_do_ngdart(dc.lib, a, &mut d.fora) else {
                    continue;
                };
                let apelido = self.primeiro_texto(dc, a);
                match (qual, &membro.kind) {
                    ("Input", ast::MemberKind::Field(l)) if !l.static_ => {
                        if l.final_ || l.const_ {
                            // Sem setter: o oficial avisa e ignora.
                            continue;
                        }
                        for v in l.variables.iter() {
                            let nome = self.nome(&v.name).to_string();
                            if nome.starts_with('_') {
                                continue;
                            }
                            let booleana = match l.ty {
                                Some(t) => self.booleana(dc, t),
                                None => match v.initializer.map(|e| &dc.ast.expr(e).kind) {
                                    Some(ast::ExprKind::Bool(_)) => Some(true),
                                    None | Some(ast::ExprKind::Null) => Some(false),
                                    _ => None,
                                },
                            };
                            de_campo.push((
                                nome.clone(),
                                Entrada {
                                    nome: apelido.clone().unwrap_or_else(|| nome.clone()),
                                    membro: nome,
                                    booleana,
                                },
                            ));
                        }
                    }
                    ("Input", ast::MemberKind::Method(f)) => {
                        let funcao = dc.ast.function(*f);
                        let (ast::FunctionKind::Setter, Some(n), false) =
                            (funcao.kind, funcao.name, funcao.static_)
                        else {
                            continue;
                        };
                        let nome = self.nome(&n).to_string();
                        if nome.starts_with('_') {
                            continue;
                        }
                        let booleana = funcao
                            .parameters
                            .as_deref()
                            .and_then(|ps| ps.first())
                            .and_then(|p| p.ty)
                            .and_then(|t| self.booleana(dc, t));
                        de_setter.push((
                            nome.clone(),
                            Entrada {
                                nome: apelido.clone().unwrap_or_else(|| nome.clone()),
                                membro: nome,
                                booleana,
                            },
                        ));
                    }
                    ("Output", ast::MemberKind::Field(l)) if !l.static_ => {
                        for v in l.variables.iter() {
                            let nome = self.nome(&v.name).to_string();
                            if nome.starts_with('_') {
                                continue;
                            }
                            saidas_campo
                                .push((apelido.clone().unwrap_or_else(|| nome.clone()), nome));
                        }
                    }
                    ("Output", ast::MemberKind::Method(f)) => {
                        let funcao = dc.ast.function(*f);
                        if let (ast::FunctionKind::Getter, Some(n), false) =
                            (funcao.kind, funcao.name, funcao.static_)
                        {
                            let nome = self.nome(&n).to_string();
                            if !nome.starts_with('_') {
                                saidas_acessor
                                    .push((apelido.clone().unwrap_or_else(|| nome.clone()), nome));
                            }
                        }
                    }
                    ("HostListener", ast::MemberKind::Method(f)) => {
                        match self.ouvinte(dc, *f, a) {
                            Ok(o) => match d.ouvintes.iter_mut().find(|x| x.evento == o.evento) {
                                // The official map: the last one wins, in the
                                // position of the first.
                                Some(x) => *x = o,
                                None => d.ouvintes.push(o),
                            },
                            Err(e) => d.fora.push(e),
                        }
                    }
                    ("HostListener", _) => d.fora.push("@HostListener outside a method".into()),
                    ("HostBinding", k) => {
                        // Static member (`_addHostBinding`): a supertype's one
                        // is not inherited; the class's own is read from it
                        // (`StaticRead`) and the official compiler writes it
                        // once, in the view constructor (case j96). In a
                        // directive it is not written yet
                        // (`hospedeiro_estatico`).
                        let estatico = match k {
                            ast::MemberKind::Field(l) => l.static_,
                            ast::MemberKind::Method(f) => dc.ast.function(*f).static_,
                            _ => false,
                        };
                        if estatico && !propria {
                            continue;
                        }
                        let membro_nome = match k {
                            ast::MemberKind::Field(l) => {
                                l.variables.first().map(|v| self.nome(&v.name).to_string())
                            }
                            ast::MemberKind::Method(f) => {
                                dc.ast.function(*f).name.map(|n| self.nome(&n).to_string())
                            }
                            _ => None,
                        }
                        .unwrap_or_default();
                        let ligacao = (
                            apelido.clone().unwrap_or_else(|| membro_nome.clone()),
                            membro_nome,
                        );
                        // `_computeHostBindingImmutability`
                        // (`compile_metadata.dart:570-600`): an immutable
                        // static outside `class.x`/`style.x` is a
                        // `hostAttribute` — it does not go into a directive's
                        // `XNgCd`; its user writes it on the element
                        // (`mergeHtmlAndDirectiveAttributes`, still refused
                        // there). In a component it stays in the list (case
                        // j96).
                        let imutavel =
                            matches!(k, ast::MemberKind::Field(l) if l.final_ || l.const_);
                        let de_classe_ou_estilo =
                            ligacao.0.starts_with("class.") || ligacao.0.starts_with("style.");
                        if estatico && imutavel && !de_classe_ou_estilo && !d.e_componente {
                            let nome = ligacao.0.strip_prefix("attr.").unwrap_or(&ligacao.0);
                            d.atributos_do_hospedeiro
                                .push((nome.to_string(), ligacao.1.clone()));
                            continue;
                        }
                        d.hospedeiro_estatico |= estatico;
                        if estatico {
                            d.estaticos_do_hospedeiro.push(ligacao.1.clone());
                        }
                        match k {
                            ast::MemberKind::Field(_) => ligacoes_campo.push(ligacao),
                            ast::MemberKind::Method(f)
                                if matches!(
                                    dc.ast.function(*f).kind,
                                    ast::FunctionKind::Getter | ast::FunctionKind::Setter
                                ) =>
                            {
                                ligacoes_acessor.push(ligacao)
                            }
                            _ => ligacoes_metodo.push(ligacao),
                        }
                    }
                    ("ContentChild" | "ContentChildren", k) => {
                        let nome = match k {
                            ast::MemberKind::Field(l) if !l.static_ && l.variables.len() == 1 => {
                                Some((self.nome(&l.variables[0].name).to_string(), false))
                            }
                            ast::MemberKind::Method(f) => {
                                let funcao = dc.ast.function(*f);
                                match (funcao.kind, funcao.name, funcao.static_) {
                                    (ast::FunctionKind::Setter, Some(n), false) => {
                                        Some((self.nome(&n).to_string(), true))
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        };
                        let Some((campo, setter)) = nome else {
                            d.fora.push("@ContentChild outside a field or setter".into());
                            continue;
                        };
                        match self.consulta_de_conteudo(dc, a, qual == "ContentChildren", campo) {
                            Ok(q) if setter => consultas_setter.push(q),
                            Ok(q) => consultas_campo.push(q),
                            Err(e) => d.fora.push(e),
                        }
                    }
                    ("ViewChild" | "ViewChildren", _) => {
                        d.consultas = true;
                    }
                    _ => {}
                }
            }
        }
        // Like the official `_queries`: setters, then fields, accumulated over
        // each class of the hierarchy (supertypes first) — the list grows, it
        // is not replaced (`find_components.dart:374`).
        d.consultas_de_conteudo.extend(consultas_setter);
        d.consultas_de_conteudo.extend(consultas_campo);
        // `_inputs..addAll(_fieldInputs)..addAll(_setterInputs)`: the key is
        // the member; repeating the key replaces the value and keeps the
        // position.
        for (chave, e) in de_campo.into_iter().chain(de_setter) {
            match entradas.iter_mut().find(|(k, _)| *k == chave) {
                Some(x) => x.1 = e,
                None => entradas.push((chave, e)),
            }
        }
        for (nome, membro) in saidas_acessor.into_iter().chain(saidas_campo) {
            match d.saidas.iter_mut().find(|(_, m)| *m == membro) {
                Some(x) => x.0 = nome,
                None => d.saidas.push((nome, membro)),
            }
        }
        for (nome, membro) in ligacoes_acessor
            .into_iter()
            .chain(ligacoes_metodo)
            .chain(ligacoes_campo)
        {
            match d
                .ligacoes_do_hospedeiro
                .iter_mut()
                .find(|(n, _)| *n == nome)
            {
                Some(x) => x.1 = membro,
                None => d.ligacoes_do_hospedeiro.push((nome, membro)),
            }
        }
    }

    /// A `@ContentChild`/`@ContentChildren`: the target (`#ref` as a string or
    /// a type, resolved in the directive's library), `descendants:` (the list
    /// one is `true` by default in ngdart 8; the single one, always) and
    /// `read:`.
    fn consulta_de_conteudo(
        &self,
        dc: &Declaracao,
        a: &ast::Annotation,
        lista: bool,
        campo: String,
    ) -> Result<crate::view::ConsultaDoFilho, String> {
        use crate::view::{AlvoDeConsulta, ConsultaDoFilho, LeituraDaConsulta};
        let args = a.arguments.as_ref().ok_or("@ContentChild sem argumentos")?;
        let primeiro = args
            .args
            .iter()
            .find(|x| x.name.is_none())
            .ok_or("@ContentChild sem alvo")?;
        let do_pacote =
            |c: &Classe| !c.uri.starts_with("package:ngdart/") && !c.uri.starts_with("dart:");
        let alvo = match self.valor(dc.unidade, primeiro.value, 0)? {
            Valor::Texto(r) => AlvoDeConsulta::Referencia(r),
            Valor::Tipo(c) if do_pacote(&c) => AlvoDeConsulta::Classe(c.uri, c.nome),
            _ => return Err("@ContentChild of an ngdart type or of unknown shape".into()),
        };
        let mut descendentes = true;
        let mut leitura = None;
        for x in args.args.iter() {
            match x.name.as_ref().map(|n| self.nome(n)) {
                None => {}
                Some("descendants") if lista => match self.valor(dc.unidade, x.value, 0)? {
                    Valor::Booleano(b) => descendentes = b,
                    _ => return Err("@ContentChildren(descendants:) that is not a literal".into()),
                },
                Some("read") => {
                    leitura = Some(match self.valor(dc.unidade, x.value, 0)? {
                        Valor::Tipo(c)
                            if crate::dialect::no_embutido(&c.uri, &c.nome).is_some() =>
                        {
                            LeituraDaConsulta::Elemento
                        }
                        Valor::Tipo(c) if do_pacote(&c) => LeituraDaConsulta::Classe(c.uri, c.nome),
                        _ => return Err("@ContentChild(.., read:) of an ngdart type".into()),
                    })
                }
                _ => return Err("@ContentChild com argumento desconhecido".into()),
            }
        }
        Ok(ConsultaDoFilho {
            campo,
            lista,
            alvo,
            descendentes,
            leitura,
        })
    }

    /// The first positional argument of an annotation, if it is a string.
    fn primeiro_texto(&self, dc: &Declaracao, a: &ast::Annotation) -> Option<String> {
        let args = a.arguments.as_ref()?;
        let x = args.args.iter().find(|x| x.name.is_none())?;
        match self.valor(dc.unidade, x.value, 0) {
            Ok(Valor::Texto(s)) => Some(s),
            _ => None,
        }
    }

    /// Is the type of an input `bool`? (`_isBoolType` of the type name, which
    /// ignores the `?`). A type parameter of the class: `None` (the official
    /// compiler uses the bound, which cannot be read from here).
    fn booleana(&self, dc: &Declaracao, t: ast::TypeId) -> Option<bool> {
        match &dc.ast.ty(t).kind {
            ast::TypeKind::Named { name, .. } => {
                let partes: Vec<&str> = name.iter().map(|n| self.nome(n)).collect();
                let simples = *partes.last()?;
                if partes.len() == 1 && dc.parametros_de_tipo.iter().any(|p| p == simples) {
                    return None;
                }
                let tipo = self.tipo(dc.lib, t, dc.ast);
                Some(
                    tipo.and_then(|t| t.classe)
                        .is_some_and(|c| c.e("dart:core", "bool")),
                )
            }
            _ => Some(false),
        }
    }

    /// A `@HostListener`, as `_addHostListener` builds it.
    fn ouvinte(
        &self,
        dc: &Declaracao,
        f: ast::FunctionId,
        a: &ast::Annotation,
    ) -> Result<Ouvinte, String> {
        let funcao = dc.ast.function(f);
        if funcao.static_ || !matches!(funcao.kind, ast::FunctionKind::Function) {
            return Err("@HostListener outside an instance method".into());
        }
        let metodo = funcao
            .name
            .map(|n| self.nome(&n).to_string())
            .ok_or("@HostListener without a name")?;
        let args = a.arguments.as_ref().ok_or("@HostListener sem argumentos")?;
        let mut evento = None;
        let mut lista = None;
        let mut posicao = 0;
        for x in args.args.iter() {
            let campo = match x.name.as_ref().map(|n| self.nome(n)) {
                Some(n) => n,
                None => {
                    posicao += 1;
                    if posicao == 1 { "eventName" } else { "args" }
                }
            };
            let v = self.valor(dc.unidade, x.value, 0)?;
            match (campo, v) {
                ("eventName", Valor::Texto(s)) => evento = Some(s),
                ("args", Valor::Lista(itens)) => {
                    let mut textos = Vec::new();
                    for i in itens {
                        let Valor::Texto(s) = i else {
                            return Err("@HostListener with args that are not strings".into());
                        };
                        textos.push(s);
                    }
                    lista = Some(textos);
                }
                _ => return Err("unreadable @HostListener".into()),
            }
        }
        let evento = evento.ok_or("@HostListener without an event")?;
        let mut argumentos = lista.unwrap_or_default();
        let parametros = funcao.parameters.as_ref().map_or(0, |p| p.len());
        if argumentos.is_empty() && parametros == 1 {
            argumentos.push("$event".into());
        }
        Ok(Ouvinte {
            evento,
            metodo,
            args: argumentos.join(", "),
        })
    }

    /// `extractLifecycleHooks`: the class is assignable to the interface.
    fn ganchos(&self, classes: &[ClassId]) -> Ganchos {
        let mut g = Ganchos::default();
        for &c in classes {
            let k = self.classe(c);
            if k.uri != LIFECYCLE_HOOKS {
                continue;
            }
            match k.nome.as_str() {
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
    fn dependencias(&self, decl: &Declaracao<'a>, d: &mut Diretiva) {
        if decl.abstrata {
            return;
        }
        let mut construtores = Vec::new();
        for &m in decl.membros {
            if let ast::MemberKind::Constructor(c) = &decl.ast.member(m).kind {
                construtores.push(c);
            }
        }
        let Some(ctor) = construtores
            .iter()
            .find(|c| c.name.is_none())
            .or(construtores.first())
        else {
            return;
        };
        if ctor.name.is_some_and(|n| self.nome(&n).starts_with('_')) {
            d.fora.push("construtor privado".into());
            return;
        }
        for p in ctor.parameters.iter() {
            if matches!(p.kind, ast::ParameterKind::Named) {
                continue;
            }
            // `super.x` without a type: that of the parameter it forwards to.
            let tipo_super = (p.super_ && p.ty.is_none())
                .then(|| self.tipo_do_super(decl, ctor, p, 0))
                .flatten();
            match self.dependencia(decl, p, tipo_super) {
                Ok(x) => d.dependencias.push(x),
                Err(f) => {
                    d.fora.push(f);
                    return;
                }
            }
        }
        d.dependencias_lidas = true;
    }

    fn dependencia(
        &self,
        decl: &Declaracao<'a>,
        p: &ast::Parameter,
        tipo_super: Option<(ast::TypeId, &'a ast::Ast, LibraryId)>,
    ) -> Result<Dependencia, String> {
        let mut dep = Dependencia {
            token: Token::Elemento,
            opcional: matches!(p.kind, ast::ParameterKind::Optional),
            proprio: false,
            hospedeiro: false,
            pular: false,
            atributo: None,
        };
        let mut injetado = None;
        for a in p.metadata.iter() {
            match self.classe_da_anotacao(decl.lib, a) {
                Some(c) if c.uri == DI_ARGUMENTS => match c.nome.as_str() {
                    "Optional" => dep.opcional = true,
                    "Self" => dep.proprio = true,
                    "Host" => dep.hospedeiro = true,
                    "SkipSelf" => dep.pular = true,
                    "Inject" => {
                        let x = a
                            .arguments
                            .as_ref()
                            .and_then(|args| args.args.first())
                            .ok_or("@Inject sem token")?;
                        let v = self.valor(decl.unidade, x.value, 0)?;
                        injetado = Some(self.token_do_valor(&v)?);
                    }
                    outro => return Err(format!("@{outro} in the directive constructor")),
                },
                Some(c) if c.uri == DIRECTIVES && c.nome == "Attribute" => {
                    let x = a
                        .arguments
                        .as_ref()
                        .and_then(|args| args.args.first())
                        .ok_or("@Attribute without a name")?;
                    match self.valor(decl.unidade, x.value, 0)? {
                        Valor::Texto(nome) => dep.atributo = Some(nome),
                        _ => return Err("@Attribute with a name that is not a string".into()),
                    }
                }
                Some(_) => {}
                None => {
                    // A token used as an annotation (`@ngValidators`).
                    let partes: Vec<&str> = a.name.iter().map(|n| self.nome(n)).collect();
                    if let [n] = partes.as_slice()
                        && let Some(Element::Variable(v)) = self.r.elemento_em(decl.lib, None, n)
                    {
                        let v = self.variavel(v, 0)?;
                        injetado = Some(self.token_do_valor(&v)?);
                    } else {
                        return Err("unresolved annotation in the directive constructor".into());
                    }
                }
            }
        }
        dep.token = match injetado {
            Some(t) => t,
            None => {
                let t = match p.ty {
                    Some(t) => Some((t, decl.ast, decl.lib)),
                    None if p.this_ => self.tipo_do_campo(decl, p).map(|(t, a)| (t, a, decl.lib)),
                    None if p.super_ => tipo_super,
                    None => None,
                }
                .ok_or("untyped parameter in the directive constructor")?;
                // The token is the class (`_idFor`): the type arguments do not
                // count, and a `T` of the generic directive does not get in the
                // way.
                let c = self
                    .classe_do_tipo(t.2, t.0, t.1)
                    .ok_or("unresolved type in the directive constructor")?
                    .ok_or("dynamic parameter in the directive constructor")?;
                if crate::dialect::no_embutido(&c.uri, &c.nome).is_some() {
                    Token::Elemento
                } else if c.uri.starts_with("package:ngdart/") && c.nome == "ChangeDetectorRef" {
                    Token::Detector
                } else {
                    Token::Classe {
                        uri: c.uri,
                        classe: c.nome,
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
    fn tipo_do_super(
        &self,
        decl: &Declaracao<'a>,
        ctor: &ast::Constructor,
        p: &ast::Parameter,
        profundidade: u32,
    ) -> Option<(ast::TypeId, &'a ast::Ast, LibraryId)> {
        if profundidade > 16 {
            return None;
        }
        let sup = self.r.programa().class(decl.id?).supertype_class?;
        let sdecl = self.declaracao(sup)?;
        let chamado = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => {
                Some(constructor.as_ref().map(|n| self.nome(n)))
            }
            _ => None,
        });
        let chamado: Option<&str> = chamado.flatten();
        let sctor = sdecl
            .membros
            .iter()
            .find_map(|&m| match &sdecl.ast.member(m).kind {
                ast::MemberKind::Constructor(c)
                    if !c.factory && c.name.as_ref().map(|n| self.nome(n)) == chamado =>
                {
                    Some(c)
                }
                _ => None,
            })?;
        let nomeado = |q: &ast::Parameter| matches!(q.kind, ast::ParameterKind::Named);
        let alvo = if nomeado(p) {
            let nome = self.nome(p.name.as_ref()?);
            sctor
                .parameters
                .iter()
                .find(|q| nomeado(q) && q.name.as_ref().map(|n| self.nome(n)) == Some(nome))?
        } else {
            let k = ctor
                .parameters
                .iter()
                .filter(|q| q.super_ && !nomeado(q))
                .position(|q| std::ptr::eq(q, p))?;
            sctor.parameters.iter().filter(|q| !nomeado(q)).nth(k)?
        };
        match alvo.ty {
            Some(t) => Some((t, sdecl.ast, sdecl.lib)),
            None if alvo.this_ => self
                .tipo_do_campo(&sdecl, alvo)
                .map(|(t, a)| (t, a, sdecl.lib)),
            None if alvo.super_ => self.tipo_do_super(&sdecl, sctor, alvo, profundidade + 1),
            None => None,
        }
    }

    /// The written type of the field of a `this.x` parameter.
    fn tipo_do_campo(
        &self,
        decl: &Declaracao<'a>,
        p: &ast::Parameter,
    ) -> Option<(ast::TypeId, &'a ast::Ast)> {
        let alvo = self.nome(p.name.as_ref()?);
        for &m in decl.membros {
            if let ast::MemberKind::Field(l) = &decl.ast.member(m).kind
                && l.variables.iter().any(|v| self.nome(&v.name) == alvo)
            {
                return l.ty.map(|t| (t, decl.ast));
            }
        }
        None
    }

    /// Resolves a written type in the scope of `lib`.
    /// The class of a written type, without looking at the arguments;
    /// `Some(None)` for `dynamic`.
    fn classe_do_tipo(
        &self,
        lib: LibraryId,
        t: ast::TypeId,
        arvore: &ast::Ast,
    ) -> Option<Option<Classe>> {
        let ast::TypeKind::Named { name, .. } = &arvore.ty(t).kind else {
            return None;
        };
        let partes: Vec<&str> = name.iter().map(|n| self.nome(n)).collect();
        let el = match partes.as_slice() {
            ["dynamic"] => return Some(None),
            [n] => self.r.elemento_em(lib, None, n),
            [p, n] => self.r.elemento_em(lib, Some(p), n),
            _ => None,
        }?;
        let Element::Class(id) = el else {
            return None;
        };
        Some(Some(self.classe(id)))
    }

    fn tipo(&self, lib: LibraryId, t: ast::TypeId, arvore: &ast::Ast) -> Option<Tipo> {
        let ast::TypeKind::Named { name, args } = &arvore.ty(t).kind else {
            return None;
        };
        let partes: Vec<&str> = name.iter().map(|n| self.nome(n)).collect();
        let el = match partes.as_slice() {
            ["dynamic"] => {
                return Some(Tipo {
                    classe: None,
                    args: Vec::new(),
                });
            }
            [n] => self.r.elemento_em(lib, None, n),
            [p, n] => self.r.elemento_em(lib, Some(p), n),
            _ => None,
        }?;
        let Element::Class(id) = el else {
            return None;
        };
        let mut argumentos = Vec::new();
        for a in args.iter() {
            argumentos.push(self.tipo(lib, *a, arvore)?);
        }
        Some(Tipo {
            classe: Some(self.classe(id)),
            args: argumentos,
        })
    }

    /// The value of a top-level variable or of a `const` static field.
    fn variavel(
        &self,
        v: dartforge_elements::model::VariableId,
        profundidade: u32,
    ) -> Result<Valor, String> {
        let p = self.r.programa();
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
            // Campo `static const` (o `useValue:` de `C.campo`).
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
            .ok_or("constante sem valor")?;
        let mut v = self.valor(unit, e, profundidade + 1)?;
        // `const OpaqueToken<T> t = OpaqueToken('x')`: without a written type
        // argument, inference takes them from the declared type.
        if let (Some(ty), Valor::Objeto { classe, tipos, .. }) = (l.ty, &mut v)
            && tipos.is_empty()
        {
            let u = p.unit(unit);
            if let Some(declarado) = self.tipo(u.library, ty, &u.ast)
                && declarado.classe.as_ref().is_some_and(|c| c.id == classe.id)
            {
                *tipos = declarado.args;
            }
        }
        Ok(v)
    }

    /// Evaluates a constant expression written in unit `unidade`.
    fn valor(&self, unidade: UnitId, e: ast::ExprId, profundidade: u32) -> Result<Valor, String> {
        if profundidade > PROFUNDIDADE {
            return Err("constante profunda demais".into());
        }
        let p = self.r.programa();
        let u = p.unit(unidade);
        let lib = u.library;
        let arvore = &u.ast;
        let elemento = |el: Element| -> Result<Valor, String> {
            match el {
                Element::Class(id) => Ok(Valor::Tipo(self.classe(id))),
                Element::Variable(v) => self.variavel(v, profundidade),
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
                    Ok(Valor::Funcao {
                        uri: crate::dialect::canonica_string(p.library(fe.library).uri.clone()),
                        nome: self.r.interner().resolve(fe.name).to_string(),
                        id: f,
                    })
                }
                _ => Err("name that is neither a class nor a constant".into()),
            }
        };
        match &arvore.expr(e).kind {
            ast::ExprKind::String(s) => s
                .constant_value()
                .map(|t| Valor::Texto(t.to_string_lossy()))
                .ok_or_else(|| "string with interpolation".into()),
            ast::ExprKind::Null => Ok(Valor::Nulo),
            ast::ExprKind::Bool(b) => Ok(Valor::Booleano(*b)),
            ast::ExprKind::Int(span) => {
                let texto = u
                    .source
                    .get(span.start..span.end)
                    .unwrap_or("")
                    .replace('_', "");
                let n = match texto
                    .strip_prefix("0x")
                    .or_else(|| texto.strip_prefix("0X"))
                {
                    Some(h) => i64::from_str_radix(h, 16).ok(),
                    None => texto.parse::<i64>().ok(),
                };
                n.map(Valor::Inteiro)
                    .ok_or_else(|| "unreadable integer".into())
            }
            ast::ExprKind::Parenthesized(x) => self.valor(unidade, *x, profundidade),
            ast::ExprKind::List { elements, .. } => {
                let mut itens = Vec::new();
                for el in elements.iter() {
                    let ast::CollectionElement::Expression(x) = el else {
                        return Err("list with a spread or `if`".into());
                    };
                    itens.push(self.valor(unidade, *x, profundidade + 1)?);
                }
                Ok(Valor::Lista(itens))
            }
            ast::ExprKind::Identifier(n) => {
                let el = self
                    .r
                    .elemento_em(lib, None, self.nome(n))
                    .ok_or("unresolved name")?;
                elemento(el)
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => {
                let ast::ExprKind::Identifier(alvo) = &arvore.expr(*target).kind else {
                    return Err("property of an expression".into());
                };
                let alvo = self.nome(alvo);
                if self.r.e_prefixo(lib, alvo) {
                    let el = self
                        .r
                        .elemento_em(lib, Some(alvo), self.nome(name))
                        .ok_or("unresolved name")?;
                    return elemento(el);
                }
                match self.r.elemento_em(lib, None, alvo) {
                    Some(Element::Class(id)) => Ok(Valor::Membro {
                        classe: self.classe(id),
                        nome: self.nome(name).to_string(),
                    }),
                    _ => Err("property that is not of a class".into()),
                }
            }
            ast::ExprKind::Call { target, arguments } => {
                let (classe, construtor) = self.construtor(lib, arvore, *target)?;
                let tipos = arguments
                    .type_args
                    .iter()
                    .map(|t| self.tipo(lib, *t, arvore).ok_or("unresolved type"))
                    .collect::<Result<Vec<_>, _>>()?;
                self.objeto(unidade, classe, construtor, tipos, arguments, profundidade)
            }
            ast::ExprKind::InstanceCreation {
                ty,
                constructor,
                arguments,
                ..
            } => {
                let t = self.tipo(lib, *ty, arvore).ok_or("unresolved type")?;
                let classe = t.classe.ok_or("instance of dynamic")?;
                self.objeto(
                    unidade,
                    classe,
                    constructor.map(|n| self.nome(&n).to_string()),
                    t.args,
                    arguments,
                    profundidade,
                )
            }
            _ => Err("expression that is not a known constant".into()),
        }
    }

    /// A classe e o construtor de `C(..)`, `C.nome(..)`, `p.C(..)` e
    /// `p.C.nome(..)`.
    fn construtor(
        &self,
        lib: LibraryId,
        arvore: &ast::Ast,
        alvo: ast::ExprId,
    ) -> Result<(Classe, Option<String>), String> {
        let classe = |el: Option<Element>| match el {
            Some(Element::Class(id)) => Ok(self.classe(id)),
            _ => Err("call that is not a constructor".to_string()),
        };
        match &arvore.expr(alvo).kind {
            ast::ExprKind::Identifier(n) => {
                Ok((classe(self.r.elemento_em(lib, None, self.nome(n)))?, None))
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => match &arvore.expr(*target).kind {
                ast::ExprKind::Identifier(a) if self.r.e_prefixo(lib, self.nome(a)) => Ok((
                    classe(self.r.elemento_em(lib, Some(self.nome(a)), self.nome(name)))?,
                    None,
                )),
                ast::ExprKind::Identifier(a) => Ok((
                    classe(self.r.elemento_em(lib, None, self.nome(a)))?,
                    Some(self.nome(name).to_string()),
                )),
                ast::ExprKind::Property {
                    target: p,
                    name: c,
                    null_aware: false,
                } => match &arvore.expr(*p).kind {
                    ast::ExprKind::Identifier(pre) if self.r.e_prefixo(lib, self.nome(pre)) => {
                        Ok((
                            classe(self.r.elemento_em(lib, Some(self.nome(pre)), self.nome(c)))?,
                            Some(self.nome(name).to_string()),
                        ))
                    }
                    _ => Err("call that is not a constructor".into()),
                },
                _ => Err("call that is not a constructor".into()),
            },
            _ => Err("call that is not a constructor".into()),
        }
    }

    fn objeto(
        &self,
        unidade: UnitId,
        classe: Classe,
        construtor: Option<String>,
        tipos: Vec<Tipo>,
        arguments: &ast::Arguments,
        profundidade: u32,
    ) -> Result<Valor, String> {
        let mut posicionais = Vec::new();
        let mut nomeados = Vec::new();
        for a in arguments.args.iter() {
            let v = self.valor(unidade, a.value, profundidade + 1)?;
            match a.name {
                Some(n) => nomeados.push((self.nome(&n).to_string(), v)),
                None => posicionais.push(v),
            }
        }
        Ok(Valor::Objeto {
            classe,
            construtor,
            tipos,
            posicionais,
            nomeados,
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
pub struct TipoEscrito {
    pub uri: String,
    pub simbolo: String,
    pub args: Vec<TipoEscrito>,
}

impl TipoEscrito {
    fn da_classe(uri: &str, simbolo: &str) -> Self {
        TipoEscrito {
            uri: uri.into(),
            simbolo: simbolo.into(),
            args: Vec::new(),
        }
    }

    fn object() -> Self {
        Self::da_classe("dart:core", "Object")
    }
}

/// A `useValue:` value as the `_reviveAny` of `InjectorReader` rebuilds it
/// (`ConstantReader.revive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revivido {
    Nulo,
    Texto(String),
    Inteiro(i64),
    Booleano(bool),
    Lista(Vec<Revivido>),
    /// `C.field`, `Enum.value` or a top-level constant, by library.
    Acesso {
        uri: String,
        nome: String,
    },
    /// `const C(..)`/`const C.nome(..)`.
    Objeto {
        uri: String,
        classe: String,
        construtor: Option<String>,
        posicionais: Vec<Revivido>,
        nomeados: Vec<(String, Revivido)>,
    },
}

/// Where a provider of the generated injector takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FonteDoInjetor {
    /// The class, the constructor (`findConstructor`: `None` is the unnamed
    /// one) and its dependencies.
    Classe {
        uri: String,
        classe: String,
        construtor: Option<String>,
        deps: Vec<Dependencia>,
    },
    Existente(Token),
    Fabrica {
        uri: String,
        nome: String,
        deps: Vec<Dependencia>,
    },
    Valor(Revivido),
}

/// A provider of the injector (`ProviderElement`), with the type the emitter
/// writes (`providerType`, or the `useClass` class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvedorDoInjetor {
    pub token: Token,
    pub multi: bool,
    pub tipo: TipoEscrito,
    pub fonte: FonteDoInjetor,
}

/// A `@GenerateInjector` read (`InjectorReader`): the variable name and the
/// providers already flattened and without repetition (`deduplicateProviders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injetor {
    pub nome: String,
    pub provedores: Vec<ProvedorDoInjetor>,
}

/// The positional and named arguments of a `const` call.
type ArgumentosConst = (Vec<Valor>, Vec<(String, Valor)>);

/// A module (`ModuleElement`): the direct providers and the included ones.
struct Modulo {
    provide: Vec<ProvedorDoInjetor>,
    include: Vec<Modulo>,
}

impl Modulo {
    /// `ModuleElement.flatten`: the included ones before the direct ones.
    fn achatar(self, saida: &mut Vec<ProvedorDoInjetor>) {
        for m in self.include {
            m.achatar(saida);
        }
        saida.extend(self.provide);
    }
}

/// The `@GenerateInjector`s of the main unit of library `uri`
/// (`InjectorReader.findInjectors`), in the order of the top-level variables.
pub fn ler_injetores(r: &Resolvedor, uri: &str) -> Vec<(String, Result<Injetor, String>)> {
    let p = r.programa();
    let Some(i) = p
        .libraries
        .iter()
        .position(|l| crate::dialect::canonica(&l.uri) == uri)
    else {
        return Vec::new();
    };
    let lib = LibraryId(i as u32);
    let Some(&unidade) = p.library(lib).units.first() else {
        return Vec::new();
    };
    let leitor = Leitor { r };
    let u = p.unit(unidade);
    let mut saida = Vec::new();
    for &d in &u.unit.declarations {
        let decl = u.ast.decl(d);
        let ast::DeclKind::Variables(l) = &decl.kind else {
            continue;
        };
        let anotacao = decl.metadata.iter().find(|a| {
            leitor
                .classe_da_anotacao(lib, a)
                .is_some_and(|c| c.e(DI_GENERATE_INJECTOR, "GenerateInjector"))
        });
        let Some(a) = anotacao else {
            continue;
        };
        let lido = leitor.injetor(unidade, a);
        for v in l.variables.iter() {
            let nome = leitor.nome(&v.name).to_string();
            saida.push((
                nome.clone(),
                lido.clone().map(|provedores| Injetor { nome, provedores }),
            ));
        }
    }
    saida
}

impl<'r, 'a> Leitor<'r, 'a> {
    /// `_computeProviders`: the annotation argument as a module, flattened,
    /// without the global services and without repetition.
    fn injetor(
        &self,
        unidade: UnitId,
        a: &ast::Annotation,
    ) -> Result<Vec<ProvedorDoInjetor>, String> {
        let arg = a
            .arguments
            .as_ref()
            .and_then(|args| args.args.first())
            .ok_or("@GenerateInjector sem provedores")?;
        let v = self.valor(unidade, arg.value, 0)?;
        let mut todos = Vec::new();
        self.modulo(v, 0)?.achatar(&mut todos);
        for p in &todos {
            if let Token::Classe { uri, classe } = &p.token
                && uri.starts_with("package:ngdart/")
                && matches!(
                    classe.as_str(),
                    "ApplicationRef" | "AppViewUtils" | "NgZone" | "Testability"
                )
            {
                return Err(format!("global service {classe} in the injector"));
            }
        }
        // `deduplicateProviders`: of the non-multi ones, the last of each
        // token, in reverse order; then the multi ones, in order.
        let mut unicos: Vec<ProvedorDoInjetor> = Vec::new();
        for p in todos.iter().rev().filter(|p| !p.multi) {
            if !unicos.iter().any(|u| u.token == p.token) {
                unicos.push(p.clone());
            }
        }
        unicos.extend(todos.into_iter().filter(|p| p.multi));
        Ok(unicos)
    }

    /// `ModuleReader.parseModule`.
    fn modulo(&self, v: Valor, profundidade: u32) -> Result<Modulo, String> {
        if profundidade > PROFUNDIDADE {
            return Err("module nested too deeply".into());
        }
        match v {
            // `_parseList`: lists and included `Module`s, the rest providers.
            Valor::Lista(itens) => {
                let mut include = Vec::new();
                let mut provide = Vec::new();
                for i in itens {
                    if Self::e_modulo(&i) {
                        include.push(self.modulo(i, profundidade + 1)?);
                    } else {
                        provide.push(self.provedor_do_injetor(i)?);
                    }
                }
                Ok(Modulo { provide, include })
            }
            Valor::Objeto {
                classe,
                construtor: None,
                posicionais,
                nomeados,
                ..
            } if classe.e(DI_MODULES, "Module") && posicionais.is_empty() => {
                let mut include = Vec::new();
                let mut provide = Vec::new();
                for (n, x) in nomeados {
                    let Valor::Lista(itens) = x else {
                        return Err(format!("Module({n}:) that is not a list"));
                    };
                    match n.as_str() {
                        "include" => {
                            for i in itens {
                                include.push(self.modulo(i, profundidade + 1)?);
                            }
                        }
                        "provide" => {
                            for i in itens {
                                provide.push(self.provedor_do_injetor(i)?);
                            }
                        }
                        outro => return Err(format!("Module({outro}:)")),
                    }
                }
                Ok(Modulo { provide, include })
            }
            _ => Err("module that is neither a list nor a Module".into()),
        }
    }

    fn e_modulo(v: &Valor) -> bool {
        match v {
            Valor::Lista(_) => true,
            Valor::Objeto { classe, .. } => classe.e(DI_MODULES, "Module"),
            _ => false,
        }
    }

    /// `ProviderReader.parseProvider`, with the fields of the `Provider` taken
    /// from the written constructor.
    fn provedor_do_injetor(&self, v: Valor) -> Result<ProvedorDoInjetor, String> {
        let (classe, construtor, tipos, posicionais, nomeados) = match v {
            // `Foo` is `ClassProvider(Foo)`
            // (`_parseTypeAsImplicitClassProvider`).
            Valor::Tipo(c) => {
                return Ok(ProvedorDoInjetor {
                    token: Token::Classe {
                        uri: c.uri.clone(),
                        classe: c.nome.clone(),
                    },
                    multi: false,
                    tipo: TipoEscrito::da_classe(&c.uri, &c.nome),
                    fonte: self.classe_do_injetor(&c)?,
                });
            }
            Valor::Objeto {
                classe,
                construtor,
                tipos,
                posicionais,
                nomeados,
            } if classe.uri == DI_PROVIDERS => (classe, construtor, tipos, posicionais, nomeados),
            Valor::Objeto { classe, .. } => {
                return Err(format!("provedor {} no injetor", classe.nome));
            }
            _ => return Err("unreadable provider in the injector".into()),
        };
        let exato = classe.nome == "Provider";
        match construtor.as_deref() {
            None => {}
            Some("forToken") if !exato => {}
            Some(c) => return Err(format!("{}.{c}", classe.nome)),
        }
        let mut pos = posicionais.into_iter();
        let token_v = pos.next().ok_or("provedor sem token")?;
        let (mut use_class, mut use_value, mut use_existing, mut use_factory, mut deps) =
            (None, None, None, None, None);
        match classe.nome.as_str() {
            "Provider" | "ClassProvider" => {}
            "ExistingProvider" => use_existing = pos.next(),
            "FactoryProvider" => use_factory = pos.next(),
            "ValueProvider" => use_value = pos.next(),
            outro => return Err(format!("provedor {outro}")),
        }
        if pos.next().is_some() {
            return Err(format!("{} com argumentos demais", classe.nome));
        }
        for (n, x) in nomeados {
            let alvo = match (classe.nome.as_str(), n.as_str()) {
                ("Provider" | "ClassProvider", "useClass") => &mut use_class,
                ("Provider", "useValue") => &mut use_value,
                ("Provider", "useExisting") => &mut use_existing,
                ("Provider", "useFactory") => &mut use_factory,
                ("Provider" | "FactoryProvider", "deps") => &mut deps,
                (_, outro) => return Err(format!("{}(.., {outro}:)", classe.nome)),
            };
            *alvo = Some(x);
        }
        // `ClassProvider._`: `useClass ?? token`.
        if classe.nome == "ClassProvider" && use_class.is_none() {
            use_class = Some(token_v.clone());
        }
        let token = self.token_lido(&token_v, true)?;
        let multi = matches!(token, Token::Multi { .. });
        // `_actualProviderType`: outside the exact `Provider`, the `T` of the
        // opaque token; otherwise the `T` of the provider — written, or
        // inferred (that of the value in `ValueProvider`; `Object`, the bound,
        // in the rest).
        let tipo_do_provedor = |use_value: Option<&Valor>| -> Result<TipoEscrito, String> {
            if !exato && let Token::Multi { tipo, .. } | Token::Opaco { tipo, .. } = &token {
                return Ok(Self::tipo_do_token(tipo));
            }
            if let Some(t) = tipos.first() {
                return self.tipo_escrito(t);
            }
            if classe.nome == "ValueProvider" {
                return self.tipo_do_valor(use_value.ok_or("ValueProvider sem valor")?);
            }
            Ok(TipoEscrito::object())
        };
        if let Some(c) = use_class {
            let Valor::Tipo(c) = c else {
                return Err("useClass of something that is not a class".into());
            };
            return Ok(ProvedorDoInjetor {
                tipo: TipoEscrito::da_classe(&c.uri, &c.nome),
                fonte: self.classe_do_injetor(&c)?,
                token,
                multi,
            });
        }
        if let Some(f) = use_factory {
            let tipo = tipo_do_provedor(None)?;
            return Ok(ProvedorDoInjetor {
                fonte: self.fabrica_do_injetor(f, deps)?,
                token,
                multi,
                tipo,
            });
        }
        if let Some(v) = use_value {
            let tipo = tipo_do_provedor(Some(&v))?;
            return Ok(ProvedorDoInjetor {
                fonte: FonteDoInjetor::Valor(self.reviver(v, 0)?),
                token,
                multi,
                tipo,
            });
        }
        if let Some(e) = use_existing {
            let tipo = tipo_do_provedor(None)?;
            return Ok(ProvedorDoInjetor {
                fonte: FonteDoInjetor::Existente(self.token_lido(&e, true)?),
                token,
                multi,
                tipo,
            });
        }
        // `Provider(Foo)` sem nada: `useClass: Foo`.
        match (&token_v, exato) {
            (Valor::Tipo(c), true) => Ok(ProvedorDoInjetor {
                tipo: TipoEscrito::da_classe(&c.uri, &c.nome),
                fonte: self.classe_do_injetor(c)?,
                token,
                multi,
            }),
            _ => Err("provedor sem use*".into()),
        }
    }

    fn tipo_do_token(t: &TipoDeToken) -> TipoEscrito {
        tipo_escrito_do_token(t)
    }

    /// A written type argument, as `linkTypeOf` writes it.
    fn tipo_escrito(&self, t: &Tipo) -> Result<TipoEscrito, String> {
        let Some(c) = &t.classe else {
            return Ok(TipoEscrito::da_classe("", "dynamic"));
        };
        let parametros = self.r.programa().class(c.id).type_params.len();
        let args = if t.args.is_empty() {
            (0..parametros)
                .map(|_| TipoEscrito::da_classe("", "dynamic"))
                .collect()
        } else {
            t.args
                .iter()
                .map(|a| self.tipo_escrito(a))
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(TipoEscrito {
            uri: c.uri.clone(),
            simbolo: c.nome.clone(),
            args,
        })
    }

    /// The static type of a `ValueProvider(Type, value)` value, which infers
    /// the `T`.
    fn tipo_do_valor(&self, v: &Valor) -> Result<TipoEscrito, String> {
        Ok(match v {
            Valor::Texto(_) => TipoEscrito::da_classe("dart:core", "String"),
            Valor::Inteiro(_) => TipoEscrito::da_classe("dart:core", "int"),
            Valor::Booleano(_) => TipoEscrito::da_classe("dart:core", "bool"),
            Valor::Objeto { classe, tipos, .. }
                if tipos.is_empty()
                    && self.r.programa().class(classe.id).type_params.is_empty() =>
            {
                TipoEscrito::da_classe(&classe.uri, &classe.nome)
            }
            Valor::Membro { classe, .. }
                if self.r.programa().class(classe.id).kind == ClassKind::Enum =>
            {
                TipoEscrito::da_classe(&classe.uri, &classe.nome)
            }
            _ => return Err("ValueProvider of a value without a known type".into()),
        })
    }

    /// `DependencyReader.findConstructor` and `_parseDependencies` of a class:
    /// the unnamed constructor (if the class is not abstract), otherwise the
    /// first public one of a concrete class or a `factory`.
    fn classe_do_injetor(&self, c: &Classe) -> Result<FonteDoInjetor, String> {
        let decl = self
            .declaracao(c.id)
            .ok_or_else(|| format!("provider class without a declaration ({})", c.nome))?;
        if !decl.parametros_de_tipo.is_empty() {
            return Err("provider of a generic class in the injector".into());
        }
        let mut construtores = Vec::new();
        for &m in decl.membros {
            if let ast::MemberKind::Constructor(k) = &decl.ast.member(m).kind {
                construtores.push(k);
            }
        }
        let sem_nome = construtores.iter().find(|k| k.name.is_none()).copied();
        let escolhido = if !decl.abstrata && (sem_nome.is_some() || construtores.is_empty()) {
            sem_nome
        } else {
            match construtores.iter().find(|k| {
                let publico = k.name.is_none_or(|n| !self.nome(&n).starts_with('_'));
                publico && !decl.abstrata || k.factory
            }) {
                Some(k) => Some(*k),
                None => return Err(format!("{} without a constructor for the injector", c.nome)),
            }
        };
        let mut deps = Vec::new();
        if let Some(k) = escolhido {
            for par in k.parameters.iter() {
                if let Some(d) = self.dependencia_do_injetor(&decl, par)? {
                    deps.push(d);
                }
            }
        }
        Ok(FonteDoInjetor::Classe {
            uri: c.uri.clone(),
            classe: c.nome.clone(),
            construtor: escolhido.and_then(|k| k.name.map(|n| self.nome(&n).to_string())),
            deps,
        })
    }

    /// A parameter as an injector dependency (`_parseDependencies`): the
    /// optional named and the optional positional ones without `@Optional` or a
    /// token are left out; optional is only `@Optional()`.
    fn dependencia_do_injetor(
        &self,
        decl: &Declaracao<'a>,
        p: &ast::Parameter,
    ) -> Result<Option<Dependencia>, String> {
        let mut optional = false;
        let mut com_token = false;
        for a in p.metadata.iter() {
            match self.classe_da_anotacao(decl.lib, a) {
                Some(c) if c.e(DI_ARGUMENTS, "Optional") => optional = true,
                Some(c) if c.e(DI_ARGUMENTS, "Inject") => com_token = true,
                Some(_) => {}
                None => com_token = true,
            }
        }
        match p.kind {
            ast::ParameterKind::Named if !p.required => return Ok(None),
            ast::ParameterKind::Named => {
                return Err("required named parameter in the injector".into());
            }
            ast::ParameterKind::Optional if !optional && !com_token => return Ok(None),
            _ => {}
        }
        let tipo_super = (p.super_ && p.ty.is_none())
            .then(|| {
                let ctor = decl
                    .membros
                    .iter()
                    .find_map(|&m| match &decl.ast.member(m).kind {
                        ast::MemberKind::Constructor(k)
                            if k.parameters.iter().any(|q| std::ptr::eq(q, p)) =>
                        {
                            Some(k)
                        }
                        _ => None,
                    })?;
                self.tipo_do_super(decl, ctor, p, 0)
            })
            .flatten();
        let mut d = self.dependencia(decl, p, tipo_super)?;
        if d.atributo.is_some() {
            return Err("@Attribute no injetor".into());
        }
        if matches!(d.token, Token::Elemento | Token::Detector) {
            return Err("HtmlElement/ChangeDetectorRef no injetor".into());
        }
        d.opcional = optional;
        Ok(Some(d))
    }

    /// `useFactory:` with `deps:` written (`parseDependenciesList`) or the
    /// function's parameters.
    fn fabrica_do_injetor(&self, f: Valor, deps: Option<Valor>) -> Result<FonteDoInjetor, String> {
        let Valor::Funcao { uri, nome, id } = f else {
            return Err("useFactory that is not a top-level function".into());
        };
        let mut saida = Vec::new();
        match deps {
            Some(Valor::Lista(itens)) => {
                for item in itens {
                    let d = self.dependencia_de_deps(item)?;
                    if matches!(d.token, Token::Elemento | Token::Detector) {
                        return Err("HtmlElement/ChangeDetectorRef no injetor".into());
                    }
                    saida.push(d);
                }
            }
            None | Some(Valor::Nulo) => {
                let p = self.r.programa();
                let dartforge_elements::model::FunctionRef::Function { unit, function } =
                    p.function(id).node
                else {
                    return Err("useFactory without a declaration".into());
                };
                let u = p.unit(unit);
                let decl = Declaracao {
                    id: None,
                    unidade: unit,
                    lib: u.library,
                    ast: &u.ast,
                    metadados: &[],
                    membros: &[],
                    parametros_de_tipo: Vec::new(),
                    abstrata: false,
                };
                let funcao = u.ast.function(function);
                if !funcao.type_params.is_empty() {
                    return Err("generic useFactory".into());
                }
                for par in funcao.parameters.iter().flat_map(|ps| ps.iter()) {
                    if let Some(d) = self.dependencia_do_injetor(&decl, par)? {
                        saida.push(d);
                    }
                }
            }
            Some(_) => return Err("unreadable deps:".into()),
        }
        Ok(FonteDoInjetor::Fabrica {
            uri,
            nome,
            deps: saida,
        })
    }

    /// `_reviveAny`: the constant value as `ConstantReader.revive` rebuilds it.
    fn reviver(&self, v: Valor, profundidade: u32) -> Result<Revivido, String> {
        if profundidade > PROFUNDIDADE {
            return Err("valor profundo demais".into());
        }
        Ok(match v {
            Valor::Nulo => Revivido::Nulo,
            Valor::Texto(s) => Revivido::Texto(s),
            Valor::Inteiro(i) => Revivido::Inteiro(i),
            Valor::Booleano(b) => Revivido::Booleano(b),
            Valor::Lista(itens) => Revivido::Lista(
                itens
                    .into_iter()
                    .map(|i| self.reviver(i, profundidade + 1))
                    .collect::<Result<_, _>>()?,
            ),
            Valor::Membro { classe, nome } => {
                if self.r.programa().class(classe.id).kind != ClassKind::Enum {
                    // The value of the static field, revived through its class.
                    let v = self.campo_estatico(classe.id, &nome, profundidade)?;
                    return self.reviver(v, profundidade + 1);
                }
                Revivido::Acesso {
                    uri: classe.uri.clone(),
                    nome: format!("{}.{nome}", classe.nome),
                }
            }
            objeto @ Valor::Objeto { .. } => self.reviver_objeto(objeto, profundidade)?,
            Valor::Tipo(_) => return Err("type value in the injector (Type)".into()),
            Valor::Funcao { .. } => return Err("function value in the injector".into()),
        })
    }

    /// `reviveInstance`: a public `const` field of the class itself with the
    /// same value; otherwise a `const` field of a class of its library;
    /// otherwise the constructor call; otherwise a top-level constant of the
    /// library.
    fn reviver_objeto(&self, v: Valor, profundidade: u32) -> Result<Revivido, String> {
        let Valor::Objeto {
            classe,
            construtor,
            tipos,
            posicionais,
            nomeados,
        } = &v
        else {
            unreachable!()
        };
        if !tipos.is_empty() || !self.r.programa().class(classe.id).type_params.is_empty() {
            return Err("constant value of a generic class in the injector".into());
        }
        let p = self.r.programa();
        let publico = |n: &str| !n.starts_with('_');
        // 1. The public `const` fields of the class itself.
        for &f in &p.class(classe.id).fields {
            let var = p.variable(f);
            let nome = self.r.interner().resolve(var.name);
            if var.const_
                && var.static_
                && publico(nome)
                && let Ok(x) = self.variavel(f, profundidade)
                && self.mesmo_objeto(&x, &v)
            {
                return Ok(Revivido::Acesso {
                    uri: classe.uri.clone(),
                    nome: format!("{}.{nome}", classe.nome),
                });
            }
        }
        // 2. The `const` fields of the classes of the library's main unit.
        let lib = p.class(classe.id).library;
        let mut candidatos: Vec<Revivido> = Vec::new();
        let unidade = *p
            .library(lib)
            .units
            .first()
            .ok_or("biblioteca sem unidade")?;
        let u = p.unit(unidade);
        for &d in &u.unit.declarations {
            let ast::DeclKind::Class(k) = &u.ast.decl(d).kind else {
                continue;
            };
            let dono = self.nome(&k.name);
            let Some(Element::Class(cid)) = self.r.elemento_em(lib, None, dono) else {
                continue;
            };
            for &f in &p.class(cid).fields {
                let var = p.variable(f);
                if !(var.const_ && var.static_) {
                    continue;
                }
                let nome = self.r.interner().resolve(var.name);
                if let Ok(x) = self.variavel(f, profundidade)
                    && self.mesmo_objeto(&x, &v)
                {
                    let r = Revivido::Acesso {
                        uri: classe.uri.clone(),
                        nome: format!("{dono}.{nome}"),
                    };
                    if publico(nome) {
                        return Ok(r);
                    }
                    candidatos.push(r);
                }
            }
        }
        // 3. A chamada do construtor.
        let privado = !publico(&classe.nome) || construtor.as_deref().is_some_and(|c| !publico(c));
        if !privado {
            return Ok(Revivido::Objeto {
                uri: classe.uri.clone(),
                classe: classe.nome.clone(),
                construtor: construtor.clone(),
                posicionais: posicionais
                    .iter()
                    .map(|x| self.reviver(x.clone(), profundidade + 1))
                    .collect::<Result<_, _>>()?,
                nomeados: nomeados
                    .iter()
                    .map(|(n, x)| Ok((n.clone(), self.reviver(x.clone(), profundidade + 1)?)))
                    .collect::<Result<_, String>>()?,
            });
        }
        // 4. As constantes de topo da biblioteca.
        for &d in &u.unit.declarations {
            let ast::DeclKind::Variables(l) = &u.ast.decl(d).kind else {
                continue;
            };
            if !l.const_ {
                continue;
            }
            for x in l.variables.iter() {
                let nome = self.nome(&x.name);
                if let Some(e) = x.initializer
                    && let Ok(val) = self.valor(unidade, e, profundidade + 1)
                    && self.mesmo_objeto(&val, &v)
                    && publico(nome)
                {
                    return Ok(Revivido::Acesso {
                        uri: crate::dialect::canonica_string(p.library(lib).uri.clone()),
                        nome: nome.to_string(),
                    });
                }
            }
        }
        candidatos
            .into_iter()
            .next()
            .ok_or_else(|| format!("value of {} without public access", classe.nome))
    }

    /// The value of a `const` static field of the class.
    fn campo_estatico(&self, c: ClassId, nome: &str, profundidade: u32) -> Result<Valor, String> {
        let p = self.r.programa();
        for &f in &p.class(c).fields {
            let var = p.variable(f);
            if var.static_ && self.r.interner().resolve(var.name) == nome {
                return self.variavel(f, profundidade);
            }
        }
        Err(format!("static field {nome} not found"))
    }

    /// Two constant values are the same object (`DartObject`'s `==`):
    /// primitives by value; objects by class, constructor and arguments with
    /// the defaults filled in.
    fn mesmo_objeto(&self, a: &Valor, b: &Valor) -> bool {
        match (a, b) {
            (Valor::Nulo, Valor::Nulo) => true,
            (Valor::Texto(x), Valor::Texto(y)) => x == y,
            (Valor::Inteiro(x), Valor::Inteiro(y)) => x == y,
            (Valor::Booleano(x), Valor::Booleano(y)) => x == y,
            (Valor::Tipo(x), Valor::Tipo(y)) => x.id == y.id,
            (Valor::Funcao { id: x, .. }, Valor::Funcao { id: y, .. }) => x == y,
            (
                Valor::Membro {
                    classe: c1,
                    nome: n1,
                },
                Valor::Membro {
                    classe: c2,
                    nome: n2,
                },
            ) => c1.id == c2.id && n1 == n2,
            (Valor::Lista(x), Valor::Lista(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(i, j)| self.mesmo_objeto(i, j))
            }
            (
                Valor::Objeto {
                    classe: c1,
                    construtor: k1,
                    ..
                },
                Valor::Objeto {
                    classe: c2,
                    construtor: k2,
                    ..
                },
            ) => {
                if c1.id != c2.id || k1 != k2 {
                    return false;
                }
                match (self.com_padroes(a), self.com_padroes(b)) {
                    (Some((p1, n1)), Some((p2, n2))) => {
                        p1.len() == p2.len()
                            && p1.iter().zip(&p2).all(|(i, j)| self.mesmo_objeto(i, j))
                            && n1.len() == n2.len()
                            && n1.iter().all(|(n, x)| {
                                n2.iter().any(|(m, y)| m == n && self.mesmo_objeto(x, y))
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
    fn com_padroes(&self, v: &Valor) -> Option<ArgumentosConst> {
        let Valor::Objeto {
            classe,
            construtor,
            posicionais,
            nomeados,
            ..
        } = v
        else {
            return None;
        };
        let decl = self.declaracao(classe.id)?;
        let k = decl
            .membros
            .iter()
            .find_map(|&m| match &decl.ast.member(m).kind {
                ast::MemberKind::Constructor(k)
                    if k.name.map(|n| self.nome(&n).to_string()) == *construtor =>
                {
                    Some(k)
                }
                _ => None,
            })?;
        let padrao = |par: &ast::Parameter| -> Option<Valor> {
            match par.default_value {
                Some(e) => self.valor(decl.unidade, e, 1).ok(),
                None => Some(Valor::Nulo),
            }
        };
        let mut pos = Vec::new();
        let mut nom = Vec::new();
        let mut i = 0;
        for par in k.parameters.iter() {
            match par.kind {
                ast::ParameterKind::Named => {
                    let n = self.nome(par.name.as_ref()?).to_string();
                    let x = match nomeados.iter().find(|(m, _)| *m == n) {
                        Some((_, x)) => x.clone(),
                        None => padrao(par)?,
                    };
                    nom.push((n, x));
                }
                _ => {
                    let x = match posicionais.get(i) {
                        Some(x) => x.clone(),
                        None => padrao(par)?,
                    };
                    pos.push(x);
                    i += 1;
                }
            }
        }
        Some((pos, nom))
    }
}

/// The [`TipoEscrito`] of the `T` of a token, with the concrete arguments or,
/// without them, `dynamic` for each parameter.
pub(crate) fn tipo_escrito_do_token(t: &TipoDeToken) -> TipoEscrito {
    if t.e_dinamico() {
        return TipoEscrito::da_classe("", "dynamic");
    }
    TipoEscrito {
        uri: t.uri.clone(),
        simbolo: t.classe.clone(),
        args: if t.args.is_empty() {
            (0..t.genericos)
                .map(|_| TipoEscrito::da_classe("", "dynamic"))
                .collect()
        } else {
            t.args.iter().map(tipo_escrito_do_token).collect()
        },
    }
}
