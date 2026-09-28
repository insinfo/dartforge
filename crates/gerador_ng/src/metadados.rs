//! Os metadados de uma `@Directive`/`@Component`, lidos do programa
//! carregado — o que o `_ComponentVisitor` do oficial
//! (`source_gen/template_compiler/find_components.dart`) tira do analyzer:
//!
//! * a anotação avaliada como constante: `selector`, `exportAs`,
//!   `visibility`, `providers` (listas e constantes de topo achatadas, como o
//!   `ModuleReader.extractProviderObjects`);
//! * `@Input`/`@Output` da classe **e dos supertipos**
//!   (`_collectInheritableMetadata`: `allSupertypes` ao contrário, depois a
//!   própria classe; em cada uma os campos antes dos setters nas entradas, os
//!   acessores antes dos campos nas saídas);
//! * `@HostListener`/`@HostBinding` pela mesma ordem (`DirectiveVisitor`:
//!   acessores, métodos, campos);
//! * os ganchos de ciclo de vida (`extractLifecycleHooks`: qualquer supertipo
//!   que seja a interface do `lifecycle_hooks.dart`);
//! * as dependências do construtor (`_getCompileDiDependencyMetadata`), com
//!   `@Inject`, `@Optional`, `@Self`, `@Host`, `@SkipSelf`.
//!
//! As anotações são reconhecidas pela classe que designam
//! (`TypeChecker.fromUrl('package:ngdart/src/meta/...')`), não pelo nome
//! escrito. O que não se consegue ler vai para `Diretiva::fora`, e quem usa
//! a diretiva recusa.
use crate::componente::Ganchos;
use crate::diretivas::{
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

/// Profundidade máxima de constantes que citam constantes.
const PROFUNDIDADE: u32 = 16;

/// Uma classe já resolvida.
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

/// Um tipo escrito, resolvido: a classe e os argumentos; `None` na classe é
/// `dynamic`.
#[derive(Debug, Clone)]
struct Tipo {
    classe: Option<Classe>,
    args: Vec<Tipo>,
}

/// O valor constante de uma expressão, na parte que os metadados usam.
#[derive(Debug, Clone)]
enum Valor {
    Texto(String),
    Inteiro(i64),
    Booleano(bool),
    Nulo,
    /// Uma função de topo (`useFactory:`), pela biblioteca que a declara.
    Funcao {
        uri: String,
        nome: String,
        id: FunctionElementId,
    },
    Lista(Vec<Valor>),
    /// Um literal de tipo (`NgModel` numa lista de provedores).
    Tipo(Classe),
    /// `C<T>.ctor(a, b, nome: c)`.
    Objeto {
        classe: Classe,
        construtor: Option<String>,
        tipos: Vec<Tipo>,
        posicionais: Vec<Valor>,
        nomeados: Vec<(String, Valor)>,
    },
    /// `C.nome`: constante de enum ou campo estático.
    Membro {
        classe: Classe,
        nome: String,
    },
}

/// Lê os metadados da classe `id`, se ela tem `@Directive` ou `@Component`.
pub fn ler(r: &Resolvedor, id: ClassId) -> Option<Diretiva> {
    Leitor { r }.diretiva(id)
}

struct Leitor<'r, 'a> {
    r: &'r Resolvedor<'a>,
}

/// Onde uma classe está declarada: a unidade, a biblioteca, os membros e os
/// nomes dos parâmetros de tipo.
struct Declaracao<'a> {
    /// A classe declarada (`None` na função de `useFactory:`).
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
            uri: p.library(c.library).uri.clone(),
            nome: self.r.interner().resolve(c.name).to_string(),
            id,
        }
    }

    /// `allSupertypes` do analyzer (`ClassHierarchy._getHierarchy`): para o
    /// supertipo, as restrições `on`, as interfaces e os mixins, nessa ordem,
    /// o próprio tipo e depois os supertipos dele, sem repetir.
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

    /// A classe que o nome de uma anotação designa (`Input`, `ng.Input`).
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

    /// Se a anotação é uma das do `directives.dart` do ngdart, o nome dela.
    /// Uma anotação com um desses nomes que não se resolve para lá é
    /// registrada em `fora`: o analyzer a teria resolvido, e perdê-la em
    /// silêncio daria metadados errados.
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
                    fora.push(format!("@{n} não resolvida"));
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
            self.membros(&dc, &mut d, &mut entradas);
        }
        d.entradas = entradas.into_iter().map(|(_, e)| e).collect();
        // Diretiva genérica com `@HostBinding`: o `XNgCd<T>` leva argumentos
        // de tipo (`lookupTypeArgumentsOf`) — ainda sem caso.
        if !d.ligacoes_do_hospedeiro.is_empty() && !decl.parametros_de_tipo.is_empty() {
            d.fora.push("diretiva genérica com @HostBinding".into());
        }
        d.ganchos = self.ganchos(&ordem);
        self.dependencias(&decl, &mut d);
        Some(d)
    }

    /// Os argumentos da anotação.
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
                    _ => d.fora.push("selector: que não é texto constante".into()),
                },
                "exportAs" => match valor() {
                    Ok(Valor::Texto(s)) => d.export_as = Some(s),
                    Ok(Valor::Nulo) => {}
                    _ => d.fora.push("exportAs: que não é texto constante".into()),
                },
                "visibility" => match valor() {
                    Ok(Valor::Membro { classe, nome }) if classe.e(VISIBILITY, "Visibility") => {
                        d.visivel = nome == "all";
                    }
                    _ => d.fora.push("visibility: ilegível".into()),
                },
                "providers" => match valor() {
                    Ok(v) => self.provedores(v, d),
                    Err(f) => d.fora.push(format!("providers: {f}")),
                },
                "viewProviders" => match valor() {
                    Ok(Valor::Lista(l)) if l.is_empty() => {}
                    _ => d.fora.push("viewProviders:".into()),
                },
                _ => {}
            }
        }
    }

    /// `ModuleReader.extractProviderObjects`: listas achatadas; cada item um
    /// provedor (`createProviderMetadata`). O que não se lê vai para `fora`.
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

    /// Um provedor: um literal de tipo (`useClass` do próprio tipo) ou um
    /// `Provider`/`ClassProvider`/`ExistingProvider`/`FactoryProvider`/
    /// `ValueProvider` (e o `.forToken` de cada).
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
            _ => return Err("provedor ilegível".into()),
        };
        let tipo_do_provedor = classe.nome.as_str();
        let por_token = match construtor.as_deref() {
            None => false,
            Some("forToken") if tipo_do_provedor != "Provider" => true,
            Some(c) => return Err(format!("{tipo_do_provedor}.{c}")),
        };
        // Os campos do `Provider`, pelo construtor usado.
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
        // `useValue: null` não se distingue da falta dele daqui.
        if matches!(use_value, Some(Valor::Nulo)) {
            return Err("useValue: null".into());
        }
        let quantos = [&use_class, &use_value, &use_existing, &use_factory]
            .iter()
            .filter(|x| x.is_some())
            .count();
        if quantos > 1 {
            return Err("provedor com mais de um use*".into());
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
                _ => return Err("useClass de algo que não é classe".into()),
            };
            self.fonte_de_classe(&c)?
        };
        // `inferProviderType`: o `T` do `MultiToken`; senão o `T` do
        // provedor (escrito ou inferido pelo analyzer), a não ser `dynamic`
        // ou `Object`.
        let tipo = if let Token::Multi { tipo, .. } = &token {
            Some(tipo.clone())
        } else if let Some(t) = tipos.first() {
            let Some(c) = &t.classe else {
                return Err("provedor com argumento de tipo dynamic".into());
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
                }),
                _ => return Err("ValueProvider de valor sem tipo conhecido".into()),
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

    fn do_core(nome: &str) -> TipoDeToken {
        TipoDeToken {
            uri: "dart:core".into(),
            classe: nome.into(),
            genericos: 0,
        }
    }

    /// Um tipo sem argumentos (ou só com `dynamic`), como o `fromDartType`
    /// o escreve.
    fn tipo_sem_argumentos(&self, c: &Classe, t: &Tipo) -> Result<TipoDeToken, String> {
        let parametros = self.r.programa().class(c.id).type_params.len();
        if !t.args.is_empty()
            && (t.args.len() != parametros || t.args.iter().any(|a| a.classe.is_some()))
        {
            return Err("tipo de provedor com argumentos".into());
        }
        Ok(TipoDeToken {
            uri: c.uri.clone(),
            classe: c.nome.clone(),
            genericos: parametros,
        })
    }

    /// `useClass:` (`_getUseClass`, `enforceClassCanBeCreated`): a classe e
    /// as dependências do construtor sem nome.
    fn fonte_de_classe(&self, c: &Classe) -> Result<Fornece, String> {
        let decl = self
            .declaracao(c.id)
            .ok_or_else(|| format!("classe de provedor sem declaração ({})", c.nome))?;
        if !decl.parametros_de_tipo.is_empty() {
            return Err("provedor de classe genérica".into());
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

    /// `useFactory:` de uma função de topo, com `deps:` (se não vazio) ou os
    /// parâmetros dela (`_factoryForFunction`).
    fn fonte_de_fabrica(&self, f: Valor, deps: Option<Valor>) -> Result<Fornece, String> {
        let Valor::Funcao { uri, nome, id } = f else {
            return Err("useFactory que não é função de topo".into());
        };
        let lista = match deps {
            None | Some(Valor::Nulo) => Vec::new(),
            Some(Valor::Lista(l)) => l,
            Some(_) => return Err("deps: ilegível".into()),
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
                return Err("useFactory sem declaração".into());
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
                return Err("useFactory genérica".into());
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

    /// Um item de `deps:` (`_factoryDiDep`): um tipo, um token, ou a lista
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
                let primeiro = it.next().ok_or("deps: com lista vazia")?;
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

    /// O token de uma dependência escrita como valor: `HtmlElement` e
    /// `ChangeDetectorRef` viram os embutidos, como no parâmetro.
    fn token_de_dependencia(&self, v: &Valor) -> Result<Token, String> {
        match v {
            Valor::Tipo(c)
                if c.uri == "dart:html" && (c.nome == "HtmlElement" || c.nome == "Element") =>
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

    /// O `_useValueExpression` na parte que o emissor escreve: texto,
    /// inteiro, booleano e objeto constante de classe não genérica com
    /// argumentos desses.
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
                    return Err("useValue de classe genérica".into());
                }
                if construtor.as_deref().is_some_and(|c| c.starts_with('_')) {
                    return Err("useValue com construtor privado".into());
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
            _ => return Err("useValue fora de texto, inteiro, booleano ou objeto simples".into()),
        })
    }

    /// O token de um valor constante: uma classe, um `MultiToken` ou um
    /// `OpaqueToken`.
    fn token_do_valor(&self, v: &Valor) -> Result<Token, String> {
        self.token_lido(v, false)
    }

    /// O token de um valor; `sem_nome`: `const OpaqueToken<T>()` vale (no
    /// injetor gerado ele vira `const OpaqueToken<T>()`; nas visões ainda
    /// não tem caso).
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
                    None => return Err("OpaqueToken sem nome".into()),
                    _ => return Err("token de nome ilegível".into()),
                };
                let [t] = tipos.as_slice() else {
                    return Err(format!("{} sem argumento de tipo", classe.nome));
                };
                let Some(c) = &t.classe else {
                    return Err(format!("{}<dynamic>", classe.nome));
                };
                let parametros = self.r.programa().class(c.id).type_params.len();
                // `fromDartType` escreve os argumentos; só `dynamic` tem caso.
                if t.args.len() != parametros || t.args.iter().any(|a| a.classe.is_some()) {
                    return Err(format!("{} de tipo com argumentos", classe.nome));
                }
                let tipo = TipoDeToken {
                    uri: c.uri.clone(),
                    classe: c.nome.clone(),
                    genericos: parametros,
                };
                Ok(if multi {
                    Token::Multi { nome, tipo }
                } else {
                    Token::Opaco { nome, tipo }
                })
            }
            Valor::Objeto { classe, .. } => Err(format!("token {}", classe.nome)),
            _ => Err("token ilegível".into()),
        }
    }

    /// Os membros anotados de uma classe da hierarquia.
    fn membros(&self, dc: &Declaracao, d: &mut Diretiva, entradas: &mut Vec<(String, Entrada)>) {
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
                                // O mapa do oficial: o último vence, na
                                // posição do primeiro.
                                Some(x) => *x = o,
                                None => d.ouvintes.push(o),
                            },
                            Err(e) => d.fora.push(e),
                        }
                    }
                    ("HostListener", _) => d.fora.push("@HostListener fora de método".into()),
                    ("HostBinding", k) => {
                        // Membro estático: o oficial o escreve uma vez, no
                        // construtor da visão (caso j96); por estes metadados
                        // (diretiva, componente que herda) ainda não.
                        let estatico = match k {
                            ast::MemberKind::Field(l) => l.static_,
                            ast::MemberKind::Method(f) => dc.ast.function(*f).static_,
                            _ => false,
                        };
                        if estatico {
                            d.hospedeiro_estatico = true;
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
                            d.fora.push("@ContentChild fora de campo ou setter".into());
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
        // Como o `_queries` do oficial: setters, depois campos.
        d.consultas_de_conteudo = consultas_setter;
        d.consultas_de_conteudo.extend(consultas_campo);
        // `_inputs..addAll(_fieldInputs)..addAll(_setterInputs)`: a chave é o
        // membro; repetir a chave troca o valor e mantém a posição.
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

    /// Um `@ContentChild`/`@ContentChildren`: o alvo (`#ref` em texto ou
    /// tipo, resolvido na biblioteca da diretiva), `descendants:` (a lista
    /// é `true` por omissão no ngdart 8; a única, sempre) e `read:`.
    fn consulta_de_conteudo(
        &self,
        dc: &Declaracao,
        a: &ast::Annotation,
        lista: bool,
        campo: String,
    ) -> Result<crate::visao::ConsultaDoFilho, String> {
        use crate::visao::{AlvoDeConsulta, ConsultaDoFilho, LeituraDaConsulta};
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
            _ => return Err("@ContentChild de tipo do ngdart ou de forma desconhecida".into()),
        };
        let mut descendentes = true;
        let mut leitura = None;
        for x in args.args.iter() {
            match x.name.as_ref().map(|n| self.nome(n)) {
                None => {}
                Some("descendants") if lista => match self.valor(dc.unidade, x.value, 0)? {
                    Valor::Booleano(b) => descendentes = b,
                    _ => return Err("@ContentChildren(descendants:) que não é literal".into()),
                },
                Some("read") => {
                    leitura = Some(match self.valor(dc.unidade, x.value, 0)? {
                        Valor::Tipo(c)
                            if c.uri == "dart:html"
                                && matches!(c.nome.as_str(), "HtmlElement" | "Element") =>
                        {
                            LeituraDaConsulta::Elemento
                        }
                        Valor::Tipo(c) if do_pacote(&c) => LeituraDaConsulta::Classe(c.uri, c.nome),
                        _ => return Err("@ContentChild(.., read:) de tipo do ngdart".into()),
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

    /// O primeiro argumento posicional de uma anotação, se é texto.
    fn primeiro_texto(&self, dc: &Declaracao, a: &ast::Annotation) -> Option<String> {
        let args = a.arguments.as_ref()?;
        let x = args.args.iter().find(|x| x.name.is_none())?;
        match self.valor(dc.unidade, x.value, 0) {
            Ok(Valor::Texto(s)) => Some(s),
            _ => None,
        }
    }

    /// O tipo de uma entrada é `bool`? (`_isBoolType` do nome do tipo, que
    /// ignora o `?`). Parâmetro de tipo da classe: `None` (o oficial usa o
    /// limite, que daqui não se lê).
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

    /// Um `@HostListener`, como o `_addHostListener` o monta.
    fn ouvinte(
        &self,
        dc: &Declaracao,
        f: ast::FunctionId,
        a: &ast::Annotation,
    ) -> Result<Ouvinte, String> {
        let funcao = dc.ast.function(f);
        if funcao.static_ || !matches!(funcao.kind, ast::FunctionKind::Function) {
            return Err("@HostListener fora de método de instância".into());
        }
        let metodo = funcao
            .name
            .map(|n| self.nome(&n).to_string())
            .ok_or("@HostListener sem nome")?;
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
                            return Err("@HostListener com args que não são texto".into());
                        };
                        textos.push(s);
                    }
                    lista = Some(textos);
                }
                _ => return Err("@HostListener ilegível".into()),
            }
        }
        let evento = evento.ok_or("@HostListener sem evento")?;
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

    /// `extractLifecycleHooks`: a classe é atribuível à interface.
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

    /// As dependências do construtor sem nome (ou o primeiro), como o
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
            // `super.x` sem tipo: o do parâmetro que ele repassa.
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
                    outro => return Err(format!("@{outro} no construtor da diretiva")),
                },
                Some(c) if c.uri == DIRECTIVES && c.nome == "Attribute" => {
                    let x = a
                        .arguments
                        .as_ref()
                        .and_then(|args| args.args.first())
                        .ok_or("@Attribute sem nome")?;
                    match self.valor(decl.unidade, x.value, 0)? {
                        Valor::Texto(nome) => dep.atributo = Some(nome),
                        _ => return Err("@Attribute com nome que não é texto".into()),
                    }
                }
                Some(_) => {}
                None => {
                    // Um token usado como anotação (`@ngValidators`).
                    let partes: Vec<&str> = a.name.iter().map(|n| self.nome(n)).collect();
                    if let [n] = partes.as_slice()
                        && let Some(Element::Variable(v)) = self.r.elemento_em(decl.lib, None, n)
                    {
                        let v = self.variavel(v, 0)?;
                        injetado = Some(self.token_do_valor(&v)?);
                    } else {
                        return Err("anotação não resolvida no construtor da diretiva".into());
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
                .ok_or("parâmetro sem tipo no construtor da diretiva")?;
                let tipo = self
                    .tipo(t.2, t.0, t.1)
                    .ok_or("tipo não resolvido no construtor da diretiva")?;
                let c = tipo
                    .classe
                    .ok_or("parâmetro dynamic no construtor da diretiva")?;
                if c.uri == "dart:html" && (c.nome == "HtmlElement" || c.nome == "Element") {
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

    /// O tipo de um parâmetro `super.x` sem tipo escrito
    /// (`SuperFormalParameterElement`): o do parâmetro que ele repassa no
    /// construtor da superclasse chamado — o do `super.nome(..)` do
    /// inicializador, ou o sem nome —, pela posição entre os `super.`
    /// posicionais ou pelo nome; lá, o tipo escrito, o do campo (`this.x`)
    /// ou, outro `super.x`, o da superclasse seguinte. Com a biblioteca em
    /// que o tipo está escrito.
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

    /// O tipo escrito do campo de um parâmetro `this.x`.
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

    /// Resolve um tipo escrito no escopo de `lib`.
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

    /// O valor de uma variável de topo ou de um campo estático `const`.
    fn variavel(
        &self,
        v: dartforge_elements::model::VariableId,
        profundidade: u32,
    ) -> Result<Valor, String> {
        let p = self.r.programa();
        let var = p.variable(v);
        if !var.const_ {
            return Err("variável que não é const".into());
        }
        let (unit, l, index) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => {
                let ast::DeclKind::Variables(l) = &p.unit(unit).ast.decl(decl).kind else {
                    return Err("constante ilegível".into());
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
                    return Err("constante ilegível".into());
                };
                (unit, l, index)
            }
            _ => return Err("constante que não é de topo".into()),
        };
        let e = l
            .variables
            .get(index)
            .and_then(|x| x.initializer)
            .ok_or("constante sem valor")?;
        self.valor(unit, e, profundidade + 1)
    }

    /// Avalia uma expressão constante escrita na unidade `unidade`.
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
                        return Err("função que não é de topo".into());
                    }
                    Ok(Valor::Funcao {
                        uri: p.library(fe.library).uri.clone(),
                        nome: self.r.interner().resolve(fe.name).to_string(),
                        id: f,
                    })
                }
                _ => Err("nome que não é classe nem constante".into()),
            }
        };
        match &arvore.expr(e).kind {
            ast::ExprKind::String(s) => s
                .constant_value()
                .map(|t| Valor::Texto(t.to_string_lossy()))
                .ok_or_else(|| "texto com interpolação".into()),
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
                    .ok_or_else(|| "inteiro ilegível".into())
            }
            ast::ExprKind::Parenthesized(x) => self.valor(unidade, *x, profundidade),
            ast::ExprKind::List { elements, .. } => {
                let mut itens = Vec::new();
                for el in elements.iter() {
                    let ast::CollectionElement::Expression(x) = el else {
                        return Err("lista com espalhamento ou `if`".into());
                    };
                    itens.push(self.valor(unidade, *x, profundidade + 1)?);
                }
                Ok(Valor::Lista(itens))
            }
            ast::ExprKind::Identifier(n) => {
                let el = self
                    .r
                    .elemento_em(lib, None, self.nome(n))
                    .ok_or("nome não resolvido")?;
                elemento(el)
            }
            ast::ExprKind::Property {
                target,
                name,
                null_aware: false,
            } => {
                let ast::ExprKind::Identifier(alvo) = &arvore.expr(*target).kind else {
                    return Err("propriedade de expressão".into());
                };
                let alvo = self.nome(alvo);
                if self.r.e_prefixo(lib, alvo) {
                    let el = self
                        .r
                        .elemento_em(lib, Some(alvo), self.nome(name))
                        .ok_or("nome não resolvido")?;
                    return elemento(el);
                }
                match self.r.elemento_em(lib, None, alvo) {
                    Some(Element::Class(id)) => Ok(Valor::Membro {
                        classe: self.classe(id),
                        nome: self.nome(name).to_string(),
                    }),
                    _ => Err("propriedade que não é de classe".into()),
                }
            }
            ast::ExprKind::Call { target, arguments } => {
                let (classe, construtor) = self.construtor(lib, arvore, *target)?;
                let tipos = arguments
                    .type_args
                    .iter()
                    .map(|t| self.tipo(lib, *t, arvore).ok_or("tipo não resolvido"))
                    .collect::<Result<Vec<_>, _>>()?;
                self.objeto(unidade, classe, construtor, tipos, arguments, profundidade)
            }
            ast::ExprKind::InstanceCreation {
                ty,
                constructor,
                arguments,
                ..
            } => {
                let t = self.tipo(lib, *ty, arvore).ok_or("tipo não resolvido")?;
                let classe = t.classe.ok_or("instância de dynamic")?;
                self.objeto(
                    unidade,
                    classe,
                    constructor.map(|n| self.nome(&n).to_string()),
                    t.args,
                    arguments,
                    profundidade,
                )
            }
            _ => Err("expressão que não é constante conhecida".into()),
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
            _ => Err("chamada que não é construtor".to_string()),
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
                    _ => Err("chamada que não é construtor".into()),
                },
                _ => Err("chamada que não é construtor".into()),
            },
            _ => Err("chamada que não é construtor".into()),
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

/// Um tipo como o `linkTypeOf` o escreve: a classe, pela biblioteca que a
/// declara, e os argumentos. `uri` vazio é `dynamic`.
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

/// Um valor de `useValue:` como o `_reviveAny` do `InjectorReader` o
/// reconstrói (`ConstantReader.revive`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revivido {
    Nulo,
    Texto(String),
    Inteiro(i64),
    Booleano(bool),
    Lista(Vec<Revivido>),
    /// `C.campo`, `Enum.valor` ou uma constante de topo, pela biblioteca.
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

/// De onde um provedor do injetor gerado tira o valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FonteDoInjetor {
    /// A classe, o construtor (`findConstructor`: `None` é o sem nome) e as
    /// dependências dele.
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

/// Um provedor do injetor (`ProviderElement`), com o tipo que o emissor
/// escreve (`providerType`, ou a classe do `useClass`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvedorDoInjetor {
    pub token: Token,
    pub multi: bool,
    pub tipo: TipoEscrito,
    pub fonte: FonteDoInjetor,
}

/// Um `@GenerateInjector` lido (`InjectorReader`): o nome da variável e os
/// provedores já achatados e sem repetição (`deduplicateProviders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Injetor {
    pub nome: String,
    pub provedores: Vec<ProvedorDoInjetor>,
}

/// Os argumentos posicionais e nomeados de uma chamada `const`.
type ArgumentosConst = (Vec<Valor>, Vec<(String, Valor)>);

/// Um módulo (`ModuleElement`): os provedores diretos e os incluídos.
struct Modulo {
    provide: Vec<ProvedorDoInjetor>,
    include: Vec<Modulo>,
}

impl Modulo {
    /// `ModuleElement.flatten`: os incluídos antes dos diretos.
    fn achatar(self, saida: &mut Vec<ProvedorDoInjetor>) {
        for m in self.include {
            m.achatar(saida);
        }
        saida.extend(self.provide);
    }
}

/// Os `@GenerateInjector` da unidade principal da biblioteca `uri`
/// (`InjectorReader.findInjectors`), na ordem das variáveis de topo.
pub fn ler_injetores(r: &Resolvedor, uri: &str) -> Vec<(String, Result<Injetor, String>)> {
    let p = r.programa();
    let Some(i) = p.libraries.iter().position(|l| l.uri == uri) else {
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
    /// `_computeProviders`: o argumento da anotação como módulo, achatado,
    /// sem os serviços globais e sem repetição.
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
                return Err(format!("serviço global {classe} no injetor"));
            }
        }
        // `deduplicateProviders`: dos não-multi, o último de cada token, na
        // ordem inversa; depois os multi, na ordem.
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
            return Err("módulo profundo demais".into());
        }
        match v {
            // `_parseList`: listas e `Module` incluídos, o resto provedores.
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
                        return Err(format!("Module({n}:) que não é lista"));
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
            _ => Err("módulo que não é lista nem Module".into()),
        }
    }

    fn e_modulo(v: &Valor) -> bool {
        match v {
            Valor::Lista(_) => true,
            Valor::Objeto { classe, .. } => classe.e(DI_MODULES, "Module"),
            _ => false,
        }
    }

    /// `ProviderReader.parseProvider`, com os campos do `Provider` tirados
    /// do construtor escrito.
    fn provedor_do_injetor(&self, v: Valor) -> Result<ProvedorDoInjetor, String> {
        let (classe, construtor, tipos, posicionais, nomeados) = match v {
            // `Foo` é `ClassProvider(Foo)` (`_parseTypeAsImplicitClassProvider`).
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
            _ => return Err("provedor ilegível no injetor".into()),
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
        // `_actualProviderType`: fora do `Provider` exato, o `T` do token
        // opaco; senão o `T` do provedor — escrito, ou inferido (o do valor
        // no `ValueProvider`; `Object`, o limite, no resto).
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
                return Err("useClass de algo que não é classe".into());
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
        TipoEscrito {
            uri: t.uri.clone(),
            simbolo: t.classe.clone(),
            args: (0..t.genericos)
                .map(|_| TipoEscrito::da_classe("", "dynamic"))
                .collect(),
        }
    }

    /// Um argumento de tipo escrito, como o `linkTypeOf` o escreve.
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

    /// O tipo estático de um valor de `ValueProvider(Tipo, valor)`, que
    /// infere o `T`.
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
            _ => return Err("ValueProvider de valor sem tipo conhecido".into()),
        })
    }

    /// `DependencyReader.findConstructor` e `_parseDependencies` de uma
    /// classe: o construtor sem nome (se a classe não é abstrata), senão o
    /// primeiro público de classe concreta ou `factory`.
    fn classe_do_injetor(&self, c: &Classe) -> Result<FonteDoInjetor, String> {
        let decl = self
            .declaracao(c.id)
            .ok_or_else(|| format!("classe de provedor sem declaração ({})", c.nome))?;
        if !decl.parametros_de_tipo.is_empty() {
            return Err("provedor de classe genérica no injetor".into());
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
                None => return Err(format!("{} sem construtor para o injetor", c.nome)),
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

    /// Um parâmetro como dependência do injetor (`_parseDependencies`): o
    /// nomeado opcional e o posicional opcional sem `@Optional` nem token
    /// ficam de fora; opcional é só o `@Optional()`.
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
                return Err("parâmetro nomeado obrigatório no injetor".into());
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

    /// `useFactory:` com `deps:` escrito (`parseDependenciesList`) ou os
    /// parâmetros da função.
    fn fabrica_do_injetor(&self, f: Valor, deps: Option<Valor>) -> Result<FonteDoInjetor, String> {
        let Valor::Funcao { uri, nome, id } = f else {
            return Err("useFactory que não é função de topo".into());
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
                    return Err("useFactory sem declaração".into());
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
                    return Err("useFactory genérica".into());
                }
                for par in funcao.parameters.iter().flat_map(|ps| ps.iter()) {
                    if let Some(d) = self.dependencia_do_injetor(&decl, par)? {
                        saida.push(d);
                    }
                }
            }
            Some(_) => return Err("deps: ilegível".into()),
        }
        Ok(FonteDoInjetor::Fabrica {
            uri,
            nome,
            deps: saida,
        })
    }

    /// `_reviveAny`: o valor constante como o `ConstantReader.revive` o
    /// reconstrói.
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
                    // O valor do campo estático, revivido pela classe dele.
                    let v = self.campo_estatico(classe.id, &nome, profundidade)?;
                    return self.reviver(v, profundidade + 1);
                }
                Revivido::Acesso {
                    uri: classe.uri.clone(),
                    nome: format!("{}.{nome}", classe.nome),
                }
            }
            objeto @ Valor::Objeto { .. } => self.reviver_objeto(objeto, profundidade)?,
            Valor::Tipo(_) => return Err("valor de tipo no injetor (Type)".into()),
            Valor::Funcao { .. } => return Err("valor de função no injetor".into()),
        })
    }

    /// `reviveInstance`: um campo `const` público da própria classe com o
    /// mesmo valor; senão um campo `const` de uma classe da biblioteca dela;
    /// senão a chamada do construtor; senão uma constante de topo da
    /// biblioteca.
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
            return Err("valor constante de classe genérica no injetor".into());
        }
        let p = self.r.programa();
        let publico = |n: &str| !n.starts_with('_');
        // 1. Os campos `const` públicos da própria classe.
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
        // 2. Os campos `const` das classes da unidade principal da biblioteca.
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
                        uri: p.library(lib).uri.clone(),
                        nome: nome.to_string(),
                    });
                }
            }
        }
        candidatos
            .into_iter()
            .next()
            .ok_or_else(|| format!("valor de {} sem acesso público", classe.nome))
    }

    /// O valor de um campo estático `const` da classe.
    fn campo_estatico(&self, c: ClassId, nome: &str, profundidade: u32) -> Result<Valor, String> {
        let p = self.r.programa();
        for &f in &p.class(c).fields {
            let var = p.variable(f);
            if var.static_ && self.r.interner().resolve(var.name) == nome {
                return self.variavel(f, profundidade);
            }
        }
        Err(format!("campo estático {nome} não achado"))
    }

    /// Dois valores constantes são o mesmo objeto (o `==` do `DartObject`):
    /// primitivos pelo valor; objetos pela classe, pelo construtor e pelos
    /// argumentos com os padrões preenchidos.
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
            // Um campo estático de enum contra o mesmo campo lido.
            _ => false,
        }
    }

    /// Os argumentos de uma chamada `const` com os padrões do construtor
    /// preenchidos (o que falta e não tem padrão é `null`).
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
