//! Diretivas de atributo que o gerador instancia no nó: as do `ngforms` de
//! um formulário simples (`NgForm`, `NgModel`, `DefaultValueAccessor`,
//! `RequiredValidator`).
//!
//! O oficial lê do analyzer o que cada uma declara — provedores,
//! dependências do construtor, entradas, saídas e os `@HostListener`, também
//! os herdados de superclasses e mixins (`AbstractForm`, `TouchHandler`). O
//! nosso índice só vê o que a própria classe declara; por isso o catálogo
//! aqui é escrito à mão, conferido contra o código do `ngforms`
//! 5.0.0-dev.3 (`lib/src/directives/*.dart`). Diretiva fora dele continua
//! recusada pela guarda de seletor.
//!
//! A resolução de provedores do nó segue o `provider_parser.dart`
//! (`_ProviderResolver.resolve`, `_getOrCreateLocalProvider`) e o
//! `ProviderResolver.addDirectiveProviders`: a ordem dos provedores é a da
//! busca em profundidade pelas dependências, o `uniqueId` do campo é o
//! tamanho da tabela de instâncias no momento (cinco embutidas do elemento
//! antes de tudo), e um `ExistingProvider` de um provedor do próprio nó vira
//! apelido, sem campo.

/// `package:ngdart/src/meta/di_tokens.dart`, onde está o `MultiToken`.
pub const DI_TOKENS: &str = "package:ngdart/src/meta/di_tokens.dart";

const NG_FORM: &str = "package:ngforms/src/directives/ng_form.dart";
const NG_MODEL: &str = "package:ngforms/src/directives/ng_model.dart";
const DEFAULT_VALUE_ACCESSOR: &str = "package:ngforms/src/directives/default_value_accessor.dart";
const VALIDATORS: &str = "package:ngforms/src/directives/validators.dart";
const CONTROL_CONTAINER: &str = "package:ngforms/src/directives/control_container.dart";
const NG_CONTROL: &str = "package:ngforms/src/directives/ng_control.dart";
const CONTROL_VALUE_ACCESSOR: &str = "package:ngforms/src/directives/control_value_accessor.dart";

/// Um token de injeção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// Uma classe, pela biblioteca que a declara.
    Classe {
        uri: &'static str,
        classe: &'static str,
    },
    /// `const MultiToken<T>('nome')`; `tipo` é o `T` (`None`: `Object`).
    Multi {
        nome: &'static str,
        tipo: Option<(&'static str, &'static str)>,
    },
    /// `HtmlElement`: o próprio nó (embutido do elemento).
    Elemento,
    /// `ChangeDetectorRef`: numa diretiva, a própria visão (`o.thisExpr`).
    Detector,
}

impl Token {
    /// O nome que vai no campo (`_NgModel_3_9`, `_NgValidators_3_6`).
    pub fn nome(&self) -> &'static str {
        match self {
            Token::Classe { classe, .. } => classe,
            Token::Multi { nome, .. } => nome,
            Token::Elemento => "HtmlElement",
            Token::Detector => "ChangeDetectorRef",
        }
    }
}

const NG_VALIDATORS: Token = Token::Multi {
    nome: "NgValidators",
    tipo: None,
};
const NG_VALUE_ACCESSOR: Token = Token::Multi {
    nome: "NgValueAccessor",
    tipo: Some((CONTROL_VALUE_ACCESSOR, "ControlValueAccessor")),
};

/// Um item de `providers:` da diretiva: `ExistingProvider(token, existente)`
/// ou `ExistingProvider.forToken(multi, existente)`.
#[derive(Debug)]
pub struct Provedor {
    pub token: Token,
    pub existente: Token,
    pub multi: bool,
}

/// Um parâmetro do construtor.
#[derive(Debug)]
pub struct Dependencia {
    pub token: Token,
    /// `@Optional()`.
    pub opcional: bool,
    /// `@Self()`.
    pub proprio: bool,
}

/// Um `@Input`: nome no template, membro, e se o tipo é `bool` (atributo
/// sem valor vira `true`, `visitEmptyExpr`).
#[derive(Debug)]
pub struct Entrada {
    pub nome: &'static str,
    pub membro: &'static str,
    pub booleana: bool,
}

/// Um `@HostListener`: o evento, o método e os argumentos como o
/// `_addHostListener` os escreve. `args` vazio: método sem parâmetro;
/// `$event`: um parâmetro; outro texto: handler complexo.
#[derive(Debug)]
pub struct Ouvinte {
    pub evento: &'static str,
    pub metodo: &'static str,
    pub args: &'static str,
}

/// O que o gerador sabe de uma diretiva do catálogo.
#[derive(Debug)]
pub struct Conhecida {
    pub classe: &'static str,
    pub uri: &'static str,
    /// `visibility: Visibility.all`: injetável por quem está abaixo.
    pub visivel: bool,
    pub provedores: &'static [Provedor],
    pub dependencias: &'static [Dependencia],
    /// Na ordem do mapa `inputs` (`_SortInputsVisitor`).
    pub entradas: &'static [Entrada],
    /// (nome no template, membro), na ordem do mapa `outputs`.
    pub saidas: &'static [(&'static str, &'static str)],
    pub ouvintes: &'static [Ouvinte],
    pub after_changes: bool,
    pub on_init: bool,
}

/// Cada diretiva do catálogo é uma só: igualdade é identidade.
impl PartialEq for Conhecida {
    fn eq(&self, outra: &Self) -> bool {
        std::ptr::eq(self, outra)
    }
}

impl Eq for Conhecida {}

impl Conhecida {
    pub fn token(&'static self) -> Token {
        Token::Classe {
            uri: self.uri,
            classe: self.classe,
        }
    }

    pub fn entrada(&self, nome: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.nome == nome)
    }

    pub fn saida(&self, nome: &str) -> Option<&'static str> {
        self.saidas
            .iter()
            .find(|(n, _)| *n == nome)
            .map(|(_, m)| *m)
    }
}

/// `ng_form.dart`: `NgForm extends AbstractNgForm<ControlGroup>`, com o
/// `@Input('ngDisabled')` de `AbstractNgForm`, os `@Output` e os
/// `@HostListener('submit')`/`('reset')` de `AbstractForm`.
static NG_FORM_D: Conhecida = Conhecida {
    classe: "NgForm",
    uri: NG_FORM,
    visivel: true,
    provedores: &[Provedor {
        token: Token::Classe {
            uri: CONTROL_CONTAINER,
            classe: "ControlContainer",
        },
        existente: Token::Classe {
            uri: NG_FORM,
            classe: "NgForm",
        },
        multi: false,
    }],
    dependencias: &[
        Dependencia {
            token: NG_VALIDATORS,
            opcional: true,
            proprio: true,
        },
        Dependencia {
            token: Token::Detector,
            opcional: false,
            proprio: false,
        },
    ],
    entradas: &[Entrada {
        nome: "ngDisabled",
        membro: "disabled",
        booleana: false,
    }],
    saidas: &[
        ("ngSubmit", "ngSubmit"),
        ("ngBeforeSubmit", "ngBeforeSubmit"),
    ],
    ouvintes: &[
        Ouvinte {
            evento: "submit",
            metodo: "onSubmit",
            args: "$event",
        },
        Ouvinte {
            evento: "reset",
            metodo: "onReset",
            args: "$event",
        },
    ],
    after_changes: false,
    on_init: false,
};

/// `ng_model.dart`: `NgModel extends NgControl implements AfterChanges,
/// OnInit`.
static NG_MODEL_D: Conhecida = Conhecida {
    classe: "NgModel",
    uri: NG_MODEL,
    visivel: true,
    provedores: &[Provedor {
        token: Token::Classe {
            uri: NG_CONTROL,
            classe: "NgControl",
        },
        existente: Token::Classe {
            uri: NG_MODEL,
            classe: "NgModel",
        },
        multi: false,
    }],
    dependencias: &[
        Dependencia {
            token: NG_VALIDATORS,
            opcional: true,
            proprio: true,
        },
        Dependencia {
            token: NG_VALUE_ACCESSOR,
            opcional: true,
            proprio: true,
        },
    ],
    entradas: &[
        Entrada {
            nome: "ngModel",
            membro: "model",
            booleana: false,
        },
        Entrada {
            nome: "ngDisabled",
            membro: "disabled",
            booleana: true,
        },
    ],
    saidas: &[("ngModelChange", "update")],
    ouvintes: &[],
    after_changes: true,
    on_init: true,
};

/// `default_value_accessor.dart`: os `@HostListener` vêm do mixin
/// `TouchHandler` (`blur`) e da própria classe (`input`).
static DEFAULT_VALUE_ACCESSOR_D: Conhecida = Conhecida {
    classe: "DefaultValueAccessor",
    uri: DEFAULT_VALUE_ACCESSOR,
    visivel: false,
    provedores: &[Provedor {
        token: NG_VALUE_ACCESSOR,
        existente: Token::Classe {
            uri: DEFAULT_VALUE_ACCESSOR,
            classe: "DefaultValueAccessor",
        },
        multi: true,
    }],
    dependencias: &[Dependencia {
        token: Token::Elemento,
        opcional: false,
        proprio: false,
    }],
    entradas: &[],
    saidas: &[],
    ouvintes: &[
        Ouvinte {
            evento: "blur",
            metodo: "touchHandler",
            args: "",
        },
        Ouvinte {
            evento: "input",
            metodo: "handleChange",
            args: "$event.target.value",
        },
    ],
    after_changes: false,
    on_init: false,
};

/// `validators.dart`: `RequiredValidator`, `@Input() bool required`.
static REQUIRED_VALIDATOR_D: Conhecida = Conhecida {
    classe: "RequiredValidator",
    uri: VALIDATORS,
    visivel: false,
    provedores: &[Provedor {
        token: NG_VALIDATORS,
        existente: Token::Classe {
            uri: VALIDATORS,
            classe: "RequiredValidator",
        },
        multi: true,
    }],
    dependencias: &[],
    entradas: &[Entrada {
        nome: "required",
        membro: "required",
        booleana: true,
    }],
    saidas: &[],
    ouvintes: &[],
    after_changes: false,
    on_init: false,
};

static CATALOGO: &[&Conhecida] = &[
    &NG_FORM_D,
    &NG_MODEL_D,
    &DEFAULT_VALUE_ACCESSOR_D,
    &REQUIRED_VALIDATOR_D,
];

/// A diretiva do catálogo declarada em `uri` com o nome `classe`.
pub fn conhecida(uri: &str, classe: &str) -> Option<&'static Conhecida> {
    CATALOGO
        .iter()
        .copied()
        .find(|d| d.uri == uri && d.classe == classe)
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
        diretiva: &'static Conhecida,
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
    pub diretivas: Vec<(&'static Conhecida, String)>,
}

#[derive(Debug)]
enum Fonte {
    Diretiva(&'static Conhecida),
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
pub fn resolver(casadas: &[&'static Conhecida], n: u32) -> Result<NoResolvido, &'static str> {
    // `_ProviderResolver.resolve`: as diretivas (ansiosas), depois os
    // `providers:` de cada uma; o mesmo token multi acumula.
    let mut todos: Vec<Resolvido> = Vec::new();
    for d in casadas {
        todos.push(Resolvido {
            token: d.token(),
            fontes: vec![Fonte::Diretiva(d)],
            multi: false,
            eager: true,
            visivel: d.visivel,
        });
    }
    for d in casadas {
        for p in d.provedores {
            match todos.iter_mut().find(|r| r.token == p.token) {
                Some(r) => {
                    if r.multi != p.multi {
                        return Err("provedor multi e não multi no mesmo token");
                    }
                    if !p.multi {
                        r.fontes.clear();
                    }
                    r.fontes.push(Fonte::Existente(p.existente));
                }
                None => todos.push(Resolvido {
                    token: p.token,
                    fontes: vec![Fonte::Existente(p.existente)],
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
                    for dep in d.dependencias {
                        match dep.token {
                            Token::Elemento | Token::Detector => {}
                            t => match todos.iter().position(|r| r.token == t) {
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
                .map(|_| *alvo)
                .or_else(|| apelidos.iter().find(|(t, _)| t == alvo).map(|(_, a)| *a))
        {
            apelidos.push((r.token, real));
            tamanho += 1;
            continue;
        }
        let campo = format!("_{}_{n}_{tamanho}", r.token.nome());
        let campo_de = |t: &Token| -> Option<String> {
            let t = apelidos
                .iter()
                .find(|(a, _)| a == t)
                .map(|(_, real)| *real)
                .unwrap_or(*t);
            campos.iter().find(|(x, _)| *x == t).map(|(_, c)| c.clone())
        };
        let criacao = match r.fontes.as_slice() {
            [Fonte::Diretiva(d)] => {
                let mut args = Vec::new();
                for dep in d.dependencias {
                    args.push(match dep.token {
                        Token::Elemento => Argumento::Elemento,
                        Token::Detector => Argumento::Detector,
                        t => match campo_de(&t) {
                            Some(c) => Argumento::Campo(c),
                            None => Argumento::Nulo,
                        },
                    });
                }
                saida.diretivas.push((d, campo.clone()));
                Criacao::Diretiva { diretiva: d, args }
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
            token: r.token,
            campo: campo.clone(),
            criacao,
            injetavel_por: if r.visivel { vec![r.token] } else { Vec::new() },
        });
        campos.push((r.token, campo));
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

    /// A ordem e a numeração de `alterar_senha_page.template.dart`: o
    /// `RequiredValidator` antes do multi que o junta, o acessor antes do
    /// seu multi, o `NgModel` por último; `NgControl` é apelido.
    #[test]
    fn input_com_ng_model_e_required() {
        let casadas = [
            &NG_MODEL_D,
            &DEFAULT_VALUE_ACCESSOR_D,
            &REQUIRED_VALIDATOR_D,
        ];
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
                diretiva: &NG_MODEL_D,
                args: vec![
                    Argumento::Campo("_NgValidators_33_6".into()),
                    Argumento::Campo("_NgValueAccessor_33_8".into())
                ]
            }
        );
        assert_eq!(ng_model.injetavel_por.len(), 2);
        assert!(r.instancias[0].injetavel_por.is_empty());
        assert_eq!(r.instancias[1].injetavel_por, vec![NG_VALIDATORS]);
    }

    #[test]
    fn form_com_ng_form() {
        let r = resolver(&[&NG_FORM_D], 19).unwrap();
        assert_eq!(r.instancias.len(), 1);
        assert_eq!(r.instancias[0].campo, "_NgForm_19_5");
        assert_eq!(
            r.instancias[0].criacao,
            Criacao::Diretiva {
                diretiva: &NG_FORM_D,
                args: vec![Argumento::Nulo, Argumento::Detector]
            }
        );
        assert_eq!(r.instancias[0].injetavel_por.len(), 2);
    }
}
