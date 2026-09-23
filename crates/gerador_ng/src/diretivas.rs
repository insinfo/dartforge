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
    /// `HtmlElement`/`Element`: o próprio nó (embutido do elemento).
    Elemento,
    /// `ChangeDetectorRef`: numa diretiva, a própria visão (`o.thisExpr`).
    Detector,
}

impl Token {
    /// O nome que vai no campo (`_NgModel_3_9`, `_NgValidators_3_6`).
    pub fn nome(&self) -> &str {
        match self {
            Token::Classe { classe, .. } => classe,
            Token::Multi { nome, .. } => nome,
            Token::Elemento => "HtmlElement",
            Token::Detector => "ChangeDetectorRef",
        }
    }
}

/// Um item de `providers:` da diretiva: `ExistingProvider(token, existente)`
/// ou `ExistingProvider.forToken(multi, existente)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provedor {
    pub token: Token,
    pub existente: Token,
    pub multi: bool,
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
        if !self.ligacoes_do_hospedeiro.is_empty() {
            return Some("@HostBinding".into());
        }
        if self.consultas {
            return Some("consulta de conteúdo ou de visão".into());
        }
        let g = &self.ganchos;
        if g.on_destroy {
            return Some("OnDestroy".into());
        }
        if g.do_check
            || g.after_content_init
            || g.after_content_checked
            || g.after_view_init
            || g.after_view_checked
        {
            return Some("gancho de ciclo de vida além de OnInit/AfterChanges".into());
        }
        if self
            .ouvintes
            .iter()
            .any(|o| !crate::visao::evento_nativo(&o.evento))
        {
            return Some("@HostListener de evento não nativo".into());
        }
        if self.dependencias.iter().any(|d| d.hospedeiro || d.pular) {
            return Some("dependência @Host/@SkipSelf".into());
        }
        for d in &self.dependencias {
            if let Token::Multi { tipo, .. } = &d.token
                && !tipo.e_object()
                && tipo.genericos == 0
            {
                return Some("MultiToken de tipo não genérico".into());
            }
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
}

/// Os provedores de diretivas de um nó, resolvidos.
#[derive(Debug, Clone, Default)]
pub struct NoResolvido {
    /// Os campos, na ordem de criação.
    pub instancias: Vec<Instancia>,
    /// As diretivas, na ordem em que o oficial as liga
    /// (`transformedDirectiveAsts`: a dos provedores), com o campo de cada.
    pub diretivas: Vec<(Arc<Diretiva>, String)>,
}

#[derive(Debug)]
enum Fonte {
    Diretiva(Arc<Diretiva>),
    Existente(Token),
}

#[derive(Debug)]
struct Resolvido {
    token: Token,
    fontes: Vec<Fonte>,
    multi: bool,
    eager: bool,
    visivel: bool,
}

/// Resolve os provedores das diretivas `casadas` (na ordem de
/// `directives:`) no nó `n`. Uma dependência que o próprio nó não satisfaz
/// e não é `@Optional() @Self()` é recusada: ela viria de outro nó ou do
/// injetor, formas ainda sem caso.
pub fn resolver(casadas: &[Arc<Diretiva>], n: u32) -> Result<NoResolvido, &'static str> {
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
        });
    }
    for d in casadas {
        for p in &d.provedores {
            match todos.iter_mut().find(|r| r.token == p.token) {
                Some(r) => {
                    if r.multi != p.multi {
                        return Err("provedor multi e não multi no mesmo token");
                    }
                    if !p.multi {
                        r.fontes.clear();
                    }
                    r.fontes.push(Fonte::Existente(p.existente.clone()));
                }
                None => todos.push(Resolvido {
                    token: p.token.clone(),
                    fontes: vec![Fonte::Existente(p.existente.clone())],
                    multi: p.multi,
                    eager: false,
                    visivel: true,
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
    ) -> Result<(), &'static str> {
        if ordem.contains(&i) {
            return Ok(());
        }
        if vistos.contains(&i) {
            return Err("dependência cíclica entre diretivas");
        }
        vistos.push(i);
        for f in &todos[i].fontes {
            match f {
                Fonte::Existente(t) => {
                    let j = todos
                        .iter()
                        .position(|r| r.token == *t)
                        .ok_or("provedor apelido de token de fora do nó")?;
                    criar(todos, j, ordem, vistos)?;
                }
                Fonte::Diretiva(d) => {
                    for dep in &d.dependencias {
                        match &dep.token {
                            Token::Elemento | Token::Detector => {}
                            t => match todos.iter().position(|r| r.token == *t) {
                                Some(j) => criar(todos, j, ordem, vistos)?,
                                None if dep.opcional && dep.proprio => {}
                                None => return Err("dependência de diretiva de fora do nó"),
                            },
                        }
                    }
                }
            }
        }
        ordem.push(i);
        Ok(())
    }
    let mut ordem = Vec::new();
    let mut vistos = Vec::new();
    for i in 0..todos.len() {
        if todos[i].eager {
            criar(&todos, i, &mut ordem, &mut vistos)?;
        }
    }
    // `afterElement`: o que sobrou (os apelidos, em geral).
    for i in 0..todos.len() {
        criar(&todos, i, &mut ordem, &mut vistos)?;
    }

    // `addDirectiveProviders`: o `uniqueId` é o tamanho da tabela, que já
    // tem as cinco embutidas do elemento.
    let mut tamanho = 5usize;
    let mut campos: Vec<(Token, String)> = Vec::new();
    let mut apelidos: Vec<(Token, Token)> = Vec::new();
    let mut saida = NoResolvido::default();
    for &i in &ordem {
        let r = &todos[i];
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
        let campo = format!("_{}_{n}_{tamanho}", r.token.nome());
        let campo_de = |t: &Token| -> Option<String> {
            let t = apelidos
                .iter()
                .find(|(a, _)| a == t)
                .map(|(_, real)| real)
                .unwrap_or(t);
            campos.iter().find(|(x, _)| x == t).map(|(_, c)| c.clone())
        };
        let criacao = match r.fontes.as_slice() {
            [Fonte::Diretiva(d)] => {
                let mut args = Vec::new();
                for dep in &d.dependencias {
                    args.push(match &dep.token {
                        Token::Elemento => Argumento::Elemento,
                        Token::Detector => Argumento::Detector,
                        t => match campo_de(t) {
                            Some(c) => Argumento::Campo(c),
                            None => Argumento::Nulo,
                        },
                    });
                }
                saida.diretivas.push((d.clone(), campo.clone()));
                Criacao::Diretiva {
                    diretiva: d.clone(),
                    args,
                }
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
        });
        campos.push((r.token.clone(), campo));
        tamanho += 1;
    }
    for (apelido, real) in apelidos {
        if let Some(inst) = saida.instancias.iter_mut().find(|x| x.token == real) {
            inst.injetavel_por.push(apelido);
        }
    }
    Ok(saida)
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
            provedores: vec![Provedor {
                token: classe("c", "NgControl"),
                existente: classe("m", "NgModel"),
                multi: false,
            }],
            dependencias: vec![
                dep(validadores(), true, true),
                dep(acessores(), true, true),
            ],
            ..Default::default()
        })
    }

    fn acessor() -> Arc<Diretiva> {
        Arc::new(Diretiva {
            classe: "DefaultValueAccessor".into(),
            uri: "d".into(),
            provedores: vec![Provedor {
                token: acessores(),
                existente: classe("d", "DefaultValueAccessor"),
                multi: true,
            }],
            dependencias: vec![dep(Token::Elemento, false, false)],
            ..Default::default()
        })
    }

    fn obrigatorio() -> Arc<Diretiva> {
        Arc::new(Diretiva {
            classe: "RequiredValidator".into(),
            uri: "v".into(),
            provedores: vec![Provedor {
                token: validadores(),
                existente: classe("v", "RequiredValidator"),
                multi: true,
            }],
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
        let r = resolver(&casadas, 33).unwrap();
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
            provedores: vec![Provedor {
                token: classe("cc", "ControlContainer"),
                existente: classe("f", "NgForm"),
                multi: false,
            }],
            dependencias: vec![
                dep(validadores(), true, true),
                dep(Token::Detector, false, false),
            ],
            ..Default::default()
        });
        let r = resolver(std::slice::from_ref(&form), 19).unwrap();
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
}
