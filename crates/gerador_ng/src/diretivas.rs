//! Diretivas de atributo que o gerador instancia no nó: o modelo do que o
//! oficial sabe de cada uma (o `CompileDirectiveMetadata` na parte que o
//! emissor usa) e a resolução dos provedores de um nó.
//!
//! Os metadados **não** são escritos aqui: `metadados.rs` os lê do programa
//! carregado, como o `find_components.dart` os lê do analyzer — seletor,
//! `@Input`/`@Output`/`@HostListener` também herdados de superclasses,
//! interfaces e mixins, provedores, visibilidade e as dependências do
//! construtor. Uma diretiva cuja leitura não fecha, ou que tem algo que o
//! emissor ainda não escreve, é recusada com o motivo
//! ([`Diretiva::pendencia`]).
//!
//! A resolução de provedores do nó segue o `provider_parser.dart`
//! (`_ProviderResolver.resolve`, `_getOrCreateLocalProvider`) e o
//! `ProviderResolver.addDirectiveProviders`: a ordem dos provedores é a da
//! busca em profundidade pelas dependências, o `uniqueId` do campo é o
//! tamanho da tabela de instâncias no momento (cinco embutidas do elemento
//! antes de tudo), e um `ExistingProvider` de um provedor do próprio nó vira
//! apelido, sem campo.
use crate::componente::Ganchos;
use std::sync::Arc;

/// `package:ngdart/src/meta/di_tokens.dart`, onde está o `MultiToken`.
pub const DI_TOKENS: &str = "package:ngdart/src/meta/di_tokens.dart";

/// O `T` de um `MultiToken<T>`: a classe e quantos argumentos de tipo ela
/// tem (todos `dynamic`, como o `fromDartType` os escreve).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TipoDeToken {
    pub uri: String,
    pub classe: String,
    pub genericos: usize,
}

impl TipoDeToken {
    /// `Object` do `dart:core`, o `T` do `ngValidators`.
    pub fn e_object(&self) -> bool {
        self.uri == "dart:core" && self.classe == "Object" && self.genericos == 0
    }
}

/// Um token de injeção.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// Uma classe, pela biblioteca que a declara.
    Classe { uri: String, classe: String },
    /// `const MultiToken<T>('nome')`.
    Multi { nome: String, tipo: TipoDeToken },
    /// `const OpaqueToken<T>('nome')`.
    Opaco { nome: String, tipo: TipoDeToken },
    /// `HtmlElement`/`Element`: o próprio nó (embutido do elemento).
    Elemento,
    /// `ChangeDetectorRef`: numa diretiva, a própria visão (`o.thisExpr`).
    Detector,
}

impl Token {
    /// O nome que vai no campo (`_NgModel_3_9`, `_NgValidators_3_6`): o
    /// `CompileTokenMetadata.name`, que num token de texto troca por `_` o
    /// que não é letra, dígito ou `_`.
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

    /// Um dos embutidos do elemento (`ElementRef`, `Injector`,
    /// `ViewContainerRef`…), que um serviço não recebe como os outros.
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

/// Um valor constante de `useValue:` na parte que o emissor escreve
/// (`_useValueExpression`): texto, inteiro, booleano, e objeto constante com
/// argumentos desses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValorConst {
    Texto(String),
    Inteiro(i64),
    Booleano(bool),
    /// `const C(..)`/`const C.nome(..)`, de classe sem parâmetro de tipo.
    Objeto {
        uri: String,
        classe: String,
        construtor: Option<String>,
        posicionais: Vec<ValorConst>,
        nomeados: Vec<(String, ValorConst)>,
    },
}

/// De onde vem o valor de um provedor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fornece {
    /// `useExisting:` — outro token.
    Existente(Token),
    /// `useClass:` (ou o próprio token), com as dependências do construtor.
    Classe {
        uri: String,
        classe: String,
        deps: Vec<Dependencia>,
    },
    /// `useValue:`.
    Valor(ValorConst),
    /// `useFactory:` de uma função de topo, com `deps:` (ou os parâmetros).
    Fabrica {
        uri: String,
        nome: String,
        deps: Vec<Dependencia>,
    },
}

/// Um item de `providers:`, já achatado (`ModuleReader`,
/// `_normalizeProviders`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provedor {
    pub token: Token,
    pub fonte: Fornece,
    pub multi: bool,
    /// O `T` do `Provider<T>` (`inferProviderType`): o tipo do campo quando
    /// não é o da própria expressão. `None` quando a inferência não acha (e
    /// o campo leva o tipo do valor, ou `dynamic`).
    pub tipo: Option<TipoDeToken>,
}

impl Provedor {
    /// `ExistingProvider(token, alvo)` sem tipo inferido — a única forma que
    /// um nó de template sabe criar.
    pub fn apelido(token: Token, alvo: Token, multi: bool) -> Self {
        Provedor {
            token,
            fonte: Fornece::Existente(alvo),
            multi,
            tipo: None,
        }
    }
}

/// Um parâmetro posicional do construtor (`_getCompileDiDependencyMetadata`
/// pula os nomeados).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependencia {
    pub token: Token,
    /// `@Optional()` ou parâmetro posicional opcional.
    pub opcional: bool,
    /// `@Self()`.
    pub proprio: bool,
    /// `@Host()`.
    pub hospedeiro: bool,
    /// `@SkipSelf()`.
    pub pular: bool,
}

/// Um `@Input`: nome no template, membro, e se o tipo é `bool` (atributo
/// sem valor vira `true`, `visitEmptyExpr`); `None` quando o tipo não se
/// sabe daqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub nome: String,
    pub membro: String,
    pub booleana: Option<bool>,
}

/// Um `@HostListener`: o evento, o método e os argumentos como o
/// `_addHostListener` os escreve. `args` vazio: método sem parâmetro;
/// `$event`: um parâmetro; outro texto: handler complexo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ouvinte {
    pub evento: String,
    pub metodo: String,
    pub args: String,
}

/// O que o gerador sabe de uma `@Directive` (ou `@Component`), lido do
/// programa.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diretiva {
    pub classe: String,
    pub uri: String,
    pub seletor: String,
    pub e_componente: bool,
    pub export_as: Option<String>,
    /// `visibility: Visibility.all`: injetável por quem está abaixo.
    pub visivel: bool,
    pub provedores: Vec<Provedor>,
    pub dependencias: Vec<Dependencia>,
    /// Na ordem do mapa `inputs` (`_SortInputsVisitor`).
    pub entradas: Vec<Entrada>,
    /// (nome no template, membro), na ordem do mapa `outputs`.
    pub saidas: Vec<(String, String)>,
    /// Na ordem do mapa `hostListeners`.
    pub ouvintes: Vec<Ouvinte>,
    /// `@HostBinding`: (nome da ligação, membro).
    pub ligacoes_do_hospedeiro: Vec<(String, String)>,
    pub ganchos: Ganchos,
    /// Algum `@ContentChild(ren)`/`@ViewChild(ren)`.
    pub consultas: bool,
    /// O que a leitura não conseguiu entender: com qualquer coisa aqui, os
    /// metadados estão incompletos e a diretiva não é usada.
    pub fora: Vec<String>,
}

impl Diretiva {
    pub fn token(&self) -> Token {
        Token::Classe {
            uri: self.uri.clone(),
            classe: self.classe.clone(),
        }
    }

    /// Todos os `providers:` são `ExistingProvider` de token de classe ou
    /// `MultiToken` — o que um nó de template sabe criar. (Os outros só a
    /// visão-hospedeira do próprio componente escreve.)
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

    /// Por que o emissor ainda não instancia esta diretiva num elemento
    /// HTML, ou `None` se instancia. Cada item é uma forma sem caso no
    /// corpus: gerar ignorando-a daria saída errada.
    pub fn pendencia(&self) -> Option<String> {
        if let Some(f) = self.fora.first() {
            return Some(f.clone());
        }
        if self.e_componente {
            return Some("componente como diretiva".into());
        }

        if self.consultas {
            return Some("consulta de conteúdo ou de visão".into());
        }
        let g = &self.ganchos;
        if g.after_content_init
            || g.after_content_checked
            || g.after_view_init
            || g.after_view_checked
        {
            return Some("gancho de ciclo de vida além de OnInit/AfterChanges/DoCheck".into());
        }
        if self
            .ouvintes
            .iter()
            .any(|o| !crate::visao::evento_nativo(&o.evento))
        {
            return Some("@HostListener de evento não nativo".into());
        }
        for d in &self.dependencias {
            if let Token::Multi { tipo, .. } = &d.token
                && !tipo.e_object()
                && tipo.genericos == 0
            {
                return Some("MultiToken de tipo não genérico".into());
            }
        }
        if !self.so_apelidos() {
            return Some("provedor que não é ExistingProvider".into());
        }
        if self
            .dependencias
            .iter()
            .any(|d| matches!(d.token, Token::Opaco { .. }))
        {
            return Some("dependência de OpaqueToken".into());
        }
        for p in &self.provedores {
            if let Token::Multi { tipo, .. } = &p.token
                && !tipo.e_object()
                && tipo.genericos == 0
            {
                return Some("MultiToken de tipo não genérico".into());
            }
        }
        None
    }
}

/// Um argumento do construtor de uma diretiva, já resolvido no nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argumento {
    /// O nó (`_el_3` ou `this._el_3`).
    Elemento,
    /// A visão (`this`).
    Detector,
    /// `@Optional() @Self()` sem provedor no nó.
    Nulo,
    /// O campo de outro provedor do nó.
    Campo(String),
    /// Um provedor de um elemento acima (`_getDependency` sobe pelos
    /// pais): a expressão que o lê desta visão.
    Acima(String),
}

/// Um provedor injetável de um elemento acima do nó, na visão dele ou numa
/// visão ancestral (a cadeia de `parentView` até ela).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvedorAcima {
    pub token: Token,
    /// A expressão que o lê da visão em que o nó está.
    pub leitura: String,
}

/// O que há acima do nó para as dependências que ele não satisfaz.
#[derive(Debug, Clone, Copy)]
pub struct Acima<'a> {
    /// Do mais próximo para o mais longe.
    pub provedores: &'a [ProvedorAcima],
    /// Algum elemento acima tem provedor que o emissor não modela (um
    /// componente): não achar não prova que não há.
    pub incerto: bool,
}

/// Como um campo de provedor é criado no `build()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Criacao {
    /// `Classe(args)`.
    Diretiva {
        diretiva: Arc<Diretiva>,
        args: Vec<Argumento>,
    },
    /// `[a, b]`: os campos que o multi-provedor junta.
    Lista(Vec<String>),
    /// Só na visão-hospedeira: um provedor de `providers:` que não é
    /// diretiva nem apelido local (`ClassProviderSource`,
    /// `FactoryProviderSource`, `ExpressionProviderSource` e o
    /// `injectorGet` de um apelido de fora).
    Expressao(Expr),
    /// Só na visão-hospedeira: o multi-provedor com itens de qualquer forma.
    Multi(Vec<Expr>),
}

/// O valor de um provedor da visão-hospedeira, como o `ProviderSource.build`
/// o escreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// `this.campo` — outro provedor do nó (`component` é o componente).
    Campo(String),
    /// `this.injectorGet(token, this.parentIndex)` (ou `injectorGetOptional`):
    /// o que o nó não provê vem do injetor de fora.
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
    /// Alguma dependência vem do injetor (`hasDynamicDependencies`): a
    /// criação sai embrulhada em `debugInjectorWrap`.
    pub fn dinamica(&self) -> bool {
        match self {
            Expr::Classe { args, .. } | Expr::Fabrica { args, .. } => {
                args.iter().any(|a| matches!(a, Expr::Injetor { .. }))
            }
            _ => false,
        }
    }
}

/// Um provedor do nó que vira campo da visão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instancia {
    pub token: Token,
    pub campo: String,
    pub criacao: Criacao,
    /// Os tokens pelos quais ele é injetável abaixo (`injectorGetInternal`):
    /// o próprio, se visível, e os apelidos.
    pub injetavel_por: Vec<Token>,
    /// Os `ExistingProvider` do nó que apontam para esta instância, visíveis
    /// ou não: também por eles uma consulta a acha.
    pub apelidos: Vec<Token>,
    /// Nenhum provedor ansioso (diretiva, componente) depende dele: o
    /// oficial o cria no `afterElement` com `eager: false`, como campo
    /// `late` com inicializador (`late List<T> _X_n_m = [..];`), e não no
    /// `build()`.
    pub preguicosa: bool,
    /// Como a instância é lida: o campo, ou `campo.instance` quando a
    /// diretiva tem `@HostBinding` e o campo guarda o `XNgCd` dela.
    pub leitura: String,
    /// O `T` inferido do provedor (o `typeArgument`), que tipa o campo.
    pub tipo: Option<TipoDeToken>,
}

/// Os provedores de diretivas de um nó, resolvidos.
#[derive(Debug, Clone, Default)]
pub struct NoResolvido {
    /// Os campos, na ordem de criação.
    pub instancias: Vec<Instancia>,
    /// As diretivas, na ordem em que o oficial as liga
    /// (`transformedDirectiveAsts`: a dos provedores), com a leitura da
    /// instância de cada (o campo, ou `campo.instance` com `XNgCd`).
    pub diretivas: Vec<(Arc<Diretiva>, String)>,
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
}

/// Resolve os provedores das diretivas `casadas` (na ordem de
/// `directives:`) no nó `n`. Uma dependência que o próprio nó não satisfaz
/// e não é `@Optional() @Self()` é recusada: ela viria de outro nó ou do
/// injetor, formas ainda sem caso.
pub fn resolver(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
) -> Result<NoResolvido, &'static str> {
    resolver_em(casadas, n, acima, false)
}

/// Os provedores do nó da visão-hospedeira (`_ViewXHost0`, nó 0): o
/// componente e os `providers:` dele. O que o nó não provê vem do injetor
/// de fora (`injectFromViewParentInjector`), e a instância do componente é
/// o campo `component` do `HostView` (`hostViewComponentFieldName`).
pub fn resolver_hospedeira(componente: Arc<Diretiva>) -> Result<NoResolvido, &'static str> {
    resolver_em(&[componente], 0, None, true)
}

/// Os provedores do nó de um componente filho num template: o filho
/// (`casadas[0]`, sem as dependências do construtor, que se resolvem à
/// parte) com os `providers:` dele de qualquer forma, e as diretivas do nó
/// (`casadas[1..]`), que continuam só com `ExistingProvider`. Os provedores
/// do filho são escritos como na hospedeira (`ProviderSource.build`), mas
/// uma dependência que o nó não satisfaz iria para os elementos acima ou
/// para o injetor de fora (`parentView.injectorGet`): ainda sem caso, é
/// recusada.
pub fn resolver_no_do_filho(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
) -> Result<NoResolvido, &'static str> {
    let r = resolver_com(casadas, n, acima, false, casadas.len().min(1))?;
    for i in &r.instancias {
        let depende_de_fora = match &i.criacao {
            Criacao::Expressao(e) => fora_do_no_de_filho(e),
            Criacao::Multi(itens) => itens.iter().any(fora_do_no_de_filho),
            _ => false,
        };
        if depende_de_fora {
            return Err("provedor do filho com dependência de fora do nó");
        }
    }
    Ok(r)
}

/// A expressão lê o injetor (dependência que o nó não satisfaz)?
fn fora_do_no_de_filho(e: &Expr) -> bool {
    match e {
        Expr::Injetor { .. } => true,
        Expr::Classe { args, .. } | Expr::Fabrica { args, .. } => {
            args.iter().any(fora_do_no_de_filho)
        }
        Expr::Campo(_) | Expr::Valor(_) => false,
    }
}

fn resolver_em(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
    hospedeira: bool,
) -> Result<NoResolvido, &'static str> {
    resolver_com(
        casadas,
        n,
        acima,
        hospedeira,
        if hospedeira { casadas.len() } else { 0 },
    )
}

/// `completas`: quantas das primeiras `casadas` têm os `providers:` de
/// qualquer forma escritos (na hospedeira, todas; no nó de um filho, o
/// filho); as outras só com `ExistingProvider`.
fn resolver_com(
    casadas: &[Arc<Diretiva>],
    n: u32,
    acima: Option<Acima>,
    hospedeira: bool,
    completas: usize,
) -> Result<NoResolvido, &'static str> {
    // `_ProviderResolver.resolve`: as diretivas (ansiosas), depois os
    // `providers:` de cada uma; o mesmo token multi acumula.
    let mut todos: Vec<Resolvido> = Vec::new();
    for d in casadas {
        todos.push(Resolvido {
            token: d.token(),
            fontes: vec![Fonte::Diretiva(d.clone())],
            multi: false,
            eager: true,
            visivel: d.visivel,
            tipo: None,
        });
    }
    for (k, d) in casadas.iter().enumerate() {
        for p in &d.provedores {
            let fonte = match &p.fonte {
                Fornece::Existente(t) => Fonte::Existente(t.clone()),
                f if k < completas => Fonte::Provedor(f.clone()),
                _ => return Err("provedor que não é ExistingProvider num nó de template"),
            };
            match todos.iter_mut().find(|r| r.token == p.token) {
                Some(r) => {
                    if r.multi != p.multi {
                        return Err("provedor multi e não multi no mesmo token");
                    }
                    if (hospedeira || k < completas) && r.eager {
                        // O token do componente sobrescrito por `providers:`:
                        // ainda sem caso.
                        return Err("provedor com o token do componente");
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
                }),
            }
        }
    }
    // `_getOrCreateLocalProvider`: em profundidade, as dependências antes.
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
            return Err("dependência cíclica entre diretivas");
        }
        vistos.push(i);
        let pedir = |t: &Token, ordem: &mut Vec<usize>, vistos: &mut Vec<usize>| match todos
            .iter()
            .position(|r| r.token == *t)
        {
            Some(j) => criar(todos, j, ordem, vistos, hospedeira),
            None => Ok(()),
        };
        for f in &todos[i].fontes {
            match f {
                Fonte::Existente(t) => match todos.iter().position(|r| r.token == *t) {
                    Some(j) => criar(todos, j, ordem, vistos, hospedeira)?,
                    // Na hospedeira, o apelido de fora lê o injetor.
                    None if hospedeira => {}
                    None => return Err("provedor apelido de token de fora do nó"),
                },
                Fonte::Diretiva(d) => {
                    for dep in &d.dependencias {
                        match &dep.token {
                            Token::Elemento | Token::Detector => {}
                            t => pedir(t, ordem, vistos)?,
                        }
                    }
                }
                Fonte::Provedor(Fornece::Classe { deps, .. } | Fornece::Fabrica { deps, .. }) => {
                    for dep in deps {
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
    let ansiosos = ordem.len();
    // `afterElement`: o que sobrou (os apelidos, em geral).
    for i in 0..todos.len() {
        criar(&todos, i, &mut ordem, &mut vistos, hospedeira)?;
    }

    // `addDirectiveProviders`: o `uniqueId` é o tamanho da tabela, que já
    // tem as cinco embutidas do elemento.
    let mut tamanho = 5usize;
    let mut campos: Vec<(Token, String)> = Vec::new();
    let mut apelidos: Vec<(Token, Token)> = Vec::new();
    let mut saida = NoResolvido::default();
    for (posicao, &i) in ordem.iter().enumerate() {
        let r = &todos[i];
        let preguicosa = posicao >= ansiosos;
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
        // Diretiva com `@HostBinding`: o campo é o `XNgCd` que a embrulha
        // (`createProvider`, `providerHasChangeDetector`). Componente não:
        // o `@HostBinding` dele é o `detectHostChanges` da visão dele, e o
        // campo é a instância (caso j30).
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
            let t = apelidos
                .iter()
                .find(|(a, _)| a == t)
                .map(|(_, real)| real)
                .unwrap_or(t);
            campos.iter().find(|(x, _)| x == t).map(|(_, c)| c.clone())
        };
        // Uma dependência de serviço (`_getDependency` na hospedeira): o
        // campo local, ou o injetor de fora.
        let dependencia = |dep: &Dependencia| -> Result<Expr, &'static str> {
            if dep.proprio || dep.hospedeiro || dep.pular {
                return Err("dependência @Self/@Host/@SkipSelf de provedor");
            }
            if dep.token.embutido() {
                return Err("provedor que depende de embutido do elemento");
            }
            Ok(match campo_de(&dep.token) {
                Some(c) => Expr::Campo(c),
                None => Expr::Injetor {
                    token: dep.token.clone(),
                    opcional: dep.opcional,
                },
            })
        };
        let expr_de = |f: &Fornece| -> Result<Expr, &'static str> {
            Ok(match f {
                Fornece::Existente(t) => match campo_de(t) {
                    Some(c) => Expr::Campo(c),
                    None if t.embutido() => return Err("apelido de embutido do elemento"),
                    None => Expr::Injetor {
                        token: t.clone(),
                        opcional: false,
                    },
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
                    args.push(match &dep.token {
                        Token::Elemento | Token::Detector if dep.pular => {
                            return Err("dependência @SkipSelf de embutido do elemento");
                        }
                        Token::Elemento => Argumento::Elemento,
                        Token::Detector => Argumento::Detector,
                        // `@SkipSelf()`: o `_getDependency` começa no pai
                        // (o `ControlContainer` do `NgControlName` é o
                        // `NgForm` do `<form>` de cima).
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
                        Fonte::Diretiva(_) => return Err("multi-provedor de diretiva"),
                    });
                }
                Criacao::Multi(itens)
            }
            fontes if r.multi => {
                let mut itens = Vec::new();
                for f in fontes {
                    let Fonte::Existente(t) = f else {
                        return Err("multi-provedor que não é apelido");
                    };
                    itens.push(campo_de(t).ok_or("multi-provedor de token de fora do nó")?);
                }
                Criacao::Lista(itens)
            }
            [Fonte::Existente(t)] if hospedeira => {
                Criacao::Expressao(expr_de(&Fornece::Existente(t.clone()))?)
            }
            [Fonte::Provedor(p)] => Criacao::Expressao(expr_de(p)?),
            _ => return Err("provedor apelido de token de fora do nó"),
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

/// Uma dependência que o próprio nó não satisfaz (`_getDependency`):
/// `@Self` para no nó; senão sobe pelos elementos acima (`@Host` até o
/// hospedeiro, que aqui é a raiz da visão do componente). Sem resultado e
/// `@Optional`, `null` — a não ser que haja acima algo que o emissor não
/// modela. Sem `@Host`, depois dos elementos viria o injetor de fora: ainda
/// não.
fn fora_do_no(dep: &Dependencia, acima: Option<Acima>) -> Result<Argumento, &'static str> {
    // Sem contexto (só a ordem dos imports interessa): o valor não importa.
    let Some(acima) = acima else {
        return Ok(Argumento::Nulo);
    };
    if !dep.proprio
        && let Some(p) = acima.provedores.iter().find(|p| p.token == dep.token)
    {
        return Ok(Argumento::Acima(p.leitura.clone()));
    }
    match (dep.opcional, dep.proprio, dep.hospedeiro) {
        (true, true, _) => Ok(Argumento::Nulo),
        (true, false, true) if !acima.incerto => Ok(Argumento::Nulo),
        (_, false, true) => Err("dependência @Host de diretiva sem provedor acima"),
        _ => Err("dependência de diretiva de fora do nó"),
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
        }
    }

    /// Diretivas de teste com a forma que `metadados.rs` lê do `ngforms`.
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

    /// A ordem e a numeração de `alterar_senha_page.template.dart`: o
    /// `RequiredValidator` antes do multi que o junta, o acessor antes do
    /// seu multi, o `NgModel` por último; `NgControl` é apelido.
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

    /// A numeração do `i66_provider_dependencias.template.dart`: o que o
    /// componente pede (e o que isso pede) sai antes dele, ansioso; o resto
    /// fica preguiçoso, depois, na ordem de `providers:`.
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
        // O apelido do componente: sem campo, injetável por `this.component`.
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

    /// Num nó de template, só `ExistingProvider` é criado; o resto é
    /// recusado (a hospedeira do próprio componente é quem o escreve).
    #[test]
    fn no_de_template_recusa_provedor_de_classe() {
        let comp = Arc::new(Diretiva {
            classe: "Comp".into(),
            uri: "s".into(),
            e_componente: true,
            provedores: vec![servico("Api", &[])],
            ..Default::default()
        });
        assert!(resolver(&[comp], 3, None).is_err());
    }
}
