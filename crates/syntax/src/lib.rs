//! Tokens e árvore sintática do subconjunto suportado de Dart 3.6.2.
use dartforge_diagnostics::Span;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Categoria de token que empresta lexemas do texto original.
pub enum TokenKind<'a> {
    Word(&'a str),
    String(&'a str),
    /// Conteúdo de uma string raw, sem interpretação de escapes.
    RawString(&'a str),
    /// String de aspas triplas sem interpolação; `raw` preserva barras invertidas.
    ///
    /// A primeira linha em branco já foi descartada pelo lexer; a normalização de
    /// CR e CRLF para LF fica com o parser, que só aloca quando é necessária.
    MultilineString {
        text: &'a str,
        raw: bool,
    },
    /// Abre uma string interpolada com o literal anterior à primeira expressão.
    StringStart {
        text: &'a str,
        multiline: bool,
    },
    /// Fecha uma interpolação e carrega o literal até a próxima expressão.
    StringMid {
        text: &'a str,
        multiline: bool,
    },
    /// Fecha a última interpolação e carrega o literal final do literal.
    StringEnd {
        text: &'a str,
        multiline: bool,
    },
    /// Identificador da forma `$nome`, que nunca continua em acesso a membro.
    ///
    /// Distingue `'$obj.campo'`, que interpola apenas `obj`, de `'${obj.campo}'`,
    /// que interpola a expressão inteira.
    InterpolatedName(&'a str),
    Number(&'a str),
    Symbol(char),
    Operator(&'a str),
}
#[derive(Debug, Clone, Copy)]
/// Token com localização em bytes no arquivo de origem.
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Tipo primitivo ou ausência de valor reconhecido neste subconjunto.
pub enum Type {
    Duration,
    Timer,
    /// Parâmetro posicional do ambiente genérico da função corrente.
    Parameter(u32),
    /// Parâmetro genérico tornado anulável explicitamente por T?.
    NullableParameter(u32),
    Object,
    NullableObject,
    /// Índice de uma forma estrutural em Program.types.
    Applied(u32),
    /// Anotação omitida que a análise contextual precisa resolver.
    Inferred,
    Void,
    /// Tipo nominal identificado pela posição da declaração de classe.
    Class(u32),
    /// Tipo nominal que também aceita null.
    NullableClass(u32),
    Null,
    NullableInt,
    NullableString,
    NullableBool,
    Int,
    Double,
    /// Supertipo numérico de `int` e `double` (união coberta pela semântica WEB/JS Number).
    Num,
    NullableDouble,
    NullableNum,
    String,
    Bool,
}
/// Forma estrutural compartilhada sem retirar Copy dos tipos da AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeShape {
    Future(Type),
    /// Mapa com tipos reificados de chave e valor.
    Map {
        key: Type,
        value: Type,
    },
    /// Tipo de record; nomes ordenados canonicamente, posições mantidas na ordem.
    Record {
        positional: Vec<Type>,
        named: Vec<(String, Type)>,
    },
    /// Nulabilidade estrutural normalizada durante a substituição genérica.
    Nullable(Type),
    List(Type),
    /// Conjunto com ordem de inserção preservada, como o `LinkedHashSet` padrão de Dart.
    Set(Type),
    Iterable(Type),
    Function {
        result: Type,
        parameters: Vec<Type>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Operação binária preservada na árvore de expressões.
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Remainder,
    /// Divisão `/`: sempre produz `double`, mesmo entre inteiros (oráculo Dart 3.6.2).
    Divide,
    /// Divisão truncada `~/`: sempre produz `int`, mesmo entre doubles.
    TruncDivide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    /// Seleciona o operando direito somente quando o esquerdo é null.
    IfNull,
    /// `&` bit a bit sobre a representação de 32 bits do alvo web.
    BitAnd,
    /// `|` bit a bit sobre a representação de 32 bits do alvo web.
    BitOr,
    /// `^` bit a bit sobre a representação de 32 bits do alvo web.
    BitXor,
    /// `<<` com contagem validada; contagem negativa lança, como em Dart.
    ShiftLeft,
    /// `>>` aritmético, preservando o sinal do operando esquerdo.
    ShiftRight,
    /// `>>>` lógico, disponível a partir de Dart 2.14.
    ShiftRightUnsigned,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Operação prefixa ou asserção pós-fixa com um único operando.
pub enum UnaryOp {
    Negate,
    Not,
    /// Asserção pós-fixa de valor não nulo.
    NullAssert,
    /// `~`: complemento de um sobre a representação de 32 bits do alvo web.
    BitNot,
}
#[derive(Debug, Clone)]
/// Expressão acompanhada do intervalo de origem.
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}
/// Unidade nomeada de Duration, preservada na ordem de avaliação da chamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    Days,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
    Microseconds,
}
/// Forma sintática de uma expressão.
#[derive(Debug, Clone)]
pub enum ExprKind<'a> {
    /// Argumento rotulado `nome: valor` de uma lista de argumentos.
    ///
    /// Modelado como expressão para não duplicar o campo `arguments` de cada
    /// forma de chamada. O parser só a produz como elemento direto de uma lista
    /// de argumentos e a análise semântica rejeita a forma em qualquer outra
    /// posição. O span cobre `nome: valor` inteiro; `label` é o rótulo externo.
    NamedArgument {
        label: &'a str,
        value: Box<Expr<'a>>,
    },
    Await(Box<Expr<'a>>),
    FutureValue {
        value: Option<Box<Expr<'a>>>,
        value_type: Option<Type>,
    },
    FutureDelayed {
        duration: Box<Expr<'a>>,
        computation: Option<Box<Expr<'a>>>,
        value_type: Option<Type>,
    },
    Duration {
        parts: Vec<(DurationUnit, Expr<'a>)>,
    },
    /// Literal de mapa que preserva a ordem de avaliação das entradas.
    ///
    /// Uma entrada comum guarda `(chave, Some(valor))`. Os elementos de
    /// controle — `...`, `...?`, `if` e `for` — guardam `(elemento, None)`
    /// porque produzem zero ou mais entradas e não têm chave própria.
    Map {
        key_type: Option<Type>,
        value_type: Option<Type>,
        entries: Vec<(Expr<'a>, Option<Expr<'a>>)>,
    },
    /// Literal de conjunto, com ordem de inserção preservada como em Dart.
    ///
    /// `{}` sem contexto é um mapa vazio; só `<T>{}` ou um contexto `Set<T>`
    /// produzem esta forma vazia, como nos SDKs 3.6.2 e 3.13.4.
    Set {
        element_type: Option<Type>,
        elements: Vec<Expr<'a>>,
    },
    /// Espalhamento `...operando` ou `...?operando` num literal de coleção.
    ///
    /// O operando é avaliado exatamente uma vez. Com `null_aware`, um operando
    /// null não acrescenta nada; sem ele, a análise exige operando não anulável.
    Spread {
        operand: Box<Expr<'a>>,
        null_aware: bool,
    },
    /// Elemento condicional `if (cond) a` ou `if (cond) a else b` de coleção.
    ///
    /// Apenas um dos ramos é avaliado; sem `else`, a condição falsa não
    /// acrescenta nada. O `else` liga-se sempre ao `if` mais interno.
    CollectionIf {
        condition: Box<Expr<'a>>,
        then_element: Box<Expr<'a>>,
        else_element: Option<Box<Expr<'a>>>,
    },
    /// Elemento repetido `for (...) elemento` de um literal de coleção.
    ///
    /// `header` é um `StatementKind::For` ou `StatementKind::ForIn` de corpo
    /// vazio: o elemento fica em `element` porque produz valores, não
    /// instruções. Assim a análise e a emissão reaproveitam o cabeçalho do laço.
    CollectionFor {
        header: Box<Statement<'a>>,
        element: Box<Expr<'a>>,
    },
    /// Cadeia null-aware `?.`/`?[`: curto-circuita todos os seletores seguintes.
    ///
    /// O receptor é avaliado exatamente uma vez e, quando é null, nenhum
    /// seletor da cadeia executa e o valor é null. `chain` é construída sobre
    /// `NullShortTarget`, o receptor sintético da cadeia — o mesmo desenho de
    /// `Cascade`/`CascadeReceiver`. O tipo resultante é sempre anulável.
    NullShort {
        receiver: Box<Expr<'a>>,
        chain: Box<Expr<'a>>,
        /// Span do operador `?.`/`?[`, que identifica o receptor sintético.
        target: Span,
    },
    /// Receptor sintético da cadeia null-aware envolvente; span do operador.
    NullShortTarget,
    /// Entrada `chave: valor` escrita dentro de um `if`/`for` de literal de mapa.
    ///
    /// As entradas de topo continuam no par `(chave, Some(valor))` de `Map`;
    /// esta forma existe porque um ramo de `if` ou o corpo de um `for` é uma
    /// expressão, e precisa carregar as duas posições juntas.
    MapEntry {
        key: Box<Expr<'a>>,
        value: Box<Expr<'a>>,
    },
    /// Invocação de fábrica nomeada, validada separadamente dos membros de instância.
    NamedConstruct {
        class_id: u32,
        name: &'a str,
        arguments: Vec<Expr<'a>>,
    },
    /// Avalia o receptor uma vez e devolve-o após executar as seções em ordem.
    Cascade {
        receiver: Box<Expr<'a>>,
        null_aware: bool,
        sections: Vec<Statement<'a>>,
    },
    /// Receptor sintético da cascata envolvente; span aponta somente seu operador.
    CascadeReceiver,
    /// Atalho de ponto `.nome` ou `.nome(args)`: o tipo vem apenas do contexto.
    ///
    /// A resolução nominal ocorre na análise semântica, que grava o tipo da
    /// expressão; o parser não conhece a classe alvo. `arguments` ausente indica
    /// acesso a um valor de enum, presente indica fábrica nomeada.
    DotShorthand {
        name: &'a str,
        arguments: Option<Vec<Expr<'a>>>,
    },
    /// Elemento condicional `?valor` de coleção: omitido quando avalia para null.
    ///
    /// Só é válido diretamente em literais de lista e nas duas posições de uma
    /// entrada de mapa. O operando é avaliado exatamente uma vez.
    NullAwareElement(Box<Expr<'a>>),
    /// Literal de record; None indica posição e a lista preserva toda ordem de avaliação.
    Record {
        fields: Vec<(Option<&'a str>, Expr<'a>)>,
    },
    TypeTest {
        operand: Box<Expr<'a>>,
        ty: Type,
        negated: bool,
    },
    Cast {
        operand: Box<Expr<'a>>,
        ty: Type,
    },
    Const(Box<Expr<'a>>),
    GenericCall {
        name: &'a str,
        type_arguments: Vec<Type>,
        arguments: Vec<Expr<'a>>,
    },
    Switch {
        scrutinee: Box<Expr<'a>>,
        arms: Vec<SwitchArm<'a>>,
    },
    Closure {
        is_async: bool,
        parameters: Vec<Parameter<'a>>,
        return_type: Type,
        body: Vec<Statement<'a>>,
        is_arrow: bool,
    },
    List {
        element_type: Option<Type>,
        elements: Vec<Expr<'a>>,
    },
    Index {
        receiver: Box<Expr<'a>>,
        index: Box<Expr<'a>>,
    },
    Invoke {
        callee: Box<Expr<'a>>,
        arguments: Vec<Expr<'a>>,
    },
    /// Valor canônico de enum, identificado nominalmente e pelo nome declarado.
    EnumValue {
        class_id: u32,
        name: &'a str,
    },
    This,
    Construct {
        class_id: u32,
        arguments: Vec<Expr<'a>>,
    },
    Member {
        receiver: Box<Expr<'a>>,
        name: &'a str,
    },
    MethodCall {
        receiver: Box<Expr<'a>>,
        name: &'a str,
        arguments: Vec<Expr<'a>>,
    },
    /// Operador condicional; apenas um dos ramos é avaliado.
    ///
    /// Liga mais forte que a cascata e mais fraco que `??`, `||` e `&&`, e é
    /// associativo à direita: `a ? b : c ? d : e` agrupa `c ? d : e`.
    Conditional {
        condition: Box<Expr<'a>>,
        then_value: Box<Expr<'a>>,
        else_value: Box<Expr<'a>>,
    },
    /// `throw valor`: interrompe o fluxo e nunca produz um valor utilizável.
    ///
    /// Dart só admite esta forma no topo de uma expressão ou num ramo do
    /// operador condicional; `a ?? throw e` exige parênteses em 3.6.2 e 3.13.4.
    Throw(Box<Expr<'a>>),
    Null,
    Int(i32),
    /// Literal double com semântica WEB (JS Number); a impressão Dart (`1.0`)
    /// é reproduzida pelo formatador `$dartforgeDouble` no backend JavaScript.
    Double(f64),
    String(&'a str),
    /// String que precisou de alocação para decodificar escapes Unicode.
    OwnedString(String),
    /// Interpolação de string: trechos literais e expressões na ordem escrita.
    ///
    /// O parser só produz esta forma quando há pelo menos uma expressão; literais
    /// adjacentes sem nenhuma interpolação continuam virando `String`/`OwnedString`.
    /// Trechos literais vazios são omitidos, então a lista não tem posições
    /// inúteis. Cada expressão é avaliada exatamente uma vez, na ordem da lista,
    /// e convertida com a semântica de `toString` do Dart.
    Interpolation(Vec<StringPart<'a>>),
    Bool(bool),
    Identifier(&'a str),
    Call {
        name: &'a str,
        arguments: Vec<Expr<'a>>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr<'a>>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr<'a>>,
        right: Box<Expr<'a>>,
    },
}
/// Parte de uma interpolação de string, na ordem em que foi escrita.
#[derive(Debug, Clone)]
pub enum StringPart<'a> {
    /// Texto literal emprestado da fonte, sem escapes nem normalização pendentes.
    Borrowed(&'a str),
    /// Texto literal que precisou de alocação para decodificar escapes.
    Owned(String),
    /// Expressão convertida com `toString` e avaliada exatamente uma vez.
    Expression(Expr<'a>),
}
#[derive(Debug, Clone)]
/// Instrução acompanhada do intervalo de origem.
pub struct Statement<'a> {
    pub kind: StatementKind<'a>,
    pub span: Span,
}
#[derive(Debug, Clone)]
/// Forma sintática de uma instrução.
pub enum StatementKind<'a> {
    /// Declaração por padrão simples; named contém campo, variável e span da variável.
    RecordDestructure {
        is_final: bool,
        positional: Vec<(&'a str, Span)>,
        named: Vec<(&'a str, &'a str, Span)>,
        initializer: Expr<'a>,
    },
    Switch {
        scrutinee: Expr<'a>,
        cases: Vec<SwitchCase<'a>>,
    },
    IndexAssign {
        receiver: Expr<'a>,
        index: Expr<'a>,
        value: Expr<'a>,
    },
    FieldAssign {
        receiver: Expr<'a>,
        name: &'a str,
        value: Expr<'a>,
    },
    Variable {
        is_const: bool,
        name: &'a str,
        annotation: Option<Type>,
        is_final: bool,
        initializer: Expr<'a>,
    },
    Assign {
        name: &'a str,
        value: Expr<'a>,
    },
    Print(Expr<'a>),
    Return(Option<Expr<'a>>),
    Expression(Expr<'a>),
    If {
        condition: Expr<'a>,
        then_body: Vec<Statement<'a>>,
        else_body: Option<Vec<Statement<'a>>>,
    },
    /// Repete o corpo enquanto a condição booleana for verdadeira.
    While {
        condition: Expr<'a>,
        body: Vec<Statement<'a>>,
    },
    /// Executa o corpo antes de testar a condição booleana.
    DoWhile {
        body: Vec<Statement<'a>>,
        condition: Expr<'a>,
    },
    /// Laço clássico com inicialização, condição e atualização opcionais.
    For {
        initializer: Option<Box<Statement<'a>>>,
        condition: Option<Expr<'a>>,
        update: Option<Box<Statement<'a>>>,
        body: Vec<Statement<'a>>,
    },
    /// Encerra o laço mais próximo; a forma rotulada é `BreakLabel`.
    Break,
    /// Inicia a próxima iteração do laço mais próximo.
    Continue,
    /// Encerra o laço rotulado, que pode ser externo ao mais próximo.
    BreakLabel(&'a str),
    /// Retoma o laço rotulado, que pode ser externo ao mais próximo.
    ContinueLabel(&'a str),
    /// Rótulo aplicado a um laço; só laços podem receber rótulo neste subconjunto.
    Labeled {
        label: &'a str,
        body: Box<Statement<'a>>,
    },
    /// Protege o corpo com cláusulas de captura e um bloco final.
    ///
    /// As cláusulas são testadas na ordem escrita; `finally_body` executa em
    /// toda saída do `try`, inclusive por `return`, `break`, `continue` e
    /// exceção, sem alterar o valor de retorno nem engolir a exceção.
    Try {
        body: Vec<Statement<'a>>,
        catches: Vec<CatchClause<'a>>,
        finally_body: Option<Vec<Statement<'a>>>,
    },
    /// Relança o valor capturado pela cláusula `catch` envolvente.
    Rethrow,
    /// Verificação de desenvolvimento `assert(condição)` ou com mensagem.
    Assert {
        condition: Expr<'a>,
        message: Option<Expr<'a>>,
    },
    /// `for (final x in iterável)`; o iterável é avaliado exatamente uma vez.
    ForIn {
        is_final: bool,
        name: &'a str,
        annotation: Option<Type>,
        iterable: Expr<'a>,
        body: Vec<Statement<'a>>,
    },
    Block(Vec<Statement<'a>>),
}
/// Cláusula de captura de uma instrução `try`.
///
/// `exception_type` vem de `on T` e restringe a captura ao valor cujo tipo em
/// execução é `T`; sem ele a cláusula captura qualquer valor lançado.
/// `exception` e `stack_trace` são as variáveis de `catch (e)` e `catch (e, s)`;
/// uma cláusula `on T { ... }` sem `catch` não liga nenhuma das duas.
#[derive(Debug, Clone)]
pub struct CatchClause<'a> {
    pub exception_type: Option<Type>,
    pub exception: Option<&'a str>,
    pub stack_trace: Option<&'a str>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
}
/// Forma de passagem de um parâmetro na assinatura declarada.
///
/// A ordem de declaração é sempre obrigatórios posicionais, depois opcionais
/// posicionais (`[...]`) **ou** nomeados (`{...}`); Dart não permite os dois
/// grupos opcionais na mesma assinatura e o parser rejeita a combinação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParameterKind {
    /// Posicional obrigatório: a forma neutra e única até este incremento.
    #[default]
    RequiredPositional,
    /// Posicional dentro de `[...]`; ausente assume o padrão ou null.
    OptionalPositional,
    /// Nomeado dentro de `{...}`; `required` torna a ausência um erro de compilação.
    Named { required: bool },
}
impl ParameterKind {
    /// Indica se o parâmetro é passado por rótulo em vez de por posição.
    #[must_use]
    pub const fn is_named(self) -> bool {
        matches!(self, Self::Named { .. })
    }
    /// Indica se a chamada precisa fornecer o argumento obrigatoriamente.
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(
            self,
            Self::RequiredPositional | Self::Named { required: true }
        )
    }
}
/// Rótulo externo de um parâmetro nomeado privado (Dart 3.12).
///
/// O nome interno `_apiKey` é visível apenas dentro da declaração; quem chama
/// escreve `apiKey:`. Nomes sem sublinhado inicial são devolvidos intactos.
///
/// # Exemplos
/// ```
/// assert_eq!(dartforge_syntax::external_label("_apiKey"), "apiKey");
/// assert_eq!(dartforge_syntax::external_label("apiKey"), "apiKey");
/// ```
#[must_use]
pub fn external_label(name: &str) -> &str {
    name.strip_prefix('_').unwrap_or(name)
}
#[derive(Debug, Clone)]
/// Parâmetro de função com tipo explícito, forma de passagem e padrão opcional.
///
/// `default` guarda a expressão escrita; a análise semântica exige que ela seja
/// um literal escalar constante, como documentado em `docs/PARAMETROS.md`. O
/// `Box` mantém o caminho comum — posicional obrigatório sem padrão — sem
/// alocação alguma.
pub struct Parameter<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub kind: ParameterKind,
    pub default: Option<Box<Expr<'a>>>,
    pub span: Span,
}
impl<'a> Parameter<'a> {
    /// Cria um parâmetro posicional obrigatório, a forma neutra da assinatura.
    ///
    /// # Exemplos
    /// ```
    /// use dartforge_syntax::{Parameter, ParameterKind, Type};
    /// use dartforge_diagnostics::Span;
    /// let p = Parameter::required("a", Type::Int, Span { start: 0, end: 1 });
    /// assert_eq!(p.kind, ParameterKind::RequiredPositional);
    /// assert!(p.default.is_none());
    /// ```
    #[must_use]
    pub const fn required(name: &'a str, ty: Type, span: Span) -> Self {
        Self {
            name,
            ty,
            kind: ParameterKind::RequiredPositional,
            default: None,
            span,
        }
    }
    /// Rótulo usado na chamada; só difere do nome interno em nomeados privados.
    #[must_use]
    pub fn label(&self) -> &'a str {
        if self.kind.is_named() {
            external_label(self.name)
        } else {
            self.name
        }
    }
}
#[derive(Debug, Clone)]
/// Função top-level com assinatura e corpo.
pub struct Function<'a> {
    /// Corpo arrow conserva regras de descarte e adoção de Future distintas de return em bloco.
    pub is_arrow: bool,
    pub is_async: bool,
    pub annotations: Vec<Annotation>,
    pub native_binding: Option<NativeBinding<'a>>,
    pub type_parameters: Vec<GenericParameter<'a>>,
    /// Marca um **acessor** de propriedade, não apenas um getter.
    ///
    /// Sem parâmetros é o getter `T get nome`; com exatamente um parâmetro e
    /// retorno `void` é o setter `set nome(T valor)`. A distinção fica na
    /// forma da assinatura, e não num campo novo, porque um setter precisa
    /// viajar dentro de [`Class::methods`]: é essa lista que o linker percorre
    /// para renomear membros privados e remapear spans entre bibliotecas. Uma
    /// lista paralela ficaria invisível para ele e produziria nomes e spans
    /// errados em programas com mais de uma biblioteca.
    ///
    /// A combinação "acessor com um parâmetro" era impossível antes: a análise
    /// semântica rejeitava getters com parâmetros. [`Function::is_setter`] e
    /// [`Function::is_property_getter`] leem a marca sem repetir a regra.
    pub is_getter: bool,
    /// Nome declarado; `==` identifica a declaração `operator ==`.
    ///
    /// Nenhum identificador Dart pode ser `==`, então o nome do operador não
    /// colide com membro algum e atravessa linker, otimizadores e emissão como
    /// um método comum, herança e contratos de override inclusive.
    pub name: &'a str,
    pub return_type: Type,
    pub parameters: Vec<Parameter<'a>>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
}
/// Nome interno da declaração `operator ==`.
///
/// # Exemplos
/// ```
/// assert_eq!(dartforge_syntax::EQUALS_OPERATOR, "==");
/// ```
pub const EQUALS_OPERATOR: &str = "==";
impl Function<'_> {
    /// Indica se a declaração é o setter `set nome(T valor)`.
    ///
    /// # Exemplos
    /// ```
    /// use dartforge_syntax::{Function, Parameter, Type};
    /// use dartforge_diagnostics::Span;
    /// let span = Span { start: 0, end: 1 };
    /// let mut f = Function {
    ///     is_arrow: false, is_async: false, annotations: vec![], native_binding: None,
    ///     type_parameters: vec![], is_getter: true, name: "x", return_type: Type::Void,
    ///     parameters: vec![Parameter::required("v", Type::Int, span)], body: vec![], span,
    /// };
    /// assert!(f.is_setter());
    /// assert!(!f.is_property_getter());
    /// f.parameters.clear();
    /// f.return_type = Type::Int;
    /// assert!(f.is_property_getter());
    /// ```
    #[must_use]
    pub fn is_setter(&self) -> bool {
        self.is_getter && self.parameters.len() == 1
    }
    /// Indica se a declaração é o getter `T get nome`, sem parâmetros.
    #[must_use]
    pub fn is_property_getter(&self) -> bool {
        self.is_getter && self.parameters.is_empty()
    }
    /// Indica se a declaração é `operator ==`.
    #[must_use]
    pub fn is_equals_operator(&self) -> bool {
        self.name == EQUALS_OPERATOR
    }
}
/// Parâmetro genérico com limite explícito; o limite omitido é Object?.
#[derive(Debug, Clone)]
pub struct GenericParameter<'a> {
    pub name: &'a str,
    pub bound: Type,
    pub span: Span,
}
/// Tipo escalar da assinatura C; permanece separado do tipo Dart da função.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeType {
    Void,
    Int32,
    Int64,
}
/// Vínculo externo declarado por Native, com símbolo preservado antes do linker.
#[derive(Debug, Clone)]
pub struct NativeBinding<'a> {
    pub prefix: Option<&'a str>,
    pub symbol: String,
    pub result: NativeType,
    pub parameters: Vec<NativeType>,
    pub is_leaf: bool,
    pub span: Span,
}
/// Anotação reconhecida, conservada para validação de alvo e ferramentas.
#[derive(Debug, Clone)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub span: Span,
}
/// Metadados suportados sem execução de construtores arbitrários.
#[derive(Debug, Clone)]
pub enum AnnotationKind {
    /// Solicita expansão da macro nativa de serialização antes da análise semântica.
    JsonCodable,
    /// Solicita a macro nativa de classe de dados: copyWith, igualdade e texto.
    DataClass,
    Override,
    Deprecated {
        message: Option<String>,
    },
}
#[derive(Debug, Clone)]
/// Programa com funções auxiliares e o corpo da entrada main.
pub struct Program<'a> {
    pub main_is_arrow: bool,
    pub main_is_async: bool,
    pub types: Vec<TypeShape>,
    pub extensions: Vec<Extension<'a>>,
    pub classes: Vec<Class<'a>>,
    pub functions: Vec<Function<'a>>,
    pub statements: Vec<Statement<'a>>,
}

/// Padrão simples de switch: constante, wildcard ou binding tipado.
#[derive(Debug, Clone)]
pub enum Pattern<'a> {
    Constant(Expr<'a>),
    Wildcard,
    /// Padrão de objeto vazio: testa o tipo sem extrair campos.
    Type(Type),
    Binding {
        ty: Type,
        name: &'a str,
    },
}
/// Braço de switch expressão, com guarda opcional e intervalo de origem.
#[derive(Debug, Clone)]
pub struct SwitchArm<'a> {
    pub pattern: Pattern<'a>,
    pub guard: Option<Expr<'a>>,
    pub value: Expr<'a>,
    pub span: Span,
}
/// Caso de switch instrução; o backend encerra o caso sem fallthrough implícito.
#[derive(Debug, Clone)]
pub struct SwitchCase<'a> {
    pub pattern: Pattern<'a>,
    pub guard: Option<Expr<'a>>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
}

/// Restrição nominal declarada por base, final ou sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    None,
    Base,
    Final,
    Sealed,
}
/// Forma da declaração nominal; mixin isolado não possui construtor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Mixin,
    MixinClass,
}
/// Classe nominal com construtor implícito, herança e aplicações de mixins.
#[derive(Debug, Clone)]
pub struct Class<'a> {
    /// Fábricas nomeadas com retorno explícito da classe e sem receptor this.
    pub factories: Vec<Function<'a>>,
    pub constructor: Option<Constructor<'a>>,
    /// Extras do construtor sem nome; `None` é o caminho comum e não aloca.
    pub constructor_extras: Option<Box<ConstructorExtras<'a>>>,
    /// Construtores nomeados na ordem escrita.
    pub named_constructors: Vec<NamedConstructor<'a>>,
    /// Campos estáticos na ordem escrita; nunca herdados.
    pub static_fields: Vec<StaticField<'a>>,
    /// Métodos estáticos na ordem escrita; resolvidos pelo nome da classe.
    pub static_methods: Vec<Function<'a>>,
    /// Declaração sintética que só agrupa as variáveis de topo da biblioteca.
    ///
    /// Não é instanciável, não tem membros de instância e não aparece como
    /// tipo: existe porque `Program` não pode ganhar campos sem quebrar o
    /// linker, que constrói a estrutura por literal.
    pub is_library_globals: bool,
    pub annotations: Vec<Annotation>,
    /// Marca a classe sintética criada ao expandir uma aplicação de mixin.
    pub is_mixin_application: bool,
    /// Identidade da declaração de mixin que originou a classe sintética.
    pub mixin_origin: Option<u32>,
    pub modifier: ClassModifier,
    pub kind: ClassKind,
    /// Aplicações na ordem escrita; a última tem precedência de implementação.
    pub mixins: Vec<u32>,
    /// Argumentos constantes de cada valor, na mesma ordem de enum_values.
    pub enum_arguments: Vec<Vec<Expr<'a>>>,
    /// Nomes dos campos associados aos parâmetros this.campo do construtor const.
    pub enum_constructor_fields: Vec<&'a str>,
    /// Modificador interface restringe extends fora da biblioteca declaradora.
    pub is_interface: bool,
    /// Identidade da biblioteca atribuída pelo linker; unidade isolada usa zero.
    pub library_id: usize,
    pub is_abstract: bool,
    pub interfaces: Vec<u32>,
    pub abstract_methods: Vec<Function<'a>>,
    /// Ordem ordinal; lista não vazia identifica um enum simples.
    pub enum_values: Vec<&'a str>,
    pub id: u32,
    pub name: &'a str,
    pub superclass: Option<u32>,
    pub fields: Vec<Field<'a>>,
    pub methods: Vec<Function<'a>>,
    pub span: Span,
}
/// Campo tipado; ausência de inicializador exige validação do construtor ou valor padrão.
#[derive(Debug, Clone)]
pub struct Field<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub is_final: bool,
    pub initializer: Option<Expr<'a>>,
    pub span: Span,
}
/// Construtor generativo sem nome, com parâmetros posicionais obrigatórios.
#[derive(Debug, Clone)]
pub struct Constructor<'a> {
    pub parameters: Vec<ConstructorParameter<'a>>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
}
/// Entrada `campo = valor` de uma lista de inicialização.
///
/// O campo precisa pertencer à própria classe; o valor é avaliado com os
/// parâmetros do construtor em escopo e antes de a superclasse inicializar.
#[derive(Debug, Clone)]
pub struct FieldInitializer<'a> {
    pub field: &'a str,
    pub value: Expr<'a>,
    pub span: Span,
}
/// Chamada explícita da superclasse: `super(...)` ou `super.nome(...)`.
///
/// `name` ausente designa o construtor sem nome da base. Os argumentos são
/// avaliados depois de toda a lista de inicialização da classe derivada,
/// conforme a ordem observada no Dart 3.6.2.
#[derive(Debug, Clone)]
pub struct SuperCall<'a> {
    pub name: Option<&'a str>,
    pub arguments: Vec<Expr<'a>>,
    pub span: Span,
}
/// Partes de um construtor que o caminho comum não paga.
///
/// Ficam fora de [`Constructor`] porque o expansor de macros constrói aquela
/// estrutura por literal; manter a forma antiga intacta evita quebrar os
/// geradores existentes. Um construtor sem `const`, sem lista de inicialização
/// e sem `super` explícito não aloca nada aqui.
#[derive(Debug, Clone, Default)]
pub struct ConstructorExtras<'a> {
    pub is_const: bool,
    pub initializers: Vec<FieldInitializer<'a>>,
    pub super_call: Option<SuperCall<'a>>,
}
impl ConstructorExtras<'_> {
    /// Indica se o construtor dispensa qualquer tratamento além do comum.
    #[must_use]
    pub const fn is_plain(&self) -> bool {
        !self.is_const && self.initializers.is_empty() && self.super_call.is_none()
    }
}
/// Construtor generativo nomeado `C.nome(...)`.
#[derive(Debug, Clone)]
pub struct NamedConstructor<'a> {
    pub name: &'a str,
    pub constructor: Constructor<'a>,
    pub extras: ConstructorExtras<'a>,
}
/// Membro estático de uma classe ou variável de topo de uma biblioteca.
///
/// Não participa de herança nem de despacho dinâmico: a resolução é sempre
/// estática pelo nome da declaração que o contém.
#[derive(Debug, Clone)]
pub struct StaticField<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub is_final: bool,
    pub is_const: bool,
    pub initializer: Option<Expr<'a>>,
    pub span: Span,
}
/// Parâmetro comum ou inicializador this.campo; field referencia somente campo próprio.
/// Inicializadores this.campo não introduzem variáveis locais no corpo do construtor.
///
/// Um initializing formal nomeado pode ter nome privado (`this._x`): o campo
/// continua `_x` e o rótulo da chamada é `x`, conforme Dart 3.12.
#[derive(Debug, Clone)]
pub struct ConstructorParameter<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub field: Option<&'a str>,
    pub kind: ParameterKind,
    pub default: Option<Box<Expr<'a>>>,
    pub span: Span,
}
impl<'a> ConstructorParameter<'a> {
    /// Cria um parâmetro posicional obrigatório do construtor.
    ///
    /// # Exemplos
    /// ```
    /// use dartforge_syntax::{ConstructorParameter, ParameterKind, Type};
    /// use dartforge_diagnostics::Span;
    /// let p = ConstructorParameter::required("x", Type::Int, Some("x"), Span { start: 0, end: 1 });
    /// assert_eq!(p.kind, ParameterKind::RequiredPositional);
    /// ```
    #[must_use]
    pub const fn required(name: &'a str, ty: Type, field: Option<&'a str>, span: Span) -> Self {
        Self {
            name,
            ty,
            field,
            kind: ParameterKind::RequiredPositional,
            default: None,
            span,
        }
    }
    /// Rótulo usado na chamada; nomeados privados perdem o sublinhado inicial.
    #[must_use]
    pub fn label(&self) -> &'a str {
        if self.kind.is_named() {
            external_label(self.name)
        } else {
            self.name
        }
    }
}

/// Extension nomeada com métodos de instância resolvidos estaticamente.
#[derive(Debug, Clone)]
pub struct Extension<'a> {
    pub id: u32,
    pub name: &'a str,
    pub on_type: Type,
    pub methods: Vec<Function<'a>>,
    pub span: Span,
}

/// Destino estático de uma chamada de método de extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionTarget {
    pub extension_id: u32,
    pub method_index: usize,
}

/// Resoluções semânticas indexadas pelo intervalo completo de cada chamada.
///
/// As chaves são (início, fim) em bytes na unidade analisada. Um linker futuro
/// deverá usar intervalos virtuais únicos ao combinar bibliotecas com extensions.
#[derive(Debug, Default)]
pub struct Resolution {
    /// Chamadas resolvidas aos intrínsecos de agendamento de dart:async.
    pub async_builtins: std::collections::BTreeSet<(usize, usize)>,
    /// Argumentos reificados explícitos ou inferidos por chamada genérica.
    pub generic_arguments: std::collections::BTreeMap<(usize, usize), Vec<Type>>,
    /// Valores constantes validados, incluindo listas canônicas e argumentos de enum.
    pub constant_values: std::collections::BTreeMap<(usize, usize), ConstValue>,
    /// Identificadores, chamadas e atribuições que usam receptor this implícito.
    pub implicit_members: std::collections::BTreeSet<(usize, usize)>,
    /// Leituras e escritas resolvidas para uma variável de topo da biblioteca.
    ///
    /// O emissor não decide pelo nome isolado: um local homônimo tem
    /// precedência e nunca aparece aqui.
    pub global_accesses: std::collections::BTreeSet<(usize, usize)>,
    /// Leituras de getters resolvidas estaticamente.
    pub getter_accesses: std::collections::BTreeSet<(usize, usize)>,
    /// Comparações `==`/`!=` que precisam do despacho de `operator ==`.
    ///
    /// Vazio em todo programa que não declara operador algum, e é isso que
    /// mantém a igualdade de escalares como `===` do JavaScript: só as
    /// comparações registradas aqui pagam a chamada de runtime.
    pub equality_operators: std::collections::BTreeSet<(usize, usize)>,
    /// Formas originais seguidas das formas inferidas durante a análise.
    pub types: Vec<TypeShape>,
    /// Tipo resolvido de cada expressão, inclusive closures e tear-offs.
    pub expr_types: std::collections::BTreeMap<(usize, usize), Type>,
    pub extension_calls: std::collections::BTreeMap<(usize, usize), ExtensionTarget>,
}

/// Valor constante portável do subconjunto; a identidade nominal de enum é preservada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Int(i32),
    /// Bits IEEE-754 de um literal double (`f64::to_bits`); `u64` preserva
    /// `Eq`/`Hash` da canonicalização const sem distinguir `-0.0` de `0.0`
    /// além dos próprios bits.
    Double(u64),
    Bool(bool),
    String(String),
    Null,
    Enum {
        class_id: u32,
        name: String,
    },
    List {
        element_type: Type,
        values: Vec<ConstValue>,
    },
    /// Instância canônica de um construtor `const`.
    ///
    /// Os campos ficam na ordem de declaração da classe, que é estável e
    /// determina a chave de canonicalização: dois `const C(1)` produzem a mesma
    /// chave e, por consequência, a mesma identidade no JavaScript emitido.
    Instance {
        class_id: u32,
        fields: Vec<(String, ConstValue)>,
    },
}
