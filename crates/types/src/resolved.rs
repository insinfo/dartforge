//! Resolution of identifier and expression nodes in bodies to their declarations and semantic targets.
//!
//! Every semantic reference in the bodies of functions or initializers is recorded
//! in the side table [`UnitBodyTypes`], compactly indexed by [`ast::ExprId`].

use crate::table::{TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, LibraryId, VariableId};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;

/// Identifier of a local variable declared in a block or expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

/// Reference to a class or extension member (function/method or field/variable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberRef {
    Function(FunctionElementId),
    Variable(VariableId),
}

/// Resolved target of a reference or identifier inside a body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resolved {
    /// Local variable declared in block scope (`var x`, `int x`, `for (var x...)`).
    Local(LocalId),
    /// Formal parameter of the enclosing function, method or closure.
    Parameter { index: u32, name: SymbolId },
    /// Generic type parameter, local or of the enclosing class/extension.
    TypeParameter(TypeParamId),
    /// Top-level element (class, top-level function, top-level variable, typedef, extension).
    Element(Element),
    /// Class member (field, getter, setter, method) accessed through an explicit or implicit (`this`) receiver.
    Member {
        class: ClassId,
        member: MemberRef,
        via_super: bool,
    },
    /// Import prefix (`import '...' as p; p.x`).
    Prefix(LibraryId),
    /// Member access on a receiver whose static type is `dynamic`.
    Dynamic,
    /// Extension member resolved for a call to a static extension method/getter/setter.
    ExtensionMember {
        extension: ExtensionId,
        member: FunctionElementId,
    },
    /// Class constructor called in an instantiation (`new C()`, `C.named()`).
    Constructor(FunctionElementId),
}

/// Side tables of static types and resolutions for a single compilation unit.
///
/// Keeps vectors indexed directly by [`ast::ExprId::0 as usize`], without hashing
/// costs or heap pointers.
#[derive(Debug, Clone, Default)]
pub struct UnitBodyTypes {
    /// Inferred static type of each expression in the `ast.exprs` arena
    /// (empty while [`Self::lazy`] holds the size).
    static_types: Vec<TypeId>,
    /// Resolved target of identifier, property or call expressions.
    resolved: Vec<Option<Resolved>>,
    /// The expression count and the fallback type of tables that nobody wrote
    /// yet: the two vectors are only allocated on the first write. A pass of
    /// inference creates tables for every unit that may be inferred, and most
    /// of them are never touched (one pass per library would otherwise be
    /// quadratic in the number of libraries).
    lazy: Option<(u32, TypeId)>,
    /// Declared type (or inferred from the initializer) of each local variable,
    /// by the offset of the name in the declaration. The native backend stores
    /// the local in that type's representation (`int?` is a box, `int` is an
    /// `i64`), which cannot be deduced from the reads: they have the type
    /// promoted by flow analysis.
    pub local_types: std::collections::HashMap<usize, TypeId>,
    /// Declaration (name offset) of the local that each expression resolved to
    /// [`Resolved::Local`] reads or writes. Only filled in when
    /// [`crate::BodyInferrer::record_locals`] is on (the LSP, for renaming
    /// and finding references); the compiler does not pay for it.
    pub local_declarations: std::collections::HashMap<ast::ExprId, usize>,
    /// Local binding of metadata, by annotation start offset. Recorded during
    /// lexical inference when local declaration recording is enabled; constant
    /// evaluation must not fall back to a shadowed library declaration.
    pub metadata_local_declarations: std::collections::HashMap<usize, usize>,
    /// Expressions whose type is the analyzer's recovery type (`InvalidType`, a
    /// read that does not resolve): in `static_types` they are `dynamic`, which
    /// is how `InvalidType` behaves; the mark only matters for display.
    pub invalid_types: std::collections::HashSet<ast::ExprId>,
    /// The runtime type (the CFE rule, which gives `runtimeType`) of a literal
    /// or local generator function (`sync*`/`async*`) without a written return
    /// type, when it differs from the static type, which follows analyzer 3.6.2:
    /// the analyzer joins the `Null` of a `return;` with the `yield`s and gives
    /// `dynamic` to a generator without any; the CFE ignores the `return;` and
    /// gives `Null`. By function (`ast::FunctionId`).
    pub function_runtime_types: std::collections::HashMap<ast::FunctionId, TypeId>,
    /// Type arguments chosen for each generic call (function, method, static,
    /// `call`), explicit or inferred, by the offset of the start of the argument
    /// list (`ast::Arguments::span`). It is outside the type of any node
    /// (`id(5)` has type `Object?`, but the instantiation `<Object?>` only
    /// appears here) and is what the backend reifies.
    pub instantiations: std::collections::HashMap<usize, Box<[TypeId]>>,
    /// The type arguments of the implicit instantiation of a generic function
    /// tear-off in a non-generic function context (`int Function(int) f = id;`),
    /// by the tear-off expression: the closure has the instantiated type and
    /// carries the arguments, as in the VM.
    pub tearoff_instantiations: std::collections::HashMap<ast::ExprId, Box<[TypeId]>>,
    /// The type of each pattern that exhaustiveness reads (the analyzer's
    /// `PatternConverter`, `an611:src/generated/exhaustiveness.dart:520-680`):
    /// the declared type of a variable, the written type of a wildcard or a
    /// cast, the required type of an object, list (`List<E>`) or map
    /// (`Map<K, V>`) pattern.
    pub pattern_types: std::collections::HashMap<ast::PatternId, TypeId>,
    /// The type of the extension property read by an object pattern field
    /// (`ExtensionKey`), by the field's subpattern.
    pub extension_fields: std::collections::HashMap<ast::PatternId, TypeId>,
    /// Patterns whose written type does not resolve (`InvalidType`): the
    /// analyzer does not check the exhaustiveness of the `switch` that contains
    /// them.
    pub invalid_patterns: std::collections::HashSet<ast::PatternId>,
    /// The matched type (`matchedValueType`) of each pattern, already promoted by
    /// pattern flow (previous `switch` cases, `&&`, `?`): what the analyzer
    /// stores in `DartPatternImpl.matchedValueType` and `ConstantVerifier` reads
    /// (`constant_pattern_never_matches_value_type`).
    pub matched_types: std::collections::HashMap<ast::PatternId, TypeId>,
    /// Bare-name patterns in a refutable context (the `ConstantPattern` of a
    /// `SimpleIdentifier`) whose name is not a constant: a local or variable without
    /// `const`, an instance member, a getter (3.6.2's `_getConstantValue` gives the
    /// generic error). `ConstantVerifier` reports at the name.
    pub non_constant_name_patterns: std::collections::HashSet<ast::PatternId>,
    /// The element of a bare name in a refutable context (the
    /// `SimpleIdentifier` of the `ConstantPattern`: `case _limite:`), which has
    /// no `ExprId` in our tree: what the `Resolved` of an identifier would give.
    pub pattern_names: std::collections::HashMap<ast::PatternId, Resolved>,
    /// The function expressions whose context type was a function type
    /// (`FunctionExpressionImpl.wasFunctionTypeSupplied`, the analyzer's
    /// `FunctionExpressionResolver`: `contextType is FunctionType`), by
    /// function id.
    pub with_function_type: std::collections::HashSet<ast::FunctionId>,
    /// The resolved type of each type annotation in the bodies (the analyzer's
    /// `TypeAnnotation.type`): `is`, `as`, `catch`, local variable, type
    /// argument.
    pub annotation_types: std::collections::HashMap<ast::TypeId, TypeId>,
    /// The declaration (the name offset) of the local that each assignment
    /// pattern variable writes (`AssignedVariablePattern.element`), with local
    /// recording on.
    pub pattern_declarations: std::collections::HashMap<ast::PatternId, usize>,
    /// The expressions that the analyzer's resolver replaces with an
    /// `ImplicitCallReference` (`_insertImplicitCallReference`,
    /// `resolver.dart:4085-4142`): an object of a non-nullable interface type
    /// with a `call` method, in a context that accepts a function. The shape
    /// (cascade target, branch of `?:`, operand of `??`) is left to the reader,
    /// through the tree.
    pub implicit_calls: std::collections::HashSet<ast::ExprId>,
    /// The type of the parameter that receives each argument (the
    /// `staticParameterElement.type`), by the argument expression (the value of
    /// a named argument): that of the already instantiated invoked type.
    pub parameter_types: std::collections::HashMap<ast::ExprId, TypeId>,
    /// The `writeType` of each assignment (simple and compound), by the
    /// assignment expression.
    pub write_types: std::collections::HashMap<ast::ExprId, TypeId>,
    /// The `PatternField.element` of each object pattern field that found a
    /// member, by the field's subpattern.
    pub field_elements: std::collections::HashMap<ast::PatternId, Resolved>,
    /// The `RelationalPattern.element` (the operator found in the matched type),
    /// by the relational pattern.
    pub relational_operators: std::collections::HashMap<ast::PatternId, Resolved>,
}

impl UnitBodyTypes {
    /// Creates side tables allocated with the exact size of the unit's AST expressions.
    pub fn new(num_exprs: usize, fallback_type: TypeId) -> Self {
        Self {
            static_types: Vec::new(),
            resolved: Vec::new(),
            lazy: (num_exprs > 0).then_some((num_exprs as u32, fallback_type)),
            local_types: std::collections::HashMap::new(),
            local_declarations: std::collections::HashMap::new(),
            metadata_local_declarations: std::collections::HashMap::new(),
            with_function_type: std::collections::HashSet::new(),
            annotation_types: std::collections::HashMap::new(),
            pattern_declarations: std::collections::HashMap::new(),
            invalid_types: std::collections::HashSet::new(),
            function_runtime_types: std::collections::HashMap::new(),
            instantiations: std::collections::HashMap::new(),
            tearoff_instantiations: std::collections::HashMap::new(),
            pattern_types: std::collections::HashMap::new(),
            extension_fields: std::collections::HashMap::new(),
            invalid_patterns: std::collections::HashSet::new(),
            matched_types: std::collections::HashMap::new(),
            non_constant_name_patterns: std::collections::HashSet::new(),
            pattern_names: std::collections::HashMap::new(),
            implicit_calls: std::collections::HashSet::new(),
            parameter_types: std::collections::HashMap::new(),
            write_types: std::collections::HashMap::new(),
            field_elements: std::collections::HashMap::new(),
            relational_operators: std::collections::HashMap::new(),
        }
    }

    /// Allocates the vectors of tables created lazily (see [`Self::new`]).
    fn materialize(&mut self) {
        if let Some((n, fallback)) = self.lazy.take() {
            self.static_types = vec![fallback; n as usize];
            self.resolved = vec![None; n as usize];
        }
    }

    /// The number of expressions these tables cover: zero for a unit without
    /// tables (an SDK unit whose bodies are not inferred).
    pub fn expression_count(&self) -> usize {
        match self.lazy {
            Some((n, _)) => n as usize,
            None => self.static_types.len(),
        }
    }

    /// The unit has tables (its bodies may be inferred).
    pub fn has_tables(&self) -> bool {
        self.expression_count() > 0
    }

    /// The resolved targets recorded, in expression order.
    pub fn resolved_targets(&self) -> impl Iterator<Item = (ast::ExprId, &Resolved)> + '_ {
        self.resolved.iter().enumerate().filter_map(|(i, r)| r.as_ref().map(|r| (ast::ExprId(i as u32), r)))
    }

    /// Sets the static type of every expression to `ty` (the dump's
    /// "not visited" mark before inference).
    pub fn fill_types(&mut self, ty: TypeId) {
        self.materialize();
        self.static_types.fill(ty);
    }

    /// The type arguments of the implicit instantiation of tear-off `e` (see
    /// [`UnitBodyTypes::tearoff_instantiations`]).
    pub fn tearoff_instantiation(&self, e: ast::ExprId) -> Option<&[TypeId]> {
        self.tearoff_instantiations.get(&e).map(|a| &**a)
    }

    /// Records the type arguments of the generic call whose argument list
    /// starts at `offset`.
    pub fn set_instantiation(&mut self, offset: usize, args: Box<[TypeId]>) {
        self.instantiations.insert(offset, args);
    }

    /// Type arguments of the generic call whose argument list starts at
    /// `offset` (see [`UnitBodyTypes::instantiations`]).
    pub fn instantiation(&self, offset: usize) -> Option<&[TypeId]> {
        self.instantiations.get(&offset).map(|a| &**a)
    }

    /// Records the type of a local variable declared at `offset`.
    pub fn set_local_type(&mut self, offset: usize, ty: TypeId) {
        self.local_types.insert(offset, ty);
    }

    /// The runtime type of a generator function without a written return type,
    /// when it differs from the static one (see
    /// [`Self::function_runtime_types`]).
    pub fn function_runtime_type(&self, f: ast::FunctionId) -> Option<TypeId> {
        self.function_runtime_types.get(&f).copied()
    }

    /// Type of the local variable declared at `offset` (the name's).
    pub fn local_type(&self, offset: usize) -> Option<TypeId> {
        self.local_types.get(&offset).copied()
    }

    /// Sets the static type of an expression.
    pub fn set_type(&mut self, expr: ast::ExprId, ty: TypeId) {
        let idx = expr.0 as usize;
        if idx < self.expression_count() {
            self.materialize();
            self.static_types[idx] = ty;
        }
    }

    /// Gets the static type associated with an expression.
    pub fn get_type(&self, expr: ast::ExprId) -> Option<TypeId> {
        match self.lazy {
            Some((n, fallback)) => ((expr.0) < n).then_some(fallback),
            None => self.static_types.get(expr.0 as usize).copied(),
        }
    }

    /// Records the semantic resolution of an expression.
    pub fn set_resolved(&mut self, expr: ast::ExprId, target: Resolved) {
        let idx = expr.0 as usize;
        if idx < self.expression_count() {
            self.materialize();
            self.resolved[idx] = Some(target);
        }
    }

    /// Offset of the name in the declaration of the local that `expr` references, when
    /// inference recorded locals (see [`UnitBodyTypes::local_declarations`]).
    pub fn local_declaration(&self, expr: ast::ExprId) -> Option<usize> {
        self.local_declarations.get(&expr).copied()
    }

    /// Gets the resolved semantic target of an expression.
    pub fn get_resolved(&self, expr: ast::ExprId) -> Option<&Resolved> {
        self.resolved.get(expr.0 as usize).and_then(|r| r.as_ref())
    }
}

/// Set of all body side tables of all units of the program.
#[derive(Debug, Clone, Default)]
pub struct BodyTypes {
    /// Tables indexed by the unit's index in `Program::units`.
    pub units: Vec<UnitBodyTypes>,
    /// The extensions chosen when resolving a member (the `ImportsTracking`'s
    /// `notifyExtensionUsed`), by the library in which the resolution happened.
    pub used_extensions: std::collections::HashSet<(dartforge_elements::model::LibraryId, dartforge_elements::model::ExtensionId)>,
}

/// A local (variable, parameter or local function) visible at a point of
/// the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLocal {
    pub name: SymbolId,
    /// Declared type (written or inferred from the initializer).
    pub ty: TypeId,
    /// Offset of the name in the declaration.
    pub offset: usize,
    /// Declared as a local function (`void f() {}` inside a body).
    pub function: bool,
}

/// The lexical scope captured at the probed identifier (see
/// [`crate::BodyInferrer::scope_probe`]): what the LSP offers when
/// completing a simple name, from the same scope stack that resolves names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbedScope {
    /// Visible locals, from innermost to outermost; a shadowed name appears only
    /// once (the innermost declaration).
    pub locals: Vec<VisibleLocal>,
    /// Type parameters visible by name.
    pub type_parameters: Vec<(SymbolId, TypeParamId)>,
    /// Class (or mixin, enum, extension type) of the enclosing member.
    pub class: Option<ClassId>,
    /// Extension of the enclosing member.
    pub extension: Option<ExtensionId>,
    /// Without `this`: static member, top level or factory constructor.
    pub the_static: bool,
    /// Type of `this`, when there is one.
    pub this_type: Option<TypeId>,
    /// Library of the body.
    pub library: Option<LibraryId>,
}
