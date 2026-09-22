//! Árvore sintática completa de Dart 3.6, em arenas indexadas.
//!
//! Nenhum nó empresta da fonte: nomes são [`SymbolId`], conteúdo de literais
//! fica como [`Span`] para decodificação posterior, e filhos são índices em
//! [`Ast`]. Os nomes dos tipos seguem os não-terminais da especificação da
//! linguagem (§ da gramática de Dart 3.6) para que a correspondência com a
//! gramática seja verificável.
//!
//! A árvore é **puramente sintática**: `dynamic`, `Function`, `Never` e `Object`
//! são [`TypeKind::Named`] como qualquer outro nome; `int.parse` é um
//! [`ExprKind::Property`] sobre um identificador. Decidir o que cada nome é
//! pertence à resolução.
use crate::text::{DartStr, DartStrBuilder};
use dartforge_diagnostics::Span;
use dartforge_intern::SymbolId;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u32);
    };
}

id!(/// Índice em [`Ast::exprs`].
    ExprId);
id!(/// Índice em [`Ast::stmts`].
    StmtId);
id!(/// Índice em [`Ast::types`].
    TypeId);
id!(/// Índice em [`Ast::patterns`].
    PatternId);
id!(/// Índice em [`Ast::decls`].
    DeclId);
id!(/// Índice em [`Ast::members`].
    MemberId);
id!(/// Índice em [`Ast::functions`] (assinatura + corpo compartilhados por
    /// funções de topo, métodos, funções locais e expressões de função).
    FunctionId);

/// Arenas de uma unidade de compilação.
#[derive(Debug, Default)]
pub struct Ast {
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub types: Vec<TypeAnnotation>,
    pub patterns: Vec<Pattern>,
    pub decls: Vec<Decl>,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

impl Ast {
    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }
    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.0 as usize]
    }
    pub fn ty(&self, id: TypeId) -> &TypeAnnotation {
        &self.types[id.0 as usize]
    }
    pub fn pattern(&self, id: PatternId) -> &Pattern {
        &self.patterns[id.0 as usize]
    }
    pub fn decl(&self, id: DeclId) -> &Decl {
        &self.decls[id.0 as usize]
    }
    pub fn member(&self, id: MemberId) -> &Member {
        &self.members[id.0 as usize]
    }
    pub fn function(&self, id: FunctionId) -> &Function {
        &self.functions[id.0 as usize]
    }
    pub fn push_expr(&mut self, expr: Expr) -> ExprId {
        self.exprs.push(expr);
        ExprId(self.exprs.len() as u32 - 1)
    }
    pub fn push_stmt(&mut self, stmt: Stmt) -> StmtId {
        self.stmts.push(stmt);
        StmtId(self.stmts.len() as u32 - 1)
    }
    pub fn push_type(&mut self, ty: TypeAnnotation) -> TypeId {
        self.types.push(ty);
        TypeId(self.types.len() as u32 - 1)
    }
    pub fn push_pattern(&mut self, pattern: Pattern) -> PatternId {
        self.patterns.push(pattern);
        PatternId(self.patterns.len() as u32 - 1)
    }
    pub fn push_decl(&mut self, decl: Decl) -> DeclId {
        self.decls.push(decl);
        DeclId(self.decls.len() as u32 - 1)
    }
    pub fn push_member(&mut self, member: Member) -> MemberId {
        self.members.push(member);
        MemberId(self.members.len() as u32 - 1)
    }
    pub fn push_function(&mut self, function: Function) -> FunctionId {
        self.functions.push(function);
        FunctionId(self.functions.len() as u32 - 1)
    }
}

/// Identificador com posição.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Name {
    pub sym: SymbolId,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Unidade e diretivas
// ---------------------------------------------------------------------------

/// `libraryDefinition` ou `partDeclaration`: um arquivo `.dart`.
#[derive(Debug, Default)]
pub struct CompilationUnit {
    pub script_tag: Option<Span>,
    pub directives: Vec<Directive>,
    pub declarations: Vec<DeclId>,
}

/// Anotação `@nome`, `@nome.membro`, `@p.Nome<T>.ctor(args)`.
#[derive(Debug)]
pub struct Annotation {
    pub span: Span,
    /// `nome`, `p.nome` ou `p.Nome.ctor` — até três partes, na ordem escrita.
    pub name: Vec<Name>,
    pub type_args: Vec<TypeId>,
    pub arguments: Option<Arguments>,
}

#[derive(Debug)]
pub struct Directive {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub kind: DirectiveKind,
}

#[derive(Debug)]
pub enum DirectiveKind {
    /// `library;` ou `library a.b.c;`
    Library { name: Vec<Name> },
    /// `import 'uri' if (cond) 'uri' deferred as p show a hide b;`
    Import {
        uri: StringLit,
        configurations: Vec<Configuration>,
        deferred: bool,
        prefix: Option<Name>,
        combinators: Vec<Combinator>,
    },
    /// `export 'uri' ... show a hide b;`
    Export {
        uri: StringLit,
        configurations: Vec<Configuration>,
        combinators: Vec<Combinator>,
    },
    /// `part 'uri';`
    Part { uri: StringLit },
    /// `part of 'uri';` ou `part of a.b.c;`
    PartOf {
        uri: Option<StringLit>,
        name: Vec<Name>,
    },
}

/// `if (dart.library.io == 'x') 'uri'`
#[derive(Debug)]
pub struct Configuration {
    pub span: Span,
    pub test: Vec<Name>,
    pub value: Option<StringLit>,
    pub uri: StringLit,
}

#[derive(Debug)]
pub enum Combinator {
    Show(Vec<Name>),
    Hide(Vec<Name>),
}

// ---------------------------------------------------------------------------
// Declarações de topo e membros
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Decl {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub kind: DeclKind,
}

#[derive(Debug)]
pub enum DeclKind {
    Class(ClassDecl),
    Mixin(MixinDecl),
    Enum(EnumDecl),
    Extension(ExtensionDecl),
    ExtensionType(ExtensionTypeDecl),
    Typedef(TypedefDecl),
    /// Função, getter ou setter de topo.
    Function(FunctionId),
    /// `int a = 1, b;` de topo (uma declaração por lista).
    Variables(VariableList),
}

/// Modificadores de classe (Dart 3).
#[derive(Debug, Default, Clone, Copy)]
pub struct ClassModifiers {
    pub abstract_: bool,
    pub base: bool,
    pub interface: bool,
    pub final_: bool,
    pub sealed: bool,
    /// `mixin class`
    pub mixin: bool,
}

#[derive(Debug)]
pub struct ClassDecl {
    pub modifiers: ClassModifiers,
    pub name: Name,
    pub type_params: Vec<TypeParameter>,
    pub extends: Option<TypeId>,
    pub with: Vec<TypeId>,
    pub implements: Vec<TypeId>,
    /// `class C = S with M implements I;` — sem corpo.
    pub mixin_application: bool,
    pub members: Vec<MemberId>,
}

#[derive(Debug)]
pub struct MixinDecl {
    pub base: bool,
    pub name: Name,
    pub type_params: Vec<TypeParameter>,
    pub on: Vec<TypeId>,
    pub implements: Vec<TypeId>,
    pub members: Vec<MemberId>,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub name: Name,
    pub type_params: Vec<TypeParameter>,
    pub with: Vec<TypeId>,
    pub implements: Vec<TypeId>,
    pub constants: Vec<EnumConstant>,
    pub members: Vec<MemberId>,
}

/// `@meta nome<T>.ctor(args)` dentro de um `enum`.
#[derive(Debug)]
pub struct EnumConstant {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub name: Name,
    pub type_args: Vec<TypeId>,
    pub constructor: Option<Name>,
    pub arguments: Option<Arguments>,
}

#[derive(Debug)]
pub struct ExtensionDecl {
    pub name: Option<Name>,
    pub type_params: Vec<TypeParameter>,
    pub on: TypeId,
    pub members: Vec<MemberId>,
}

#[derive(Debug)]
pub struct ExtensionTypeDecl {
    pub const_: bool,
    pub name: Name,
    pub type_params: Vec<TypeParameter>,
    /// Nome do construtor primário: `extension type E.name(int x)`.
    pub constructor: Option<Name>,
    pub representation_metadata: Vec<Annotation>,
    pub representation_type: TypeId,
    pub representation_name: Name,
    pub implements: Vec<TypeId>,
    pub members: Vec<MemberId>,
}

#[derive(Debug)]
pub struct TypedefDecl {
    pub name: Name,
    pub type_params: Vec<TypeParameter>,
    pub kind: TypedefKind,
}

#[derive(Debug)]
pub enum TypedefKind {
    /// `typedef F<T> = int Function(T);` — qualquer tipo.
    Alias(TypeId),
    /// `typedef int F<T>(T x);` — forma antiga, só tipo de função.
    Legacy {
        return_type: Option<TypeId>,
        parameters: Vec<Parameter>,
    },
}

/// `T extends Bound` numa lista `<...>`, com metadata.
#[derive(Debug)]
pub struct TypeParameter {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub name: Name,
    pub bound: Option<TypeId>,
}

/// Lista `tipo a = 1, b;` com os modificadores comuns.
#[derive(Debug)]
pub struct VariableList {
    pub external: bool,
    pub static_: bool,
    pub abstract_: bool,
    pub covariant: bool,
    pub late: bool,
    pub final_: bool,
    pub const_: bool,
    /// `var` foi escrito (então `ty` é `None`).
    pub var_: bool,
    pub ty: Option<TypeId>,
    pub variables: Vec<Variable>,
}

#[derive(Debug)]
pub struct Variable {
    pub name: Name,
    pub initializer: Option<ExprId>,
}

#[derive(Debug)]
pub struct Member {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub kind: MemberKind,
}

#[derive(Debug)]
pub enum MemberKind {
    Field(VariableList),
    /// Método, getter, setter ou operador; `static`/`abstract` na função.
    Method(FunctionId),
    Constructor(Constructor),
}

#[derive(Debug)]
pub struct Constructor {
    pub external: bool,
    pub const_: bool,
    pub factory: bool,
    /// Nome da classe como escrito (`C` em `C.named`).
    pub class_name: Name,
    /// `named` em `C.named`; `None` para o construtor sem nome.
    pub name: Option<Name>,
    pub parameters: Vec<Parameter>,
    pub initializers: Vec<Initializer>,
    /// `= Outra<T>.ctor` (factory redirecionadora) ou `: this.x()` fica em
    /// `initializers` como [`Initializer::Redirect`].
    pub redirect: Option<RedirectTarget>,
    pub body: FunctionBody,
}

/// Alvo de `factory C() = Outra.nome;`
#[derive(Debug)]
pub struct RedirectTarget {
    pub span: Span,
    pub ty: TypeId,
    pub constructor: Option<Name>,
}

#[derive(Debug)]
pub enum Initializer {
    /// `this.x = e` ou `x = e`.
    Field {
        span: Span,
        this_: bool,
        name: Name,
        value: ExprId,
    },
    /// `super(args)` ou `super.nome(args)`.
    Super {
        span: Span,
        constructor: Option<Name>,
        arguments: Arguments,
    },
    /// `this(args)` ou `this.nome(args)` — construtor redirecionador.
    Redirect {
        span: Span,
        constructor: Option<Name>,
        arguments: Arguments,
    },
    /// `assert(cond, msg)`.
    Assert {
        span: Span,
        condition: ExprId,
        message: Option<ExprId>,
    },
}

// ---------------------------------------------------------------------------
// Funções
// ---------------------------------------------------------------------------

/// Assinatura e corpo compartilhados por todas as formas de função.
#[derive(Debug)]
pub struct Function {
    pub span: Span,
    pub external: bool,
    pub static_: bool,
    /// Declarado explicitamente `abstract` (só campos) ou sem corpo num tipo.
    pub kind: FunctionKind,
    /// `None` em expressões de função e em setters sem tipo.
    pub return_type: Option<TypeId>,
    /// `None` em expressões de função.
    pub name: Option<Name>,
    pub type_params: Vec<TypeParameter>,
    /// Getters não têm lista (`None`); setters e demais têm.
    pub parameters: Option<Vec<Parameter>>,
    pub modifier: AsyncModifier,
    pub body: FunctionBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    Function,
    Getter,
    Setter,
    /// `operator +`; o nome guarda o texto do operador (`[]=`, `unary-`…).
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsyncModifier {
    #[default]
    None,
    Async,
    AsyncStar,
    SyncStar,
}

#[derive(Debug)]
pub enum FunctionBody {
    /// `{ ... }`
    Block(StmtId),
    /// `=> e`
    Expression(ExprId),
    /// `;` — abstrato, externo ou redirecionador.
    Empty,
    /// `native 'nome';` ou `native;` (SDK).
    Native(Option<StringLit>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    /// Posicional obrigatório.
    Required,
    /// Entre `[ ]`.
    Optional,
    /// Entre `{ }`.
    Named,
}

#[derive(Debug)]
pub struct Parameter {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub kind: ParameterKind,
    /// `required` (só em nomeados).
    pub required: bool,
    pub covariant: bool,
    pub final_: bool,
    pub var_: bool,
    pub const_: bool,
    pub ty: Option<TypeId>,
    /// `this.x`
    pub this_: bool,
    /// `super.x`
    pub super_: bool,
    /// `None` só em parâmetros de tipo de função sem nome: `int Function(int)`.
    pub name: Option<Name>,
    /// Forma antiga `int f(int x)`: parâmetros de tipo e lista do parâmetro-função.
    pub function_type_params: Vec<TypeParameter>,
    pub function_parameters: Option<Vec<Parameter>>,
    pub default_value: Option<ExprId>,
}

// ---------------------------------------------------------------------------
// Tipos
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TypeAnnotation {
    pub span: Span,
    /// `?` final.
    pub nullable: bool,
    pub kind: TypeKind,
}

#[derive(Debug)]
pub enum TypeKind {
    /// `nome`, `p.nome`, com argumentos: inclui `dynamic`, `void` **não**,
    /// `Function` cru, `Never`, `Object`.
    Named {
        /// Uma ou duas partes (`p.Nome`).
        name: Vec<Name>,
        args: Vec<TypeId>,
    },
    /// `void`
    Void,
    /// `R Function<T>(P, {n})`
    Function {
        return_type: Option<TypeId>,
        type_params: Vec<TypeParameter>,
        parameters: Vec<Parameter>,
    },
    /// `(int, {String nome})`
    Record {
        positional: Vec<TypeId>,
        named: Vec<(Name, TypeId)>,
    },
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Debug)]
pub enum StmtKind {
    Block(Vec<StmtId>),
    /// `late final int x = 1, y;` local.
    Variables(VariableList),
    /// `var (a, b) = e;` / `final [x, y] = e;`
    PatternVariables {
        final_: bool,
        pattern: PatternId,
        value: ExprId,
    },
    /// Função local (nomeada).
    Function(FunctionId),
    Expression(ExprId),
    If {
        condition: ExprId,
        /// `if (e case p when g)`
        case_pattern: Option<PatternId>,
        guard: Option<ExprId>,
        then: StmtId,
        else_: Option<StmtId>,
    },
    For {
        await_: bool,
        init: Option<ForInit>,
        condition: Option<ExprId>,
        updates: Vec<ExprId>,
        body: StmtId,
    },
    ForIn {
        await_: bool,
        /// `for (final x in e)` / `for (var (a, b) in e)` / `for (x in e)`.
        target: ForInTarget,
        iterable: ExprId,
        body: StmtId,
    },
    While {
        condition: ExprId,
        body: StmtId,
    },
    DoWhile {
        body: StmtId,
        condition: ExprId,
    },
    Switch {
        value: ExprId,
        cases: Vec<SwitchCase>,
    },
    Break(Option<Name>),
    Continue(Option<Name>),
    Return(Option<ExprId>),
    Yield {
        star: bool,
        value: ExprId,
    },
    Try {
        body: StmtId,
        catches: Vec<CatchClause>,
        finally_: Option<StmtId>,
    },
    Labeled {
        labels: Vec<Name>,
        body: StmtId,
    },
    Assert {
        condition: ExprId,
        message: Option<ExprId>,
    },
    Empty,
}

#[derive(Debug)]
pub enum ForInit {
    Variables(VariableList),
    Expression(ExprId),
}

#[derive(Debug)]
pub enum ForInTarget {
    /// `for (int x in e)` / `for (var x in e)` / `for (final x in e)`.
    Declared {
        metadata: Vec<Annotation>,
        final_: bool,
        var_: bool,
        ty: Option<TypeId>,
        name: Name,
    },
    /// `for (final (a, b) in e)` / `for (var [x] in e)`.
    Pattern { final_: bool, pattern: PatternId },
    /// `for (x in e)` — variável existente.
    Expression(ExprId),
}

#[derive(Debug)]
pub struct SwitchCase {
    pub span: Span,
    pub labels: Vec<Name>,
    /// `case p when g:`; `None` para `default:`.
    pub pattern: Option<PatternId>,
    pub guard: Option<ExprId>,
    pub body: Vec<StmtId>,
}

#[derive(Debug)]
pub struct CatchClause {
    pub span: Span,
    /// `on T`
    pub on_type: Option<TypeId>,
    /// `catch (e, st)`
    pub exception: Option<Name>,
    pub stack_trace: Option<Name>,
    pub body: StmtId,
}

// ---------------------------------------------------------------------------
// Expressões
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

/// Literal de string, possivelmente adjacente e interpolado.
///
/// Cada elemento de `parts` é um literal sintático (`'a' 'b'` produz dois);
/// cada literal é uma sequência de trechos e interpolações.
#[derive(Debug, Clone)]
pub struct StringLit {
    pub span: Span,
    pub parts: Vec<StringPart>,
}

#[derive(Debug, Clone)]
pub enum StringPart {
    /// Trecho literal, **já decodificado** (escapes resolvidos); `raw` vem sem
    /// alteração. [`DartStr`] e não `str` porque `'\uD800'` é literal válido.
    Text(DartStr),
    /// `$x` ou `${e}`.
    Interpolation(ExprId),
}

impl StringLit {
    /// Valor quando a string não tem interpolação. Literais adjacentes são
    /// concatenados por unidades UTF-16, então `'\uD83D' '\uDC6D'` forma o par.
    pub fn constant_value(&self) -> Option<DartStr> {
        let mut out = DartStrBuilder::default();
        for part in &self.parts {
            match part {
                StringPart::Text(text) => {
                    for unit in text.code_units() {
                        out.push_unit(unit);
                    }
                }
                StringPart::Interpolation(_) => return None,
            }
        }
        Some(out.finish())
    }
}

#[derive(Debug)]
pub struct Arguments {
    pub span: Span,
    pub type_args: Vec<TypeId>,
    pub args: Vec<Argument>,
}

#[derive(Debug)]
pub struct Argument {
    pub name: Option<Name>,
    pub value: ExprId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    TruncDiv,
    Rem,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    IfNull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    PrefixInc,
    PrefixDec,
    PostfixInc,
    PostfixDec,
    /// `e!`
    NullAssert,
}

/// Operador composto de atribuição; `None` é `=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Compound(BinaryOp),
}

#[derive(Debug)]
pub enum ExprKind {
    Int(Span),
    Double(Span),
    Bool(bool),
    Null,
    String(StringLit),
    /// `#nome`, `#a.b.c`, `#+`, `#void`.
    Symbol(Vec<Name>),
    Identifier(Name),
    This,
    /// `super` só aparece como alvo de `.`, `[]`, operador ou chamada.
    Super,
    /// `(e)`
    Parenthesized(ExprId),
    /// `<T>[a, b, ...c, if (x) d, for (...) e]`
    List {
        const_: bool,
        type_args: Vec<TypeId>,
        elements: Vec<CollectionElement>,
    },
    /// `<K, V>{k: v}` ou `<T>{a, b}`; sem argumentos de tipo a forma é
    /// decidida pelos elementos (`{}` vazio é mapa).
    SetOrMap {
        const_: bool,
        type_args: Vec<TypeId>,
        elements: Vec<CollectionElement>,
    },
    /// `(a, b, nome: c)`
    Record {
        const_: bool,
        positional: Vec<ExprId>,
        named: Vec<(Name, ExprId)>,
    },
    /// `new T<A>.nome(args)` / `const T(args)`; `new`/`const` explícitos.
    InstanceCreation {
        keyword: Option<CreationKeyword>,
        ty: TypeId,
        constructor: Option<Name>,
        arguments: Arguments,
    },
    /// `(params) { }`, `(params) => e`, `<T>(params) async { }`
    FunctionExpression(FunctionId),
    Property {
        target: ExprId,
        name: Name,
        null_aware: bool,
    },
    /// `e[i]` / `e?[i]`
    Index {
        target: ExprId,
        index: ExprId,
        null_aware: bool,
    },
    /// `f(args)`, `e.m<T>(args)`, `e?.m(args)` — o alvo é a expressão da
    /// função (um `Property` para chamadas de método).
    Call {
        target: ExprId,
        arguments: Arguments,
    },
    /// `e<T>` — instanciação explícita de tearoff ou tipo genérico.
    TypeArguments {
        target: ExprId,
        type_args: Vec<TypeId>,
    },
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },
    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },
    Conditional {
        condition: ExprId,
        then: ExprId,
        else_: ExprId,
    },
    /// `e is T` / `e is! T`
    Is {
        value: ExprId,
        ty: TypeId,
        negated: bool,
    },
    /// `e as T`
    As {
        value: ExprId,
        ty: TypeId,
    },
    Assign {
        op: AssignOp,
        target: ExprId,
        value: ExprId,
    },
    /// `(a, b) = e` / `[x, y] = e` — atribuição por padrão.
    PatternAssign {
        pattern: PatternId,
        value: ExprId,
    },
    /// `e..a = 1..b()` : cada seção é uma expressão cujo alvo mais interno é
    /// o marcador [`ExprKind::CascadeTarget`]. `null_aware` é `?..` na
    /// primeira seção.
    Cascade {
        target: ExprId,
        sections: Vec<ExprId>,
        null_aware: bool,
    },
    /// Receptor implícito de uma seção de cascata (`..a` → `CascadeTarget.a`).
    CascadeTarget,
    Await(ExprId),
    Throw(ExprId),
    Rethrow,
    /// `switch (e) { p when g => v, _ => w }`
    Switch {
        value: ExprId,
        cases: Vec<SwitchExprCase>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationKeyword {
    New,
    Const,
}

#[derive(Debug)]
pub struct SwitchExprCase {
    pub span: Span,
    pub pattern: PatternId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
}

/// Elemento de literal de lista/conjunto/mapa.
#[derive(Debug)]
pub enum CollectionElement {
    Expression(ExprId),
    /// `?e` (elementos null-aware, Dart 3.8): entra só quando não é nulo.
    NullAwareExpression(ExprId),
    /// `k: v`, `?k: v`, `k: ?v`, `?k: ?v` — os `?` são null-aware (Dart 3.8).
    MapEntry {
        key: ExprId,
        value: ExprId,
        null_aware_key: bool,
        null_aware_value: bool,
    },
    /// `...e` / `...?e`
    Spread {
        value: ExprId,
        null_aware: bool,
    },
    /// `if (c) a else b` / `if (e case p when g) a`
    If {
        condition: ExprId,
        case_pattern: Option<PatternId>,
        guard: Option<ExprId>,
        then: Box<CollectionElement>,
        else_: Option<Box<CollectionElement>>,
    },
    /// `for (...) e`
    For {
        await_: bool,
        init: Option<ForInit>,
        condition: Option<ExprId>,
        updates: Vec<ExprId>,
        body: Box<CollectionElement>,
    },
    ForIn {
        await_: bool,
        target: ForInTarget,
        iterable: ExprId,
        body: Box<CollectionElement>,
    },
}

// ---------------------------------------------------------------------------
// Padrões
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Debug)]
pub enum PatternKind {
    /// `_` ou `int _`
    Wildcard {
        ty: Option<TypeId>,
    },
    /// `var x`, `final int x`, ou `x` solto dentro de um padrão de declaração.
    Variable {
        final_: bool,
        var_: bool,
        ty: Option<TypeId>,
        name: Name,
    },
    /// Constante: literal, `const C()`, `-1`, identificador qualificado.
    Constant(ExprId),
    /// `== e`, `< e`, …
    Relational {
        op: BinaryOp,
        value: ExprId,
    },
    Or(PatternId, PatternId),
    And(PatternId, PatternId),
    /// `p?`
    NullCheck(PatternId),
    /// `p!`
    NullAssert(PatternId),
    /// `p as T`
    Cast {
        pattern: PatternId,
        ty: TypeId,
    },
    Parenthesized(PatternId),
    /// `<T>[a, b, ...rest]`
    List {
        type_args: Vec<TypeId>,
        elements: Vec<ListPatternElement>,
    },
    /// `<K, V>{k: p, ...}`
    Map {
        type_args: Vec<TypeId>,
        entries: Vec<MapPatternEntry>,
        rest: bool,
    },
    /// `(a, nome: p, :x)`
    Record {
        fields: Vec<PatternField>,
    },
    /// `Nome<T>(campo: p, :x)`
    Object {
        ty: TypeId,
        fields: Vec<PatternField>,
    },
}

#[derive(Debug)]
pub enum ListPatternElement {
    Pattern(PatternId),
    /// `...` ou `...p`
    Rest(Option<PatternId>),
}

#[derive(Debug)]
pub struct MapPatternEntry {
    pub key: ExprId,
    pub value: PatternId,
}

/// Campo de padrão de record/objeto. `name` ausente é posicional; `:x` é um
/// campo com nome inferido do padrão de variável.
#[derive(Debug)]
pub struct PatternField {
    pub span: Span,
    pub name: Option<Name>,
    pub pattern: PatternId,
}
