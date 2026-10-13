//! The analyzer 3.6.2 `FfiVerifier`
//! (`analyzer/lib/src/generated/ffi_verifier.dart`), in full: the 48
//! `FfiCode` codes about the use of the `dart:ffi` APIs.
//!
//! The traversal is that of the original `RecursiveAstVisitor`:
//! * the class (`visitClassDeclaration`, only the `ClassDeclaration`:
//!   `class A = B with C;` does not go through): the `dart:ffi` superclass
//!   (`Struct`, `Union`, `AbiSpecificInteger`) and the compound subtypes in
//!   `extends`, `implements` and `with`; inside a compound, the instance
//!   fields (`_validateFieldsInCompound`);
//! * the `@Native` of fields, top-level variables, functions (top-level and
//!   local) and methods (`_checkFfiNative`); the `@DefaultAsset` of the
//!   `library` directive;
//! * the invocations: `Pointer.fromFunction`, `Pointer.elementAt`,
//!   `Struct.create`/`Union.create`, `Native.addressOf`, `asFunction`,
//!   `lookupFunction`, `sizeOf`, the `call` of `AllocatorAlloc`, `ref`, `[]`,
//!   `address` and the creation of `Struct`/`Union`/`NativeCallable`.
//!
//! `visitIndexExpression` does not call `super`: nothing inside an index
//! expression (the target, the index, closures in them) is visited, and the
//! port follows that rule.
//!
//! The value of a `dart:ffi` annotation (`computeConstantValue`) is the object
//! that the SDK 3.6.2 constructor builds: `Native({assetId, isLeaf = false,
//! symbol})`, `Packed(memberAlignment)`, the four of `Array` (which redirect
//! to `_ArraySize`), `AbiSpecificIntegerMapping(mapping)` and
//! `DefaultAsset(id)`. The constant engine does not execute constructors of
//! libraries that were not inferred (the SDK ones); the fields are built here
//! from the arguments, which the engine evaluates, following the text of those
//! constructors. The type of the value (`Native<T>`) is that of the
//! instantiation the inference recorded for the annotation's argument list.
//!
//! `strict-casts` does not exist in the engine: the `isAssignableTo` of
//! `Native.addressOf` is the one without `strict-casts`.
//! Written without compiling or running (2026-10-05).

use crate::const_eval::evaluation::{Constant, Ctx, Engine};
use crate::const_eval::value::{InstanceState, Value};
use crate::display::{Arg, Displayer};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::subtyping::{SubtypeEnv, is_subtype};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::codes::ffi as cf;
use dartforge_diagnostics::{Code, Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, Library, LibraryId, Program, UnitId, VariableId,
    VariableRef,
};
use dartforge_frontend::ast::{self, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind, MemberKind, ParameterKind, StmtKind, TypeKind};
use dartforge_frontend::parents::{Parent, Parents};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// `_primitiveIntegerNativeTypesFixedSize`.
const FIXED_INTEGERS: [&str; 8] = ["Int8", "Int16", "Int32", "Int64", "Uint8", "Uint16", "Uint32", "Uint64"];
/// `_addressOfCompoundExtensionNames`.
const COMPOUND_ADDRESS: [&str; 3] = ["ArrayAddress", "StructAddress", "UnionAddress"];
/// `_addressOfPrimitiveExtensionNames`.
const PRIMITIVE_ADDRESS: [&str; 3] = ["BoolAddress", "DoubleAddress", "IntAddress"];
/// `_addressOfTypedDataExtensionNames`.
const TYPED_DATA_ADDRESS: [&str; 10] = [
    "Float32ListAddress",
    "Float64ListAddress",
    "Int16ListAddress",
    "Int32ListAddress",
    "Int64ListAddress",
    "Int8ListAddress",
    "Uint16ListAddress",
    "Uint32ListAddress",
    "Uint64ListAddress",
    "Uint8ListAddress",
];

/// `_PrimitiveDartType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Primitive {
    Double,
    Int,
    Bool,
    Void,
    Handle,
    None_,
}

/// `_FfiTypeCheckDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    NativeToDart,
    DartToNative,
}

impl Direction {
    fn inverse(self) -> Direction {
        match self {
            Direction::NativeToDart => Direction::DartToNative,
            Direction::DartToNative => Direction::NativeToDart,
        }
    }
}

/// The `allow*` flags of `_isValidFfiNativeType`.
#[derive(Clone, Copy, Default)]
struct Allows {
    void_: bool,
    empty_struct: bool,
    array: bool,
    handle: bool,
    opaque: bool,
}

/// The value of an annotation (`DartObject`), with the fields the verifier
/// consults (`getField`).
struct Instance {
    /// The class of the value's type.
    class: ClassId,
    /// The type of the value (`Native<T>`).
    ty: TypeId,
    /// `isLeaf`.
    is_leaf: Option<bool>,
    /// `memberAlignment`.
    alignment: Option<i64>,
    /// `dimensions` (`toListValue` of the `toIntValue`s).
    list: Option<Vec<i64>>,
    /// `dimension1` a `dimension5`.
    dimensions: [Option<i64>; 5],
    /// `variableLength`.
    variable: Option<bool>,
    /// The types of the values of `mapping`.
    map: Option<Vec<TypeId>>,
}

/// The element an annotation refers to (`Annotation.element`).
#[derive(Clone, Copy)]
enum AnnotationTarget {
    Constructor(ClassId, FunctionElementId),
    Class(ClassId),
    Variable(VariableId),
    Other,
}

/// The node of a declaration that `_checkFfiNative` examines.
#[derive(Clone, Copy)]
enum No {
    Function(ast::FunctionId),
    TopLevelVariable(ast::DeclId, usize),
    Field(ast::MemberId, usize),
    /// Local function: its type.
    Local(TypeId),
}

/// The declared element (`declarationElement`).
#[derive(Clone, Copy)]
enum Declaration {
    Variable(VariableId),
    Function(FunctionElementId),
    Local(TypeId),
}

/// A constant reference (`ConstVariableElement`).
enum ConstantRef {
    Variable(VariableId),
    Local,
    Parameter(Option<ExprId>),
}

/// The type table, loose until the engine is precise.
enum Table<'a> {
    Free(&'a mut TypeTable),
    Engine(Box<Engine<'a>>),
    Empty,
}

#[derive(Default)]
struct Indices {
    functions: HashMap<u32, FunctionElementId>,
    top: HashMap<(u32, usize), VariableId>,
    fields: HashMap<(u32, usize), VariableId>,
    classes: HashMap<u32, ClassId>,
}

struct V<'a> {
    program: &'a Program,
    interner: &'a Interner,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    bodies: &'a BodyTypes,
    the_inferred: &'a HashSet<LibraryId>,
    body: &'a UnitBodyTypes,
    u: UnitId,
    a: &'a ast::Ast,
    source: &'a str,
    ffi: LibraryId,
    typed_data: Option<LibraryId>,
    t: Table<'a>,
    out: Vec<Diagnostic>,
    cascades: HashMap<ExprId, ExprId>,
    parents: Parents,
    indices: Option<Indices>,
    /// The spans of the index expressions (`visitIndexExpression` does not visit
    /// the children).
    bracket_indices: Vec<Span>,
    /// The 3.6.2 `FfiVerifier` threw (see [`AnalyzerCrash`]).
    the_break: bool,
}

/// The analyzer 3.6.2 `FfiVerifier` throws an exception on the unit: the
/// `LibraryAnalyzer.analyze()` of the whole library fails, and the driver
/// (`driver.dart:1473-1490`) completes the requests with an error, without a
/// result for any unit. `dart analyze` shows no diagnostic at all for the
/// files of the library, and the caller discards all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalyzerCrash;

fn library_name(interner: &Interner, l: &Library) -> String {
    l.name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
}

/// The `FfiVerifier` reports in unit `u`, or the analyzer 3.6.2 crash
/// ([`AnalyzerCrash`]).
#[allow(clippy::too_many_arguments)]
pub fn verify<'a>(
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    bodies: &'a BodyTypes,
    the_inferred: &'a HashSet<LibraryId>,
    u: UnitId,
) -> Result<Vec<Diagnostic>, AnalyzerCrash> {
    let Some(ffi) = program.libraries.iter().position(|l| library_name(interner, l) == "dart.ffi").map(|i| LibraryId(i as u32)) else {
        return Ok(Vec::new());
    };
    let typed_data = program.libraries.iter().position(|l| library_name(interner, l) == "dart.typed_data").map(|i| LibraryId(i as u32));
    let Some(body) = bodies.units.get(u.0 as usize) else { return Ok(Vec::new()) };
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    let bracket_indices = a.exprs.iter().filter(|e| matches!(e.kind, ExprKind::Index { .. })).map(|e| e.span).collect();
    let mut v = V {
        program,
        interner,
        core,
        outline,
        bodies,
        the_inferred,
        body,
        u,
        a,
        source: &unit_id.source,
        ffi,
        typed_data,
        t: Table::Free(table),
        out: Vec::new(),
        cascades: crate::phase_catch_error::cascade_targets(a),
        parents: crate::typed_lints::unit_parents(program, u),
        indices: None,
        bracket_indices,
        the_break: false,
    };
    v.declarations();
    if v.the_break {
        return Err(AnalyzerCrash);
    }
    v.expressions();
    if v.the_break {
        return Err(AnalyzerCrash);
    }
    Ok(v.out)
}

impl<'a> V<'a> {
    // -- Infraestrutura --------------------------------------------------------

    fn table(&mut self) -> &mut TypeTable {
        match &mut self.t {
            Table::Free(t) => &mut **t,
            Table::Engine(m) => &mut *m.table,
            Table::Empty => unreachable!("tabela emprestada"),
        }
    }

    fn table_ref(&self) -> &TypeTable {
        match &self.t {
            Table::Free(t) => &**t,
            Table::Engine(m) => &*m.table,
            Table::Empty => unreachable!("tabela emprestada"),
        }
    }

    fn engine(&mut self) -> &mut Engine<'a> {
        if let Table::Free(_) = self.t
            && let Table::Free(t) = std::mem::replace(&mut self.t, Table::Empty)
        {
            self.t = Table::Engine(Box::new(Engine::new(self.program, self.interner, t, self.core, self.outline, self.bodies, self.the_inferred)));
        }
        match &mut self.t {
            Table::Engine(m) => &mut **m,
            _ => unreachable!("engine created above"),
        }
    }

    fn ty(&self, t: TypeId) -> Type {
        self.table_ref().get(t).clone()
    }

    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let outline = self.outline;
        let core = self.core;
        let mut env = SubtypeEnv::new(self.table(), &outline.hierarchy, core);
        is_subtype(a, b, &mut env)
    }

    fn report(&mut self, code: Code, span: Span, args: &[Arg<'_>]) {
        let (texts, context) = Displayer { table: self.table_ref(), interner: self.interner, program: self.program }.arguments_and_context(args);
        let mut d = Diagnostic::with_code(code, span, texts);
        d.context.extend(context);
        self.out.push(d);
    }

    fn name(&self, s: SymbolId) -> &'a str {
        self.interner.resolve(s)
    }

    /// Inside an index expression (without being it).
    fn inside_index(&self, s: Span) -> bool {
        self.bracket_indices.iter().any(|r| r.start <= s.start && s.end <= r.end && (r.start, r.end) != (s.start, s.end))
    }

    fn span(&self, e: ExprId) -> Span {
        self.a.expr(e).span
    }

    /// The node of the argument (the whole `NamedExpression`, if named).
    fn argument_span(&self, x: &ast::Argument) -> Span {
        let s = self.span(x.value);
        match x.name {
            Some(n) => Span { start: n.span.start, end: s.end },
            None => s,
        }
    }

    /// `Annotation.name` (the identifier, prefixed or not).
    fn annotation_name(&self, m: &ast::Annotation) -> Span {
        match (m.name.first(), m.name.get(1).or(m.name.first())) {
            (Some(a), Some(b)) => Span { start: a.span.start, end: b.span.end },
            _ => m.span,
        }
    }

    fn real_target(&self, e: ExprId) -> ExprId {
        self.cascades.get(&e).copied().unwrap_or(e)
    }

    fn parent(&self, e: ExprId) -> Option<ExprId> {
        match self.parents.parent(e) {
            Parent::Expr(p) => Some(p),
            _ => None,
        }
    }

    fn written_type(&self, t: ast::TypeId) -> Option<TypeId> {
        self.body.annotation_types.get(&t).or_else(|| self.outline.written_types.get(&(self.u, t))).copied()
    }

    /// `typeArgumentTypes` of the invocation whose argument list is `args`.
    fn instantiation(&self, args: &ast::Arguments) -> Vec<TypeId> {
        self.body.instantiation(args.span.start).map(|v| v.to_vec()).unwrap_or_default()
    }

    fn indices(&mut self) -> &Indices {
        if self.indices.is_none() {
            let u = self.u;
            let mut ix = Indices::default();
            for (i, f) in self.program.functions.iter().enumerate() {
                if let FunctionRef::Function { unit, function } = f.node
                    && unit == u
                {
                    ix.functions.insert(function.0, FunctionElementId(i as u32));
                }
            }
            for (i, v) in self.program.variables.iter().enumerate() {
                match v.node {
                    VariableRef::TopLevel { unit, decl, index } if unit == u => {
                        ix.top.insert((decl.0, index), VariableId(i as u32));
                    }
                    VariableRef::Field { unit, member, index } if unit == u => {
                        ix.fields.insert((member.0, index), VariableId(i as u32));
                    }
                    _ => {}
                }
            }
            for (i, c) in self.program.classes.iter().enumerate() {
                if let Some(d) = c.decl
                    && d.unit == u
                {
                    ix.classes.insert(d.decl.0, ClassId(i as u32));
                }
            }
            self.indices = Some(ix);
        }
        self.indices.as_ref().expect("indices built")
    }

    fn declaration(&mut self, no: No) -> Option<Declaration> {
        Some(match no {
            No::Function(f) => Declaration::Function(*self.indices().functions.get(&f.0)?),
            No::TopLevelVariable(d, i) => Declaration::Variable(*self.indices().top.get(&(d.0, i))?),
            No::Field(m, i) => Declaration::Variable(*self.indices().fields.get(&(m.0, i))?),
            No::Local(t) => Declaration::Local(t),
        })
    }

    // -- `dart:ffi` elements and types ----------------------------------------

    fn of_ffi(&self, c: ClassId) -> bool {
        self.program.class(c).library == self.ffi
    }

    fn ffi_class(&self, c: ClassId, the_name: &str) -> bool {
        let k = self.program.class(c);
        k.library == self.ffi && self.name(k.name) == the_name
    }

    fn ffi_class_by_name(&self, the_name: &str) -> Option<ClassId> {
        let s = self.interner.lookup(the_name)?;
        match self.program.library(self.ffi).declared.get(&s)?.getter? {
            Element::Class(c) => Some(c),
            _ => None,
        }
    }

    fn ffi_extension(&self, x: ExtensionId, names: &[&str]) -> bool {
        let e = self.program.extension(x);
        e.library == self.ffi && e.name.is_some_and(|n| names.contains(&self.name(n)))
    }

    fn compound(&self, c: ClassId) -> bool {
        self.ffi_class(c, "Struct") || self.ffi_class(c, "Union")
    }

    /// The element of an `InterfaceType` (class, enum, mixin, extension type,
    /// `Null`).
    fn class_of_type(&self, t: TypeId) -> Option<ClassId> {
        match self.table_ref().get(t) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            Type::Null => self.core.null_class,
            _ => None,
        }
    }

    /// `dartType is InterfaceType` (in the analyzer, `Null` and `FutureOr` too).
    fn is_interface(&self, t: TypeId) -> bool {
        matches!(
            self.table_ref().get(t),
            Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. }
        )
    }

    fn arguments_of_type(&self, t: TypeId) -> Vec<TypeId> {
        match self.table_ref().get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            Type::FutureOr { arg, .. } => vec![*arg],
            _ => Vec::new(),
        }
    }

    fn is_ffi_t(&self, t: TypeId, the_name: &str) -> bool {
        matches!(self.table_ref().get(t), Type::Interface { .. }) && self.class_of_type(t).is_some_and(|c| self.ffi_class(c, the_name))
    }

    /// `element.supertype` of the type.
    fn superclass(&self, t: TypeId) -> Option<ClassId> {
        self.class_of_type(t).and_then(|c| self.program.class(c).supertype_class)
    }

    fn compound_subtype(&self, t: TypeId) -> bool {
        self.superclass(t).is_some_and(|s| self.compound(s))
    }

    fn abi_subtype(&self, t: TypeId) -> bool {
        self.superclass(t).is_some_and(|s| self.ffi_class(s, "AbiSpecificInteger"))
    }

    fn opaque_subtype(&self, t: TypeId) -> bool {
        self.superclass(t).is_some_and(|s| self.ffi_class(s, "Opaque"))
    }

    fn is_core(&self, t: TypeId, c: Option<ClassId>) -> bool {
        matches!(self.table_ref().get(t), Type::Interface { class, .. } if Some(*class) == c)
    }

    fn is_int(&self, t: TypeId) -> bool {
        self.is_core(t, self.core.int_class)
    }

    fn is_double(&self, t: TypeId) -> bool {
        self.is_core(t, self.core.double_class)
    }

    fn is_bool(&self, t: TypeId) -> bool {
        self.is_core(t, self.core.bool_class)
    }

    fn is_void(&self, t: TypeId) -> bool {
        matches!(self.table_ref().get(t), Type::Void)
    }

    fn is_parameter(&self, t: TypeId) -> bool {
        matches!(self.table_ref().get(t), Type::TypeParameter { .. })
    }

    /// `isTypedDataClass`.
    fn typed_data_class(&self, c: ClassId) -> bool {
        Some(self.program.class(c).library) == self.typed_data
    }

    /// `isTypedData`: only the lists that correspond to a `Pointer`.
    fn is_typed_data(&self, t: TypeId) -> bool {
        let Some(c) = self.class_of_type(t) else { return false };
        if !self.typed_data_class(c) {
            return false;
        }
        let the_name = self.name(self.program.class(c).name);
        if !the_name.ends_with("List") {
            return false;
        }
        if the_name == "Float32List" || the_name == "Float64List" {
            return true;
        }
        FIXED_INTEGERS.contains(&the_name.replace("List", "").as_str())
    }

    /// `_primitiveNativeType`.
    fn primitive(&self, t: TypeId) -> Primitive {
        let Some(c) = self.class_of_type(t) else { return Primitive::None_ };
        if !self.of_ffi(c) {
            return Primitive::None_;
        }
        match self.name(self.program.class(c).name) {
            "Int8" | "Int16" | "Int32" | "Int64" | "Uint8" | "Uint16" | "Uint32" | "Uint64" | "IntPtr" => Primitive::Int,
            "Float" | "Double" => Primitive::Double,
            "Bool" => Primitive::Bool,
            "Void" => Primitive::Void,
            "Handle" => Primitive::Handle,
            _ => Primitive::None_,
        }
    }

    /// `arrayDimensions`.
    fn array_dimensions(&self, t: TypeId) -> usize {
        let mut current = t;
        let mut n = 0;
        while self.is_ffi_t(current, "Array") {
            n += 1;
            match self.arguments_of_type(current).as_slice() {
                [x] => current = *x,
                _ => break,
            }
        }
        n
    }

    fn variable_type(&self, v: VariableId) -> TypeId {
        self.outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred)).unwrap_or(self.core.dynamic_)
    }

    /// The types of the class fields (`InterfaceElement.fields`: the declared
    /// ones and the synthetic ones of explicit accessors).
    fn field_types(&self, c: ClassId) -> Vec<TypeId> {
        let k = self.program.class(c);
        let mut v = Vec::new();
        let mut names: HashSet<SymbolId> = HashSet::new();
        for &f in &k.fields {
            names.insert(self.program.variable(f).name);
            v.push(self.variable_type(f));
        }
        let members = || k.instance_members.values().chain(k.static_members.values()).copied();
        for f in members() {
            let g = self.program.function(f);
            if g.variable.is_none() && g.kind == FunctionKind::Getter && names.insert(g.name) {
                v.push(self.outline.functions.get(f.0 as usize).map_or(self.core.dynamic_, |d| d.return_type));
            }
        }
        for f in members() {
            let g = self.program.function(f);
            if g.variable.is_none() && g.kind == FunctionKind::Setter && names.insert(g.name) {
                let t = self.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.first()).map_or(self.core.dynamic_, |p| p.ty);
                v.push(t);
            }
        }
        v
    }

    /// `isEmptyStruct`.
    fn empty_struct(&self, c: ClassId) -> bool {
        for t in self.field_types(c) {
            if self.is_int(t)
                || self.is_double(t)
                || self.is_bool(t)
                || self.is_ffi_t(t, "Pointer")
                || self.compound_subtype(t)
                || self.is_ffi_t(t, "Array")
            {
                return false;
            }
        }
        true
    }

    /// `_isSized`.
    fn sized(&self, t: TypeId) -> bool {
        match self.primitive(t) {
            Primitive::Double | Primitive::Int | Primitive::Bool => true,
            Primitive::Void | Primitive::Handle => false,
            Primitive::None_ => {
                self.compound_subtype(t) || self.is_ffi_t(t, "Pointer") || self.is_ffi_t(t, "Array") || self.abi_subtype(t)
            }
        }
    }

    /// `flattenVarArgs`.
    fn flatten(&self, v: &[TypeId]) -> Vec<TypeId> {
        let Some(&last) = v.last() else { return Vec::new() };
        if !self.is_ffi_t(last, "VarArgs") {
            return v.to_vec();
        }
        let args = self.arguments_of_type(last);
        let [r] = args.as_slice() else { return v.to_vec() };
        match self.table_ref().get(*r) {
            Type::Record { positional, named, .. } if named.is_empty() => {
                let mut s = v[..v.len() - 1].to_vec();
                s.extend(positional.iter().copied());
                s
            }
            _ => v.to_vec(),
        }
    }

    /// `_isValidFfiNativeFunctionType`.
    fn valid_native_function(&self, t: TypeId) -> bool {
        let Type::Function { positional, optional, named, ret, .. } = self.ty(t) else { return false };
        if !named.is_empty() || !optional.is_empty() {
            return false;
        }
        if !self.valid_native(ret, Allows { void_: true, handle: true, ..Allows::default() }) {
            return false;
        }
        self.flatten(&positional).into_iter().all(|x| self.valid_native(x, Allows { handle: true, ..Allows::default() }))
    }

    /// `_isValidFfiNativeType`.
    fn valid_native(&self, t: TypeId, p: Allows) -> bool {
        if matches!(self.table_ref().get(t), Type::Function { .. }) {
            return self.valid_native_function(t);
        }
        if !self.is_interface(t) {
            return false;
        }
        match self.primitive(t) {
            Primitive::Void => return p.void_,
            Primitive::Handle => return p.handle,
            Primitive::Double | Primitive::Int | Primitive::Bool => return true,
            Primitive::None_ => {}
        }
        let args = self.arguments_of_type(t);
        if self.is_ffi_t(t, "NativeFunction") {
            return matches!(args.as_slice(), [x] if self.valid_native_function(*x));
        }
        if self.is_ffi_t(t, "Pointer") {
            let [x] = args.as_slice() else { return false };
            let everything = Allows { void_: true, empty_struct: true, handle: true, opaque: true, array: false };
            return self.valid_native(*x, everything) || self.compound_subtype(*x) || self.is_ffi_t(*x, "NativeType");
        }
        if self.compound_subtype(t) {
            if !p.empty_struct
                && let Some(c) = self.class_of_type(t)
                && self.empty_struct(c)
            {
                return false;
            }
            return true;
        }
        if self.is_ffi_t(t, "Opaque") {
            return p.opaque;
        }
        if self.opaque_subtype(t) || self.abi_subtype(t) {
            return true;
        }
        if p.array && self.is_ffi_t(t, "Array") {
            return matches!(args.as_slice(), [x] if self.valid_native(*x, Allows::default()));
        }
        false
    }

    /// `_extendsNativeFieldWrapperClass1`: the text of the type and supertypes.
    fn extends_nfwc1(&self, t: TypeId) -> bool {
        if self.table_ref().format_without_alias(t, self.interner, self.program) == "NativeFieldWrapperClass1" {
            return true;
        }
        let start = self.class_of_type(t).and_then(|c| self.program.class(c).supertype_class);
        self.chain_has_nfwc1(start)
    }

    fn chain_has_nfwc1(&self, mut current: Option<ClassId>) -> bool {
        let mut steps = 0;
        while let Some(c) = current {
            steps += 1;
            if steps > 256 {
                break;
            }
            let k = self.program.class(c);
            if self.name(k.name) == "NativeFieldWrapperClass1" && k.type_params.is_empty() {
                return true;
            }
            current = k.supertype_class;
        }
        false
    }

    /// `_extendsNativeFieldWrapperClass1(cls.thisType)`.
    fn class_extends_nfwc1(&self, c: ClassId) -> bool {
        let k = self.program.class(c);
        (self.name(k.name) == "NativeFieldWrapperClass1" && k.type_params.is_empty()) || self.chain_has_nfwc1(k.supertype_class)
    }

    /// `_isValidTypedData`.
    fn valid_typed_data(&self, native: TypeId, dart: TypeId) -> bool {
        if !self.is_ffi_t(native, "Pointer") {
            return false;
        }
        let args = self.arguments_of_type(native);
        let [element] = args.as_slice() else { return false };
        let element_name = match self.table_ref().get(*element) {
            Type::TypeParameter { param, .. } => Some(self.name(self.table_ref().param(*param).name)),
            _ => self.class_of_type(*element).map(|c| self.name(self.program.class(c).name)),
        };
        let Some(dc) = self.class_of_type(dart) else { return false };
        if !self.typed_data_class(dc) {
            return false;
        }
        let dart_name = self.name(self.program.class(dc).name);
        match element_name {
            Some("Float") => dart_name == "Float32List",
            Some("Double") => dart_name == "Float64List",
            Some(n) => FIXED_INTEGERS.contains(&n) && dart_name == format!("{n}List"),
            None => false,
        }
    }

    /// `_validateCompatibleNativeType`.
    fn compat_native(&mut self, dir: Direction, dart: TypeId, native: TypeId, nfw: bool, functions: bool) -> bool {
        let prim = self.primitive(native);
        let super_abi = matches!(self.table_ref().get(native), Type::Interface { .. })
            && self.superclass(native).is_some_and(|s| self.name(self.program.class(s).name) == "AbiSpecificInteger");
        if prim == Primitive::Int || super_abi {
            return self.is_int(dart);
        }
        if prim == Primitive::Double {
            return self.is_double(dart);
        }
        if prim == Primitive::Bool {
            return self.is_bool(dart);
        }
        if prim == Primitive::Void {
            return dir == Direction::DartToNative || self.is_void(dart);
        }
        if self.is_void(dart) {
            return false;
        }
        if prim == Primitive::Handle {
            return match dir {
                Direction::DartToNative => true,
                Direction::NativeToDart => {
                    let object = self.core.object;
                    self.sub(object, dart)
                }
            };
        }
        if self.is_interface(dart) && self.is_interface(native) {
            if nfw && self.extends_nfwc1(dart) {
                return self.is_ffi_t(native, "Pointer") && matches!(self.arguments_of_type(native).as_slice(), [x] if self.primitive(*x) == Primitive::Void);
            }
            if self.valid_typed_data(native, dart) {
                return true;
            }
            return match dir {
                Direction::DartToNative => self.sub(dart, native),
                Direction::NativeToDart => self.sub(native, dart),
            };
        }
        if functions && matches!(self.table_ref().get(dart), Type::Function { .. }) && self.is_ffi_t(native, "NativeFunction") {
            let Some(&f) = self.arguments_of_type(native).first() else { return false };
            return self.compat_functions(dir, dart, f, nfw, false);
        }
        false
    }

    /// `_validateCompatibleFunctionTypes`.
    fn compat_functions(&mut self, dir: Direction, dart: TypeId, native: TypeId, nfw: bool, permissive: bool) -> bool {
        let (
            Type::Function { type_params: tpd, ret: rd, positional: pd, optional: od, named: nd, .. },
            Type::Function { type_params: tpn, ret: rn, positional: pn, optional: on, named: nn, .. },
        ) = (self.ty(dart), self.ty(native))
        else {
            return false;
        };
        let pn = self.flatten(&pn);
        if pd.len() != pn.len() {
            return false;
        }
        if !tpd.is_empty() || !tpn.is_empty() {
            return false;
        }
        if !nd.is_empty() || !od.is_empty() || !nn.is_empty() || !on.is_empty() {
            return false;
        }
        if permissive {
            if !(self.compat_native(Direction::NativeToDart, rd, rn, false, false) || self.compat_native(Direction::DartToNative, rd, rn, false, false)) {
                return false;
            }
        } else if !self.compat_native(dir, rd, rn, false, false) {
            return false;
        }
        for i in 0..pd.len() {
            if !self.compat_native(dir.inverse(), pd[i], pn[i], nfw, false) {
                return false;
            }
        }
        true
    }

    /// `isAssignableTo` without `strict-casts`.
    fn assignable(&mut self, of: TypeId, for_: TypeId) -> bool {
        if self.sub(of, for_) || self.table_ref().is_invalid(of) {
            return true;
        }
        // The tear-off of `call`.
        if let Type::Interface { nullable: false, .. } = self.table_ref().get(of)
            && self.accepts_function(for_)
            && let Some(the_call) = self.call_type(of)
            && self.assignable(the_call, for_)
        {
            return true;
        }
        matches!(self.table_ref().get(of), Type::Dynamic)
    }

    /// `acceptsFunctionType`.
    fn accepts_function(&self, t: TypeId) -> bool {
        match self.table_ref().get(t) {
            Type::FutureOr { arg, .. } => self.accepts_function(*arg),
            Type::Function { .. } => true,
            Type::Interface { class, .. } => Some(*class) == self.core.function_class,
            _ => false,
        }
    }

    /// `getCallMethodType`: the interface's `call` method, instantiated.
    fn call_type(&mut self, t: TypeId) -> Option<TypeId> {
        let Type::Interface { class, args, .. } = self.ty(t) else { return None };
        let call = self.interner.lookup("call")?;
        // The class and the superclasses, in lookup order.
        let mut owner = Some(class);
        let mut finding = None;
        while let Some(k) = owner {
            if let Some(&f) = self.program.class(k).instance_members.get(&call)
                && self.program.function(f).kind == FunctionKind::Function
            {
                finding = Some((k, f));
                break;
            }
            owner = self.program.class(k).supertype_class;
        }
        let (k, f) = finding?;
        let signature = self.outline.functions.get(f.0 as usize)?.signature;
        // The parameters of `k` seen from `class`, then those of `class` by the arguments.
        let class_formals = self.outline.classes.get(class.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let mut map_class: HashMap<crate::table::TypeParamId, TypeId> = HashMap::new();
        for (p, a) in class_formals.iter().zip(args.iter()) {
            map_class.insert(*p, *a);
        }
        let mut sig = signature;
        if k != class {
            let seen = self.outline.hierarchy.get(class).and_then(|h| h.supertypes.get(&k).copied())?;
            let arguments_k = self.arguments_of_type(seen);
            let formals_k = self.outline.classes.get(k.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
            let map_k: HashMap<crate::table::TypeParamId, TypeId> = formals_k.iter().copied().zip(arguments_k).collect();
            sig = crate::ops::substitute(sig, &map_k, self.table());
        }
        Some(crate::ops::substitute(sig, &map_class, self.table()))
    }

    // -- Annotations ----------------------------------------------------------

    /// `Annotation.element` of annotation `m`, written in unit `u`.
    fn annotation_target(&self, u: UnitId, m: &ast::Annotation) -> AnnotationTarget {
        let a = &self.program.unit(u).ast;
        let Some(first) = m.name.first() else { return AnnotationTarget::Other };
        if crate::annotations::is_shadowed(a, self.interner, m.span.start, first.sym) {
            return AnnotationTarget::Other;
        }
        let p = self.program;
        let (tgt, mem) = match &m.name[..] {
            [c] => (p.lookup_in_unit(u, c.sym).and_then(|b| b.getter), None),
            [x, y] => match p.lookup_in_unit(u, x.sym).and_then(|b| b.getter) {
                Some(el @ (Element::Class(_) | Element::Typedef(_))) => (Some(el), Some(y.sym)),
                _ => (p.lookup_prefixed_in_unit(u, x.sym, y.sym).and_then(|b| b.getter), None),
            },
            [x, c, n] => (p.lookup_prefixed_in_unit(u, x.sym, c.sym).and_then(|b| b.getter), Some(n.sym)),
            _ => (None, None),
        };
        // `@DWORD()` with `typedef DWORD = Uint32;`: the annotation's element is
        // the constructor of the class the alias names.
        let tgt = match tgt {
            Some(Element::Typedef(t)) if m.arguments.is_some() || mem.is_some() => self
                .outline
                .typedefs
                .get(t.0 as usize)
                .and_then(|d| match self.table_ref().get(d.target_type) {
                    Type::Interface { class, .. } => Some(Element::Class(*class)),
                    _ => None,
                })
                .or(tgt),
            other => other,
        };
        match (tgt, m.arguments.is_some(), mem) {
            (Some(Element::Class(c)), true, _) => {
                let key = mem.or_else(|| self.interner.lookup(""));
                match key.and_then(|k| p.class(c).constructors.get(&k)) {
                    Some(&f) => AnnotationTarget::Constructor(c, f),
                    None => AnnotationTarget::Other,
                }
            }
            (Some(Element::Class(c)), false, None) => AnnotationTarget::Class(c),
            (Some(Element::Class(c)), false, Some(n)) => {
                let k = p.class(c);
                match k.enum_constants.iter().chain(k.fields.iter()).copied().find(|v| p.variable(*v).name == n && p.variable(*v).static_) {
                    Some(v) => AnnotationTarget::Variable(v),
                    None => AnnotationTarget::Other,
                }
            }
            (Some(Element::Variable(v)), false, None) => AnnotationTarget::Variable(v),
            (Some(Element::Function(g)), false, None) => match p.function(g).variable {
                Some(v) => AnnotationTarget::Variable(v),
                None => AnnotationTarget::Other,
            },
            _ => AnnotationTarget::Other,
        }
    }

    /// `annotation.element` is a constructor of the `dart:ffi` class `the_name`
    /// (`isArray`, `isPacked`, `isAbiSpecificIntegerMapping`).
    fn ffi_constructor(&self, u: UnitId, m: &ast::Annotation, the_name: &str) -> bool {
        matches!(self.annotation_target(u, m), AnnotationTarget::Constructor(c, _) if self.ffi_class(c, the_name))
    }

    /// `annotation.element.ffiClass`.
    fn annotation_ffi_class(&self, u: UnitId, m: &ast::Annotation) -> Option<ClassId> {
        match self.annotation_target(u, m) {
            AnnotationTarget::Constructor(c, _) | AnnotationTarget::Class(c) if self.of_ffi(c) => Some(c),
            _ => None,
        }
    }

    /// `computeConstantValue` of the annotation.
    fn annotation_instance(&mut self, u: UnitId, m: &'a ast::Annotation) -> Option<Instance> {
        match self.annotation_target(u, m) {
            AnnotationTarget::Constructor(c, f) => {
                let args = m.arguments.as_ref()?;
                self.assemble(u, c, f, args)
            }
            AnnotationTarget::Variable(v) => self.variable_instance(v),
            _ => None,
        }
    }

    /// The value of a constant variable used as an annotation: the creation in
    /// the initializer, or the value the engine computes.
    fn variable_instance(&mut self, v: VariableId) -> Option<Instance> {
        let x = self.program.variable(v);
        if !x.const_ {
            return None;
        }
        let (unit_id, initial) = match x.node {
            VariableRef::TopLevel { unit, decl, index } => {
                let DeclKind::Variables(l) = &self.program.unit(unit).ast.decls[decl.0 as usize].kind else { return None };
                (unit, l.variables.get(index)?.initializer?)
            }
            VariableRef::Field { unit, member, index } => {
                let MemberKind::Field(l) = &self.program.unit(unit).ast.member(member).kind else { return None };
                (unit, l.variables.get(index)?.initializer?)
            }
            _ => return None,
        };
        let a = &self.program.unit(unit_id).ast;
        let args = match &a.expr(initial).kind {
            ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => Some(&**arguments),
            _ => None,
        };
        let resolved = self.bodies.units.get(unit_id.0 as usize).and_then(|b| b.get_resolved(initial));
        if let (Some(args), Some(Resolved::Constructor(f))) = (args, resolved)
            && let Some(c) = self.program.function(*f).class
        {
            return self.assemble(unit_id, c, *f, args);
        }
        let the_value = match self.engine().variable_value(v)? {
            Constant::Value(x) => x,
            Constant::Invalid(_) => return None,
        };
        let cls = self.class_of_type(the_value.ty)?;
        Some(Instance {
            class: cls,
            ty: the_value.ty,
            is_leaf: None,
            alignment: None,
            list: None,
            dimensions: [None; 5],
            variable: None,
            map: None,
        })
    }

    /// The object that constructor `f` of class `c` builds with the arguments
    /// `args` (written in unit `u`).
    fn assemble(&mut self, u: UnitId, c: ClassId, f: FunctionElementId, args: &'a ast::Arguments) -> Option<Instance> {
        let n = self.program.class(c).type_params.len();
        let type_arguments = match self.bodies.units.get(u.0 as usize).and_then(|b| b.instantiation(args.span.start)) {
            Some(v) if v.len() == n => v.to_vec(),
            _ => vec![self.core.dynamic_; n],
        };
        let typ = self.table().intern(Type::Interface { class: c, args: type_arguments.into_boxed_slice(), nullable: false });
        let mut i = Instance {
            class: c,
            ty: typ,
            is_leaf: None,
            alignment: None,
            list: None,
            dimensions: [None; 5],
            variable: None,
            map: None,
        };
        if !self.of_ffi(c) {
            return Some(i);
        }
        // The arguments, evaluated (one that is not constant leaves the annotation
        // without a value).
        let lib = self.program.unit(u).library;
        let cx = Ctx::simple(u, lib);
        let mut positional_args: Vec<Value> = Vec::new();
        let mut named_args: Vec<(SymbolId, Value)> = Vec::new();
        for x in args.args.iter() {
            let Constant::Value(v) = self.engine().evaluate(&cx, x.value, true) else { return None };
            match x.name {
                Some(n) => named_args.push((n.sym, v)),
                None => positional_args.push(v),
            }
        }
        let integer = |v: &Value| match &v.state {
            InstanceState::Int(x) => *x,
            _ => None,
        };
        let list = |v: &Value| match &v.state {
            InstanceState::List { elements: elems, unknown: false, .. } => Some(elems.iter().filter_map(integer).collect::<Vec<i64>>()),
            _ => None,
        };
        let cls = self.name(self.program.class(c).name);
        let ctor = self.name(self.program.function(f).name);
        match (cls, ctor) {
            ("Native", "") => {
                i.is_leaf = match named_args.iter().find(|(n, _)| self.name(*n) == "isLeaf") {
                    Some((_, v)) => match &v.state {
                        InstanceState::Bool(b) => *b,
                        _ => None,
                    },
                    None => Some(false),
                };
            }
            ("Packed", "") => i.alignment = positional_args.first().and_then(integer),
            ("Array", "") => {
                for k in 0..5 {
                    i.dimensions[k] = positional_args.get(k).and_then(integer);
                }
                i.variable = Some(false);
            }
            ("Array", "multi") => {
                i.list = positional_args.first().and_then(list);
                i.variable = Some(false);
            }
            ("Array", "variable") => {
                i.dimensions[0] = Some(0);
                for k in 0..4 {
                    i.dimensions[k + 1] = positional_args.get(k).and_then(integer);
                }
                i.variable = Some(true);
            }
            ("Array", "variableMulti") => {
                i.list = positional_args.first().and_then(list);
                i.variable = Some(true);
            }
            ("AbiSpecificIntegerMapping", "") => {
                i.map = positional_args.first().and_then(|v| match &v.state {
                    InstanceState::Map { entries, unknown: false } => Some(entries.iter().map(|(_, x)| x.ty).collect()),
                    _ => None,
                });
            }
            _ => {}
        }
        Some(i)
    }

    /// The annotations of a function or variable declaration, with the unit.
    fn element_annotations(&self, d: Declaration) -> (UnitId, &'a [ast::Annotation]) {
        let p = self.program;
        match d {
            Declaration::Function(f) => {
                let unit_id = match p.function(f).node {
                    FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => unit,
                    FunctionRef::None => return (self.u, &[]),
                };
                (unit_id, crate::phase_result::function_annotations(p, f))
            }
            Declaration::Variable(v) => match p.variable(v).node {
                VariableRef::TopLevel { unit, decl, .. } => (unit, &p.unit(unit).ast.decls[decl.0 as usize].metadata[..]),
                VariableRef::Field { unit, member, .. } => (unit, &p.unit(unit).ast.member(member).metadata[..]),
                _ => (self.u, &[]),
            },
            Declaration::Local(_) => (self.u, &[]),
        }
    }

    // -- Constantes ------------------------------------------------------------

    /// The constant reference of an identifier (`staticElement is
    /// ConstVariableElement`, directly or through the accessor's variable).
    fn constant_reference(&mut self, e: ExprId) -> Option<ConstantRef> {
        let a = self.a;
        match &a.expr(e).kind {
            ExprKind::Identifier(_) => {}
            ExprKind::Property { target, .. } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) => {}
            _ => return None,
        }
        let r = self.body.get_resolved(e)?.clone();
        let p = self.program;
        let of_field = |v: VariableId| {
            let x = p.variable(v);
            if x.const_ {
                return true;
            }
            // `constFieldsForFinalInstance`: a `final` instance field with an
            // initializer, of a class or enum.
            let of_class = x.class.is_some_and(|c| matches!(p.class(c).kind, ClassKind::Class | ClassKind::Enum));
            let has_initializer = match x.node {
                VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Field(l) => l.variables.get(index).is_some_and(|y| y.initializer.is_some()),
                    _ => false,
                },
                _ => false,
            };
            x.final_ && !x.static_ && of_class && has_initializer
        };
        match r {
            Resolved::Local(_) => {
                let u = self.u;
                self.engine().constant_local(u, e).then_some(ConstantRef::Local)
            }
            Resolved::Parameter { name, .. } => {
                let par = self.declared_parameter(a.expr(e).span.start, name)?;
                (par.kind != ParameterKind::Required).then_some(ConstantRef::Parameter(par.default_value))
            }
            Resolved::Element(Element::Variable(v)) => p.variable(v).const_.then_some(ConstantRef::Variable(v)),
            Resolved::Element(Element::Function(g)) => {
                let v = p.function(g).variable?;
                p.variable(v).const_.then_some(ConstantRef::Variable(v))
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } => of_field(v).then_some(ConstantRef::Variable(v)),
            Resolved::Member { member: MemberRef::Function(g), .. } | Resolved::ExtensionMember { member: g, .. } => {
                let v = p.function(g).variable?;
                of_field(v).then_some(ConstantRef::Variable(v))
            }
            _ => None,
        }
    }

    /// The parameter `the_name` of the innermost function that contains `pos` and
    /// declares it.
    fn declared_parameter(&self, pos: usize, the_name: SymbolId) -> Option<&'a ast::Parameter> {
        let a = self.a;
        let mut best: Option<(usize, &'a ast::Parameter)> = None;
        let mut consider = |span: Span, ps: &'a [ast::Parameter]| {
            if span.start <= pos
                && pos < span.end
                && let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == the_name))
                && best.is_none_or(|(t, _)| span.end - span.start < t)
            {
                best = Some((span.end - span.start, p));
            }
        };
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                consider(f.span, ps);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                consider(m.span, &k.parameters);
            }
        }
        best.map(|(_, p)| p)
    }

    /// `_isConst`.
    fn is_const(&mut self, e: ExprId) -> bool {
        match &self.a.expr(e).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Symbol(_)
            | ExprKind::List { .. }
            | ExprKind::SetOrMap { .. }
            | ExprKind::Record { .. } => true,
            _ => self.constant_reference(e).is_some(),
        }
    }

    /// `_maybeGetBoolConstValue`.
    fn constant_bool(&mut self, e: ExprId) -> Option<bool> {
        if let ExprKind::Bool(b) = self.a.expr(e).kind {
            return Some(b);
        }
        let u = self.u;
        let lib = self.program.unit(u).library;
        let r = match self.constant_reference(e)? {
            ConstantRef::Variable(v) => self.engine().variable_value(v)?,
            ConstantRef::Local => self.engine().evaluate(&Ctx::simple(u, lib), e, true),
            ConstantRef::Parameter(Some(d)) => self.engine().default_result(u, lib, d),
            ConstantRef::Parameter(None) => return None,
        };
        match r {
            Constant::Value(v) => match v.state {
                InstanceState::Bool(b) => b,
                _ => None,
            },
            Constant::Invalid(_) => None,
        }
    }

    /// `_isLeaf`.
    fn the_is_leaf(&mut self, args: &'a ast::Arguments) -> bool {
        for x in args.args.iter() {
            if x.name.is_some_and(|n| self.name(n.sym) == "isLeaf") {
                return self.constant_bool(x.value).unwrap_or(false);
            }
        }
        false
    }

    /// `_validateIsLeafIsConst`.
    fn validate_constant_leaf(&mut self, args: &'a ast::Arguments) {
        for x in args.args.iter() {
            if x.name.is_some_and(|n| self.name(n.sym) == "isLeaf") && !self.is_const(x.value) {
                let s = self.span(x.value);
                self.report(cf::ARGUMENT_MUST_BE_A_CONSTANT, s, &[Arg::from("isLeaf")]);
            }
        }
    }

    // -- Declarations ----------------------------------------------------------

    fn declarations(&mut self) {
        let a = self.a;
        let u = self.u;
        let program = self.program;
        for d in program.unit(u).unit.directives.iter() {
            if let DirectiveKind::Library { .. } = d.kind {
                self.default_active(&d.metadata);
            }
        }
        for (di, d) in a.decls.iter().enumerate() {
            match &d.kind {
                DeclKind::Class(x) => {
                    if !x.mixin_application {
                        self.class(di, d, x);
                    }
                }
                DeclKind::Mixin(x) => self.members(&x.members, None),
                DeclKind::Enum(x) => self.members(&x.members, None),
                DeclKind::ExtensionType(x) => self.members(&x.members, None),
                DeclKind::Extension(x) => self.members(&x.members, None),
                DeclKind::Function(fid) => {
                    let f = a.function(*fid);
                    if let Some(n) = f.name {
                        self.check_native(n.span, No::Function(*fid), &d.metadata, f.parameters.as_deref().unwrap_or(&[]), f.external);
                    }
                }
                DeclKind::Variables(l) => {
                    for (i, v) in l.variables.iter().enumerate() {
                        self.check_native(v.name.span, No::TopLevelVariable(ast::DeclId(di as u32), i), &d.metadata, &[], l.external);
                    }
                }
                DeclKind::Typedef(_) => {}
            }
        }
        // The local functions (`FunctionDeclarationStatement`).
        for (si, s) in a.stmts.iter().enumerate() {
            let StmtKind::Function(fid) = &s.kind else { continue };
            let Some((_, metadata)) = a.local_metadata.iter().find(|(id, _)| id.0 as usize == si) else { continue };
            let f = a.function(*fid);
            let Some(n) = f.name else { continue };
            if self.inside_index(f.span) {
                continue;
            }
            let Some(t) = self.body.local_type(n.span.start) else { continue };
            self.check_native(n.span, No::Local(t), metadata, f.parameters.as_deref().unwrap_or(&[]), f.external);
        }
    }

    /// `visitLibraryDirective`: at most one `@DefaultAsset`.
    fn default_active(&mut self, metadata: &'a [ast::Annotation]) {
        let mut happened = false;
        for m in metadata {
            let Some(inst) = self.annotation_instance(self.u, m) else { continue };
            if !self.ffi_class(inst.class, "DefaultAsset") {
                continue;
            }
            if happened {
                let s = self.annotation_name(m);
                self.report(cf::FFI_NATIVE_INVALID_DUPLICATE_DEFAULT_ASSET, s, &[]);
            }
            happened = true;
        }
    }

    /// The element of the `NamedType` `t` written in the header of a class with
    /// the type parameters `tps`.
    fn named_type_element(&self, t: ast::TypeId, tps: &[ast::TypeParameter]) -> Option<Element> {
        let TypeKind::Named { name, .. } = &self.a.ty(t).kind else { return None };
        match &name[..] {
            [n] => {
                if tps.iter().any(|p| p.name.sym == n.sym) {
                    return None;
                }
                self.program.lookup_in_unit(self.u, n.sym).and_then(|b| b.getter)
            }
            [p, n] => self.program.lookup_prefixed_in_unit(self.u, p.sym, n.sym).and_then(|b| b.getter),
            _ => None,
        }
    }

    /// `name2.lexeme` of the `NamedType`.
    fn lexeme(&self, t: ast::TypeId) -> &'a str {
        match &self.a.ty(t).kind {
            TypeKind::Named { name, .. } => name.last().map_or("", |n| self.name(n.sym)),
            _ => "",
        }
    }

    /// `ClassElement` (class or class alias; not a mixin, enum or extension
    /// type).
    fn is_class_element(&self, c: ClassId) -> bool {
        matches!(self.program.class(c).kind, ClassKind::Class | ClassKind::MixinApplication)
    }

    /// `allSupertypes` (without the class itself).
    fn supertypes(&self, c: ClassId) -> Vec<ClassId> {
        self.outline.hierarchy.get(c).map(|h| h.supertypes.keys().copied().filter(|k| *k != c).collect()).unwrap_or_default()
    }

    /// `NamedType.isCompoundSubtype`.
    fn compound_named(&self, c: ClassId) -> bool {
        self.is_class_element(c) && self.supertypes(c).into_iter().any(|k| self.compound(k))
    }

    /// `NamedType.isAbiSpecificIntegerSubtype`.
    fn abi_named(&self, c: ClassId) -> bool {
        self.is_class_element(c) && self.supertypes(c).into_iter().any(|k| self.ffi_class(k, "AbiSpecificInteger"))
    }

    /// `visitClassDeclaration`.
    fn class(&mut self, di: usize, d: &'a ast::Decl, x: &'a ast::ClassDecl) {
        let a = self.a;
        let mut compound: Option<(ClassId, &'a str)> = None;
        let c = if x.extends.is_some() || !x.implements.is_empty() || !x.with.is_empty() {
            self.indices().classes.get(&(di as u32)).copied()
        } else {
            None
        };
        if let (Some(c), Some(sup)) = (c, x.extends) {
            let lexeme = self.lexeme(sup);
            let class_name = self.name(x.name.sym);
            match self.named_type_element(sup, &x.type_params) {
                Some(Element::Class(sc)) if self.is_class_element(sc) && self.of_ffi(sc) => {
                    let the_name = self.name(self.program.class(sc).name);
                    if the_name == "Struct" || the_name == "Union" {
                        compound = Some((c, lexeme));
                        if self.empty_struct(c) {
                            self.report(cf::EMPTY_STRUCT, x.name.span, &[Arg::from(class_name), Arg::from(the_name)]);
                        }
                        if the_name == "Struct" {
                            self.validate_packed(&d.metadata);
                        }
                    } else if the_name == "AbiSpecificInteger" {
                        self.validate_abi(x);
                        self.validate_mapping(x.name, &d.metadata);
                    }
                }
                Some(Element::Class(sc)) if self.compound_named(sc) || self.abi_named(sc) => {
                    let s = a.ty(sup).span;
                    self.report(cf::SUBTYPE_OF_STRUCT_CLASS_IN_EXTENDS, s, &[Arg::from(class_name), Arg::from(lexeme)]);
                }
                _ => {}
            }
        }
        for &t in x.implements.iter() {
            self.check_supertype(x, t, cf::SUBTYPE_OF_STRUCT_CLASS_IN_IMPLEMENTS);
        }
        for &t in x.with.iter() {
            self.check_supertype(x, t, cf::SUBTYPE_OF_STRUCT_CLASS_IN_WITH);
        }
        if let Some((c, _)) = compound {
            let class_name = self.name(x.name.sym);
            if !x.type_params.is_empty() {
                self.report(cf::GENERIC_STRUCT_SUBCLASS, x.name.span, &[Arg::from(class_name)]);
            }
            if !x.implements.is_empty()
                && let Some(fin) = self.ffi_class_by_name("Finalizable")
                && self.supertypes(c).contains(&fin)
            {
                self.report(cf::COMPOUND_IMPLEMENTS_FINALIZABLE, x.name.span, &[Arg::from(class_name)]);
            }
        }
        self.class_members(&x.members, compound);
    }

    /// `checkSupertype` of `implements` and `with`.
    fn check_supertype(&mut self, x: &'a ast::ClassDecl, t: ast::TypeId, code: Code) {
        let el = self.named_type_element(t, &x.type_params);
        let the_name = match el {
            Some(Element::Class(c)) => Some(self.name(self.program.class(c).name)),
            Some(Element::Typedef(td)) => Some(self.name(self.program.typedef(td).name)),
            _ => None,
        };
        if matches!(the_name, Some("Allocator" | "Finalizable")) {
            return;
        }
        if let Some(Element::Class(c)) = el
            && (self.compound_named(c) || self.abi_named(c))
        {
            let s = self.a.ty(t).span;
            let class_name = self.name(x.name.sym);
            let lexeme = self.lexeme(t);
            self.report(code, s, &[Arg::from(class_name), Arg::from(lexeme)]);
        }
    }

    fn members(&mut self, members: &'a [ast::MemberId], compound: Option<(ClassId, &'a str)>) {
        self.class_members(members, compound);
    }

    fn class_members(&mut self, members: &'a [ast::MemberId], compound: Option<(ClassId, &'a str)>) {
        let a = self.a;
        for &mid in members {
            let m = a.member(mid);
            match &m.kind {
                MemberKind::Field(l) => {
                    if let Some((c, sup)) = compound {
                        self.fields_in_compound(mid, m, l, c, sup, members);
                    }
                    for (i, v) in l.variables.iter().enumerate() {
                        self.check_native(v.name.span, No::Field(mid, i), &m.metadata, &[], l.external);
                    }
                }
                MemberKind::Method(fid) => {
                    let f = a.function(*fid);
                    if let Some(n) = f.name {
                        self.check_native(n.span, No::Function(*fid), &m.metadata, f.parameters.as_deref().unwrap_or(&[]), f.external);
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
    }

    /// `_validatePackedAnnotation`.
    fn validate_packed(&mut self, metadata: &'a [ast::Annotation]) {
        let u = self.u;
        let packed: Vec<&'a ast::Annotation> = metadata.iter().filter(|m| self.ffi_constructor(u, m, "Packed")).collect();
        let Some(&the_first) = packed.first() else { return };
        for m in &packed[1..] {
            self.report(cf::PACKED_ANNOTATION, m.span, &[]);
        }
        let the_value = self.annotation_instance(u, the_first).and_then(|i| i.alignment);
        if !matches!(the_value, Some(1 | 2 | 4 | 8 | 16)) {
            let s = the_first.arguments.as_ref().and_then(|x| x.args.first()).map_or(the_first.span, |x| self.argument_span(x));
            self.report(cf::PACKED_ANNOTATION_ALIGNMENT, s, &[]);
        }
    }

    /// `_validateAbiSpecificIntegerAnnotation`.
    fn validate_abi(&mut self, x: &'a ast::ClassDecl) {
        let the_valid = x.type_params.is_empty()
            && x.members.len() == 1
            && matches!(&self.a.member(x.members[0]).kind, MemberKind::Constructor(k) if k.const_);
        if !the_valid {
            self.report(cf::ABI_SPECIFIC_INTEGER_INVALID, x.name.span, &[]);
        }
    }

    /// `_validateAbiSpecificIntegerMappingAnnotation`.
    fn validate_mapping(&mut self, the_name: ast::Name, metadata: &'a [ast::Annotation]) {
        let u = self.u;
        let maps: Vec<&'a ast::Annotation> = metadata.iter().filter(|m| self.ffi_constructor(u, m, "AbiSpecificIntegerMapping")).collect();
        let Some(&the_first) = maps.first() else {
            self.report(cf::ABI_SPECIFIC_INTEGER_MAPPING_MISSING, the_name.span, &[]);
            return;
        };
        for m in &maps[1..] {
            let s = self.annotation_name(m);
            self.report(cf::ABI_SPECIFIC_INTEGER_MAPPING_EXTRA, s, &[]);
        }
        let Some(args) = &the_first.arguments else { return };
        for x in args.args.iter() {
            if x.name.is_some() {
                continue;
            }
            let ExprKind::SetOrMap { elements, .. } = &self.a.expr(x.value).kind else { continue };
            for el in elements.iter() {
                let CollectionElement::MapEntry { value, .. } = el else { continue };
                let Some(t) = self.body.get_type(*value) else { continue };
                if !self.is_interface(t) {
                    continue;
                }
                let Some(c) = self.class_of_type(t) else { continue };
                let n = self.name(self.program.class(c).name);
                if !FIXED_INTEGERS.contains(&n) {
                    let s = self.span(*value);
                    self.report(cf::ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED, s, &[Arg::from(n)]);
                }
            }
            return;
        }
        let Some(values) = self.annotation_instance(u, the_first).and_then(|i| i.map) else { return };
        let Some(first) = args.args.first() else { return };
        let s = self.argument_span(first);
        for t in values {
            let Some(c) = self.class_of_type(t) else { continue };
            let n = self.name(self.program.class(c).name);
            if !FIXED_INTEGERS.contains(&n) {
                self.report(cf::ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED, s, &[Arg::from(n)]);
            }
        }
    }

    /// `_validateFieldsInCompound`.
    fn fields_in_compound(&mut self, mid: ast::MemberId, m: &'a ast::Member, l: &'a ast::VariableList, c: ClassId, sup: &'a str, members: &'a [ast::MemberId]) {
        if l.static_ {
            return;
        }
        let Some(the_first) = l.variables.first() else { return };
        if !l.external {
            self.report(cf::FIELD_MUST_BE_EXTERNAL_IN_STRUCT, the_first.name.span, &[]);
        }
        let Some(ty) = l.ty else {
            self.report(cf::MISSING_FIELD_TYPE_IN_STRUCT, the_first.name.span, &[]);
            return;
        };
        let type_span = self.a.ty(ty).span;
        let Some(declared) = self.written_type(ty) else { return };
        let (tree, source, interner) = (self.a, self.source, self.interner);
        let type_source = move || dartforge_frontend::source::from_type(tree, source, interner, ty);
        if self.table_ref().get(declared).is_declared_nullable() {
            let text = type_source();
            self.report(cf::INVALID_FIELD_TYPE_IN_STRUCT, type_span, &[Arg::from(text)]);
        } else if self.is_int(declared) {
            self.validate_annotations(type_span, declared, &m.metadata, Primitive::Int, sup);
        } else if self.is_double(declared) {
            self.validate_annotations(type_span, declared, &m.metadata, Primitive::Double, sup);
        } else if self.is_bool(declared) {
            self.validate_annotations(type_span, declared, &m.metadata, Primitive::Bool, sup);
        } else if self.is_ffi_t(declared, "Pointer") {
            self.without_annotations(&m.metadata);
        } else if self.is_ffi_t(declared, "Array") {
            if let [arg] = self.arguments_of_type(declared).as_slice()
                && !self.sized(*arg)
            {
                let s = match &self.a.ty(ty).kind {
                    TypeKind::Named { args, .. } if !args.is_empty() => self.a.ty(args[0]).span,
                    _ => type_span,
                };
                self.report(cf::NON_SIZED_TYPE_ARGUMENT, s, &[Arg::from("Array"), Arg::Type_(*arg)]);
            }
            let dimensions = self.array_dimensions(declared);
            let last = self.is_last_field(c, mid, members);
            self.validate_size(type_span, &m.metadata, dimensions, last);
        } else if self.compound_subtype(declared) {
            if let Some(k) = self.class_of_type(declared)
                && self.empty_struct(k)
            {
                let the_name = self.name(self.program.class(k).name);
                let the_supertype = self
                    .outline
                    .classes
                    .get(k.0 as usize)
                    .and_then(|d| d.supertype)
                    .map(|t| self.table_ref().format_without_alias(t, self.interner, self.program))
                    .unwrap_or_default();
                self.report(cf::EMPTY_STRUCT, m.span, &[Arg::from(the_name), Arg::from(the_supertype)]);
            }
        } else {
            let text = type_source();
            self.report(cf::INVALID_FIELD_TYPE_IN_STRUCT, type_span, &[Arg::from(text)]);
        }
    }

    /// The first field of declaration `mid` is the last external instance field
    /// of the class (`fields.reversed`, with the synthetic ones of the accessors
    /// after the declared ones).
    fn is_last_field(&mut self, c: ClassId, mid: ast::MemberId, members: &'a [ast::MemberId]) -> bool {
        let Some(&first) = self.indices().fields.get(&(mid.0, 0)) else { return false };
        let k = self.program.class(c);
        let mut last: Option<Option<VariableId>> = None;
        let mut the_declared: HashSet<SymbolId> = HashSet::new();
        for &v in &k.fields {
            let x = self.program.variable(v);
            the_declared.insert(x.name);
            if !x.static_ && x.external {
                last = Some(Some(v));
            }
        }
        // The synthetic fields of the explicit accessors, in order.
        let mut synthetic: Vec<(SymbolId, bool, bool)> = Vec::new();
        for &m in members {
            let MemberKind::Method(fid) = &self.a.member(m).kind else { continue };
            let f = self.a.function(*fid);
            if !matches!(f.kind, ast::FunctionKind::Getter | ast::FunctionKind::Setter) {
                continue;
            }
            let Some(n) = f.name else { continue };
            if the_declared.contains(&n.sym) {
                continue;
            }
            match synthetic.iter_mut().find(|(s, _, _)| *s == n.sym) {
                Some(x) => x.2 |= f.external,
                None => synthetic.push((n.sym, f.static_, f.external)),
            }
        }
        if synthetic.iter().any(|(_, static_, external)| !static_ && *external) {
            last = Some(None);
        }
        last == Some(Some(first))
    }

    /// `_validateAnnotations`.
    fn validate_annotations(&mut self, type_span: Span, declared: TypeId, metadata: &'a [ast::Annotation], the_required: Primitive, sup: &'a str) {
        let u = self.u;
        let mut found = false;
        let mut extras: Vec<&'a ast::Annotation> = Vec::new();
        for m in metadata {
            let tgt = self.annotation_target(u, m);
            let (of_ffi, of_abi) = match tgt {
                AnnotationTarget::Constructor(c, _) => {
                    let k = self.program.class(c);
                    let abi = k.kind == ClassKind::Class && k.supertype_class.is_some_and(|s| self.ffi_class(s, "AbiSpecificInteger"));
                    (self.of_ffi(c), abi)
                }
                AnnotationTarget::Class(c) => (self.of_ffi(c), false),
                _ => (false, false),
            };
            if !(of_ffi || of_abi) {
                continue;
            }
            if found {
                extras.push(m);
            } else if self.annotation_type(tgt) == the_required {
                found = true;
            } else {
                extras.push(m);
            }
        }
        if !extras.is_empty() {
            let mut rest = &extras[..];
            if !found {
                self.report(cf::MISMATCHED_ANNOTATION_ON_STRUCT_FIELD, extras[0].span, &[]);
                rest = &extras[1..];
            }
            for m in rest {
                self.report(cf::EXTRA_ANNOTATION_ON_STRUCT_FIELD, m.span, &[]);
            }
        } else if !found {
            self.report(cf::MISSING_ANNOTATION_ON_STRUCT_FIELD, type_span, &[Arg::Type_(declared), Arg::from(sup)]);
        }
    }

    /// `_typeForAnnotation`.
    fn annotation_type(&self, tgt: AnnotationTarget) -> Primitive {
        let AnnotationTarget::Constructor(c, _) = tgt else { return Primitive::None_ };
        let k = self.program.class(c);
        match self.name(k.name) {
            "Int8" | "Int16" | "Int32" | "Int64" | "Uint8" | "Uint16" | "Uint32" | "Uint64" | "IntPtr" => Primitive::Int,
            "Float" | "Double" => Primitive::Double,
            "Bool" => Primitive::Bool,
            _ => {
                if k.supertype_class.is_some_and(|s| self.ffi_class(s, "AbiSpecificInteger")) {
                    Primitive::Int
                } else {
                    Primitive::None_
                }
            }
        }
    }

    /// `_validateNoAnnotations`.
    fn without_annotations(&mut self, metadata: &'a [ast::Annotation]) {
        let u = self.u;
        for m in metadata {
            if self.annotation_ffi_class(u, m).is_some() {
                self.report(cf::ANNOTATION_ON_POINTER_FIELD, m.span, &[]);
            }
        }
    }

    /// `_validateSizeOfAnnotation`.
    fn validate_size(&mut self, error: Span, metadata: &'a [ast::Annotation], expected_dimensions: usize, allows_variable: bool) {
        let u = self.u;
        let arrays: Vec<&'a ast::Annotation> = metadata.iter().filter(|m| self.ffi_constructor(u, m, "Array")).collect();
        let Some(&the_first) = arrays.first() else {
            self.report(cf::MISSING_SIZE_ANNOTATION_CARRAY, error, &[]);
            return;
        };
        for m in &arrays[1..] {
            self.report(cf::EXTRA_SIZE_ANNOTATION_CARRAY, m.span, &[]);
        }
        // `arraySizeDimensions`.
        let (dimensions, variable) = match self.annotation_instance(u, the_first) {
            Some(i) => {
                let variable = i.variable.unwrap_or(false);
                match i.list {
                    Some(l) => {
                        let mut d = Vec::new();
                        if variable {
                            d.push(0);
                        }
                        d.extend(l);
                        (d, variable)
                    }
                    None => (i.dimensions.iter().flatten().copied().collect::<Vec<i64>>(), variable),
                }
            }
            None => (Vec::new(), false),
        };
        if dimensions.len() != expected_dimensions {
            self.report(cf::SIZE_ANNOTATION_DIMENSIONS, the_first.span, &[]);
        }
        if variable && !allows_variable {
            self.report(cf::VARIABLE_LENGTH_ARRAY_NOT_LAST, the_first.span, &[]);
        }
        // The argument nodes: the elements of the single list, or the arguments.
        let mut nodes: Vec<Span> = Vec::new();
        if let Some(args) = &the_first.arguments {
            match &args.args[..] {
                [x] if x.name.is_none() && matches!(self.a.expr(x.value).kind, ExprKind::List { .. }) => {
                    if let ExprKind::List { elements, .. } = &self.a.expr(x.value).kind {
                        for el in elements.iter() {
                            let s = self.engine().element_span(u, el);
                            nodes.push(s);
                        }
                    }
                }
                xs => {
                    for x in xs {
                        nodes.push(self.argument_span(x));
                    }
                }
            }
        }
        // The error node. In 3.6.2 (`ffi_verifier.dart:1995-2022`), argument `i`
        // when there are arguments: `argumentNodes[i]` without a guard, and
        // `@Array.variable(-1)` (dimensions `[0, -1]`, a single argument) throws
        // `RangeError` — the library ends up without diagnostics
        // ([`AnalyzerCrash`]). In 3.13.4 (`:2171-2220`), the last argument
        // that exists up to `i`, and the annotation when there are no arguments.
        let reference_313 = self.program.reference(self.u) == dartforge_diagnostics::AnalyzerReference::V3_13;
        let mut error_node = the_first.span;
        for (i, d) in dimensions.iter().enumerate() {
            if reference_313 && let Some(&s) = nodes.get(i) {
                error_node = s;
            }
            if i == 0 && variable {
                continue;
            }
            if *d <= 0 {
                let s = if reference_313 || nodes.is_empty() {
                    error_node
                } else {
                    match nodes.get(i) {
                        Some(&s) => s,
                        None => {
                            self.the_break = true;
                            return;
                        }
                    }
                };
                self.report(cf::NON_POSITIVE_ARRAY_DIMENSION, s, &[]);
            }
        }
    }

    /// `_checkFfiNative`.
    fn check_native(&mut self, the_name: Span, no: No, metadata: &'a [ast::Annotation], parameters: &'a [ast::Parameter], external: bool) {
        if metadata.is_empty() {
            return;
        }
        let u = self.u;
        let mut happened = false;
        for m in metadata {
            // Only the value of `Native` matters; the rest is not even evaluated.
            let candidate = match self.annotation_target(u, m) {
                AnnotationTarget::Constructor(c, _) => self.ffi_class(c, "Native"),
                AnnotationTarget::Variable(v) => self.program.variable(v).const_,
                _ => false,
            };
            if !candidate {
                continue;
            }
            let Some(inst) = self.annotation_instance(u, m) else { continue };
            if !self.ffi_class(inst.class, "Native") {
                continue;
            }
            if happened {
                let s = self.annotation_name(m);
                self.report(cf::FFI_NATIVE_INVALID_MULTIPLE_ANNOTATIONS, s, &[]);
                break;
            }
            happened = true;
            if !external {
                self.report(cf::FFI_NATIVE_MUST_BE_EXTERNAL, the_name, &[]);
            }
            let Some(decl) = self.declaration(no) else { continue };
            let Some(&signature) = self.arguments_of_type(inst.ty).first() else { continue };
            if matches!(self.table_ref().get(signature), Type::Function { .. }) {
                match decl {
                    Declaration::Function(_) | Declaration::Local(_) => self.check_native_function(the_name, decl, signature, inst.is_leaf, parameters),
                    Declaration::Variable(_) => self.report(cf::NATIVE_FIELD_INVALID_TYPE, the_name, &[Arg::Type_(signature)]),
                }
            } else {
                let func = match decl {
                    Declaration::Function(f) => !matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter),
                    Declaration::Local(_) => true,
                    Declaration::Variable(_) => false,
                };
                if func {
                    self.report(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, the_name, &[Arg::from("T"), Arg::from("Native")]);
                } else {
                    self.check_native_field(the_name, decl, metadata, signature, false);
                }
            }
        }
    }

    /// The type of the i-th formal parameter of the declaration.
    fn parameter_type(&self, d: Declaration, i: usize, parameters: &'a [ast::Parameter]) -> Option<TypeId> {
        match d {
            Declaration::Function(f) => self.outline.functions.get(f.0 as usize).and_then(|x| x.parameters.get(i)).map(|p| p.ty),
            Declaration::Local(t) => {
                let Type::Function { positional, optional, named, .. } = self.ty(t) else { return None };
                let p = parameters.get(i)?;
                match p.kind {
                    ParameterKind::Required => positional.get(parameters[..i].iter().filter(|x| x.kind == ParameterKind::Required).count()).copied(),
                    ParameterKind::Optional => optional.get(parameters[..i].iter().filter(|x| x.kind == ParameterKind::Optional).count()).copied(),
                    ParameterKind::Named => {
                        let n = p.name?.sym;
                        named.iter().find(|(s, _, _)| *s == n).map(|x| x.1)
                    }
                }
            }
            Declaration::Variable(_) => None,
        }
    }

    /// `_checkFfiNativeFunction`.
    fn check_native_function(&mut self, the_name: Span, d: Declaration, signature: TypeId, is_leaf: Option<bool>, parameters: &'a [ast::Parameter]) {
        if is_leaf == Some(true) {
            self.leaf_without_handles(signature, the_name);
        }
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.ty(signature) else { return };
        let mut ffi_types = self.flatten(&positional);
        let mut ffi_positional = positional.to_vec();
        let of_instance = match d {
            Declaration::Function(f) => {
                let g = self.program.function(f);
                (g.class.is_some() || g.extension.is_some()) && !g.static_
            }
            _ => false,
        };
        let formals = parameters.len();
        if of_instance {
            if formals + 1 != ffi_types.len() {
                let (a, b) = ((formals + 1).to_string(), ffi_types.len().to_string());
                self.report(cf::FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS_WITH_RECEIVER, the_name, &[Arg::from(a), Arg::from(b)]);
                return;
            }
            if let Some(&receiver) = positional.first()
                && self.is_ffi_t(receiver, "Pointer")
            {
                let extends = match d {
                    Declaration::Function(f) => self.program.function(f).class.is_some_and(|c| self.class_extends_nfwc1(c)),
                    _ => false,
                };
                if !extends {
                    self.report(cf::FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER, the_name, &[]);
                }
            }
            ffi_types.remove(0);
            if !ffi_positional.is_empty() {
                ffi_positional.remove(0);
            }
        } else if formals != ffi_types.len() {
            let (a, b) = (ffi_types.len().to_string(), formals.to_string());
            self.report(cf::FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS, the_name, &[Arg::from(a), Arg::from(b)]);
            return;
        }
        for i in 0..formals {
            if !self.is_ffi_t(ffi_types[i], "Pointer") {
                continue;
            }
            let t = self.parameter_type(d, i, parameters);
            let ok = t.is_some_and(|t| {
                matches!(self.table_ref().get(t), Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. })
                    && (self.is_ffi_t(t, "Pointer") || self.extends_nfwc1(t) || self.is_typed_data(t))
            });
            if !ok {
                self.report(cf::FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER, the_name, &[]);
            }
        }
        let dart_type = match d {
            Declaration::Function(f) => self.outline.functions.get(f.0 as usize).map(|x| x.signature),
            Declaration::Local(t) => Some(t),
            Declaration::Variable(_) => None,
        };
        let Some(dart_type) = dart_type else { return };
        let native = self.table().intern(Type::Function {
            type_params,
            ret,
            positional: ffi_positional.into_boxed_slice(),
            optional,
            named,
            nullable,
        });
        if !self.valid_native_function(native) {
            self.report(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, the_name, &[Arg::Type_(native), Arg::from("Native")]);
            return;
        }
        if !self.compat_functions(Direction::NativeToDart, dart_type, native, true, true) {
            self.report(cf::MUST_BE_A_SUBTYPE, the_name, &[Arg::Type_(native), Arg::Type_(dart_type), Arg::from("Native")]);
        }
    }

    /// `_checkFfiNativeField`.
    fn check_native_field(&mut self, the_name: Span, d: Declaration, metadata: &'a [ast::Annotation], signature: TypeId, allows_variable: bool) {
        let typ = match d {
            Declaration::Variable(v) => {
                let x = self.program.variable(v);
                if (x.class.is_some() || x.extension.is_some()) && !x.static_ {
                    self.report(cf::NATIVE_FIELD_NOT_STATIC, the_name, &[]);
                }
                self.variable_type(v)
            }
            Declaration::Function(f) => {
                // The accessor: the type of the synthetic variable (the return type of the
                // getter of the same name, or the setter's parameter).
                let g = self.program.function(f);
                if let Some(v) = g.variable {
                    self.variable_type(v)
                } else {
                    let getter = if g.kind == FunctionKind::Getter {
                        Some(f)
                    } else {
                        let map = match (g.class, g.extension) {
                            (Some(c), _) => {
                                let k = self.program.class(c);
                                if g.static_ { &k.static_members } else { &k.instance_members }
                            }
                            (None, Some(x)) => {
                                let k = self.program.extension(x);
                                if g.static_ { &k.static_members } else { &k.instance_members }
                            }
                            (None, None) => {
                                let tgt = self.program.library(g.library).declared.get(&g.name).and_then(|b| b.getter);
                                match tgt {
                                    Some(Element::Function(h)) if self.program.function(h).kind == FunctionKind::Getter => {
                                        return self.check_typed_field(the_name, metadata, signature, allows_variable, self.the_return(h));
                                    }
                                    _ => {
                                        let t = self.setter_parameter(f);
                                        return self.check_typed_field(the_name, metadata, signature, allows_variable, t);
                                    }
                                }
                            }
                        };
                        map.get(&g.name).copied().filter(|h| self.program.function(*h).kind == FunctionKind::Getter)
                    };
                    match getter {
                        Some(h) => self.the_return(h),
                        None => self.setter_parameter(f),
                    }
                }
            }
            Declaration::Local(_) => {
                self.report(cf::NATIVE_FIELD_NOT_STATIC, the_name, &[]);
                return;
            }
        };
        self.check_typed_field(the_name, metadata, signature, allows_variable, typ);
    }

    fn the_return(&self, f: FunctionElementId) -> TypeId {
        self.outline.functions.get(f.0 as usize).map_or(self.core.dynamic_, |d| d.return_type)
    }

    fn setter_parameter(&self, f: FunctionElementId) -> TypeId {
        self.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.first()).map_or(self.core.dynamic_, |p| p.ty)
    }

    fn check_typed_field(&mut self, the_name: Span, metadata: &'a [ast::Annotation], signature: TypeId, allows_variable: bool, typ: TypeId) {
        let mut signature = signature;
        if matches!(self.table_ref().get(signature), Type::Dynamic) && !self.table_ref().is_invalid(signature) {
            // `_canonicalFfiTypeForDartType`.
            if self.is_ffi_t(typ, "Pointer") || self.compound_subtype(typ) || self.is_ffi_t(typ, "Array") {
                signature = typ;
            } else {
                self.report(cf::NATIVE_FIELD_MISSING_TYPE, the_name, &[]);
                return;
            }
        }
        if !self.compat_native(Direction::NativeToDart, typ, signature, false, true) {
            self.report(cf::MUST_BE_A_SUBTYPE, the_name, &[Arg::Type_(typ), Arg::Type_(signature), Arg::from("Native")]);
        } else if self.is_ffi_t(signature, "Array") {
            let d = self.array_dimensions(signature);
            self.validate_size(the_name, metadata, d, allows_variable);
        } else if self.is_ffi_t(signature, "Handle") || self.is_ffi_t(signature, "NativeFunction") {
            self.report(cf::NATIVE_FIELD_INVALID_TYPE, the_name, &[Arg::Type_(signature)]);
        }
    }

    /// `_validateFfiLeafCallUsesNoHandles`.
    fn leaf_without_handles(&mut self, native: TypeId, span: Span) {
        let Type::Function { ret, positional, .. } = self.ty(native) else { return };
        if self.primitive(ret) == Primitive::Handle {
            self.report(cf::LEAF_CALL_MUST_NOT_RETURN_HANDLE, span, &[]);
        }
        for p in positional.iter() {
            if self.primitive(*p) == Primitive::Handle {
                self.report(cf::LEAF_CALL_MUST_NOT_TAKE_HANDLE, span, &[]);
            }
        }
    }

    // -- Expressions -----------------------------------------------------------

    fn expressions(&mut self) {
        let a = self.a;
        for (i, e) in a.exprs.iter().enumerate() {
            let id = ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    if !self.inside_index(e.span) {
                        self.call(id, *target, arguments);
                    }
                }
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    if self.inside_index(e.span) {
                        continue;
                    }
                    if let Some(Resolved::Constructor(f)) = self.body.get_resolved(id) {
                        let start = a.ty(*ty).span.start;
                        let end = constructor.map_or(a.ty(*ty).span.end, |n| n.span.end);
                        self.creation(id, *f, Span { start: start, end: end }, arguments);
                    }
                }
                ExprKind::Index { target, .. } => {
                    if !self.inside_index(e.span) {
                        self.index(id, *target);
                    }
                }
                ExprKind::Property { target, name, .. } => {
                    if !self.inside_index(e.span) {
                        self.property(id, *target, *name);
                    }
                }
                _ => {}
            }
        }
    }

    /// `MethodInvocation` and `FunctionExpressionInvocation` (and the implicit
    /// creation, which the analyzer rewrites into `InstanceCreationExpression`).
    fn call(&mut self, id: ExprId, target: ExprId, args: &'a ast::Arguments) {
        let a = self.a;
        match self.body.get_resolved(id) {
            Some(Resolved::ExtensionMember { extension, member }) => {
                // `the_value<T>(n)`: the `call` of `AllocatorAlloc`. A written `.call(…)` is a
                // `MethodInvocation`, which is not checked.
                let written = matches!(&a.expr(target).kind, ExprKind::Property { name, .. } if self.name(name.sym) == "call");
                if !written && self.ffi_extension(*extension, &["AllocatorAlloc"]) && self.name(self.program.function(*member).name) == "call" {
                    self.validate_allocation(id, args);
                }
                if !written {
                    return;
                }
            }
            Some(Resolved::Constructor(f)) => {
                let s = self.span(target);
                self.creation(id, *f, s, args);
                return;
            }
            _ => {}
        }
        let (method_name, r) = match &a.expr(target).kind {
            ExprKind::Property { name, .. } | ExprKind::Identifier(name) => (*name, self.body.get_resolved(target)),
            _ => return,
        };
        let f = match r {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => *f,
            Some(Resolved::Element(Element::Function(f))) => *f,
            _ => return,
        };
        let g = self.program.function(f);
        if g.kind != FunctionKind::Function {
            return;
        }
        let the_name = self.name(g.name);
        if let Some(c) = g.class {
            if self.ffi_class(c, "Pointer") {
                match the_name {
                    "fromFunction" => self.validate_from_function(method_name, args),
                    "elementAt" => self.validate_element_at(id, target),
                    _ => {}
                }
            } else if self.compound(c) {
                if the_name == "create" {
                    let cls = self.name(self.program.class(c).name);
                    self.validate_create(id, args, cls);
                }
            } else if self.ffi_class(c, "Native") && the_name == "addressOf" {
                self.validate_address_of(id, args);
            }
        } else if let Some(x) = g.extension {
            if self.ffi_extension(x, &["NativeFunctionPointer"]) {
                if the_name == "asFunction" {
                    self.validate_as_function(id, target, method_name, args);
                }
            } else if self.ffi_extension(x, &["DynamicLibraryExtension"]) && the_name == "lookupFunction" {
                self.validate_lookup_function(args);
            }
        } else if g.library == self.ffi && the_name == "sizeOf" {
            self.validate_size_of(id, args);
        }
    }

    /// The receiver (`realTarget`) of the invocation whose method name is `target`.
    fn receiver(&self, target: ExprId) -> Option<ExprId> {
        match &self.a.expr(target).kind {
            ExprKind::Property { target: r, .. } => Some(self.real_target(*r)),
            _ => None,
        }
    }

    /// `visitInstanceCreationExpression`.
    fn creation(&mut self, id: ExprId, f: FunctionElementId, constructor_name: Span, args: &'a ast::Arguments) {
        let g = self.program.function(f);
        let Some(c) = g.class else { return };
        let sup = self.program.class(c).supertype_class;
        if sup.is_some_and(|s| self.compound(s)) && self.is_class_element(c) {
            if !self.program.function(self.program.public(f)).factory {
                self.report(cf::CREATION_OF_STRUCT_OR_UNION, constructor_name, &[]);
            }
        } else if self.ffi_class(c, "NativeCallable") {
            self.validate_native_callable(id, f, constructor_name, args);
        }
    }

    /// `_validateNativeCallable`.
    fn validate_native_callable(&mut self, id: ExprId, f: FunctionElementId, constructor_name: Span, args: &'a ast::Arguments) {
        let the_name = self.name(self.program.function(f).name);
        let isolated = the_name == "isolateLocal";
        let n = args.args.len();
        if !(n == 1 || (isolated && n == 2)) {
            return;
        }
        let Some(st) = self.body.get_type(id) else { return };
        let Some(&argument) = self.arguments_of_type(st).first() else { return };
        if !self.valid_native_function(argument) {
            self.report(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, constructor_name, &[Arg::Type_(argument), Arg::from("NativeCallable")]);
            return;
        }
        let f0 = &args.args[0];
        let ft = self.body.get_type(f0.value).unwrap_or(self.core.dynamic_);
        if !self.compat_functions(Direction::DartToNative, ft, argument, false, false) {
            let s = self.argument_span(f0);
            self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(ft), Arg::Type_(argument), Arg::from("NativeCallable")]);
            return;
        }
        let Type::Function { ret, .. } = self.ty(argument) else { return };
        if isolated {
            if self.primitive(ret) == Primitive::Void || self.is_ffi_t(ret, "Pointer") || self.is_ffi_t(ret, "Handle") || self.compound_subtype(ret) {
                if n != 1 {
                    let s = self.argument_span(&args.args[1]);
                    self.report(cf::INVALID_EXCEPTION_VALUE, s, &[Arg::from(the_name)]);
                }
            } else if n != 2 {
                let s = self.span(id);
                self.report(cf::MISSING_EXCEPTION_VALUE, s, &[Arg::from(the_name)]);
            } else {
                // `(arguments[1] as NamedExpression)` (3.6.2; `as NamedArgument` in
                // 3.13.4): an `exceptionalReturn` passed as a positional
                // (`isolateLocal(f, 0)`) makes the cast throw, and the whole library ends
                // up without diagnostics ([`AnalyzerCrash`]).
                if args.args[1].name.is_none() {
                    self.the_break = true;
                    return;
                }
                let e = args.args[1].value;
                let et = self.body.get_type(e).unwrap_or(self.core.dynamic_);
                let s = self.span(e);
                if !self.compat_native(Direction::DartToNative, et, ret, false, false) {
                    self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(et), Arg::Type_(ret), Arg::from(the_name)]);
                }
                if !self.is_const(e) {
                    self.report(cf::ARGUMENT_MUST_BE_A_CONSTANT, s, &[Arg::from("exceptionalReturn")]);
                }
            }
        } else if self.primitive(ret) != Primitive::Void {
            let s = self.argument_span(f0);
            self.report(cf::MUST_RETURN_VOID, s, &[Arg::Type_(ret)]);
        }
    }

    /// `_validateAllocate`.
    fn validate_allocation(&mut self, id: ExprId, args: &'a ast::Arguments) {
        let types = self.instantiation(args);
        let [t] = types.as_slice() else { return };
        if !self.valid_native(*t, Allows { void_: true, empty_struct: true, ..Allows::default() }) {
            let s = self.span(id);
            self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("AllocatorAlloc.call")]);
        }
    }

    /// `_validateFromFunction`.
    fn validate_from_function(&mut self, method_name: ast::Name, args: &'a ast::Arguments) {
        let n = args.args.len();
        if !(1..=2).contains(&n) {
            return;
        }
        let Some(&t) = self.instantiation(args).first() else { return };
        if !self.valid_native_function(t) {
            let s = args.type_args.first().map_or(method_name.span, |&x| self.a.ty(x).span);
            self.report(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, s, &[Arg::Type_(t), Arg::from("fromFunction")]);
            return;
        }
        let f0 = &args.args[0];
        let ft = self.body.get_type(f0.value).unwrap_or(self.core.dynamic_);
        if !self.compat_functions(Direction::DartToNative, ft, t, false, false) {
            let s = self.argument_span(f0);
            self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(ft), Arg::Type_(t), Arg::from("fromFunction")]);
            return;
        }
        let Type::Function { ret: r, .. } = self.ty(t) else { return };
        if self.primitive(r) == Primitive::Void || self.is_ffi_t(r, "Pointer") || self.is_ffi_t(r, "Handle") || self.compound_subtype(r) {
            if n != 1 {
                let s = self.argument_span(&args.args[1]);
                self.report(cf::INVALID_EXCEPTION_VALUE, s, &[Arg::from("fromFunction")]);
            }
        } else if n != 2 {
            self.report(cf::MISSING_EXCEPTION_VALUE, method_name.span, &[Arg::from("fromFunction")]);
        } else {
            let x = &args.args[1];
            let et = self.body.get_type(x.value).unwrap_or(self.core.dynamic_);
            let s = self.argument_span(x);
            if !self.compat_native(Direction::DartToNative, et, r, false, false) {
                self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(et), Arg::Type_(r), Arg::from("fromFunction")]);
            }
            let constant = x.name.is_none() && self.is_const(x.value);
            if !constant {
                self.report(cf::ARGUMENT_MUST_BE_A_CONSTANT, s, &[Arg::from("exceptionalReturn")]);
            }
        }
    }

    /// `_validateElementAt`.
    fn validate_element_at(&mut self, id: ExprId, target: ExprId) {
        let Some(r) = self.receiver(target) else { return };
        let Some(tt) = self.body.get_type(r) else { return };
        if !self.is_ffi_t(tt, "Pointer") {
            return;
        }
        let Some(&t) = self.arguments_of_type(tt).first() else { return };
        if !self.valid_native(t, Allows { void_: true, empty_struct: true, ..Allows::default() }) {
            let s = self.span(id);
            self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("elementAt")]);
        }
    }

    /// `_validateCreate`.
    fn validate_create(&mut self, id: ExprId, args: &'a ast::Arguments, cls: &str) {
        let types = self.instantiation(args);
        let [t] = types.as_slice() else { return };
        if !self.valid_native(*t, Allows::default()) {
            let s = self.span(id);
            self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from(format!("{cls}.create"))]);
        }
    }

    /// `_validateSizeOf`.
    fn validate_size_of(&mut self, id: ExprId, args: &'a ast::Arguments) {
        let types = self.instantiation(args);
        let [t] = types.as_slice() else { return };
        if !self.valid_native(*t, Allows { void_: true, empty_struct: true, ..Allows::default() }) {
            let s = self.span(id);
            self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("sizeOf")]);
        }
    }

    /// `_validateAsFunction`.
    fn validate_as_function(&mut self, id: ExprId, target: ExprId, method_name: ast::Name, args: &'a ast::Arguments) {
        let written_ones = &args.type_args;
        let error_span = written_ones.first().map_or(self.span(id), |&x| self.a.ty(x).span);
        if let [x] = &written_ones[..]
            && let Some(t) = self.written_type(*x)
            && self.is_parameter(t)
        {
            let s = self.a.ty(*x).span;
            self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("asFunction")]);
            return;
        }
        if let Some(tgt) = self.receiver(target)
            && let Some(tt) = self.body.get_type(tgt)
            && self.is_ffi_t(tt, "Pointer")
        {
            let Some(&t) = self.arguments_of_type(tt).first() else { return };
            if !self.is_ffi_t(t, "NativeFunction") {
                return;
            }
            let Some(&argument) = self.arguments_of_type(t).first() else { return };
            if self.is_parameter(argument) {
                let s = self.span(tgt);
                self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("asFunction")]);
                return;
            }
            if !self.valid_native_function(argument) {
                self.report(cf::NON_NATIVE_FUNCTION_TYPE_ARGUMENT_TO_POINTER, error_span, &[Arg::Type_(t)]);
                return;
            }
            let Some(&f) = self.instantiation(args).first() else { return };
            let leaf = self.the_is_leaf(args);
            if !self.compat_functions(Direction::NativeToDart, f, argument, false, false) {
                let s = self.span(id);
                self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(argument), Arg::Type_(f), Arg::from("asFunction")]);
            }
            if leaf {
                self.leaf_without_handles(argument, method_name.span);
            }
        }
        self.validate_constant_leaf(args);
    }

    /// `_validateLookupFunction`.
    fn validate_lookup_function(&mut self, args: &'a ast::Arguments) {
        let written_ones = &args.type_args;
        if written_ones.len() != 2 {
            return;
        }
        let types = self.instantiation(args);
        let (Some(&s), Some(&f)) = (types.first(), types.get(1)) else { return };
        let span0 = self.a.ty(written_ones[0]).span;
        let span1 = self.a.ty(written_ones[1]).span;
        if !self.valid_native_function(s) {
            self.report(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, span0, &[Arg::Type_(s), Arg::from("lookupFunction")]);
            return;
        }
        let leaf = self.the_is_leaf(args);
        if !self.compat_functions(Direction::NativeToDart, f, s, false, false) {
            self.report(cf::MUST_BE_A_SUBTYPE, span1, &[Arg::Type_(s), Arg::Type_(f), Arg::from("lookupFunction")]);
        }
        self.validate_constant_leaf(args);
        if leaf {
            self.leaf_without_handles(s, span0);
        }
    }

    /// The (non-synthetic) element an identifier references.
    fn referenced_element(&self, e: ExprId) -> Option<Declaration> {
        match &self.a.expr(e).kind {
            ExprKind::Identifier(_) => {}
            ExprKind::Property { target, .. } if matches!(self.a.expr(*target).kind, ExprKind::Identifier(_)) => {}
            _ => return None,
        }
        let p = self.program;
        let through_function = |f: FunctionElementId| match p.function(f).variable {
            Some(v) => Declaration::Variable(v),
            None => Declaration::Function(f),
        };
        match self.body.get_resolved(e)? {
            Resolved::Element(Element::Function(f)) => Some(through_function(*f)),
            Resolved::Element(Element::Variable(v)) => Some(Declaration::Variable(*v)),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => Some(through_function(*f)),
            Resolved::Member { member: MemberRef::Variable(v), .. } => Some(Declaration::Variable(*v)),
            _ => None,
        }
    }

    /// `_validateNativeAddressOf`.
    fn validate_address_of(&mut self, id: ExprId, args: &'a ast::Arguments) {
        let types = self.instantiation(args);
        if types.len() != 1 || args.args.len() != 1 {
            return;
        }
        let argument = &args.args[0];
        let tgt = types[0];
        let mut valid = false;
        let referenced = if argument.name.is_none() { self.referenced_element(argument.value) } else { None };
        if let Some(r) = referenced {
            let (unit_id, metadata) = self.element_annotations(r);
            for m in metadata {
                let Some(inst) = self.annotation_instance(unit_id, m) else { continue };
                if !self.ffi_class(inst.class, "Native") {
                    continue;
                }
                let Some(&native) = self.arguments_of_type(inst.ty).first() else { continue };
                let s = self.span(id);
                if matches!(self.table_ref().get(native), Type::Function { .. }) {
                    if !self.is_ffi_t(tgt, "NativeFunction") {
                        self.report(cf::MUST_BE_A_NATIVE_FUNCTION_TYPE, s, &[Arg::Type_(tgt), Arg::from("Native.addressOf")]);
                    } else if let Some(&target_function) = self.arguments_of_type(tgt).first()
                        && !self.assignable(native, target_function)
                    {
                        self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(native), Arg::Type_(target_function), Arg::from("Native.addressOf")]);
                    }
                } else {
                    let mut native = native;
                    if matches!(self.table_ref().get(native), Type::Dynamic)
                        && !self.table_ref().is_invalid(native)
                        && let Some(st) = self.body.get_type(argument.value)
                        && (self.is_ffi_t(st, "Pointer") || self.compound_subtype(st) || self.is_ffi_t(st, "Array"))
                    {
                        native = st;
                    }
                    if !self.assignable(native, tgt) {
                        self.report(cf::MUST_BE_A_SUBTYPE, s, &[Arg::Type_(native), Arg::Type_(tgt), Arg::from("Native.addressOf")]);
                    }
                }
                valid = true;
                break;
            }
        }
        if !valid {
            let s = self.argument_span(argument);
            self.report(cf::ARGUMENT_MUST_BE_NATIVE, s, &[]);
        }
    }

    /// `visitIndexExpression`: the `[]` of `StructPointer`, `StructArray`,
    /// `UnionPointer` and `UnionArray`.
    fn index(&mut self, id: ExprId, target: ExprId) {
        let Some(Resolved::ExtensionMember { extension, member }) = self.body.get_resolved(id) else { return };
        if !self.ffi_extension(*extension, &["StructPointer", "StructArray", "UnionPointer", "UnionArray"]) || self.name(self.program.function(*member).name) != "[]" {
            return;
        }
        let tgt = self.real_target(target);
        let Some(t) = self.body.get_type(tgt) else { return };
        if !self.valid_native(t, Allows { empty_struct: true, array: true, ..Allows::default() }) {
            let s = self.span(id);
            self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("[]")]);
        }
    }

    /// `visitPrefixedIdentifier` and `visitPropertyAccess`: `ref` and `address`.
    fn property(&mut self, id: ExprId, target: ExprId, the_name: ast::Name) {
        let Some(Resolved::ExtensionMember { extension, member }) = self.body.get_resolved(id) else { return };
        let (x, f) = (*extension, *member);
        if self.program.function(f).kind == FunctionKind::Setter {
            return;
        }
        let member_name = self.name(self.program.function(f).name);
        if self.ffi_extension(x, &["StructPointer", "UnionPointer"]) {
            if member_name == "ref" {
                let tgt = self.real_target(target);
                let Some(t) = self.body.get_type(tgt) else { return };
                if !self.valid_native(t, Allows { empty_struct: true, ..Allows::default() }) {
                    let s = self.span(id);
                    self.report(cf::NON_CONSTANT_TYPE_ARGUMENT, s, &[Arg::from("ref")]);
                }
            }
        } else if self.ffi_extension(x, &COMPOUND_ADDRESS) || self.ffi_extension(x, &PRIMITIVE_ADDRESS) || self.ffi_extension(x, &TYPED_DATA_ADDRESS) {
            if member_name == "address" {
                self.validate_address_position(id, the_name.span);
                let the_extension = self.program.extension(x).name.map(|n| self.name(n)).unwrap_or("");
                self.validate_address_receiver(target, the_extension, the_name.span);
            }
        }
    }

    /// `_validateAddressPosition`: `.address` only as an argument of a call to a
    /// `@Native(isLeaf: true)` function (or through `.cast()`).
    fn validate_address_position(&mut self, no: ExprId, error: Span) {
        let a = self.a;
        let mut current = no;
        if let Some(p) = self.parent(no)
            && let ExprKind::Property { target, name, .. } = &a.expr(p).kind
            && *target == no
            && self.name(name.sym) == "cast"
            && let Some(Resolved::Member { member: MemberRef::Function(f), .. }) = self.body.get_resolved(p)
            && self.program.function(*f).kind == FunctionKind::Function
            && self.program.function(*f).class.is_some_and(|c| self.ffi_class(c, "Pointer"))
            && let Some(c) = self.parent(p)
            && matches!(&a.expr(c).kind, ExprKind::Call { target, .. } if *target == p)
        {
            current = c;
        }
        let ok = match self.parent(current) {
            Some(g) => match &a.expr(g).kind {
                ExprKind::Call { target, arguments } => {
                    *target != current && arguments.args.iter().any(|x| x.name.is_none() && x.value == current) && self.leaf_call(*target)
                }
                _ => false,
            },
            None => false,
        };
        if !ok {
            self.report(cf::ADDRESS_POSITION, error, &[]);
        }
    }

    /// `isNativeLeafInvocation`: the method invocation whose element is a
    /// function or method with `@Native(isLeaf: true)`.
    fn leaf_call(&mut self, tgt: ExprId) -> bool {
        if !matches!(self.a.expr(tgt).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
            return false;
        }
        let f = match self.body.get_resolved(tgt) {
            Some(Resolved::Element(Element::Function(f))) => *f,
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => *f,
            _ => return false,
        };
        if self.program.function(f).kind != FunctionKind::Function {
            return false;
        }
        let (unit_id, metadata) = self.element_annotations(Declaration::Function(f));
        for m in metadata {
            let Some(inst) = self.annotation_instance(unit_id, m) else { continue };
            if self.ffi_class(inst.class, "Native") && inst.is_leaf == Some(true) {
                return true;
            }
        }
        false
    }

    /// `_validateAddressReceiver`.
    fn validate_address_receiver(&mut self, receiver: ExprId, the_extension: &str, error: Span) {
        if COMPOUND_ADDRESS.contains(&the_extension) || TYPED_DATA_ADDRESS.contains(&the_extension) {
            return;
        }
        let receiver = self.real_target(receiver);
        let ok = match &self.a.expr(receiver).kind {
            // Element of an `Array` or of a typed list.
            ExprKind::Index { target, .. } => {
                let t = self.body.get_type(self.real_target(*target));
                t.is_some_and(|t| self.is_ffi_t(t, "Array") || self.is_typed_data(t))
            }
            // Field of a `Struct` or `Union`.
            ExprKind::Property { target, .. } => {
                let t = self.body.get_type(self.real_target(*target));
                t.is_some_and(|t| self.compound_subtype(t))
            }
            _ => false,
        };
        if !ok {
            self.report(cf::ADDRESS_RECEIVER, error, &[]);
        }
    }
}
