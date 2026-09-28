//! Porte da árvore do csslib 1.0.2 (`src/tree.dart`, `src/tree_base.dart`):
//! os nós que o parser constrói, o visitante e a impressora percorrem.
//!
//! As hierarquias de classes do Dart viram enums; os testes `is` do oficial
//! viram os métodos `e_*`. O que o Dart compartilha por referência e o shim
//! modifica — a `SimpleSelectorSequence`, cujo combinador o `_CompoundSelector`
//! reescreve — é `Rc` com `Cell`, para que a mesma sequência vista de dois
//! lugares mude nos dois, como no oficial. Os seletores simples são imutáveis
//! e também compartilhados (`SimpleSelectorSequence.clone` não os copia).
//!
//! Das posições (`span`) só se guarda o que muda a saída: o texto do
//! `Identifier` (o `toString` dele devolve o lexema original, com escapes) e
//! se um seletor simples tem posição (o shim lê `span!` ao avisar).
use std::cell::Cell;
use std::rc::Rc;

use super::token_kind as tk;

/// `Identifier`: o nome (sem escapes) e o texto do trecho de onde veio.
#[derive(Clone, Debug)]
pub struct Identificador {
    pub nome: String,
    /// `span?.text`; `None` nos criados pelo shim.
    pub texto: Option<String>,
}

impl Identificador {
    /// `Identifier(name, null)`.
    pub fn sem_span(nome: &str) -> Self {
        Identificador {
            nome: nome.to_string(),
            texto: None,
        }
    }

    /// `Identifier.toString`: o lexema, ou o nome.
    pub fn como_texto(&self) -> &str {
        self.texto.as_deref().unwrap_or(&self.nome)
    }
}

/// O `_name` dinâmico de um `SimpleSelector`: `Identifier`, `Wildcard`,
/// `ThisOperator`, `Negation` ou, no `NamespaceSelector`, o
/// `ElementSelector`.
#[derive(Debug)]
pub enum NomeSimples {
    Ident(Identificador),
    /// `Wildcard`.
    Curinga,
    /// `ThisOperator`.
    Este,
    /// `Negation`.
    Negacao,
    Seletor(Rc<SeletorSimples>),
}

impl NomeSimples {
    /// `_name.name`.
    pub fn nome(&self) -> &str {
        match self {
            NomeSimples::Ident(i) => &i.nome,
            NomeSimples::Curinga => "*",
            NomeSimples::Este => "&",
            NomeSimples::Negacao => "not",
            NomeSimples::Seletor(s) => s.nome(),
        }
    }
}

/// O valor de um `AttributeSelector`: `Identifier` ou `String`.
#[derive(Debug)]
pub enum ValorAtributo {
    Ident(Identificador),
    Texto(String),
}

/// O argumento de `PseudoClassFunctionSelector`: `Selector` ou
/// `SelectorExpression`.
#[derive(Debug)]
pub enum ArgumentoPseudo {
    Seletor(Rc<Seletor>),
    Expressao(Vec<Expr>),
}

/// As subclasses de `SimpleSelector`.
#[derive(Debug)]
pub enum TipoSimples {
    /// `ElementSelector`.
    Elemento,
    /// `NamespaceSelector`: `_namespace` (`null`, `Wildcard` ou
    /// `Identifier`); o elemento vai no `_name`.
    Namespace { namespace: Option<NomeSimples> },
    /// `AttributeSelector`.
    Atributo {
        op: i32,
        valor: Option<ValorAtributo>,
    },
    /// `IdSelector`.
    Id,
    /// `ClassSelector`.
    Classe,
    /// `PseudoClassSelector`.
    PseudoClasse,
    /// `PseudoElementSelector`.
    PseudoElemento { legado: bool },
    /// `PseudoClassFunctionSelector`.
    PseudoClasseFuncao(ArgumentoPseudo),
    /// `PseudoElementFunctionSelector`.
    PseudoElementoFuncao(Vec<Expr>),
    /// `NegationSelector`.
    Negacao(Option<Rc<SeletorSimples>>),
}

/// `SimpleSelector`.
#[derive(Debug)]
pub struct SeletorSimples {
    pub tipo: TipoSimples,
    pub nome: NomeSimples,
    /// `span != null`.
    pub tem_span: bool,
}

impl SeletorSimples {
    /// `SimpleSelector.name`.
    pub fn nome(&self) -> &str {
        self.nome.nome()
    }

    /// `is ElementSelector`.
    pub fn e_elemento(&self) -> bool {
        matches!(self.tipo, TipoSimples::Elemento)
    }

    /// `is NamespaceSelector`.
    pub fn e_namespace(&self) -> bool {
        matches!(self.tipo, TipoSimples::Namespace { .. })
    }

    /// `is PseudoClassSelector` (inclui a forma com função).
    pub fn e_pseudo_classe(&self) -> bool {
        matches!(
            self.tipo,
            TipoSimples::PseudoClasse | TipoSimples::PseudoClasseFuncao(_)
        )
    }

    /// `is PseudoClassFunctionSelector`.
    pub fn e_pseudo_classe_funcao(&self) -> bool {
        matches!(self.tipo, TipoSimples::PseudoClasseFuncao(_))
    }

    /// `is PseudoElementSelector` (inclui a forma com função).
    pub fn e_pseudo_elemento(&self) -> bool {
        matches!(
            self.tipo,
            TipoSimples::PseudoElemento { .. } | TipoSimples::PseudoElementoFuncao(_)
        )
    }

    /// `ElementSelector(Identifier(nome, null), null)` e afins: um seletor
    /// criado pelo shim, sem posição.
    pub fn criado(tipo: TipoSimples, nome: &str) -> Rc<SeletorSimples> {
        Rc::new(SeletorSimples {
            tipo,
            nome: NomeSimples::Ident(Identificador::sem_span(nome)),
            tem_span: false,
        })
    }
}

/// `SimpleSelectorSequence`: o combinador é mutável e a sequência é
/// compartilhada.
#[derive(Debug)]
pub struct Sequencia {
    pub combinador: Cell<i32>,
    pub seletor: Rc<SeletorSimples>,
}

impl Sequencia {
    /// `SimpleSelectorSequence(simpleSelector, span, combinator)`.
    pub fn nova(seletor: Rc<SeletorSimples>, combinador: i32) -> Rc<Sequencia> {
        Rc::new(Sequencia {
            combinador: Cell::new(combinador),
            seletor,
        })
    }

    /// `SimpleSelectorSequence.clone`: outra sequência, o mesmo seletor.
    pub fn clonar(&self) -> Rc<Sequencia> {
        Sequencia::nova(Rc::clone(&self.seletor), self.combinador.get())
    }

    /// `isCombinatorNone`.
    pub fn sem_combinador(&self) -> bool {
        self.combinador.get() == tk::COMBINATOR_NONE
    }

    /// `_combinatorToString`.
    pub fn combinador_como_texto(&self) -> &'static str {
        match self.combinador.get() {
            tk::COMBINATOR_DESCENDANT => " ",
            tk::COMBINATOR_GREATER => " > ",
            tk::COMBINATOR_PLUS => " + ",
            tk::COMBINATOR_TILDE => " ~ ",
            _ => "",
        }
    }
}

/// `Selector`.
#[derive(Debug, Default)]
pub struct Seletor {
    pub sequencias: Vec<Rc<Sequencia>>,
}

/// `SelectorGroup`.
#[derive(Debug)]
pub struct GrupoSeletores {
    pub seletores: Vec<Seletor>,
}

/// O valor (`Object value`) de um `LiteralTerm`.
#[derive(Clone, Debug)]
pub enum Valor {
    Int(i64),
    Double(f64),
    Ident(Identificador),
    Texto(String),
    /// `BAD_HEX_VALUE`.
    HexInvalido,
}

/// As subclasses de `LiteralTerm`.
#[derive(Clone, Debug)]
pub enum TipoTermo {
    /// `LiteralTerm`.
    Literal,
    /// `NumberTerm`.
    Numero,
    /// `ItemTerm` (um `NumberTerm`).
    Item,
    /// `UnitTerm` e subclasses: `LengthTerm` (`comprimento`), `AngleTerm`,
    /// `TimeTerm`, `FreqTerm`, `ResolutionTerm`, `ChTerm`, `RemTerm`,
    /// `ViewportTerm`, `LineHeightTerm`.
    Unidade { unidade: i32, comprimento: bool },
    /// `PercentageTerm`.
    Porcentagem,
    /// `EmTerm`.
    Em,
    /// `ExTerm`.
    Ex,
    /// `FractionTerm`.
    Fracao,
    /// `UriTerm`.
    Uri,
    /// `HexColorTerm`.
    Hex,
    /// `IE8Term`.
    Ie8,
    /// `FunctionTerm`.
    Funcao(Expressoes),
    /// `CalcTerm`.
    Calc(Box<Termo>),
}

/// `LiteralTerm` (e subclasses).
#[derive(Clone, Debug)]
pub struct Termo {
    pub tipo: TipoTermo,
    pub valor: Valor,
    pub texto: String,
}

impl Termo {
    pub fn novo(tipo: TipoTermo, valor: Valor, texto: String) -> Self {
        Termo { tipo, valor, texto }
    }

    /// `is NumberTerm`.
    pub fn e_numero(&self) -> bool {
        matches!(self.tipo, TipoTermo::Numero | TipoTermo::Item)
    }
}

/// `Expression` (fora `KeyFrameBlock`, que fica no `KeyFrameDirective`).
#[derive(Clone, Debug)]
pub enum Expr {
    Termo(Termo),
    /// `UnicodeRangeTerm`.
    UnicodeRange {
        primeiro: Option<String>,
        segundo: Option<String>,
    },
    /// `VarUsage`.
    VarUsage {
        nome: String,
        padroes: Vec<Expr>,
    },
    /// `OperatorSlash`.
    Barra,
    /// `OperatorComma`.
    Virgula,
    /// `OperatorPlus`.
    Mais,
    /// `OperatorMinus`.
    Menos,
    /// `GroupTerm`.
    Grupo(Vec<Termo>),
}

/// `Expressions`.
#[derive(Clone, Debug, Default)]
pub struct Expressoes {
    pub expressoes: Vec<Expr>,
}

/// `VarDefinition`.
#[derive(Debug)]
pub struct VarDef {
    pub nome: Option<Identificador>,
    pub expressao: Option<Expressoes>,
}

/// `IncludeDirective`.
#[derive(Debug)]
pub struct Include {
    pub nome: String,
    pub args: Vec<Vec<Expr>>,
}

/// `Declaration` e subclasses.
#[derive(Debug)]
pub enum Declaracao {
    /// `Declaration`.
    Comum {
        propriedade: Identificador,
        expressao: Expressoes,
        importante: bool,
        ie7: bool,
    },
    /// `VarDefinition`.
    VarDef(VarDef),
    /// `IncludeMixinAtDeclaration`.
    Include(Include),
    /// `ExtendDeclaration`.
    Extend(Vec<Rc<SeletorSimples>>),
}

/// `DeclarationGroup`; com `margem`, o `MarginGroup` de um `@page`.
#[derive(Debug, Default)]
pub struct GrupoDeclaracoes {
    pub declaracoes: Vec<No>,
    pub margem: Option<i32>,
}

/// `RuleSet`.
#[derive(Debug)]
pub struct RuleSet {
    pub grupo: GrupoSeletores,
    pub declaracoes: GrupoDeclaracoes,
}

/// `MediaExpression`.
#[derive(Debug)]
pub struct ExprMedia {
    pub e: bool,
    pub recurso: Identificador,
    pub exprs: Expressoes,
}

/// `MediaQuery`.
#[derive(Debug)]
pub struct ConsultaMedia {
    pub unario: i32,
    pub tipo: Option<Identificador>,
    pub expressoes: Vec<ExprMedia>,
}

/// `KeyFrameBlock`.
#[derive(Debug)]
pub struct BlocoKeyframe {
    pub seletores: Expressoes,
    pub declaracoes: GrupoDeclaracoes,
}

/// `SupportsCondition` e subclasses.
#[derive(Debug)]
pub enum CondSupports {
    /// `SupportsConditionInParens`: uma `Declaration` ou uma condição.
    EmParenteses(Option<Box<DentroDeParenteses>>),
    /// `SupportsNegation`.
    Negacao(Box<CondSupports>),
    /// `SupportsConjunction`.
    Conjuncao(Vec<CondSupports>),
    /// `SupportsDisjunction`.
    Disjuncao(Vec<CondSupports>),
}

/// O `condition` de `SupportsConditionInParens`.
#[derive(Debug)]
pub enum DentroDeParenteses {
    Declaracao(Declaracao),
    Condicao(CondSupports),
}

/// Os `TreeNode` que vivem em listas: regras, diretivas e declarações.
#[derive(Debug)]
pub enum No {
    Regra(RuleSet),
    Declaracao(Declaracao),
    /// `ImportDirective`.
    Import {
        import: String,
        medias: Vec<ConsultaMedia>,
    },
    /// `MediaDirective`.
    Media {
        consultas: Vec<ConsultaMedia>,
        regras: Vec<No>,
    },
    /// `HostDirective`.
    Host {
        regras: Vec<No>,
    },
    /// `PageDirective`.
    Pagina {
        ident: String,
        pseudo: String,
        grupos: Vec<GrupoDeclaracoes>,
    },
    /// `CharsetDirective`.
    Charset(String),
    /// `KeyFrameDirective`.
    Keyframes {
        tipo: i32,
        nome: Option<Identificador>,
        blocos: Vec<BlocoKeyframe>,
    },
    /// `FontFaceDirective`.
    FontFace(GrupoDeclaracoes),
    /// `NamespaceDirective`.
    Namespace {
        prefixo: String,
        uri: Option<String>,
    },
    /// `VarDefinitionDirective`.
    VarDefDiretiva(VarDef),
    /// `MixinRulesetDirective`.
    MixinRegras {
        nome: String,
        regras: Vec<No>,
    },
    /// `MixinDeclarationDirective`.
    MixinDeclaracoes {
        nome: String,
        declaracoes: GrupoDeclaracoes,
    },
    /// `IncludeDirective`.
    Include(Include),
    /// `DocumentDirective`.
    Documento {
        funcoes: Vec<Termo>,
        corpo: Vec<No>,
    },
    /// `SupportsDirective`.
    Supports {
        condicao: Option<CondSupports>,
        corpo: Vec<No>,
    },
    /// `ViewportDirective`.
    Viewport {
        nome: String,
        declaracoes: GrupoDeclaracoes,
    },
}

/// `StyleSheet`.
#[derive(Debug, Default)]
pub struct Folha {
    pub topo: Vec<No>,
}
