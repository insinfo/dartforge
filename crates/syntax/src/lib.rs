//! Tokens e árvore sintática do subconjunto suportado de Dart 3.6.2.
use dartforge_diagnostics::Span;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Categoria de token que empresta lexemas do texto original.
pub enum TokenKind<'a> {
    Word(&'a str),
    String(&'a str),
    /// Conteúdo de uma string raw, sem interpretação de escapes.
    RawString(&'a str),
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
    String,
    Bool,
}
/// Forma estrutural compartilhada sem retirar Copy dos tipos da AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeShape {
    /// Nulabilidade estrutural normalizada durante a substituição genérica.
    Nullable(Type),
    List(Type),
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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Operação prefixa ou asserção pós-fixa com um único operando.
pub enum UnaryOp {
    Negate,
    Not,
    /// Asserção pós-fixa de valor não nulo.
    NullAssert,
}
#[derive(Debug, Clone)]
/// Expressão acompanhada do intervalo de origem.
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}
#[derive(Debug, Clone)]
/// Forma sintática de uma expressão.
pub enum ExprKind<'a> {
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
    Null,
    Int(i32),
    String(&'a str),
    /// String que precisou de alocação para decodificar escapes Unicode.
    OwnedString(String),
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
#[derive(Debug, Clone)]
/// Instrução acompanhada do intervalo de origem.
pub struct Statement<'a> {
    pub kind: StatementKind<'a>,
    pub span: Span,
}
#[derive(Debug, Clone)]
/// Forma sintática de uma instrução.
pub enum StatementKind<'a> {
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
    /// Encerra o laço mais próximo; rótulos ainda não são suportados.
    Break,
    /// Inicia a próxima iteração do laço mais próximo.
    Continue,
    Block(Vec<Statement<'a>>),
}
#[derive(Debug, Clone)]
/// Parâmetro posicional obrigatório com tipo explícito.
pub struct Parameter<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub span: Span,
}
#[derive(Debug, Clone)]
/// Função top-level com assinatura e corpo.
pub struct Function<'a> {
    pub annotations: Vec<Annotation>,
    pub native_binding: Option<NativeBinding<'a>>,
    pub type_parameters: Vec<GenericParameter<'a>>,
    pub is_getter: bool,
    pub name: &'a str,
    pub return_type: Type,
    pub parameters: Vec<Parameter<'a>>,
    pub body: Vec<Statement<'a>>,
    pub span: Span,
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
    Override,
    Deprecated { message: Option<String> },
}
#[derive(Debug, Clone)]
/// Programa com funções auxiliares e o corpo da entrada main.
pub struct Program<'a> {
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
    pub constructor: Option<Constructor<'a>>,
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
/// Parâmetro comum ou inicializador this.campo; field referencia somente campo próprio.
/// Inicializadores this.campo não introduzem variáveis locais no corpo do construtor.
#[derive(Debug, Clone)]
pub struct ConstructorParameter<'a> {
    pub name: &'a str,
    pub ty: Type,
    pub field: Option<&'a str>,
    pub span: Span,
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
    /// Argumentos reificados explícitos ou inferidos por chamada genérica.
    pub generic_arguments: std::collections::BTreeMap<(usize, usize), Vec<Type>>,
    /// Valores constantes validados, incluindo listas canônicas e argumentos de enum.
    pub constant_values: std::collections::BTreeMap<(usize, usize), ConstValue>,
    /// Identificadores, chamadas e atribuições que usam receptor this implícito.
    pub implicit_members: std::collections::BTreeSet<(usize, usize)>,
    /// Leituras de getters resolvidas estaticamente.
    pub getter_accesses: std::collections::BTreeSet<(usize, usize)>,
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
}
