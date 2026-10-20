//! Complete Dart 3.6 syntax tree, in indexed arenas.
//!
//! No node borrows from the source: names are [`SymbolId`], literal content
//! stays as a [`Span`] for later decoding, and children are indices into
//! [`Ast`]. Type names follow the non-terminals of the language
//! specification (§ of the Dart 3.6 grammar) so that the correspondence with
//! the grammar can be verified.
//!
//! The tree is **purely syntactic**: `dynamic`, `Function`, `Never` and
//! `Object` are [`TypeKind::Named`] like any other name; `int.parse` is an
//! [`ExprKind::Property`] on an identifier. Deciding what each name is
//! belongs to resolution.
use crate::text::{DartStr, DartStrBuilder};
use dartforge_diagnostics::Span;
use dartforge_intern::SymbolId;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        pub struct $name(pub u32);
    };
}

id!(/// Index into [`Ast::exprs`].
    ExprId);
id!(/// Index into [`Ast::stmts`].
    StmtId);
id!(/// Index into [`Ast::types`].
    TypeId);
id!(/// Index into [`Ast::patterns`].
    PatternId);
id!(/// Index into [`Ast::decls`].
    DeclId);
id!(/// Index into [`Ast::members`].
    MemberId);
id!(/// Index into [`Ast::functions`] (signature + body shared by
    /// top-level functions, methods, local functions and function expressions).
    FunctionId);

/// Arenas of a compilation unit.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Ast {
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub types: Vec<TypeAnnotation>,
    pub patterns: Vec<Pattern>,
    pub decls: Vec<Decl>,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
    /// The metadata of annotated local declarations (`@pragma('x') final a =
    /// 1;`, an annotated local function), by statement: the statement nodes
    /// have no field for it (the analyzer's
    /// `VariableDeclarationStatement.variables.metadata` and
    /// `FunctionDeclaration.metadata`).
    #[serde(default)]
    pub local_metadata: Vec<(StmtId, Box<[Annotation]>)>,
    /// The `assert(…)` calls read as an expression
    /// (`parseAssert(Assert.Expression)`): `AstBuilder.endAssert` builds them as
    /// a `FunctionExpressionInvocation` of a `SimpleIdentifier` `assert`, not as
    /// a `MethodInvocation`.
    #[serde(default)]
    pub assert_invocations: Vec<ExprId>,
    /// The `C<T>.name(…)` and `p.C<T>.name(…)` calls that fasta's
    /// `parseUnaryExpression` reads as an implicit creation
    /// (`parseImplicitCreationExpression`, `parser_impl.dart:6426-6447`): in the
    /// analyzer they are an `InstanceCreationExpression`, which the
    /// `AstRewriter` only undoes when the name is not a type.
    #[serde(default)]
    pub implicit_creations: Vec<ExprId>,
    /// Data derived from the tree by later phases, built once per tree on the
    /// first question (a new parse starts empty): see [`Derived`].
    #[serde(skip)]
    pub derived: Derived,
}

/// A slot for one structure derived from an [`Ast`] (the lexical scopes that
/// `dartforge_types::annotations::is_shadowed` queries): whoever owns the
/// type fills it on first use. The tree is never changed after the parse, so
/// what is derived from it stays valid for its lifetime.
#[derive(Default)]
pub struct Derived(std::sync::OnceLock<Box<dyn std::any::Any + Send + Sync>>);

impl Derived {
    /// The derived value of type `T`, built by `make` on the first call.
    /// Returns `None` when the slot already holds a value of another type.
    pub fn get_or_init<T: std::any::Any + Send + Sync>(
        &self,
        make: impl FnOnce() -> T,
    ) -> Option<&T> {
        self.0.get_or_init(|| Box::new(make())).downcast_ref::<T>()
    }
}

impl std::fmt::Debug for Derived {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() {
            "Derived(filled)"
        } else {
            "Derived(empty)"
        })
    }
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
    /// The dot shorthand (`.id`, Dart 3.10) at the root of the selector chain
    /// that `e` is — `e` itself, or the innermost target of `.x`, `(args)`,
    /// `[i]`, `<T>` and `!`. The context type of the whole chain is its
    /// *shorthand context* (spec, "Type inference").
    pub fn shorthand_root(&self, e: ExprId) -> Option<ExprId> {
        let mut cur = e;
        loop {
            match &self.expr(cur).kind {
                ExprKind::DotShorthand { .. } => return Some(cur),
                ExprKind::Property { target, .. }
                | ExprKind::Call { target, .. }
                | ExprKind::Index { target, .. }
                | ExprKind::TypeArguments { target, .. } => cur = *target,
                ExprKind::Unary {
                    op: UnaryOp::NullAssert,
                    operand,
                } => cur = *operand,
                _ => return None,
            }
        }
    }

    /// Returns each arena's unused capacity to the allocator. Called at the end
    /// of parsing, when the tree stops growing.
    pub fn shrink_to_fit(&mut self) {
        self.exprs.shrink_to_fit();
        self.stmts.shrink_to_fit();
        self.types.shrink_to_fit();
        self.patterns.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.members.shrink_to_fit();
        self.functions.shrink_to_fit();
    }
}

/// Identifier with a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Name {
    pub sym: SymbolId,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Unit and directives
// ---------------------------------------------------------------------------

/// `libraryDefinition` or `partDeclaration`: a `.dart` file.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CompilationUnit {
    pub script_tag: Option<Span>,
    pub directives: Vec<Directive>,
    pub declarations: Vec<DeclId>,
}

/// Annotation `@name`, `@name.member`, `@p.Name<T>.ctor(args)`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Annotation {
    pub span: Span,
    /// `name`, `p.name` or `p.Name.ctor` — up to three parts, in written order.
    pub name: Vec<Name>,
    pub type_args: Vec<TypeId>,
    pub arguments: Option<Arguments>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Directive {
    pub span: Span,
    pub metadata: Vec<Annotation>,
    pub kind: DirectiveKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum DirectiveKind {
    /// `library;` or `library a.b.c;`
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
    /// `part of 'uri';` or `part of a.b.c;`
    PartOf {
        uri: Option<StringLit>,
        name: Vec<Name>,
    },
    /// `import augment 'uri';` — an augmentation library in the SDK 3.6 form
    /// (`macros` experiment, `working/augmentation-libraries` up to v1.20): the
    /// unit pointed to belongs to this library and augments its declarations
    /// (docs/AUGMENTATIONS.md).
    ImportAugment { uri: StringLit },
    /// `augment library 'uri';` — the header of the augmentation unit, the
    /// counterpart of [`DirectiveKind::ImportAugment`] (like `part of` for
    /// `part`).
    AugmentLibrary { uri: StringLit },
}

/// `if (dart.library.io == 'x') 'uri'`
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Configuration {
    pub span: Span,
    pub test: Vec<Name>,
    pub value: Option<StringLit>,
    pub uri: StringLit,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Combinator {
    Show(Vec<Name>),
    Hide(Vec<Name>),
}

// ---------------------------------------------------------------------------
// Top-level declarations and members
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Decl {
    pub span: Span,
    pub metadata: Box<[Annotation]>,
    pub kind: DeclKind,
    /// `augment` (`augmentations`/`macros` experiments): the declaration
    /// augments the declaration of the same name that comes before it in the
    /// library (docs/AUGMENTATIONS.md), instead of introducing a name.
    pub augment: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum DeclKind {
    Class(ClassDecl),
    Mixin(MixinDecl),
    Enum(EnumDecl),
    Extension(ExtensionDecl),
    ExtensionType(ExtensionTypeDecl),
    Typedef(TypedefDecl),
    /// Top-level function, getter or setter.
    Function(FunctionId),
    /// Top-level `int a = 1, b;` (one declaration per list).
    Variables(VariableList),
}

/// Class modifiers (Dart 3).
#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ClassModifiers {
    pub abstract_: bool,
    pub base: bool,
    pub interface: bool,
    pub final_: bool,
    pub sealed: bool,
    /// `mixin class`
    pub mixin: bool,
    /// `macro class` (`macros` experiment): the class is a macro, and an
    /// annotation that resolves to one of its constructors is a macro
    /// application (docs/MACROS-PROTOCOLO.md).
    pub macro_: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClassDecl {
    pub modifiers: ClassModifiers,
    pub name: Name,
    pub type_params: Box<[TypeParameter]>,
    pub extends: Option<TypeId>,
    pub with: Box<[TypeId]>,
    pub implements: Box<[TypeId]>,
    /// `class C = S with M implements I;` — without a body.
    pub mixin_application: bool,
    pub members: Vec<MemberId>,
    /// The primary constructor (Dart 3.13), already elaborated: member `k2` in
    /// `members`. The non-`late` field initializers of this class are evaluated
    /// in the scope of its parameters (spec, "primary initializer scope");
    /// otherwise, it is a regular constructor.
    pub primary_constructor: Option<MemberId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MixinDecl {
    pub base: bool,
    pub name: Name,
    pub type_params: Box<[TypeParameter]>,
    pub on: Box<[TypeId]>,
    pub implements: Box<[TypeId]>,
    pub members: Vec<MemberId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EnumDecl {
    pub name: Name,
    pub type_params: Box<[TypeParameter]>,
    pub with: Box<[TypeId]>,
    pub implements: Box<[TypeId]>,
    pub constants: Vec<EnumConstant>,
    pub members: Vec<MemberId>,
    /// Elaborated primary constructor (Dart 3.13), as in [`ClassDecl`].
    pub primary_constructor: Option<MemberId>,
}

/// `@meta name<T>.ctor(args)` inside an `enum`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EnumConstant {
    pub span: Span,
    pub metadata: Box<[Annotation]>,
    pub name: Name,
    pub type_args: Box<[TypeId]>,
    pub constructor: Option<Name>,
    pub arguments: Option<Arguments>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExtensionDecl {
    pub name: Option<Name>,
    pub type_params: Box<[TypeParameter]>,
    pub on: TypeId,
    pub members: Vec<MemberId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExtensionTypeDecl {
    pub const_: bool,
    pub name: Name,
    pub type_params: Box<[TypeParameter]>,
    /// Name of the primary constructor: `extension type E.name(int x)`.
    pub constructor: Option<Name>,
    pub representation_metadata: Box<[Annotation]>,
    pub representation_type: TypeId,
    pub representation_name: Name,
    /// The whole `( … )` list of the primary constructor. What it has besides
    /// the representation (parameters discarded by recovery, with the defaults)
    /// belongs to no declaration.
    pub representation_span: Span,
    pub implements: Box<[TypeId]>,
    pub members: Vec<MemberId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TypedefDecl {
    pub name: Name,
    pub type_params: Box<[TypeParameter]>,
    pub kind: TypedefKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TypedefKind {
    /// `typedef F<T> = int Function(T);` — any type.
    Alias(TypeId),
    /// `typedef int F<T>(T x);` — old form, function types only.
    Legacy {
        return_type: Option<TypeId>,
        parameters: Box<[Parameter]>,
    },
}

/// Variance modifier of a type parameter (`in`, `out`, `inout`), from the
/// `variance` experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Variance {
    In,
    Out,
    Inout,
}

impl Variance {
    /// The modifier by its text.
    pub fn from_text_(text: &str) -> Option<Variance> {
        match text {
            "in" => Some(Variance::In),
            "out" => Some(Variance::Out),
            "inout" => Some(Variance::Inout),
            _ => None,
        }
    }

    /// The modifier's text.
    pub fn text(self) -> &'static str {
        match self {
            Variance::In => "in",
            Variance::Out => "out",
            Variance::Inout => "inout",
        }
    }
}

/// `T extends Bound` in a `<...>` list, with metadata.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TypeParameter {
    pub span: Span,
    pub metadata: Box<[Annotation]>,
    pub name: Name,
    pub bound: Option<TypeId>,
    /// The variance modifier as written (`class C<out T>`), with its token. As
    /// in the analyzer (`TypeParameter.varianceKeyword`), it is kept even with
    /// the experiment disabled: the parser reports the experiment and the use
    /// site is still checked.
    pub variance: Option<(Variance, Span)>,
}

/// List `kind a = 1, b;` with the common modifiers.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct VariableList {
    pub external: bool,
    pub static_: bool,
    pub abstract_: bool,
    pub covariant: bool,
    pub late: bool,
    pub final_: bool,
    pub const_: bool,
    /// `var` was written (so `ty` is `None`).
    pub var_: bool,
    pub ty: Option<TypeId>,
    pub variables: Box<[Variable]>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Variable {
    pub name: Name,
    pub initializer: Option<ExprId>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Member {
    pub span: Span,
    pub metadata: Box<[Annotation]>,
    pub kind: MemberKind,
    /// `augment`: the member augments the member of the same name that comes
    /// before it in the class chain (docs/AUGMENTATIONS.md).
    pub augment: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum MemberKind {
    Field(VariableList),
    /// Method, getter, setter or operator; `static`/`abstract` on the function.
    Method(FunctionId),
    Constructor(Constructor),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Constructor {
    pub external: bool,
    pub const_: bool,
    pub factory: bool,
    /// Class name as written (`C` in `C.named`).
    pub class_name: Name,
    /// `named` in `C.named`; `None` for the unnamed constructor.
    pub name: Option<Name>,
    pub parameters: Box<[Parameter]>,
    pub initializers: Box<[Initializer]>,
    /// `= Other<T>.ctor` (redirecting factory) or `: this.x()` stays in
    /// `initializers` as [`Initializer::Redirect`].
    pub redirect: Option<RedirectTarget>,
    pub body: FunctionBody,
    /// `this : inits { body }` (Dart 3.13): the body part of a primary
    /// constructor, before elaboration. After it, only what is an error remains
    /// (without a primary header, or repeated).
    pub primary_body_part: bool,
}

/// Target of `factory C() = Other.name;`
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RedirectTarget {
    pub span: Span,
    pub ty: TypeId,
    pub constructor: Option<Name>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Initializer {
    /// `this.x = e` or `x = e`.
    Field {
        span: Span,
        this_: bool,
        name: Name,
        value: ExprId,
    },
    /// `super(args)` or `super.name(args)`.
    Super {
        span: Span,
        constructor: Option<Name>,
        arguments: Arguments,
    },
    /// `this(args)` or `this.name(args)` — redirecting constructor.
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
// Functions
// ---------------------------------------------------------------------------

/// Signature and body shared by every form of function.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub span: Span,
    pub external: bool,
    pub static_: bool,
    /// Explicitly declared `abstract` (fields only) or bodiless in a type.
    pub kind: FunctionKind,
    /// `None` in function expressions and in untyped setters.
    pub return_type: Option<TypeId>,
    /// `None` in function expressions.
    pub name: Option<Name>,
    pub type_params: Box<[TypeParameter]>,
    /// Getters have no list (`None`); setters and the rest do.
    pub parameters: Option<Box<[Parameter]>>,
    pub modifier: AsyncModifier,
    pub body: FunctionBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FunctionKind {
    Function,
    Getter,
    Setter,
    /// `operator +`; the name holds the operator's text (`[]=`, `unary-`…).
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AsyncModifier {
    #[default]
    None,
    Async,
    AsyncStar,
    SyncStar,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum FunctionBody {
    /// `{ ... }`
    Block(StmtId),
    /// `=> e`
    Expression(ExprId),
    /// `;` — abstract, external or redirecting.
    Empty,
    /// `native 'name';` or `native;` (SDK).
    Native(Option<StringLit>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParameterKind {
    /// Required positional.
    Required,
    /// Inside `[ ]`.
    Optional,
    /// Inside `{ }`.
    Named,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Parameter {
    pub span: Span,
    pub metadata: Box<[Annotation]>,
    pub kind: ParameterKind,
    /// `required` (named only).
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
    /// `None` only in unnamed function type parameters: `int Function(int)`.
    pub name: Option<Name>,
    /// Old form `int f(int x)`: type parameters and the function parameter's
    /// list.
    pub function_type_params: Box<[TypeParameter]>,
    pub function_parameters: Option<Box<[Parameter]>>,
    /// `int f(int x)?` — the `?` after the old-form list.
    pub function_nullable: bool,
    pub default_value: Option<ExprId>,
    /// Private named parameter that initializes or declares a field (Dart 3.12,
    /// `{this._x}`): the **external** name — that of the signature and of the
    /// call — is this one (`x`); `name` stays the local and field name (`_x`).
    pub public_name: Option<Name>,
    /// Declaring parameter (`var`/`final` of a primary constructor, 3.13) that
    /// the parser rewrote as `this.x`: syntactically a `SimpleFormalParameter`,
    /// not a `FieldFormalParameter`.
    pub declaring: bool,
}

impl Parameter {
    /// The name under which the parameter is passed and appears in the
    /// signature: the public one of a private named parameter (Dart 3.12),
    /// otherwise the name itself.
    pub fn external_name(&self) -> Option<Name> {
        self.public_name.or(self.name)
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TypeAnnotation {
    pub span: Span,
    /// `?` final.
    pub nullable: bool,
    pub kind: TypeKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum TypeKind {
    /// `name`, `p.name`, with arguments: includes `dynamic`, **not** `void`, raw
    /// `Function`, `Never`, `Object`.
    Named {
        /// One or two parts (`p.Name`).
        name: Box<[Name]>,
        args: Box<[TypeId]>,
    },
    /// `void`
    Void,
    /// `R Function<T>(P, {n})`
    Function {
        return_type: Option<TypeId>,
        type_params: Box<[TypeParameter]>,
        parameters: Box<[Parameter]>,
    },
    /// `(int, {String name})`
    Record {
        positional: Box<[TypeId]>,
        named: Box<[(Name, TypeId)]>,
    },
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum StmtKind {
    Block(Box<[StmtId]>),
    /// `late final int x = 1, y;` local.
    Variables(VariableList),
    /// `var (a, b) = e;` / `final [x, y] = e;`
    PatternVariables {
        final_: bool,
        pattern: PatternId,
        value: ExprId,
    },
    /// Local function (named).
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
        updates: Box<[ExprId]>,
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
        cases: Box<[SwitchCase]>,
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
        catches: Box<[CatchClause]>,
        finally_: Option<StmtId>,
    },
    Labeled {
        labels: Box<[Name]>,
        body: StmtId,
    },
    Assert {
        condition: ExprId,
        message: Option<ExprId>,
    },
    Empty,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ForInit {
    Variables(VariableList),
    Expression(ExprId),
    /// `for (var (a, b) = e; …; …)` / `for (final [x, y] = e; …; …)`: the
    /// pattern declaration as initialization (the analyzer's
    /// `ForPartsWithPattern`). The pattern's variables are in scope in the
    /// condition, the updaters and the body, like those of
    /// [`ForInit::Variables`].
    Pattern {
        final_: bool,
        pattern: PatternId,
        value: ExprId,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ForInTarget {
    /// `for (int x in e)` / `for (var x in e)` / `for (final x in e)`.
    Declared {
        metadata: Box<[Annotation]>,
        final_: bool,
        var_: bool,
        ty: Option<TypeId>,
        name: Name,
        /// The written `const` (`for (const x in e)`, `for_in_with_const_variable`).
        #[serde(default)]
        const_: Option<Span>,
    },
    /// `for (final (a, b) in e)` / `for (var [x] in e)`.
    Pattern { final_: bool, pattern: PatternId },
    /// `for (x in e)` — an existing variable.
    Expression(ExprId),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SwitchCase {
    pub span: Span,
    pub labels: Box<[Name]>,
    /// `case p when g:`; `None` for `default:`.
    pub pattern: Option<PatternId>,
    pub guard: Option<ExprId>,
    pub body: Box<[StmtId]>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

/// String literal, possibly adjacent and interpolated.
///
/// Each element of `parts` is a syntactic literal (`'a' 'b'` produces two);
/// each literal is a sequence of pieces and interpolations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StringLit {
    pub span: Span,
    /// Exact size (`Box<[T]>`): most literals have a single piece.
    pub parts: Box<[StringPart]>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StringPart {
    /// A literal piece, **already decoded** (escapes resolved); `raw` comes
    /// unchanged. [`DartStr`] and not `str` because `'\uD800'` is a valid
    /// literal.
    Text(DartStr),
    /// `$x` or `${e}`.
    Interpolation(ExprId),
}

impl StringLit {
    /// Value when the string has no interpolation. Adjacent literals are
    /// concatenated by UTF-16 units, so `'\uD83D' '\uDC6D'` forms the pair.
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Arguments {
    pub span: Span,
    /// `Box<[T]>` and not `Vec`: exact capacity (the `Vec` doubled — 4 slots
    /// for 2 arguments) and 16 bytes instead of 24. Reads the same as a `Vec`
    /// (`iter`, `len`, index); only the parser builds it.
    pub type_args: Box<[TypeId]>,
    pub args: Box<[Argument]>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Argument {
    pub name: Option<Name>,
    pub value: ExprId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// `a ~ b`: the `~` (a prefix operator) between two operands, which fasta
    /// reads as a binary expression at prefix level (without a syntax error)
    /// and the analyzer reports as `NOT_BINARY_OPERATOR`.
    NotBinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Compound assignment operator; `None` is `=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssignOp {
    Assign,
    Compound(BinaryOp),
}

/// Lists inside the variants are `Box<[T]>` (exact size, 16 bytes) and
/// large auxiliary structures live in a `Box`: the size of `Expr` is that
/// of the largest variant times the number of expressions (397 thousand in
/// `new_sali`), and each 8 bytes here are 3 MiB there. Reads the same as a
/// `Vec`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ExprKind {
    Int(Span),
    Double(Span),
    Bool(bool),
    Null,
    String(StringLit),
    /// `#name`, `#a.b.c`, `#+`, `#void`.
    Symbol(Box<[Name]>),
    Identifier(Name),
    This,
    /// `super` only appears as the target of `.`, `[]`, an operator or a call.
    Super,
    /// `(e)`
    Parenthesized(ExprId),
    /// `<T>[a, b, ...c, if (x) d, for (...) e]`
    List {
        const_: bool,
        type_args: Box<[TypeId]>,
        elements: Box<[CollectionElement]>,
    },
    /// `<K, V>{k: v}` or `<T>{a, b}`; without type arguments the shape is
    /// decided by the elements (an empty `{}` is a map).
    SetOrMap {
        const_: bool,
        type_args: Box<[TypeId]>,
        elements: Box<[CollectionElement]>,
    },
    /// `(a, b, name: c)`
    Record {
        const_: bool,
        positional: Box<[ExprId]>,
        named: Box<[(Name, ExprId)]>,
    },
    /// `new T<A>.name(args)` / `const T(args)`; explicit `new`/`const`.
    InstanceCreation {
        keyword: Option<CreationKeyword>,
        ty: TypeId,
        constructor: Option<Name>,
        /// In a `Box` because `Arguments` has 64 bytes and was the variant that
        /// dictated the size of every `Expr` (120 → 80 bytes): only calls pay for
        /// it. A `Box` of an auxiliary structure, not of a node — nodes stay in
        /// arenas.
        arguments: Box<Arguments>,
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
    /// `f(args)`, `e.m<T>(args)`, `e?.m(args)` — the target is the function
    /// expression (a `Property` for method calls).
    Call {
        target: ExprId,
        /// Veja [`ExprKind::InstanceCreation::arguments`].
        arguments: Box<Arguments>,
    },
    /// `e<T>` — explicit instantiation of a tearoff or generic type.
    TypeArguments {
        target: ExprId,
        type_args: Box<[TypeId]>,
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
    /// `(a, b) = e` / `[x, y] = e` — pattern assignment.
    PatternAssign {
        pattern: PatternId,
        value: ExprId,
    },
    /// `e..a = 1..b()` : each section is an expression whose innermost target
    /// is the [`ExprKind::CascadeTarget`] marker. `null_aware` is `?..` in the
    /// first section.
    Cascade {
        target: ExprId,
        sections: Box<[ExprId]>,
        null_aware: bool,
    },
    /// Implicit receiver of a cascade section (`..a` → `CascadeTarget.a`).
    CascadeTarget,
    Await(ExprId),
    Throw(ExprId),
    Rethrow,
    /// `switch (e) { p when g => v, _ => w }`
    Switch {
        value: ExprId,
        cases: Box<[SwitchExprCase]>,
    },
    /// Dot shorthand (Dart 3.10): the head `.id`, `.new`, `const .id` or
    /// `const .new`; the following selectors are regular nodes around it
    /// (`.parse('1')` is `Call { target: DotShorthand }`). Denotes `D.id`, with
    /// `D` the declaration of the context type of the whole chain; `types`
    /// records `D` in `Resolved::Element(Element::Class(D))` of this node
    /// (`docs/VERSOES-LINGUAGEM.md` §4.3).
    DotShorthand {
        /// `id`, or `new` for `.new`.
        name: Name,
        /// `const .id(…)`/`const .new(…)`: constant creation.
        const_: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CreationKeyword {
    New,
    Const,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SwitchExprCase {
    pub span: Span,
    pub pattern: PatternId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
}

/// List/set/map literal element.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum CollectionElement {
    Expression(ExprId),
    /// `?e` (null-aware elements, Dart 3.8): included only when not null.
    NullAwareExpression(ExprId),
    /// `k: v`, `?k: v`, `k: ?v`, `?k: ?v` — the `?` are null-aware (Dart 3.8).
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
        updates: Box<[ExprId]>,
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
// Patterns
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Pattern {
    pub span: Span,
    pub kind: PatternKind,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum PatternKind {
    /// `_` or `int _`
    Wildcard {
        ty: Option<TypeId>,
    },
    /// `var x`, `final int x`, or a bare `x` inside a declaration pattern.
    Variable {
        final_: bool,
        var_: bool,
        ty: Option<TypeId>,
        name: Name,
    },
    /// Constant: literal, `const C()`, `-1`, qualified identifier.
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
        type_args: Box<[TypeId]>,
        elements: Box<[ListPatternElement]>,
    },
    /// `<K, V>{k: p, ...}`
    Map {
        type_args: Box<[TypeId]>,
        entries: Box<[MapPatternEntry]>,
        rest: bool,
    },
    /// `(a, name: p, :x)`
    Record {
        fields: Box<[PatternField]>,
    },
    /// `Nome<T>(field_: p, :x)`
    Object {
        ty: TypeId,
        fields: Box<[PatternField]>,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ListPatternElement {
    Pattern(PatternId),
    /// `...` or `...p`
    Rest(Option<PatternId>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MapPatternEntry {
    pub key: ExprId,
    pub value: PatternId,
}

/// Record/object pattern field. A missing `name` is positional; `:x` is a
/// field with the name inferred from the variable pattern.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PatternField {
    pub span: Span,
    pub name: Option<Name>,
    pub pattern: PatternId,
}
