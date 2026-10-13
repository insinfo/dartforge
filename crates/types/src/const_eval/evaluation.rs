//! The `ConstantVisitor` and `_InstanceCreationEvaluator` of analyzer 6.11
//! (`src/dart/constant/evaluation.dart`): evaluation of constant
//! expressions, with the same errors (`InvalidConstant`), the same positions
//! and the same marks (`avoidReporting`, `isUnresolved`,
//! `isRuntimeException`).
//!
//! **On the safe side.** Anything that depends on information that this
//! pass's inference did not produce (bodies and initializers of libraries
//! that were not inferred, like the SDK's) becomes a valid unknown value
//! (`validWithUnknownValue`): no error is proven about it.

use super::value::{the_text, Field, InstanceState, Exception, Function, Value};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{codes::compile_time_error as c, Code, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, VariableId,
    VariableRef,
};
use dartforge_frontend::ast::{self, BinaryOp, CollectionElement, ExprId, ExprKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `InvalidConstant`.
#[derive(Clone, Debug)]
pub struct Invalid {
    pub code: Code,
    pub unit: UnitId,
    pub span: Span,
    pub args: Vec<String>,
    pub avoid_report: bool,
    pub unresolved: bool,
    pub exception: bool,
    /// `contextMessages`: unit, range and text (the unit decides the file of the
    /// message when the error is reported).
    pub context: Vec<(UnitId, Span, String)>,
}

/// `Constant`: a valid value or an error.
#[derive(Clone, Debug)]
pub enum Constant {
    Value(Value),
    Invalid(Box<Invalid>),
}

impl Constant {
    pub fn value(&self) -> Option<&Value> {
        match self {
            Constant::Value(v) => Some(v),
            Constant::Invalid(_) => None,
        }
    }
}

/// The state of `ConstantVisitor`: the unit of the expression, the lexical
/// environment of the parameters (`_lexicalEnvironment`) and that of the
/// type parameters (`_lexicalTypeEnvironment`, which also does the class
/// substitution).
#[derive(Clone)]
pub struct Ctx {
    pub unit: UnitId,
    pub lib: LibraryId,
    pub lexical: Option<Rc<HashMap<SymbolId, Value>>>,
    pub types: Option<Rc<HashMap<TypeParamId, TypeId>>>,
    /// The visitor's reporter is the `ConstantVerifier`'s (and not one that
    /// discards): the errors of `_valueOf` come out.
    pub report: bool,
}

impl Ctx {
    pub fn simple(unit_id: UnitId, lib: LibraryId) -> Ctx {
        Ctx { unit: unit_id, lib, lexical: None, types: None, report: false }
    }
}

#[derive(Clone)]
enum VarState {
    InProgress,
    Ready(Constant),
}

/// The evaluation engine (`ConstantEvaluationEngine` with the results stored
/// in the elements).
pub struct Engine<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a mut TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub body: &'a BodyTypes,
    /// Libraries whose bodies (and initializers) were inferred.
    pub the_inferred: &'a HashSet<LibraryId>,
    vars: HashMap<VariableId, VarState>,
    vars_stack: Vec<VariableId>,
    cyclic: HashSet<VariableId>,
    /// Default values already evaluated, by expression.
    patterns: HashMap<(UnitId, ExprId), Constant>,
    /// Local constants: by the name's offset, the list and the index.
    unit_locals: HashMap<UnitId, HashMap<usize, (ast::StmtId, usize)>>,
    locals: HashMap<(UnitId, usize), Option<Constant>>,
    /// Constructors being evaluated (cycle: `isCycleFree` false).
    constructors_in_progress: Vec<FunctionElementId>,
    /// The constant dependency graph and who is in a cycle
    /// ([`super::cycles::compute`], before each library's verifier).
    pub graph: super::cycles::State,
    /// Direct positional arguments of a constant creation: their
    /// `genericError` is `CONST_WITH_NON_CONSTANT_ARGUMENT`.
    creation_args: HashSet<(UnitId, ExprId)>,
    /// Errors reported by the visitor itself (`_valueOf`) when `relatar`.
    pub reports: Vec<Invalid>,
    /// Analyzer 3.6.2 would throw here (exhaustiveness, see
    /// [`crate::table::TypeTable::without_default_type`]).
    pub analyzer_break: bool,
    /// When `Some`, the verifier stores here, by (unit, offset of the
    /// `switch`), the witnesses with parts of each `non_exhaustive_switch_*`
    /// (the analyzer's `diagnostic.data`, used by `AddMissingSwitchCases`).
    pub witnesses: Option<HashMap<(UnitId, usize), Vec<Vec<super::exhaustiveness::WitnessPart>>>>,
    /// Offsets of the names of formal parameters, per inferred unit (inference
    /// resolves a read of a parameter as that of a local).
    parameters: HashMap<UnitId, HashSet<usize>>,
    depth: u32,
    sym_identical: Option<SymbolId>,
    sym_length: Option<SymbolId>,
    sym_from_environment: Option<SymbolId>,
    sym_has_environment: Option<SymbolId>,
    sym_default_value: Option<SymbolId>,
}

type R = Constant;

impl<'a> Engine<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a mut TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        body: &'a BodyTypes,
        the_inferred: &'a HashSet<LibraryId>,
    ) -> Self {
        let mut params_list: HashMap<UnitId, HashSet<usize>> = HashMap::new();
        for (i, u) in program.units.iter().enumerate() {
            if the_inferred.contains(&u.library) {
                params_list.insert(UnitId(i as u32), parameter_offsets(&u.ast));
            }
        }
        Engine {
            program,
            interner,
            table,
            core,
            outline,
            body,
            the_inferred,
            vars: HashMap::new(),
            vars_stack: Vec::new(),
            cyclic: HashSet::new(),
            patterns: HashMap::new(),
            unit_locals: HashMap::new(),
            locals: HashMap::new(),
            constructors_in_progress: Vec::new(),
            graph: super::cycles::State::default(),
            creation_args: HashSet::new(),
            reports: Vec::new(),
            analyzer_break: false,
            witnesses: None,
            parameters: params_list,
            depth: 0,
            sym_identical: interner.lookup("identical"),
            sym_length: interner.lookup("length"),
            sym_from_environment: interner.lookup("fromEnvironment"),
            sym_has_environment: interner.lookup("hasEnvironment"),
            sym_default_value: interner.lookup("defaultValue"),
        }
    }

    // -- Access --------------------------------------------------------------

    pub fn ast(&self, u: UnitId) -> &'a ast::Ast {
        &self.program.unit(u).ast
    }

    pub fn span(&self, u: UnitId, e: ExprId) -> Span {
        self.ast(u).expr(e).span
    }

    fn inferred_(&self, u: UnitId) -> bool {
        self.the_inferred.contains(&self.program.unit(u).library)
    }

    /// Static type of `e` (`dynamic` in a unit that was not inferred).
    pub fn the_static(&self, u: UnitId, e: ExprId) -> TypeId {
        self.body.units.get(u.0 as usize).and_then(|b| b.get_type(e)).unwrap_or(self.core.dynamic_)
    }

    pub fn resolved(&self, u: UnitId, e: ExprId) -> Option<&'a Resolved> {
        self.body.units.get(u.0 as usize).and_then(|b| b.get_resolved(e))
    }

    /// The static type of `e` is the recovery type (`InvalidType`): the invalid
    /// recorded type, or the local with an invalid initializer.
    pub(crate) fn invalid_type(&self, u: UnitId, e: ExprId) -> bool {
        self.body.units.get(u.0 as usize).is_some_and(|b| {
            b.invalid_types.contains(&e) || b.get_type(e).is_some_and(|t| self.table.is_invalid(t))
        })
    }

    pub fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    pub fn format(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// Without the alias (`getDisplayString()`): where the analyzer already
    /// passes text.
    pub fn format_without_alias(&self, t: TypeId) -> String {
        self.table.format_without_alias(t, self.interner, self.program)
    }

    /// `extensionTypeErasure`.
    pub fn erase(&mut self, t: TypeId) -> TypeId {
        let outline = self.outline;
        let program = self.program;
        let rep = |decl: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &map, table))
        };
        crate::ops::erase_extension_type(t, self.table, &rep)
    }

    /// `runtimeTypeMatch`: the object's type is a subtype of the erased type. An
    /// object of type `dynamic` (unresolved expression) matches everything.
    pub fn matches_type(&mut self, v: &Value, t: TypeId) -> bool {
        if v.ty == self.core.dynamic_ || matches!(v.state, InstanceState::Null { invalid: true }) {
            return true;
        }
        let t = self.erase(t);
        self.sub(v.ty, t)
    }

    /// `isAssignableTo` of the static type.
    pub fn assignable(&mut self, a: TypeId, b: TypeId) -> bool {
        a == self.core.dynamic_ || self.sub(a, b)
    }

    fn value(&mut self, typ: TypeId, state: InstanceState) -> Value {
        let typ = self.erase(typ);
        Value::new(typ, state)
    }

    pub fn unknown(&mut self, typ: TypeId) -> Value {
        let typ = self.erase(typ);
        let element = match self.table.get(typ) {
            Type::Interface { class, args, .. } if Some(*class) == self.core.list_class && args.len() == 1 => Some(args[0]),
            _ => None,
        };
        Value::unknown(self.table, self.core, typ, element)
    }

    fn interface(&mut self, class: Option<ClassId>, args: Vec<TypeId>) -> TypeId {
        match class {
            Some(class) => self.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable: false }),
            None => self.core.dynamic_,
        }
    }

    // -- Errors --------------------------------------------------------------

    pub fn error(&self, u: UnitId, span: Span, code: Code) -> Invalid {
        Invalid { code, unit: u, span, args: Vec::new(), avoid_report: false, unresolved: false, exception: false, context: Vec::new() }
    }

    /// `ConstructorElement.displayName`: `C` or `C.the_name`.
    fn constructor_display(&self, f: FunctionElementId) -> String {
        let fe = self.program.function(f);
        let cls = fe.class.map(|k| self.interner.resolve(self.program.class(k).name).to_string()).unwrap_or_default();
        let the_name = self.interner.resolve(fe.name);
        if the_name.is_empty() || the_name == "new" {
            cls
        } else {
            format!("{cls}.{the_name}")
        }
    }

    /// `_constructor.source`: the unit of the constructor's declaration (that of
    /// the class, for the synthetic one).
    fn constructor_unit(&self, f: FunctionElementId) -> Option<UnitId> {
        let fe = self.program.function(f);
        match fe.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
            FunctionRef::None => fe.class.and_then(|k| self.program.class(k).decl).map(|d| d.unit),
        }
    }

    /// `_checkInitializers`: the error (not a runtime one) in the initializer
    /// moves to the creation node; without a context yet, it gets the one where
    /// it occurs.
    fn error_in_initializer(&self, i: &Invalid, f: Option<FunctionElementId>, where_: &str, error: ErrorAt) -> Invalid {
        let mut n = i.clone();
        if n.context.is_empty() {
            if let Some(f) = f {
                if let Some(u) = self.constructor_unit(f) {
                    let text = format!("The error is in the {where_} of '{}', and occurs here.", self.constructor_display(f));
                    n.context.push((u, i.span, text));
                }
            }
        }
        n.unit = error.unit;
        n.span = error.span;
        n
    }

    /// `_stackTraceContextMessage`: at the name of the calling constructor (none
    /// for the synthetic one, whose `nameOffset` is -1).
    fn constructor_stack(&self, super_: FunctionElementId, f: FunctionElementId) -> Option<(UnitId, Span, String)> {
        if !matches!(self.program.function(f).node, FunctionRef::Constructor { .. }) {
            return None;
        }
        let (u, span) = self.program.non_synthetic_function_name(f)?;
        let who = self.constructor_display(f);
        let text = format!("The evaluated constructor '{}' is called by '{who}' and '{who}' is defined here.", self.constructor_display(super_));
        Some((u, span, text))
    }

    fn inv(&self, u: UnitId, e: ExprId, code: Code) -> R {
        Constant::Invalid(Box::new(self.error(u, self.span(u, e), code)))
    }

    /// `InvalidConstant.genericError`.
    fn generic(&self, u: UnitId, e: ExprId, unresolved: bool) -> R {
        let code = if self.creation_args.contains(&(u, e)) { c::CONST_WITH_NON_CONSTANT_ARGUMENT } else { c::INVALID_CONSTANT };
        let mut i = self.error(u, self.span(u, e), code);
        i.unresolved = unresolved;
        Constant::Invalid(Box::new(i))
    }

    fn exception(&self, u: UnitId, e: ExprId, x: Exception) -> R {
        let mut i = self.error(u, self.span(u, e), x.code);
        i.exception = x.runtime;
        Constant::Invalid(Box::new(i))
    }

    // -- Expressions -----------------------------------------------------------

    /// `evaluateConstant`: `in_const` says whether the node is in a constant
    /// context (`inConstantContext`).
    pub fn evaluate(&mut self, cx: &Ctx, e: ExprId, in_const: bool) -> R {
        if self.depth > 400 {
            return self.generic(cx.unit, e, true);
        }
        self.depth += 1;
        let r = self.evaluate_at(cx, e, in_const);
        self.depth -= 1;
        self.instantiate_reference(cx, e, r)
    }

    /// `visitFunctionReference` without written arguments (3.6.2
    /// `evaluation.dart:852-882`): the implicit instantiation of any expression
    /// (`tearoff_instantiation`). An inferred argument that mentions a type
    /// parameter is `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`; otherwise
    /// `_instantiateFunctionType` (`:1939-1959`): the value whose function is
    /// generic (the type **of the element**) gets that type instantiated by the
    /// arguments and then substituted by the constructor's environment.
    fn instantiate_reference(&mut self, cx: &Ctx, e: ExprId, r: R) -> R {
        let Constant::Value(mut val) = r else { return r };
        let u = cx.unit;
        // A constructor tear-off is a `ConstructorReference`
        // (`visitConstructorReference`), not a `FunctionReference`.
        if matches!(self.resolved(u, e), Some(Resolved::Constructor(_))) {
            return Constant::Value(val);
        }
        let Some(args) = self.body.units.get(u.0 as usize).and_then(|b| b.tearoff_instantiation(e)).map(|a| a.to_vec()) else {
            return Constant::Value(val);
        };
        if self.instantiation_with_parameter(cx, e) {
            return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
        }
        let InstanceState::Function { element: Function::Element(f), .. } = val.state else { return Constant::Value(val) };
        let Some(sig) = self.outline.functions.get(f.0 as usize).map(|d| d.signature) else { return Constant::Value(val) };
        let Type::Function { type_params, ret, positional, optional, named, nullable } = self.table.get(sig).clone() else {
            return Constant::Value(val);
        };
        if type_params.is_empty() || args.is_empty() || type_params.len() != args.len() {
            return Constant::Value(val);
        }
        let without = self.table.intern(Type::Function { type_params: Box::new([]), ret, positional, optional, named, nullable });
        let m: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(args.iter().copied()).collect();
        let t = crate::ops::substitute(without, &m, self.table);
        val.ty = match &cx.types {
            Some(m) => crate::ops::substitute(t, m, self.table),
            None => t,
        };
        Constant::Value(val)
    }

    fn evaluate_at(&mut self, cx: &Ctx, e: ExprId, in_const: bool) -> R {
        let u = cx.unit;
        let a = self.ast(u);
        match &a.expr(e).kind {
            ExprKind::Int(span) => {
                let t = &self.program.unit(u).source[span.start..span.end];
                let v = crate::constant::integer_literal(t);
                if self.the_static(u, e) == self.core.double {
                    let d = v.map(|i| i as f64).or_else(|| t.replace('_', "").parse::<f64>().ok());
                    Constant::Value(Value::new(self.core.double, InstanceState::Double(d)))
                } else {
                    Constant::Value(Value::new(self.core.int, InstanceState::Int(v)))
                }
            }
            ExprKind::Double(span) => {
                let t = &self.program.unit(u).source[span.start..span.end];
                Constant::Value(Value::new(self.core.double, InstanceState::Double(crate::constant::real_literal(t))))
            }
            ExprKind::Bool(b) => Constant::Value(Value::bool_(self.core, Some(*b))),
            ExprKind::Null => Constant::Value(Value::null(self.core)),
            ExprKind::String(lit) => self.string(cx, e, lit, in_const),
            ExprKind::Symbol(parts) => {
                let s: Vec<&str> = parts.iter().map(|n| self.interner.resolve(n.sym)).collect();
                Constant::Value(Value::new(self.core.symbol, InstanceState::Symbol(Some(s.join(".").into()))))
            }
            ExprKind::Identifier(n) => {
                if let Some(v) = cx.lexical.as_ref().and_then(|l| l.get(&n.sym)) {
                    return Constant::Value(v.clone());
                }
                self.constant_value(cx, e, e, true)
            }
            ExprKind::Parenthesized(x) => self.evaluate(cx, *x, in_const),
            ExprKind::List { const_, elements, .. } => {
                if !(*const_ || in_const) {
                    return self.inv(u, e, c::MISSING_CONST_IN_LIST_LITERAL);
                }
                let typ = self.the_static(u, e);
                let element = match self.table.get(typ) {
                    Type::Interface { args, .. } if args.len() == 1 => args[0],
                    _ => self.core.dynamic_,
                };
                let element = self.erase(element);
                let list_type = self.interface(self.core.list_class, vec![element]);
                let mut list = Vec::new();
                if let Some(i) = self.build_list(cx, &mut list, elements, true) {
                    return match i {
                        Ok(()) => Constant::Value(self.unknown(list_type)),
                        Err(i) => Constant::Invalid(i),
                    };
                }
                Constant::Value(self.value(list_type, InstanceState::List { element, elements: Rc::new(list), unknown: false }))
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                let typ = self.the_static(u, e);
                let (cls, args) = match self.table.get(typ) {
                    Type::Interface { class, args, .. } => (Some(*class), args.to_vec()),
                    _ => (None, Vec::new()),
                };
                let is_set = cls.is_some() && cls == self.core.set_class;
                let is_map = cls.is_some() && cls == self.core.map_class;
                if !is_set {
                    if !(*const_ || in_const) {
                        return self.inv(u, e, c::MISSING_CONST_IN_MAP_LITERAL);
                    }
                    let (k, v) = if args.len() >= 2 { (args[0], args[1]) } else { (self.core.dynamic_, self.core.dynamic_) };
                    let map_type = self.interface(self.core.map_class, vec![k, v]);
                    let mut map = Vec::new();
                    return match self.build_map(cx, &mut map, elements) {
                        Some(Ok(())) => Constant::Value(self.unknown(map_type)),
                        Some(Err(mut i)) => {
                            if !is_map {
                                i.avoid_report = true;
                            }
                            Constant::Invalid(i)
                        }
                        None => Constant::Value(self.value(map_type, InstanceState::Map { entries: Rc::new(map), unknown: false })),
                    };
                }
                if !(*const_ || in_const) {
                    return self.inv(u, e, c::MISSING_CONST_IN_SET_LITERAL);
                }
                let el = args.first().copied().unwrap_or(self.core.dynamic_);
                let set_type = self.interface(self.core.set_class, vec![el]);
                let mut set = Vec::new();
                match self.build_set(cx, &mut set, elements) {
                    Some(Ok(())) => Constant::Value(self.unknown(set_type)),
                    Some(Err(i)) => Constant::Invalid(i),
                    None => Constant::Value(self.value(set_type, InstanceState::Set { elements: Rc::new(set), unknown: false })),
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let const_child = *const_ || in_const;
                let mut pos = Vec::new();
                for &x in positional.iter() {
                    match self.evaluate(cx, x, const_child) {
                        Constant::Value(v) => pos.push(v),
                        i => return i,
                    }
                }
                let mut the_named = Vec::new();
                for (n, x) in named.iter() {
                    match self.evaluate(cx, *x, const_child) {
                        Constant::Value(v) => the_named.push((n.sym, v)),
                        i => return i,
                    }
                }
                let mut named_t: Vec<(SymbolId, TypeId)> = the_named.iter().map(|(n, v)| (*n, v.ty)).collect();
                named_t.sort_by_key(|(n, _)| *n);
                let typ = self.table.intern(Type::Record {
                    positional: pos.iter().map(|v| v.ty).collect(),
                    named: named_t.into_boxed_slice(),
                    nullable: false,
                });
                Constant::Value(Value::new(typ, InstanceState::Record { positional: Rc::new(pos), named: Rc::new(the_named) }))
            }
            ExprKind::InstanceCreation { keyword, arguments, constructor, .. } => {
                if !matches!(keyword, Some(ast::CreationKeyword::Const)) && !(keyword.is_none() && in_const) {
                    return self.generic(u, e, false);
                }
                let Some(Resolved::Constructor(f)) = self.resolved(u, e) else {
                    // The forwarded constructor of a mixin application.
                    if let Some((f, typ)) = self.forwarded_constructor(u, e, constructor.map(|n| n.sym)) {
                        let kw = self.creation_keyword(u, e, keyword.is_some());
                        let r = self.call_constructor(cx, e, f, arguments, kw, Some(typ));
                        return self.format_constructor_error(u, e, r);
                    }
                    // `const E(0)` through the primary constructor of an extension
                    // type (without a function in the model): the representation.
                    let static_ = self.body.units.get(u.0 as usize).and_then(|b| b.get_type(e));
                    if let Some(t) = static_
                        && let Type::ExtensionType { decl: k, .. } = self.table.get(t).clone()
                        && let Some(r) = self.extension_primary(cx, e, k, constructor.map(|n| n.sym), arguments, true)
                    {
                        return r;
                    }
                    return self.inv(u, e, c::INVALID_CONSTANT);
                };
                let kw = self.creation_keyword(u, e, keyword.is_some());
                let r = self.call_constructor(cx, e, *f, arguments, kw, None);
                self.format_constructor_error(u, e, r)
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = self.resolved(u, e) {
                    // Implicit creation: constant only in a constant context.
                    if !in_const {
                        return self.generic(u, e, false);
                    }
                    let f = *f;
                    let r = self.call_constructor(cx, e, f, arguments, None, None);
                    return self.format_constructor_error(u, e, r);
                }
                let tgt = *target;
                // The primary constructor of an extension type (`A(true)`,
                // `A.n(1)`), which the model does not keep as a function: the creation
                // is worth the representation (`_fieldMap[representation]`, 3.6.2
                // `evaluation.dart:2573-2578`), the evaluated argument.
                if let Some(r) = self.creation_via_extension_primary(cx, e, tgt, arguments, in_const) {
                    return r;
                }
                // `assert(…)` as an expression is a `FunctionExpressionInvocation`
                // (the generic `visitNode`), not a `MethodInvocation`.
                let is_method = matches!(a.expr(tgt).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && !a.assert_invocations.contains(&e);
                if !is_method {
                    return self.generic(u, e, false);
                }
                // `identical(a, b)` from `dart:core`.
                if let ExprKind::Identifier(n) = &a.expr(tgt).kind {
                    if Some(n.sym) == self.sym_identical && arguments.args.len() == 2 && arguments.args.iter().all(|x| x.name.is_none()) {
                        if let Some(Resolved::Element(Element::Function(f))) = self.resolved(u, tgt) {
                            if Some(self.program.function(*f).library) == self.core.core_library {
                                let (x, y) = (arguments.args[0].value, arguments.args[1].value);
                                let l = match self.evaluate(cx, x, in_const) {
                                    Constant::Value(v) => v,
                                    i => return i,
                                };
                                let r = match self.evaluate(cx, y, in_const) {
                                    Constant::Value(v) => v,
                                    i => return i,
                                };
                                let b = self.identical(&l, &r);
                                return Constant::Value(Value::bool_(self.core, b));
                            }
                        }
                    }
                }
                if self.invalid_type(u, e) {
                    let mut i = self.error(u, self.span(u, e), c::INVALID_CONSTANT);
                    i.unresolved = true;
                    return Constant::Invalid(Box::new(i));
                }
                self.inv(u, e, c::CONST_EVAL_METHOD_INVOCATION)
            }
            ExprKind::Property { target, name, .. } => self.property(cx, e, *target, *name, in_const),
            ExprKind::TypeArguments { target, .. } => {
                // Generic type literal or function instantiation: the exact
                // value of the type is not reconstructed here.
                match self.evaluate(cx, *target, in_const) {
                    Constant::Value(v) => match v.state {
                        InstanceState::Type(_) => Constant::Value(Value::new(self.core.type_, InstanceState::Type(None))),
                        InstanceState::Function { element, .. } => {
                            // `_instantiateFunctionType`: the instantiated type goes
                            // through the constructor's substitution.
                            let t = self.the_static(u, e);
                            let t = match &cx.types {
                                Some(m) => crate::ops::substitute(t, m, self.table),
                                None => t,
                            };
                            Constant::Value(self.value(t, InstanceState::Function { element, args: None }))
                        }
                        _ => self.inv(u, e, c::INVALID_CONSTANT),
                    },
                    i => i,
                }
            }
            ExprKind::Unary { op, operand } => self.unary(cx, e, *op, *operand, in_const),
            ExprKind::Binary { op, left, right } => self.binary(cx, e, *op, *left, *right, in_const),
            ExprKind::Conditional { condition, then, else_ } => {
                let cond = match self.evaluate(cx, *condition, in_const) {
                    Constant::Value(v) => v,
                    i => return i,
                };
                if !cond.state.is_bool() {
                    return self.inv(u, *condition, c::CONST_EVAL_TYPE_BOOL);
                }
                match cond.as_bool() {
                    Some(true) => {
                        if let Some(i) = self.not_potentially_constant(cx, *else_) {
                            return i;
                        }
                        self.evaluate(cx, *then, in_const)
                    }
                    Some(false) => {
                        if let Some(i) = self.not_potentially_constant(cx, *then) {
                            return i;
                        }
                        self.evaluate(cx, *else_, in_const)
                    }
                    None => {
                        if let i @ Constant::Invalid(_) = self.evaluate(cx, *then, in_const) {
                            return i;
                        }
                        if let i @ Constant::Invalid(_) = self.evaluate(cx, *else_, in_const) {
                            return i;
                        }
                        let t = self.the_static(u, e);
                        Constant::Value(self.unknown(t))
                    }
                }
            }
            ExprKind::Is { value, negated, .. } => {
                let v = match self.evaluate(cx, *value, in_const) {
                    Constant::Value(v) => v,
                    i => return i,
                };
                // The tested type is not reconstructed here: the result is
                // unknown (never proves an error).
                let _ = (v, negated);
                Constant::Value(Value::bool_(self.core, None))
            }
            ExprKind::As { value, ty } => {
                let ty = *ty;
                let v = match self.evaluate(cx, *value, in_const) {
                    Constant::Value(v) => v,
                    i => return i,
                };
                // `node.type.type` (`visitAsExpression`): the written type, not the
                // static type of the `as` (which the implicit instantiation changes).
                let tgt = self
                    .body
                    .units
                    .get(u.0 as usize)
                    .and_then(|b| b.annotation_types.get(&ty).copied())
                    .or_else(|| self.outline.written_types.get(&(u, ty)).copied())
                    .unwrap_or_else(|| self.the_static(u, e));
                // The type of the `as` with the constructor's lexical environment
                // applied (`_substitution`, `x as List<T>` in `C<int>`): only what
                // still mentions a type parameter is left unchecked.
                let tgt = match &cx.types {
                    Some(m) => crate::ops::substitute(tgt, m, self.table),
                    None => tgt,
                };
                if v.ty == self.core.dynamic_ || tgt == self.core.dynamic_ || self.mentions_parameter(tgt) {
                    return Constant::Value(v);
                }
                let tgt = self.erase(tgt);
                if !self.sub(v.ty, tgt) {
                    return self.inv(u, e, c::CONST_EVAL_THROWS_EXCEPTION);
                }
                Constant::Value(v)
            }
            ExprKind::DotShorthand { .. } => {
                // A shorthand resolved to a static member or enum constant.
                match self.resolved(u, e) {
                    Some(Resolved::Element(Element::Class(d))) => {
                        let d = *d;
                        let ExprKind::DotShorthand { name, .. } = &a.expr(e).kind else { unreachable!() };
                        match self.static_variable_member(d, name.sym) {
                            Some(v) => self.variable_reference_value(cx, e, v),
                            None => self.generic(u, e, false),
                        }
                    }
                    _ => self.generic(u, e, true),
                }
            }
            _ => self.generic(u, e, false),
        }
    }

    pub(crate) fn mentions_parameter(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::TypeParameter { .. } | Type::Intersection { .. } => true,
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| self.mentions_parameter(*a)),
            Type::FutureOr { arg, .. } => self.mentions_parameter(*arg),
            Type::Function { ret, positional, optional, named, .. } => {
                self.mentions_parameter(*ret)
                    || positional.iter().chain(optional.iter()).any(|a| self.mentions_parameter(*a))
                    || named.iter().any(|(_, a, _)| self.mentions_parameter(*a))
            }
            Type::Record { positional, named, .. } => {
                positional.iter().any(|a| self.mentions_parameter(*a)) || named.iter().any(|(_, a)| self.mentions_parameter(*a))
            }
            _ => false,
        }
    }

    /// The span of the `new`/`const` keyword of an explicit creation.
    fn creation_keyword(&self, u: UnitId, e: ExprId, has: bool) -> Option<Span> {
        if !has {
            return None;
        }
        let s = self.span(u, e);
        let source = &self.program.unit(u).source;
        let text = source.get(s.start..s.end)?;
        let n = text.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(text.len());
        Some(Span { start: s.start, end: s.start + n })
    }

    /// `evaluateAndFormatErrorsInConstructorCall`: a runtime exception becomes
    /// `CONST_EVAL_THROWS_EXCEPTION` at the creation.
    pub fn format_constructor_error(&self, u: UnitId, e: ExprId, r: R) -> R {
        match r {
            Constant::Invalid(i) if i.exception => {
                let mut n = self.error(u, self.span(u, e), c::CONST_EVAL_THROWS_EXCEPTION);
                n.avoid_report = i.avoid_report;
                // The exception's context, in the library's file
                // (`library.source`), after those the error already had.
                let text = dartforge_diagnostics::Diagnostic::with_code(i.code, i.span, i.args.iter().map(|s| s.as_str())).message;
                let defining = self.program.library(self.program.unit(u).library).units.first().copied().unwrap_or(u);
                n.context = i.context.clone();
                n.context.push((defining, i.span, format!("The exception is '{text}' and occurs here.")));
                Constant::Invalid(Box::new(n))
            }
            r => r,
        }
    }

    // -- Strings ---------------------------------------------------------------

    fn string(&mut self, cx: &Ctx, e: ExprId, lit: &ast::StringLit, in_const: bool) -> R {
        let u = cx.unit;
        let mut acc: Vec<u16> = Vec::new();
        let mut unknown = false;
        for p in lit.parts.iter() {
            match p {
                ast::StringPart::Text(t) => acc.extend(t.code_units()),
                ast::StringPart::Interpolation(x) => {
                    let v = match self.evaluate(cx, *x, in_const) {
                        Constant::Value(v) => v,
                        i => return i,
                    };
                    if !v.state.is_bool_num_string_or_null() {
                        let s = self.interpolation_span(u, *x);
                        return Constant::Invalid(Box::new(self.error(u, s, c::CONST_EVAL_TYPE_BOOL_NUM_STRING)));
                    }
                    match v.state.to_text(&|_| String::new(), &|_| String::new()) {
                        Ok(InstanceState::Str(Some(s))) => acc.extend(s.iter()),
                        Ok(_) => unknown = true,
                        Err(x) => return self.exception(u, e, x),
                    }
                }
            }
        }
        let st = if unknown { None } else { Some(acc.into()) };
        Constant::Value(Value::new(self.core.string, InstanceState::Str(st)))
    }

    /// The range of the `InterpolationExpression`: `$x` or `${e}`.
    fn interpolation_span(&self, u: UnitId, x: ExprId) -> Span {
        let s = self.span(u, x);
        let b = self.program.unit(u).source.as_bytes();
        if s.start >= 1 && b[s.start - 1] == b'$' {
            return Span { start: s.start - 1, end: s.end };
        }
        let mut i = s.start;
        while i > 0 && b[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        if i >= 2 && &b[i - 2..i] == b"${" {
            let mut j = s.end;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let end = if j < b.len() && b[j] == b'}' { j + 1 } else { s.end };
            return Span { start: i - 2, end: end };
        }
        s
    }

    // -- Collections -------------------------------------------------------------

    /// `_buildListConstant`: `None` complete; `Some(Ok(()))` unknown (unknown
    /// `if` condition); `Some(Err)` error.
    fn build_list(
        &mut self,
        cx: &Ctx,
        list: &mut Vec<Value>,
        elems: &[CollectionElement],
        const_child: bool,
    ) -> Option<Result<(), Box<Invalid>>> {
        let u = cx.unit;
        for el in elems {
            match el {
                CollectionElement::Expression(x) => match self.evaluate(cx, *x, const_child) {
                    Constant::Value(v) => list.push(v),
                    Constant::Invalid(i) => return Some(Err(i)),
                },
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.evaluate(cx, *condition, const_child) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if cond.actually_unknown() {
                        return Some(Ok(()));
                    }
                    match cond.as_bool() {
                        None => {
                            return Some(Err(Box::new(self.error(u, self.span(u, *condition), c::NON_BOOL_CONDITION))));
                        }
                        Some(true) => {
                            if let Some(Err(i)) = self.build_list(cx, list, std::slice::from_ref(then), const_child) {
                                return Some(Err(i));
                            }
                        }
                        Some(false) => {
                            if let Some(el) = else_ {
                                if let Some(Err(i)) = self.build_list(cx, list, std::slice::from_ref(el), const_child) {
                                    return Some(Err(i));
                                }
                            }
                        }
                    }
                }
                CollectionElement::MapEntry { .. } => {
                    return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::MAP_ENTRY_NOT_IN_MAP))));
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.evaluate(cx, *value, const_child) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if v.state.is_null() && *null_aware {
                        continue;
                    }
                    match v.as_list().or_else(|| v.as_set()) {
                        Some(xs) => list.extend(xs.iter().cloned()),
                        None => {
                            return Some(Err(Box::new(self.error(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_LIST_OR_SET))));
                        }
                    }
                }
                CollectionElement::NullAwareExpression(x) => {
                    let v = match self.evaluate(cx, *x, const_child) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if v.state.is_null() {
                        continue;
                    }
                    list.push(v);
                }
            }
        }
        None
    }

    fn build_set(&mut self, cx: &Ctx, set: &mut Vec<Value>, elems: &[CollectionElement]) -> Option<Result<(), Box<Invalid>>> {
        let u = cx.unit;
        for el in elems {
            match el {
                CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => match self.evaluate(cx, *x, true) {
                    Constant::Value(v) => {
                        if matches!(el, CollectionElement::NullAwareExpression(_)) && v.state.is_null() {
                            continue;
                        }
                        self.insert_into_set(set, v);
                    }
                    Constant::Invalid(i) => return Some(Err(i)),
                },
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.evaluate(cx, *condition, true) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if cond.actually_unknown() {
                        return Some(Ok(()));
                    }
                    let branch = match cond.as_bool() {
                        None => return Some(Err(Box::new(self.error(u, self.span(u, *condition), c::NON_BOOL_CONDITION)))),
                        Some(true) => Some(&**then),
                        Some(false) => else_.as_deref(),
                    };
                    if let Some(r) = branch {
                        if let Some(Err(i)) = self.build_set(cx, set, std::slice::from_ref(r)) {
                            return Some(Err(i));
                        }
                    }
                }
                CollectionElement::MapEntry { .. } => {
                    return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::MAP_ENTRY_NOT_IN_MAP))));
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.evaluate(cx, *value, true) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if v.state.is_null() && *null_aware {
                        continue;
                    }
                    match v.as_set().or_else(|| v.as_list()) {
                        Some(xs) => {
                            for x in xs.iter() {
                                self.insert_into_set(set, x.clone());
                            }
                        }
                        None => {
                            return Some(Err(Box::new(self.error(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_LIST_OR_SET))));
                        }
                    }
                }
            }
        }
        None
    }

    fn insert_into_set(&mut self, set: &mut Vec<Value>, v: Value) {
        if !set.iter().any(|x| self.equal(x, &v)) {
            set.push(v);
        }
    }

    fn build_map(&mut self, cx: &Ctx, map: &mut Vec<(Value, Value)>, elems: &[CollectionElement]) -> Option<Result<(), Box<Invalid>>> {
        let u = cx.unit;
        for el in elems {
            match el {
                CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => {
                    return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::EXPRESSION_IN_MAP))));
                }
                CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                    return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::CONST_EVAL_FOR_ELEMENT))));
                }
                CollectionElement::If { condition, case_pattern, then, else_, .. } => {
                    if case_pattern.is_some() {
                        return Some(Err(Box::new(self.error(u, self.element_span(u, el), c::INVALID_CONSTANT))));
                    }
                    let cond = match self.evaluate(cx, *condition, true) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if cond.actually_unknown() {
                        return Some(Ok(()));
                    }
                    let branch = match cond.as_bool() {
                        None => return Some(Err(Box::new(self.error(u, self.span(u, *condition), c::NON_BOOL_CONDITION)))),
                        Some(true) => Some(&**then),
                        Some(false) => else_.as_deref(),
                    };
                    if let Some(r) = branch {
                        if let Some(Err(i)) = self.build_map(cx, map, std::slice::from_ref(r)) {
                            return Some(Err(i));
                        }
                    }
                }
                CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
                    let k = self.evaluate(cx, *key, true);
                    let v = self.evaluate(cx, *value, true);
                    let k = match k {
                        Constant::Value(k) => k,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    let v = match v {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if (*null_aware_key && k.state.is_null()) || (*null_aware_value && v.state.is_null()) {
                        continue;
                    }
                    if let Some(p) = map.iter().position(|(x, _)| self.equal(x, &k)) {
                        map[p].1 = v;
                    } else {
                        map.push((k, v));
                    }
                }
                CollectionElement::Spread { value, null_aware } => {
                    let v = match self.evaluate(cx, *value, true) {
                        Constant::Value(v) => v,
                        Constant::Invalid(i) => return Some(Err(i)),
                    };
                    if v.state.is_null() && *null_aware {
                        continue;
                    }
                    match v.as_map() {
                        Some(xs) => {
                            for (k, x) in xs.iter() {
                                if let Some(p) = map.iter().position(|(y, _)| self.equal(y, k)) {
                                    map[p].1 = x.clone();
                                } else {
                                    map.push((k.clone(), x.clone()));
                                }
                            }
                        }
                        None => return Some(Err(Box::new(self.error(u, self.span(u, *value), c::CONST_SPREAD_EXPECTED_MAP)))),
                    }
                }
            }
        }
        None
    }

    /// The range of a collection element.
    pub fn element_span(&self, u: UnitId, el: &CollectionElement) -> Span {
        let a = self.ast(u);
        let source = self.program.unit(u).source.as_bytes();
        match el {
            CollectionElement::Expression(x) => a.expr(*x).span,
            CollectionElement::NullAwareExpression(x) => {
                let s = a.expr(*x).span;
                Span { start: back_up_to(source, s.start, b'?'), end: s.end }
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let (k, v) = (a.expr(*key).span, a.expr(*value).span);
                Span { start: back_up_to(source, k.start, b'?'), end: v.end }
            }
            CollectionElement::Spread { value, .. } => {
                let s = a.expr(*value).span;
                Span { start: back_up_word(source, s.start, &["...?", "..."]), end: s.end }
            }
            CollectionElement::If { condition, then, else_, .. } => {
                let s = a.expr(*condition).span;
                let end = else_.as_ref().map(|x| self.element_span(u, x)).unwrap_or_else(|| self.element_span(u, then)).end;
                Span { start: back_up_word(source, s.start, &["if"]), end: end }
            }
            CollectionElement::For { body, init, condition, updates, .. } => {
                let end = self.element_span(u, body).end;
                let first = condition
                    .map(|x| a.expr(x).span.start)
                    .or_else(|| updates.first().map(|x| a.expr(*x).span.start))
                    .unwrap_or(end);
                let _ = init;
                Span { start: for_start(source, first), end: end }
            }
            CollectionElement::ForIn { body, iterable, .. } => {
                let end = self.element_span(u, body).end;
                Span { start: for_start(source, a.expr(*iterable).span.start), end: end }
            }
        }
    }

    // -- Operadores ------------------------------------------------------------

    fn is_of_extension(&self, u: UnitId, e: ExprId) -> Option<Code> {
        match self.resolved(u, e)? {
            Resolved::ExtensionMember { .. } => Some(c::CONST_EVAL_EXTENSION_METHOD),
            Resolved::Member { class, .. } if self.program.class(*class).kind == ClassKind::ExtensionType => {
                Some(c::CONST_EVAL_EXTENSION_TYPE_METHOD)
            }
            _ => None,
        }
    }

    fn unary(&mut self, cx: &Ctx, e: ExprId, op: UnaryOp, operand: ExprId, in_const: bool) -> R {
        let u = cx.unit;
        if let Some(code) = self.is_of_extension(u, e) {
            return self.inv(u, e, code);
        }
        // Postfix operators (`a++`, `a!`) have no visit of their own
        // (`visitNode`): the generic error at the node. `visitPrefixExpression`:
        // the operand first (its error wins); only then are `++`/`--` the
        // generic error.
        if matches!(op, UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert) {
            return self.generic(u, e, false);
        }
        let v = match self.evaluate(cx, operand, in_const) {
            Constant::Value(v) => v,
            i => return i,
        };
        if !matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot) {
            return self.generic(u, e, false);
        }
        let r = match op {
            UnaryOp::Not => v.state.logical_not().map(|s| (self.core.bool_, s)),
            UnaryOp::BitNot => v.state.negate_bits().map(|s| (self.core.int, s)),
            _ => v.state.negate().map(|s| {
                let t = if matches!(s, InstanceState::Double(_)) { self.core.double } else { self.core.int };
                (t, s)
            }),
        };
        match r {
            Ok((t, s)) => Constant::Value(Value::new(t, s)),
            Err(x) => self.exception(u, e, x),
        }
    }

    fn binary(&mut self, cx: &Ctx, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId, in_const: bool) -> R {
        let u = cx.unit;
        if let Some(code) = self.is_of_extension(u, e) {
            return self.inv(u, e, code);
        }
        let l = match self.evaluate(cx, left, in_const) {
            Constant::Value(v) => v,
            i => return i,
        };
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let short_circuit = if op == BinaryOp::And { Some(false) } else { Some(true) };
                if l.as_bool() == short_circuit {
                    if let Some(i) = self.not_potentially_constant(cx, right) {
                        return i;
                    }
                }
                // `lazyAnd`/`lazyOr`.
                let r = (|| -> Result<InstanceState, Exception> {
                    match &l.state {
                        InstanceState::Bool(v) => {
                            if *v == short_circuit {
                                return Ok(InstanceState::Bool(*v));
                            }
                            let d = match self.evaluate(cx, right, in_const) {
                                Constant::Value(d) => d,
                                Constant::Invalid(i) => return Err(Exception::new(i.code)),
                            };
                            if !d.state.is_bool() {
                                return Err(Exception::new(c::CONST_EVAL_TYPE_BOOL));
                            }
                            if v.is_none() {
                                return Ok(InstanceState::Bool(None));
                            }
                            d.state.to_bool()
                        }
                        _ => Err(Exception::new(c::CONST_EVAL_TYPE_BOOL)),
                    }
                })();
                return match r {
                    Ok(s) => Constant::Value(Value::new(self.core.bool_, s)),
                    Err(x) => self.exception(u, e, x),
                };
            }
            BinaryOp::IfNull => {
                if !l.state.is_null() {
                    if let Some(i) = self.not_potentially_constant(cx, right) {
                        return i;
                    }
                    return Constant::Value(l);
                }
                return self.evaluate(cx, right, in_const);
            }
            _ => {}
        }
        let r = match self.evaluate(cx, right, in_const) {
            Constant::Value(v) => v,
            i => return i,
        };
        let res: Result<(TypeId, InstanceState), Exception> = match op {
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                if l.state.is_bool() && r.state.is_bool() {
                    let s = match op {
                        BinaryOp::BitAnd => l.state.logical_and(&r.state),
                        BinaryOp::BitOr => l.state.logical_or(&r.state),
                        _ => l.state.logical_xor(&r.state),
                    };
                    s.map(|s| (self.core.bool_, s))
                } else if l.state.is_int() && r.state.is_int() {
                    let s = match op {
                        BinaryOp::BitAnd => l.state.bit_and(&r.state),
                        BinaryOp::BitOr => l.state.bit_or(&r.state),
                        _ => l.state.bit_xor(&r.state),
                    };
                    s.map(|s| (self.core.int, s))
                } else {
                    Err(Exception::new(c::CONST_EVAL_TYPE_BOOL_INT))
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let lib = cx.lib;
                self.equal_equal(lib, &l, &r).map(|b| {
                    let b = if op == BinaryOp::NotEq { b.map(|x| !x) } else { b };
                    (self.core.bool_, InstanceState::Bool(b))
                })
            }
            BinaryOp::Gt => l.state.greater(&r.state).map(|s| (self.core.bool_, s)),
            BinaryOp::GtEq => l.state.greater_or_equal(&r.state).map(|s| (self.core.bool_, s)),
            BinaryOp::Lt => l.state.less(&r.state).map(|s| (self.core.bool_, s)),
            BinaryOp::LtEq => l.state.less_or_equal(&r.state).map(|s| (self.core.bool_, s)),
            BinaryOp::Shl => l.state.shift_left(&r.state).map(|s| (self.core.int, s)),
            BinaryOp::Shr => l.state.shift_right(&r.state).map(|s| (self.core.int, s)),
            BinaryOp::UShr => l.state.logical_shift_right(&r.state).map(|s| (self.core.int, s)),
            BinaryOp::TruncDiv => l.state.integer_divide(&r.state).map(|s| (self.core.int, s)),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let s = match op {
                    BinaryOp::Add => l.state.add(&r.state),
                    BinaryOp::Sub => l.state.subtract(&r.state),
                    BinaryOp::Mul => l.state.multiply(&r.state),
                    BinaryOp::Div => l.state.divide(&r.state),
                    _ => l.state.rest(&r.state),
                };
                s.map(|s| {
                    let t = match s {
                        InstanceState::Int(_) => self.core.int,
                        InstanceState::Double(_) => self.core.double,
                        _ => self.core.string,
                    };
                    (t, s)
                })
            }
            _ => return self.generic(u, e, false),
        };
        match res {
            Ok((t, s)) => Constant::Value(Value::new(t, s)),
            Err(x) => self.exception(u, e, x),
        }
    }

    /// `equalEqual`: with `patterns` on (3.0+), primitive or `double`.
    fn equal_equal(&mut self, lib: LibraryId, l: &Value, r: &Value) -> Result<Option<bool>, Exception> {
        if l.state.is_null() || r.state.is_null() {
            return Ok(Some(l.state.is_null() && r.state.is_null()));
        }
        let patterns = self.program.library(lib).features.version() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
        let can = if patterns {
            matches!(l.state, InstanceState::Double(_)) || self.primitive_equality(l, lib)
        } else {
            l.state.is_bool_num_string_or_null()
        };
        if !can {
            return Err(Exception::new(c::CONST_EVAL_TYPE_BOOL_NUM_STRING));
        }
        Ok(self.identical_state(&l.state, &r.state))
    }

    /// `identical` (`isIdentical2`).
    pub fn identical(&mut self, l: &Value, r: &Value) -> Option<bool> {
        let (int, double) = (self.core.int, self.core.double);
        if (l.ty == int && r.ty == double) || (l.ty == double && r.ty == int) {
            return None;
        }
        if l.ty != r.ty {
            return Some(false);
        }
        self.identical_state(&l.state, &r.state)
    }

    /// `InstanceState.isIdentical` (and `equalEqual`, which is the same here).
    fn identical_state(&mut self, l: &InstanceState, r: &InstanceState) -> Option<bool> {
        match (l, r) {
            (InstanceState::Bool(a), b) => {
                let a = (*a)?;
                match b {
                    InstanceState::Bool(b) => Some(a == (*b)?),
                    _ => Some(false),
                }
            }
            (InstanceState::Int(a), b) => {
                let a = (*a)?;
                match b {
                    InstanceState::Int(b) => Some(a == (*b)?),
                    InstanceState::Double(b) => Some((*b)? == a as f64),
                    _ => Some(false),
                }
            }
            (InstanceState::Double(a), b) => {
                let a = (*a)?;
                if a.is_nan() {
                    return Some(false);
                }
                match b {
                    InstanceState::Double(b) => {
                        let b = (*b)?;
                        if b.is_nan() {
                            return Some(false);
                        }
                        Some(a.to_bits() == b.to_bits())
                    }
                    InstanceState::Int(b) => Some(a == (*b)? as f64),
                    _ => Some(false),
                }
            }
            (InstanceState::Str(a), b) => {
                let a = a.as_ref()?;
                match b {
                    InstanceState::Str(b) => Some(a == b.as_ref()?),
                    _ => Some(false),
                }
            }
            (InstanceState::Null { .. }, b) => Some(b.is_null()),
            (InstanceState::Symbol(a), b) => {
                let a = a.as_ref()?;
                match b {
                    InstanceState::Symbol(b) => Some(a == b.as_ref()?),
                    _ => Some(false),
                }
            }
            (InstanceState::Type(a), b) => {
                let a = (*a)?;
                match b {
                    InstanceState::Type(b) => Some(a == (*b)?),
                    _ => Some(false),
                }
            }
            (InstanceState::Function { element: a, args: x }, b) => match b {
                InstanceState::Function { element: b, args: y } => Some(a == b && x == y),
                _ => Some(false),
            },
            (InstanceState::List { unknown, .. }, b) | (InstanceState::Set { unknown, .. }, b) | (InstanceState::Map { unknown, .. }, b)
                if *unknown || b.unknown() =>
            {
                None
            }
            (InstanceState::Record { .. }, _) => {
                if !self.equal_states(l, r) {
                    Some(false)
                } else {
                    None
                }
            }
            _ => Some(self.equal_states(l, r)),
        }
    }

    /// `DartObjectImpl`'s `==` (the structural equality of map keys and set
    /// elements).
    pub fn equal(&mut self, a: &Value, b: &Value) -> bool {
        a.ty == b.ty && self.equal_states(&a.state, &b.state)
    }

    fn equal_states(&mut self, a: &InstanceState, b: &InstanceState) -> bool {
        // A value that is not known (an opaque constant from another library) is
        // not provably equal to anything: two `CompileTimeErrorCode.X` coming
        // from outside are not the same set element.
        if a.unknown() || b.unknown() {
            return false;
        }
        match (a, b) {
            (InstanceState::Bool(x), InstanceState::Bool(y)) => x == y,
            (InstanceState::Int(x), InstanceState::Int(y)) => x == y,
            (InstanceState::Double(x), InstanceState::Double(y)) => x.map(f64::to_bits) == y.map(f64::to_bits),
            (InstanceState::Str(x), InstanceState::Str(y)) => x == y,
            (InstanceState::Null { .. }, InstanceState::Null { .. }) => true,
            (InstanceState::Symbol(x), InstanceState::Symbol(y)) => x == y,
            (InstanceState::Type(x), InstanceState::Type(y)) => x == y,
            (InstanceState::Function { element: x, args: p }, InstanceState::Function { element: y, args: q }) => x == y && p == q,
            (InstanceState::List { elements: x, .. }, InstanceState::List { elements: y, .. })
            | (InstanceState::Set { elements: x, .. }, InstanceState::Set { elements: y, .. }) => {
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| self.equal(p, q))
            }
            (InstanceState::Map { entries: x, .. }, InstanceState::Map { entries: y, .. }) => {
                x.len() == y.len()
                    && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| self.equal(k, k2) && self.equal(v, v2)))
            }
            (InstanceState::Record { positional: p1, named: n1 }, InstanceState::Record { positional: p2, named: n2 }) => {
                p1.len() == p2.len()
                    && n1.len() == n2.len()
                    && p1.iter().zip(p2.iter()).all(|(a, b)| self.equal(a, b))
                    && n1.iter().all(|(n, v)| n2.iter().any(|(m, w)| m == n && self.equal(v, w)))
            }
            (InstanceState::Generic { fields: x, .. }, InstanceState::Generic { fields: y, .. }) => {
                x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && self.equal(v, v2)))
            }
            _ => false,
        }
    }

    /// `hasPrimitiveEquality`.
    pub fn primitive_equality(&mut self, v: &Value, lib: LibraryId) -> bool {
        match &v.state {
            InstanceState::Record { positional: positional_args, named: named_args } => {
                let (p, n) = (positional_args.clone(), named_args.clone());
                p.iter().all(|x| self.primitive_equality(x, lib)) && n.iter().all(|(_, x)| self.primitive_equality(x, lib))
            }
            InstanceState::Generic { .. } => {
                let Type::Interface { class, .. } = self.table.get(v.ty).clone() else { return false };
                let patterns = self.program.library(lib).features.version() >= dartforge_frontend::features::LanguageVersion::new(3, 0);
                let eq = self.interner.lookup("==");
                let hash = self.interner.lookup("hashCode");
                let of_object = |m: &Engine<'_>, s: Option<SymbolId>| match s.and_then(|s| m.concrete_member(class, s)) {
                    Some(c) => Some(c) == m.core.object_class,
                    None => true,
                };
                of_object(self, eq) && (!patterns || of_object(self, hash))
            }
            InstanceState::Double(_) => false,
            _ => true,
        }
    }

    /// The class that declares the concrete instance member `the_name` as seen from
    /// `c` (superclasses and mixins, in lookup order).
    fn concrete_member(&self, c: ClassId, the_name: SymbolId) -> Option<ClassId> {
        let mut current = Some(c);
        let mut seen = 0;
        while let Some(k) = current {
            seen += 1;
            if seen > 100 {
                return None;
            }
            let cls = self.program.class(k);
            if let Some(f) = cls.instance_members.get(&the_name) {
                if !self.program.function(*f).abstract_ {
                    return Some(k);
                }
            }
            for m in cls.mixin_classes.iter().rev() {
                if let Some(f) = self.program.class(*m).instance_members.get(&the_name) {
                    if !self.program.function(*f).abstract_ {
                        return Some(*m);
                    }
                }
            }
            current = cls.supertype_class;
        }
        None
    }

    // -- Names -----------------------------------------------------------------

    fn property(&mut self, cx: &Ctx, e: ExprId, tgt: ExprId, the_name: ast::Name, in_const: bool) -> R {
        let u = cx.unit;
        let a = self.ast(u);
        let simple_target = matches!(a.expr(tgt).kind, ExprKind::Identifier(_));
        let res_target = self.resolved(u, tgt).cloned();
        match res_target {
            // A top-level constant imported with a prefix; for a deferred prefix,
            // the deferred library error at the name (`visitPrefixedIdentifier`,
            // 3.6.2 `evaluation.dart:1108-1115`).
            Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))) => {
                if self.deferred_prefix(u, tgt) {
                    return self.deferred_library_error(u, e, the_name.span);
                }
                return self.constant_value(cx, e, e, true);
            }
            // `p.Ext.m` (`visitPropertyAccess`, `:1176-1185`): with `p`
            // deferred, the error at the extension's name.
            Some(Resolved::Element(Element::Extension(_))) => {
                if let ExprKind::Property { target: p, name: ext_name, .. } = &a.expr(tgt).kind
                    && matches!(self.resolved(u, *p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))))
                    && self.deferred_prefix(u, *p)
                {
                    let sp = ext_name.span;
                    return self.deferred_library_error(u, e, sp);
                }
                return self.constant_value(cx, e, e, true);
            }
            _ => {}
        }
        if let Some(Resolved::Element(Element::Class(k))) = &res_target {
            if self.program.class(*k).kind == ClassKind::ExtensionType && !simple_target {
                return self.constant_value(cx, e, e, true);
            }
        }
        let prefix = match self.evaluate(cx, tgt, in_const) {
            Constant::Value(v) => v,
            i => return i,
        };
        let is_interface = simple_target && matches!(res_target, Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))));
        // `C<T>.new`, `C.the_name`: the `ConstructorReference` (constructor
        // tear-off), not a property access on the `Type`.
        if matches!(self.resolved(u, e), Some(Resolved::Constructor(_))) {
            return self.constant_value(cx, e, e, true);
        }
        if !is_interface {
            if let Some(r) = self.property_access(u, e, &prefix, the_name) {
                return r;
            }
        }
        self.constant_value(cx, e, e, true)
    }

    /// `PrefixedIdentifier.isDeferred` (3.6.2 `ast.dart:14386-14396`): the prefix
    /// `p` (an identifier) belongs to exactly one import of the unit, and that
    /// import is `deferred`.
    fn deferred_prefix(&self, u: UnitId, p: ExprId) -> bool {
        let ExprKind::Identifier(n) = &self.ast(u).expr(p).kind else {
            return false;
        };
        let lib = self.program.unit(u).library;
        let mut imports = self.program.library(lib).imports.iter().filter(|i| i.unit == u && i.prefix == Some(n.sym));
        match (imports.next(), imports.next()) {
            (Some(i), None) => i.deferred,
            _ => false,
        }
    }

    /// `_getDeferredLibraryError` (3.6.2 `evaluation.dart:1873-1933`): the code
    /// from the first ancestor of `no` that decides it, at `tgt`; with none,
    /// `INVALID_CONSTANT` at `no` itself.
    fn deferred_library_error(&self, u: UnitId, no: ExprId, tgt: Span) -> R {
        match self.deferred_library_code(u, no) {
            Some(code) => Constant::Invalid(Box::new(self.error(u, tgt, code))),
            None => self.inv(u, no, c::INVALID_CONSTANT),
        }
    }

    /// The ascent of `_getDeferredLibraryError`: `Annotation`, `DefaultFormalParameter`,
    /// `IfElement` whose condition is `no` itself, `InstanceCreationExpression`,
    /// `ListLiteral`, `MapLiteralEntry` (key or value, by the child), `RecordLiteral`,
    /// `SetOrMapLiteral`, `SpreadElement`, `SwitchCase` (the old one), `SwitchPatternCase`
    /// and `VariableDeclaration`. The parents come from `dartforge_frontend::parents`;
    /// where the parent is not an expression (statement, pattern, default value…),
    /// the ascent continues through the smallest expression that contains the
    /// fragment (a function expression, a `switch` expression, the collection of
    /// an `if (… case …)`).
    fn deferred_library_code(&self, u: UnitId, no: ExprId) -> Option<Code> {
        let a = self.ast(u);
        let parents = crate::typed_lints::unit_parents(self.program, u);
        let lib = self.program.unit(u).library;
        let before_3 = self.program.library(lib).features.version() < dartforge_frontend::features::LanguageVersion::new(3, 0);
        let contains = |of: Span, sp: Span| of.start <= sp.start && sp.end <= of.end;
        let mut current = no;
        for _ in 0..a.exprs.len() + 1 {
            let sp = a.expr(current).span;
            let next_ = match parents.parent(current) {
                dartforge_frontend::parents::Parent::Expr(p) => Some(p),
                dartforge_frontend::parents::Parent::Annotation => return Some(c::INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY),
                dartforge_frontend::parents::Parent::Variable { .. } => return Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY),
                // `EnumConstantArguments` → `EnumConstantDeclaration` →
                // `EnumDeclaration`: none decides.
                dartforge_frontend::parents::Parent::EnumConstant => return None,
                dartforge_frontend::parents::Parent::ConstantPattern { .. } => {
                    // In the `case` of a `switch` statement, the case's code.
                    let in_case = a.stmts.iter().any(|s| match &s.kind {
                        ast::StmtKind::Switch { cases, .. } => {
                            cases.iter().any(|k| k.pattern.is_some_and(|p| contains(a.pattern(p).span, sp)))
                        }
                        _ => false,
                    });
                    if in_case {
                        return Some(if before_3 {
                            c::NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY
                        } else {
                            c::PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY
                        });
                    }
                    None
                }
                dartforge_frontend::parents::Parent::Boundary => {
                    let pattern = |ps: &[ast::Parameter]| ps.iter().any(|p| p.default_value == Some(current));
                    fn nested(ps: &[ast::Parameter], current: ExprId) -> bool {
                        ps.iter().any(|p| {
                            p.default_value == Some(current) || p.function_parameters.as_deref().is_some_and(|fs| nested(fs, current))
                        })
                    }
                    let is_pattern = a.members.iter().any(|m| matches!(&m.kind, ast::MemberKind::Constructor(k) if pattern(&k.parameters) || nested(&k.parameters, current)))
                        || a.functions.iter().any(|f| f.parameters.as_deref().is_some_and(|ps| nested(ps, current)))
                        || a.types.iter().any(|t| matches!(&t.kind, ast::TypeKind::Function { parameters, .. } if nested(parameters, current)));
                    if is_pattern {
                        return Some(c::NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY);
                    }
                    None
                }
            };
            // A parent that is not an expression: the smallest expression containing
            // the fragment.
            let p = match next_ {
                Some(p) => p,
                None => {
                    let mut best: Option<(usize, ExprId)> = None;
                    for (i, x) in a.exprs.iter().enumerate() {
                        let id = ExprId(i as u32);
                        if id == current || !contains(x.span, sp) || x.span == sp {
                            continue;
                        }
                        let size = x.span.end - x.span.start;
                        if best.is_none_or(|(t, _)| size < t) {
                            best = Some((size, id));
                        }
                    }
                    match best {
                        Some((_, id)) => id,
                        None => return None,
                    }
                }
            };
            if let Some(code) = self.role_on_ascent(u, p, sp, no) {
                return Some(code);
            }
            current = p;
        }
        None
    }

    /// The code that expression `p`, parent of fragment `child`, decides in the
    /// ascent.
    fn role_on_ascent(&self, u: UnitId, p: ExprId, child: Span, no: ExprId) -> Option<Code> {
        let a = self.ast(u);
        let contains = |of: Span, sp: Span| of.start <= sp.start && sp.end <= of.end;
        // The collection element that contains the child, from deepest to
        // shallowest.
        fn the_element(a: &ast::Ast, el: &ast::CollectionElement, child: Span, no: ExprId, contains: &dyn Fn(Span, Span) -> bool) -> Option<Option<Code>> {
            let span = |e: ExprId| a.expr(e).span;
            match el {
                ast::CollectionElement::Expression(e) | ast::CollectionElement::NullAwareExpression(e) => {
                    contains(span(*e), child).then_some(None)
                }
                ast::CollectionElement::MapEntry { key, value, .. } => {
                    if contains(span(*key), child) {
                        Some(Some(c::NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY))
                    } else if contains(span(*value), child) {
                        Some(Some(c::NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY))
                    } else {
                        None
                    }
                }
                ast::CollectionElement::Spread { value, .. } => {
                    contains(span(*value), child).then_some(Some(c::SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY))
                }
                ast::CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                    for sub in std::iter::once(&**then).chain(else_.as_deref()) {
                        if let Some(r) = the_element(a, sub, child, no, contains) {
                            return Some(r);
                        }
                    }
                    let inside = contains(span(*condition), child)
                        || case_pattern.is_some_and(|p| contains(a.pattern(p).span, child))
                        || guard.is_some_and(|g| contains(span(g), child));
                    if !inside {
                        return None;
                    }
                    Some((*condition == no).then_some(c::IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY))
                }
                ast::CollectionElement::For { condition, updates, body, .. } => {
                    if let Some(r) = the_element(a, body, child, no, contains) {
                        return Some(r);
                    }
                    let inside = condition.is_some_and(|x| contains(span(x), child)) || updates.iter().any(|x| contains(span(*x), child));
                    inside.then_some(None)
                }
                ast::CollectionElement::ForIn { iterable, body, .. } => {
                    if let Some(r) = the_element(a, body, child, no, contains) {
                        return Some(r);
                    }
                    contains(span(*iterable), child).then_some(None)
                }
            }
        }
        match &a.expr(p).kind {
            ExprKind::InstanceCreation { .. } => Some(c::CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY),
            ExprKind::Call { .. } if matches!(self.resolved(u, p), Some(Resolved::Constructor(_))) => {
                Some(c::CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY)
            }
            ExprKind::Record { .. } => Some(c::NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                let from_literal = if matches!(a.expr(p).kind, ExprKind::List { .. }) {
                    c::NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY
                } else {
                    c::SET_ELEMENT_FROM_DEFERRED_LIBRARY
                };
                for el in elements.iter() {
                    if let Some(r) = the_element(a, el, child, no, &contains) {
                        return Some(r.unwrap_or(from_literal));
                    }
                }
                Some(from_literal)
            }
            _ => None,
        }
    }

    /// `_evaluatePropertyAccess`.
    fn property_access(&mut self, u: UnitId, e: ExprId, tgt: &Value, the_name: ast::Name) -> Option<R> {
        let res = self.resolved(u, e).cloned();
        let static_ = |m: &Engine<'_>, f: FunctionElementId| m.program.function(f).static_;
        match &res {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) if static_(self, *f) => return None,
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) if self.program.variable(*v).static_ => return None,
            Some(Resolved::ExtensionMember { .. }) => return Some(self.inv(u, e, c::CONST_EVAL_EXTENSION_METHOD)),
            Some(Resolved::Member { class, .. }) if self.program.class(*class).kind == ClassKind::ExtensionType => {
                return Some(self.inv(u, e, c::CONST_EVAL_EXTENSION_TYPE_METHOD));
            }
            _ => {}
        }
        if Some(the_name.sym) == self.sym_length && tgt.ty == self.core.string {
            return Some(match tgt.state.length() {
                Ok(s) => Constant::Value(Value::new(self.core.int, s)),
                Err(x) => self.exception(u, e, x),
            });
        }
        let mut i = self.error(u, self.span(u, e), c::CONST_EVAL_PROPERTY_ACCESS);
        i.args = vec![self.interner.resolve(the_name.sym).to_string(), self.format(tgt.ty)];
        Some(Constant::Invalid(Box::new(i)))
    }

    /// `tgt(args)` that creates through the primary constructor of an extension
    /// type: the target is the type (`A`) or `A.the_name` with the primary's name.
    /// Outside a constant context it is the generic one; the primary without
    /// `const`, too.
    fn creation_via_extension_primary(&mut self, cx: &Ctx, e: ExprId, tgt: ExprId, arguments: &ast::Arguments, in_const: bool) -> Option<R> {
        let u = cx.unit;
        let a = self.ast(u);
        let (cls, the_name) = match &a.expr(tgt).kind {
            ExprKind::Identifier(_) => (self.resolved(u, tgt).cloned(), None),
            ExprKind::Property { target, name, .. } => (self.resolved(u, *target).cloned(), Some(name.sym)),
            _ => return None,
        };
        let Some(Resolved::Element(Element::Class(k))) = cls else { return None };
        self.extension_primary(cx, e, k, the_name, arguments, in_const)
    }

    /// The creation through the primary constructor of extension type `k`
    /// (`the_name`: that of the written constructor): it is worth the
    /// representation, the evaluated argument.
    fn extension_primary(&mut self, cx: &Ctx, e: ExprId, k: ClassId, the_name: Option<SymbolId>, arguments: &ast::Arguments, in_const: bool) -> Option<R> {
        let u = cx.unit;
        let d = self.program.class(k).decl?;
        let ast::DeclKind::ExtensionType(et) = &self.ast(d.unit).decl(d.decl).kind else { return None };
        let of_primary = match (the_name, et.constructor) {
            (None, None) => true,
            (Some(n), Some(c)) => n == c.sym,
            (Some(n), None) => self.interner.resolve(n) == "new",
            (None, Some(_)) => false,
        };
        if !of_primary {
            return None;
        }
        if !in_const || !et.const_ {
            return Some(self.generic(u, e, false));
        }
        let x = arguments.args.iter().find(|x| x.name.is_none())?;
        Some(self.evaluate(cx, x.value, true))
    }

    /// The implicit instantiation of tear-off `e` (`typeArgumentTypes` of a
    /// `FunctionReference` without written arguments) with some argument that
    /// mentions a type parameter after the constructor's lexical environment
    /// (3.6.2 `evaluation.dart:862-876`).
    fn instantiation_with_parameter(&mut self, cx: &Ctx, e: ExprId) -> bool {
        let inferred: Option<Vec<TypeId>> = self.body.units.get(cx.unit.0 as usize).and_then(|b| b.tearoff_instantiation(e)).map(|a| a.to_vec());
        let Some(args) = inferred else { return false };
        // Only an argument that is itself a type parameter goes through
        // `_lexicalTypeEnvironment` (`:866-871`); `List<U>` stays as it is.
        args.iter().any(|&a| {
            let a = match (self.table.get(a), &cx.types) {
                (Type::TypeParameter { param, .. }, Some(m)) => m.get(param).copied().unwrap_or(a),
                _ => a,
            };
            self.mentions_parameter(a)
        })
    }

    /// `_getConstantValue` of the element that `e` resolves to (`error_at` is the
    /// error node).
    fn constant_value(&mut self, cx: &Ctx, e: ExprId, error_at: ExprId, with_identifier: bool) -> R {
        let u = cx.unit;
        let res = self.resolved(u, e).cloned();
        let _ = with_identifier;
        match res {
            Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                // A variable holding a generic function, implicitly instantiated:
                // the value first, then the arguments.
                let r = self.variable_reference_value(cx, error_at, v);
                if matches!(r, Constant::Value(_)) && self.instantiation_with_parameter(cx, e) {
                    return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
                }
                return r;
            }
            // An extension static cited by its simple name (the `foo` of
            // `@A(foo)` in a member of the extension) is a static
            // `ExecutableElement` like a class's: the value is the function
            // (`_getConstantValue`, 3.6.2 `evaluation.dart`).
            Some(Resolved::Element(Element::Function(f)))
            | Some(Resolved::Member { member: MemberRef::Function(f), .. })
            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                let fe = self.program.function(f);
                // An enum's `values`: the synthetic `const` field whose initializer
                // is the list of the constants (`LibraryBuilder.buildEnumChildren`);
                // in the model, a static getter without a node.
                if fe.node == FunctionRef::None
                    && fe.static_
                    && fe.kind == FunctionKind::Getter
                    && let Some(k) = fe.class
                    && self.program.class(k).kind == ClassKind::Enum
                    && self.interner.resolve(fe.name) == "values"
                {
                    return self.enum_values(u, error_at, k);
                }
                if fe.kind == FunctionKind::ImplicitAccessor {
                    if let Some(v) = fe.variable {
                        return self.variable_reference_value(cx, error_at, v);
                    }
                }
                let the_static_ = fe.class.is_none() && fe.extension.is_none() || fe.static_;
                if the_static_ && matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator) {
                    // Implicit instantiation of the generic tear-off
                    // (`visitFunctionReference` without written arguments, 3.6.2
                    // `evaluation.dart:862-876`): an inferred argument that mentions a
                    // type parameter (after the constructor's lexical environment) is
                    // `CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF`.
                    if self.instantiation_with_parameter(cx, e) {
                        return self.inv(u, e, c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF);
                    }
                    let t = self.outline.functions.get(f.0 as usize).map(|d| d.signature).unwrap_or(self.core.dynamic_);
                    let t = if self.inferred_(u) { self.the_static(u, e) } else { t };
                    // `_instantiateFunctionType` (`evaluation.dart:1939-1959`):
                    // the instantiated type goes through the constructor's substitution.
                    let t = match &cx.types {
                        Some(m) => crate::ops::substitute(t, m, self.table),
                        None => t,
                    };
                    return Constant::Value(self.value(t, InstanceState::Function { element: Function::Element(f), args: None }));
                }
            }
            Some(Resolved::Constructor(f)) => {
                let t = self.the_static(u, e);
                return Constant::Value(self.value(t, InstanceState::Function { element: Function::Element(f), args: None }));
            }
            Some(Resolved::Element(Element::Class(k))) => {
                let n = self.program.class(k).type_params.len();
                let args = vec![self.core.dynamic_; n];
                let t = if self.program.class(k).kind == ClassKind::ExtensionType {
                    self.table.intern(Type::ExtensionType { decl: k, args: args.into_boxed_slice(), nullable: false })
                } else {
                    self.table.intern(Type::Interface { class: k, args: args.into_boxed_slice(), nullable: false })
                };
                let t = self.erase(t);
                return Constant::Value(Value::new(self.core.type_, InstanceState::Type(Some(t))));
            }
            Some(Resolved::Element(Element::Typedef(_))) => {
                return Constant::Value(Value::new(self.core.type_, InstanceState::Type(None)));
            }
            Some(Resolved::TypeParameter(tp)) => {
                if let Some(t) = cx.types.as_ref().and_then(|m| m.get(&tp)) {
                    let t = *t;
                    return Constant::Value(Value::new(self.core.type_, InstanceState::Type(Some(t))));
                }
                return self.inv(u, error_at, c::CONST_TYPE_PARAMETER);
            }
            Some(Resolved::Local(_)) => {
                if let Some(r) = self.local_value(cx, e, error_at) {
                    return r;
                }
            }
            // A read before the declaration (`const x = [x]`): the analyzer
            // binds the name to the local ahead (`REFERENCED_BEFORE_DECLARATION`),
            // and the constant in the cycle is the silenced invalid one.
            None if self.body.units.get(u.0 as usize).and_then(|b| b.local_declaration(e)).is_some() => {
                if let Some(r) = self.local_value(cx, e, error_at) {
                    return r;
                }
            }
            _ => {}
        }
        // `dynamic`/`Never`/`void` as type literals.
        if let ExprKind::Identifier(n) = &self.ast(u).expr(e).kind {
            match self.interner.resolve(n.sym) {
                "dynamic" if res.is_none() => {
                    return Constant::Value(Value::new(self.core.type_, InstanceState::Type(Some(self.core.dynamic_))));
                }
                "Never" if res.is_none() => {
                    return Constant::Value(Value::new(self.core.type_, InstanceState::Type(Some(self.core.never))));
                }
                _ => {}
            }
        }
        if !self.inferred_(u) {
            let t = self.the_static(u, e);
            return Constant::Value(self.unknown(t));
        }
        if self.invalid_type(u, e) {
            return self.generic(u, error_at, true);
        }
        self.generic(u, error_at, false)
    }

    /// The value of `E.values`: `List<E>` with the value of each constant, in
    /// order; an invalid constant invalidates the list (without its own
    /// report).
    fn enum_values(&mut self, u: UnitId, error_at: ExprId, k: ClassId) -> R {
        let n = self.program.class(k).type_params.len();
        let element = self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false });
        let element = self.erase(element);
        let typ = self.interface(self.core.list_class, vec![element]);
        let mut list = Vec::new();
        for v in self.program.class(k).enum_constants.clone() {
            match self.variable_value(v) {
                Some(Constant::Value(x)) => list.push(x),
                // An invalid constant (in a cycle with `values` itself, `e1(values)`):
                // `values` becomes invalid like a `const` variable with an invalid
                // result, silenced (`avoidReporting`, `evaluation.dart:1867-1878`).
                Some(Constant::Invalid(_)) => {
                    let mut i = self.error(u, self.span(u, error_at), c::INVALID_CONSTANT);
                    i.unresolved = true;
                    i.avoid_report = true;
                    return Constant::Invalid(Box::new(i));
                }
                None => return self.generic(u, error_at, true),
            }
        }
        Constant::Value(self.value(typ, InstanceState::List { element, elements: Rc::new(list), unknown: false }))
    }

    /// The static variable member `the_name` of `d` (enum constant or static
    /// field).
    fn static_variable_member(&self, d: ClassId, the_name: SymbolId) -> Option<VariableId> {
        let k = self.program.class(d);
        k.enum_constants
            .iter()
            .chain(k.fields.iter())
            .copied()
            .find(|v| self.program.variable(*v).name == the_name && self.program.variable(*v).static_)
    }

    /// Reference to a top-level or static variable: its value, if it is
    /// `const`; otherwise the generic error at the node.
    fn variable_reference_value(&mut self, cx: &Ctx, error_at: ExprId, v: VariableId) -> R {
        let u = cx.unit;
        let var = self.program.variable(v);
        if var.const_ {
            return match self.variable_value(v) {
                Some(Constant::Value(x)) => Constant::Value(x),
                Some(Constant::Invalid(_)) => {
                    let mut i = self.error(u, self.span(u, error_at), c::INVALID_CONSTANT);
                    i.unresolved = true;
                    i.avoid_report = true;
                    Constant::Invalid(Box::new(i))
                }
                None => self.generic(u, error_at, true),
            };
        }
        if self.invalid_type(u, error_at) {
            return self.generic(u, error_at, true);
        }
        self.generic(u, error_at, false)
    }

    /// `ElementAnnotation.computeConstantValue` of annotation `m`, written in
    /// unit `u`: the constructor invocation (`@C(…)`, `@C.n(…)`, `@p.C(…)`,
    /// `@p.C.n(…)`, with the type arguments that inference recorded for the
    /// argument list) or the variable read (`@x`, `@p.x`, `@C.x`). `None` when it
    /// does not resolve or is not constant.
    pub fn evaluate_annotation(&mut self, u: UnitId, m: &ast::Annotation) -> Option<Value> {
        let lib = self.program.unit(u).library;
        let cx = Ctx::simple(u, lib);
        let names: Vec<SymbolId> = m.name.iter().map(|n| n.sym).collect();
        let (tgt, mem) = match names.as_slice() {
            [c] => (self.program.lookup_in_unit(u, *c).and_then(|b| b.getter), None),
            [a, b] => match self.program.lookup_in_unit(u, *a).and_then(|x| x.getter) {
                Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
                _ => (self.program.lookup_prefixed_in_unit(u, *a, *b).and_then(|x| x.getter), None),
            },
            [p, c, n] => (self.program.lookup_prefixed_in_unit(u, *p, *c).and_then(|x| x.getter), Some(*n)),
            _ => (None, None),
        };
        match (tgt?, &m.arguments) {
            (Element::Class(c), Some(args)) => {
                let the_key = mem.or_else(|| self.interner.lookup(""))?;
                let f = *self.program.class(c).constructors.get(&the_key)?;
                let n = self.program.class(c).type_params.len();
                let args_list: Vec<TypeId> = match self.body.units.get(u.0 as usize).and_then(|b| b.instantiation(args.span.start)) {
                    Some(v) if v.len() == n => v.to_vec(),
                    _ => vec![self.core.dynamic_; n],
                };
                let typ = self.table.intern(Type::Interface { class: c, args: args_list.into_boxed_slice(), nullable: false });
                let error = ErrorAt { unit: u, span: m.span };
                match self.evaluate_call(&cx, error, f, typ, &CallArguments::Ast(args), None, true) {
                    Constant::Value(v) => Some(v),
                    Constant::Invalid(_) => None,
                }
            }
            (Element::Class(c), None) => {
                let v = self.static_variable_member(c, mem?)?;
                match self.variable_value(v)? {
                    Constant::Value(x) => Some(x),
                    Constant::Invalid(_) => None,
                }
            }
            (Element::Variable(v), None) => match self.variable_value(v)? {
                Constant::Value(x) => Some(x),
                Constant::Invalid(_) => None,
            },
            (Element::Function(g), None) => {
                // Reading a top-level variable through the synthetic getter.
                let v = self.program.function(g).variable?;
                match self.variable_value(v)? {
                    Constant::Value(x) => Some(x),
                    Constant::Invalid(_) => None,
                }
            }
            _ => None,
        }
    }

    /// The value of a variable with a constant initializer (`const`, or an
    /// instance `final` in a class with a `const` generative constructor),
    /// computed once. `None` while it is being computed (cycle).
    /// The `_IsSerializableNodeVisitor` over `init`: some node of the subtree is
    /// a `ForElement`, `FunctionExpression`, `PatternAssignment` or
    /// `SwitchExpression`. The nodes of an expression are created before it, in
    /// contiguous ids and within its range.
    fn not_serializable(&self, u: UnitId, init: ExprId) -> bool {
        fn has_for(el: &ast::CollectionElement) -> bool {
            match el {
                ast::CollectionElement::For { .. } | ast::CollectionElement::ForIn { .. } => true,
                ast::CollectionElement::If { then, else_, .. } => has_for(then) || else_.as_deref().is_some_and(has_for),
                _ => false,
            }
        }
        let a = self.ast(u);
        let sp = a.expr(init).span;
        let mut i = init.0 as usize + 1;
        while i > 0 {
            i -= 1;
            let x = &a.exprs[i];
            if x.span.start < sp.start || x.span.end > sp.end {
                break;
            }
            let bad = match &x.kind {
                ExprKind::FunctionExpression(_) | ExprKind::PatternAssign { .. } | ExprKind::Switch { .. } => true,
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements.iter().any(has_for),
                _ => false,
            };
            if bad {
                return true;
            }
        }
        false
    }

    pub fn variable_value(&mut self, v: VariableId) -> Option<Constant> {
        match self.vars.get(&v) {
            Some(VarState::Ready(r)) => return Some(r.clone()),
            Some(VarState::InProgress) => {
                if let Some(p) = self.vars_stack.iter().position(|x| *x == v) {
                    let cycle: Vec<VariableId> = self.vars_stack[p..].to_vec();
                    self.cyclic.extend(cycle);
                }
                return None;
            }
            None => {}
        }
        // The component of the dependency graph that the variable belongs to
        // has a cycle (`generateCycleError`): the stored result is the
        // invalid one at its name, and whoever reads it gets a silenced invalid.
        if self.graph.variables_in_cycle.contains(&v) {
            // Also the enum constant: the error at its name.
            let unit_id = match self.program.variable(v).node {
                VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } | VariableRef::EnumConstant { unit, .. } => Some(unit),
                _ => None,
            };
            if let (Some(unit_id), Some(the_name)) = (unit_id, self.variable_name_span(v)) {
                let r = Constant::Invalid(Box::new(self.error(unit_id, the_name, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
                self.vars.insert(v, VarState::Ready(r.clone()));
                return Some(r);
            }
        }
        let var = self.program.variable(v);
        let lib = var.library;
        let var_type = self.outline.variables[v.0 as usize].declared_type.or(self.outline.variables[v.0 as usize].inferred);
        let var_type = var_type.unwrap_or(self.core.dynamic_);
        let (unit_id, initial) = match var.node {
            VariableRef::TopLevel { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Variables(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => (unit, None),
            },
            VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                ast::MemberKind::Field(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
                _ => (unit, None),
            },
            VariableRef::EnumConstant { unit, decl, index } => {
                self.vars.insert(v, VarState::InProgress);
                self.vars_stack.push(v);
                let r = self.enum_value(v, unit, decl, index);
                self.vars_stack.pop();
                self.vars.insert(v, VarState::Ready(r.clone()));
                return Some(r);
            }
            _ => return Some(Constant::Value(self.unknown(var_type))),
        };
        let Some(init) = initial else {
            let r = Constant::Value(self.unknown(var_type));
            return Some(r);
        };
        if !self.the_inferred.contains(&lib) {
            let r = Constant::Value(self.unknown(var_type));
            self.vars.insert(v, VarState::Ready(r.clone()));
            return Some(r);
        }
        // `DetachNodes._detachConstVariable` (3.6.2 `summary2/detach_nodes.dart:103-115`,
        // `replaceNotSerializableNode` at `:18-51`): the initializer of a top-level
        // or field constant that has a collection `for`, a function expression, a
        // pattern assignment or a `switch` expression becomes a synthetic
        // identifier; the `evaluationResult` is invalid, at the initializer.
        if self.not_serializable(unit_id, init) {
            let r = Constant::Invalid(Box::new(self.error(unit_id, self.span(unit_id, init), c::INVALID_CONSTANT)));
            self.vars.insert(v, VarState::Ready(r.clone()));
            return Some(r);
        }
        self.vars.insert(v, VarState::InProgress);
        self.vars_stack.push(v);
        let cx = Ctx::simple(unit_id, lib);
        let const_ = self.program.variable(v).const_;
        let saved = std::mem::take(&mut self.creation_args);
        let mut r = self.evaluate(&cx, init, const_);
        self.creation_args = saved;
        self.vars_stack.pop();
        if let Constant::Value(x) = &r {
            if const_ && !self.matches_type(x, var_type) {
                let static_ = self.the_static(unit_id, init);
                if self.assignable(static_, var_type) {
                    let mut i = self.error(unit_id, self.span(unit_id, init), c::VARIABLE_TYPE_MISMATCH);
                    i.args = vec![self.format(x.ty), self.format(var_type)];
                    r = Constant::Invalid(Box::new(i));
                }
            }
        }
        if let Constant::Value(x) = &mut r {
            if const_ {
                x.variable = Some(v);
            }
        }
        if self.cyclic.contains(&v) {
            let the_name = self.variable_name_span(v).unwrap_or(Span { start: 0, end: 0 });
            r = Constant::Invalid(Box::new(self.error(unit_id, the_name, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
        }
        self.vars.insert(v, VarState::Ready(r.clone()));
        Some(r)
    }

    pub fn variable_name_span(&self, v: VariableId) -> Option<Span> {
        match self.program.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Variables(l) => l.variables.get(index).map(|x| x.name.span),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                ast::MemberKind::Field(l) => l.variables.get(index).map(|x| x.name.span),
                _ => None,
            },
            VariableRef::EnumConstant { unit, decl, index } => match &self.ast(unit).decl(decl).kind {
                ast::DeclKind::Enum(en) => en.constants.get(index).map(|x| x.name.span),
                _ => None,
            },
            _ => None,
        }
    }

    /// An enum constant: the object with `index` and `_name`.
    fn enum_value(&mut self, v: VariableId, unit: UnitId, decl: ast::DeclId, index: usize) -> Constant {
        let _ = (unit, decl);
        let Some(k) = self.program.variable(v).class else {
            return Constant::Value(Value::new(self.core.dynamic_, InstanceState::Generic { fields: Rc::new(Vec::new()), unknown: true }));
        };
        // The constant's type (the written instantiation, `a<int>()`, or the one
        // from the bounds), like the analyzer's `DartObject`.
        let vd = &self.outline.variables[v.0 as usize];
        let typ = match vd.declared_type.or(vd.inferred) {
            Some(t) => t,
            None => {
                let n = self.program.class(k).type_params.len();
                self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
            }
        };
        let the_name = self.interner.resolve(self.program.variable(v).name);
        let mut the_fields = vec![
            (Field::Index, Value::new(self.core.int, InstanceState::Int(Some(index as i64)))),
            (Field::EnumName, Value::new(self.core.string, InstanceState::Str(Some(the_text(the_name))))),
        ];
        // The constant's initializer is the creation `E.the_name(args)` with the
        // constant's declaration as the error node (`_errorNodes`,
        // `evaluation.dart:39-61`); the evaluation exception becomes
        // `CONST_EVAL_THROWS_EXCEPTION` on it (`:350-376`).
        let lib = self.program.class(k).library;
        if self.the_inferred.contains(&lib)
            && let ast::DeclKind::Enum(en) = &self.ast(unit).decl(decl).kind
            && let Some(cst) = en.constants.get(index)
        {
            let the_key = match cst.constructor {
                Some(n) if self.interner.resolve(n.sym) != "new" => Some(n.sym),
                _ => self.interner.lookup(""),
            };
            // The primary (3.13) in a library without the feature: the 3.13.4 oracle
            // does not evaluate it.
            let primary_disabled = |f: FunctionElementId| {
                matches!(self.program.function(f).node, FunctionRef::Constructor { member, .. } if en.primary_constructor == Some(member))
                    && !self.program.library(lib).features.has(dartforge_frontend::Feature::PrimaryConstructors)
            };
            if let Some(&f) = the_key.and_then(|c| self.program.class(k).constructors.get(&c))
                && !primary_disabled(f)
            {
                let span = Span { start: cst.name.span.start, end: cst.span.end };
                let error = ErrorAt { unit, span };
                let cx = Ctx::simple(unit, lib);
                let empty = CallArguments::Values { positional: Vec::new(), named: Vec::new() };
                let r = match &cst.arguments {
                    Some(a) => self.evaluate_call(&cx, error, f, typ, &CallArguments::Ast(a), None, true),
                    None => self.evaluate_call(&cx, error, f, typ, &empty, None, true),
                };
                match r {
                    Constant::Invalid(i) if i.exception => {
                        let mut n = self.error(unit, span, c::CONST_EVAL_THROWS_EXCEPTION);
                        n.avoid_report = i.avoid_report;
                        let msg = dartforge_diagnostics::Diagnostic::with_code(i.code, i.span, i.args.iter().map(|s| s.as_str())).message;
                        let defining = self.program.library(self.program.unit(unit).library).units.first().copied().unwrap_or(unit);
                        n.context = i.context.clone();
                        n.context.push((defining, i.span, format!("The exception is '{msg}' and occurs here.")));
                        return Constant::Invalid(Box::new(n));
                    }
                    Constant::Invalid(i) => return Constant::Invalid(i),
                    Constant::Value(x) => {
                        if let InstanceState::Generic { fields: c2, .. } = &x.state {
                            for par in c2.iter() {
                                if !matches!(par.0, Field::Index | Field::EnumName) {
                                    the_fields.push(par.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        Constant::Value(Value { ty: typ, state: InstanceState::Generic { fields: Rc::new(the_fields), unknown: false }, variable: Some(v) })
    }

    /// Local constant: the declaration by the name's offset.
    fn local_value(&mut self, cx: &Ctx, e: ExprId, error_at: ExprId) -> Option<R> {
        let u = cx.unit;
        let offset = self.body.units.get(u.0 as usize)?.local_declaration(e)?;
        if !self.unit_locals.contains_key(&u) {
            let map = constant_locals(self.ast(u));
            self.unit_locals.insert(u, map);
        }
        let Some(&(stmt, i)) = self.unit_locals[&u].get(&offset) else {
            // A local that is not `const`: the generic error.
            return None;
        };
        if let Some(r) = self.locals.get(&(u, offset)) {
            return Some(match r {
                Some(Constant::Value(v)) => Constant::Value(v.clone()),
                Some(Constant::Invalid(_)) => {
                    let mut x = self.error(u, self.span(u, error_at), c::INVALID_CONSTANT);
                    x.unresolved = true;
                    x.avoid_report = true;
                    Constant::Invalid(Box::new(x))
                }
                None => self.generic(u, error_at, true),
            });
        }
        let r = self.declared_local_value(u, cx.lib, stmt, i);
        Some(match r {
            Constant::Value(v) => Constant::Value(v),
            Constant::Invalid(_) => {
                let mut x = self.error(u, self.span(u, error_at), c::INVALID_CONSTANT);
                x.unresolved = true;
                x.avoid_report = true;
                Constant::Invalid(Box::new(x))
            }
        })
    }

    /// The value of local constant `i` of the list in `stmt` (cached).
    pub fn declared_local_value(&mut self, u: UnitId, lib: LibraryId, stmt: ast::StmtId, i: usize) -> Constant {
        let list = match &self.ast(u).stmt(stmt).kind {
            ast::StmtKind::Variables(l) => l,
            ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => return Constant::Value(Value::new(self.core.dynamic_, InstanceState::Null { invalid: true })),
        };
        let var = &list.variables[i];
        let offset = var.name.span.start;
        if let Some(Some(r)) = self.locals.get(&(u, offset)) {
            return r.clone();
        }
        if self.graph.locals_in_cycle.contains(&(u, offset)) {
            let r = Constant::Invalid(Box::new(self.error(u, var.name.span, c::RECURSIVE_COMPILE_TIME_CONSTANT)));
            self.locals.insert((u, offset), Some(r.clone()));
            return r;
        }
        let Some(init) = var.initializer else {
            return Constant::Value(Value::null(self.core));
        };
        self.locals.insert((u, offset), None);
        let cx = Ctx::simple(u, lib);
        let saved = std::mem::take(&mut self.creation_args);
        let mut r = self.evaluate(&cx, init, true);
        self.creation_args = saved;
        if let Constant::Value(x) = &r {
            let typ = self.body.units[u.0 as usize].local_type(offset).unwrap_or(self.core.dynamic_);
            if !self.matches_type(x, typ) {
                let static_ = self.the_static(u, init);
                if self.assignable(static_, typ) {
                    let mut i = self.error(u, self.span(u, init), c::VARIABLE_TYPE_MISMATCH);
                    i.args = vec![self.format(x.ty), self.format(typ)];
                    r = Constant::Invalid(Box::new(i));
                }
            }
        }
        self.locals.insert((u, offset), Some(r.clone()));
        r
    }

    /// The local read by `e` is a formal parameter.
    pub fn parameter_local(&self, u: UnitId, e: ExprId) -> bool {
        let Some(offset) = self.body.units.get(u.0 as usize).and_then(|b| b.local_declaration(e)) else { return false };
        self.parameters.get(&u).is_some_and(|s| s.contains(&offset))
    }

    /// The local read by `e` is a constant (`const`).
    pub fn constant_local(&self, u: UnitId, e: ExprId) -> bool {
        let Some(offset) = self.body.units.get(u.0 as usize).and_then(|b| b.local_declaration(e)) else { return false };
        match self.unit_locals.get(&u) {
            Some(m) => m.contains_key(&offset),
            None => constant_locals(self.ast(u)).contains_key(&offset),
        }
    }

    // -- Potencialmente constante ---------------------------------------------

    /// `_reportNotPotentialConstants`: the first node that is not potentially
    /// constant, as `INVALID_CONSTANT`.
    fn not_potentially_constant(&self, cx: &Ctx, e: ExprId) -> Option<R> {
        let lib_nnbd = true;
        let _ = lib_nnbd;
        let mut nodes = Vec::new();
        super::potential::the_collect(self, cx, e, &mut nodes);
        let n = nodes.first()?;
        Some(Constant::Invalid(Box::new(self.error(cx.unit, *n, c::INVALID_CONSTANT))))
    }

    // -- Constructors ----------------------------------------------------------

    /// The type of a creation: the node's static type, otherwise the raw class.
    fn creation_type(&mut self, u: UnitId, e: ExprId, f: FunctionElementId) -> TypeId {
        let t = self.the_static(u, e);
        if matches!(self.table.get(t), Type::Interface { .. } | Type::ExtensionType { .. }) {
            return t;
        }
        let Some(k) = self.program.function(f).class else { return self.core.dynamic_ };
        let n = self.program.class(k).type_params.len();
        self.table.intern(Type::Interface { class: k, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
    }

    /// `_InstanceCreationEvaluator.evaluate` for a written creation (`e` is the
    /// error node; `the_keyword`, the written `new`/`const`).
    /// The forwarded constructor of `class B = A with M` (without an element in
    /// the model): the one with the same name in the first superclass that is
    /// not a mixin application, with the creation's type (`B`). Evaluating the
    /// forwarder is evaluating the superclass constructor with the same
    /// arguments.
    fn forwarded_constructor(&self, u: UnitId, e: ExprId, the_name: Option<SymbolId>) -> Option<(FunctionElementId, TypeId)> {
        let typ = self.the_static(u, e);
        let Type::Interface { class, .. } = self.table.get(typ) else { return None };
        if self.program.class(*class).kind != ClassKind::MixinApplication {
            return None;
        }
        // `C.new` and `C()` are the unnamed constructor (the empty symbol).
        let empty = self.interner.lookup("");
        let the_key = match the_name {
            Some(n) if self.interner.resolve(n) != "new" => Some(n),
            _ => empty,
        };
        let mut current = self.program.class(*class).supertype_class;
        for _ in 0..64 {
            let k = current?;
            let cls = self.program.class(k);
            if cls.kind != ClassKind::MixinApplication {
                return the_key.and_then(|k| cls.constructors.get(&k).copied()).map(|f| (f, typ));
            }
            current = cls.supertype_class;
        }
        None
    }

    fn call_constructor(
        &mut self,
        cx: &Ctx,
        e: ExprId,
        f: FunctionElementId,
        args: &ast::Arguments,
        the_keyword: Option<Span>,
        typ: Option<TypeId>,
    ) -> R {
        let u = cx.unit;
        let typ = match typ {
            Some(t) => t,
            None => self.creation_type(u, e, f),
        };
        let error = ErrorAt { unit: u, span: self.span(u, e) };
        for arg in args.args.iter() {
            if arg.name.is_none() {
                self.creation_args.insert((u, arg.value));
            }
        }
        self.evaluate_call(cx, error, f, typ, &CallArguments::Ast(args), the_keyword, true)
    }

    /// Evaluates the call of constructor `f` with the arguments (`cx`'s
    /// `ConstantVisitor` for them), reporting at `error`.
    fn evaluate_call(
        &mut self,
        cx: &Ctx,
        error: ErrorAt,
        f: FunctionElementId,
        typ: TypeId,
        args: &CallArguments<'_>,
        the_keyword: Option<Span>,
        in_const_args: bool,
    ) -> R {
        // T3: the `const` is that of the public declaration (in the SDK, the
        // patch of `bool.fromEnvironment` and its siblings is a factory without
        // `const`).
        let fe = self.program.function(self.program.public(f));
        if !fe.const_ {
            let span = the_keyword.unwrap_or(error.span);
            return Constant::Invalid(Box::new(self.error(error.unit, span, c::CONST_WITH_NON_CONST)));
        }
        if self.constructors_in_progress.contains(&f) || self.constructors_in_progress.len() > 64 {
            return Constant::Value(self.unknown(typ));
        }
        // Arguments (`_valueOf`: an unresolved one is reported and becomes a
        // fictitious object of the parameter's type).
        let params = self.outline.functions.get(f.0 as usize).map(|d| d.parameters.clone()).unwrap_or_default();
        let mut positional_args: Vec<Value> = Vec::new();
        let mut named_args: Vec<(SymbolId, Value, Option<(UnitId, Span, TypeId)>)> = Vec::new();
        let mut positional_targets: Vec<Option<(UnitId, Span, TypeId)>> = Vec::new();
        match args {
            CallArguments::Ast(a) => {
                let mut i_pos = 0usize;
                let u = cx.unit;
                for arg in a.args.iter() {
                    let type_param = match arg.name {
                        Some(n) => params.iter().find(|p| p.external_name.or(p.name) == Some(n.sym)).map(|p| p.ty),
                        None => params.get(i_pos).map(|p| p.ty),
                    }
                    .unwrap_or(self.core.dynamic_);
                    let v = self.value_of(cx, arg.value, type_param, in_const_args);
                    let v = match v {
                        Constant::Value(v) => v,
                        i => return i,
                    };
                    let tgt = Some((u, self.span(u, arg.value), self.the_static(u, arg.value)));
                    match arg.name {
                        Some(n) => {
                            let s = Span { start: n.span.start, end: self.span(u, arg.value).end };
                            named_args.push((n.sym, v, Some((u, s, self.the_static(u, arg.value)))));
                        }
                        None => {
                            positional_args.push(v);
                            positional_targets.push(tgt);
                            i_pos += 1;
                        }
                    }
                }
            }
            CallArguments::Values { positional: p, named: n } => {
                positional_args = p.clone();
                positional_targets = vec![None; p.len()];
                named_args = n.iter().map(|(s, v)| (*s, v.clone(), None)).collect();
            }
        }
        // `const` factory redirections up to a constructor that is not one.
        let (f, typ) = self.follow_redirections(f, typ);
        // The constructor is not cycle-free (`isCycleFree`,
        // `evaluation.dart:3090-3099`): an unknown of the type, without an error.
        if self.graph.constructors_in_cycle.contains(&f) {
            return Constant::Value(self.unknown(typ));
        }
        let fe = self.program.function(f);
        let Some(k) = fe.class else { return Constant::Value(self.unknown(typ)) };
        if fe.factory || !self.the_inferred.contains(&fe.library) || fe.kind == FunctionKind::SyntheticConstructor && self.program.class(k).kind == ClassKind::MixinApplication {
            return self.call_factory(error, f, typ, &positional_args, &named_args);
        }
        self.constructors_in_progress.push(f);
        let r = self.call_generator(error, f, k, typ, positional_args, positional_targets, named_args);
        self.constructors_in_progress.pop();
        r
    }

    /// `_valueOf`.
    fn value_of(&mut self, cx: &Ctx, e: ExprId, default_type: TypeId, in_const: bool) -> R {
        match self.evaluate(cx, e, in_const) {
            Constant::Invalid(i) if i.unresolved => {
                if cx.report && !i.avoid_report {
                    self.reports.push((*i).clone());
                }
                let t = self.erase(default_type);
                Constant::Value(Value::new(t, InstanceState::Null { invalid: true }))
            }
            r => r,
        }
    }

    /// `_followConstantRedirectionChain`.
    fn follow_redirections(&mut self, mut f: FunctionElementId, mut typ: TypeId) -> (FunctionElementId, TypeId) {
        let mut seen = vec![f];
        loop {
            let fe = self.program.function(f);
            if !fe.factory {
                break;
            }
            if fe.class.is_some_and(|k| self.interner.resolve(self.program.class(k).name) == "Symbol")
                && Some(fe.library) == self.core.core_library
            {
                break;
            }
            let Some((g, t)) = self.redirection_target(f, typ) else { break };
            if !self.program.function(self.program.public(g)).const_ || seen.contains(&g) {
                break;
            }
            seen.push(g);
            f = g;
            typ = t;
        }
        (f, typ)
    }

    /// The constructor that factory `f` redirects to (`= C.the_name`), and the
    /// instantiated type.
    fn redirection_target(&mut self, f: FunctionElementId, typ: TypeId) -> Option<(FunctionElementId, TypeId)> {
        let FunctionRef::Constructor { unit, member } = self.program.function(f).node else { return None };
        let ast::MemberKind::Constructor(k) = &self.ast(unit).member(member).kind else { return None };
        let r = k.redirect.as_ref()?;
        let ast::TypeKind::Named { name, .. } = &self.ast(unit).ty(r.ty).kind else { return None };
        // `C.the_name` without an import prefix is class `C` and constructor `the_name`
        // (the parser does not tell it apart from a prefixed type).
        let mut the_constructor = r.constructor.map(|n| n.sym);
        let b = match &name[..] {
            [n] => self.program.lookup_in_unit(unit, n.sym),
            [p, n] => match self.program.lookup_prefixed_in_unit(unit, p.sym, n.sym) {
                Some(b) => Some(b),
                None if the_constructor.is_none() => {
                    the_constructor = Some(n.sym);
                    self.program.lookup_in_unit(unit, p.sym)
                }
                None => None,
            },
            _ => None,
        }?;
        let Some(Element::Class(tgt)) = b.getter else { return None };
        let the_key = match the_constructor {
            Some(n) => n,
            None => self.interner.lookup("")?,
        };
        let g = *self.program.class(tgt).constructors.get(&the_key)?;
        // The type arguments go through the declaration: the target is handled
        // with those of the original type when the class is the same; otherwise,
        // raw.
        let t = match self.table.get(typ) {
            Type::Interface { class, .. } if *class == tgt => typ,
            _ => {
                let n = self.program.class(tgt).type_params.len();
                self.table.intern(Type::Interface { class: tgt, args: vec![self.core.dynamic_; n].into_boxed_slice(), nullable: false })
            }
        };
        Some((g, t))
    }

    /// `evaluateFactoryConstructorCall`.
    fn call_factory(
        &mut self,
        error: ErrorAt,
        f: FunctionElementId,
        typ: TypeId,
        positional_args: &[Value],
        named_args: &[(SymbolId, Value, Option<(UnitId, Span, TypeId)>)],
    ) -> R {
        let fe = self.program.function(f);
        let the_name = fe.name;
        let cls = fe.class;
        let of_core = Some(fe.library) == self.core.core_library;
        let first = positional_args.first();
        let n_args = positional_args.len() + named_args.len();
        let exception = |m: &Engine<'_>| Constant::Invalid(Box::new(m.error(error.unit, error.span, c::CONST_EVAL_THROWS_EXCEPTION)));
        if of_core && Some(the_name) == self.sym_from_environment {
            // `_checkFromEnvironmentArguments`.
            let ok = (1..=2).contains(&n_args)
                && positional_args.len() == 1
                && first.is_some_and(|p| p.ty == self.core.string)
                && (named_args.is_empty()
                    || named_args.len() == 1
                        && Some(named_args[0].0) == self.sym_default_value
                        && (named_args[0].1.ty == typ || named_args[0].1.ty == self.core.null));
            if !ok {
                return exception(self);
            }
            return Constant::Value(self.unknown(typ));
        }
        if of_core && Some(the_name) == self.sym_has_environment && cls.is_some() && cls == self.core.bool_class {
            return Constant::Value(Value::bool_(self.core, None));
        }
        if of_core
            && self.interner.resolve(the_name).is_empty()
            && cls.is_some_and(|k| self.interner.resolve(self.program.class(k).name) == "Symbol")
            && n_args == 1
        {
            let ok = positional_args.len() == 1 && first.is_some_and(|p| p.ty == self.core.string && p.as_text().is_some());
            if !ok {
                return exception(self);
            }
            let s = String::from_utf16_lossy(first.unwrap().as_text().unwrap());
            return Constant::Value(Value::new(typ, InstanceState::Symbol(Some(s.into()))));
        }
        let t = self.erase(typ);
        Constant::Value(self.unknown(t))
    }

    /// `evaluateGenerativeConstructorCall`.
    #[allow(clippy::too_many_arguments)]
    fn call_generator(
        &mut self,
        error: ErrorAt,
        f: FunctionElementId,
        k: ClassId,
        typ: TypeId,
        positional_args: Vec<Value>,
        positional_targets: Vec<Option<(UnitId, Span, TypeId)>>,
        named_args: Vec<(SymbolId, Value, Option<(UnitId, Span, TypeId)>)>,
    ) -> R {
        let lib = self.program.function(f).library;
        // Substitution of the class's type parameters.
        let class_params = self.outline.classes.get(k.0 as usize).map(|d| d.type_params.clone()).unwrap_or_default();
        let type_args: Vec<TypeId> = match self.table.get(typ) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        let mut types_map: HashMap<TypeParamId, TypeId> = HashMap::new();
        if class_params.len() == type_args.len() {
            for (p, a) in class_params.iter().zip(type_args.iter()) {
                types_map.insert(*p, *a);
            }
        }
        let subst = |m: &mut Engine<'_>, t: TypeId| -> TypeId {
            if types_map.is_empty() { t } else { crate::ops::substitute(t, &types_map, m.table) }
        };
        let mut the_fields: Vec<(Field, Value)> = Vec::new();
        // `_checkFields`: `final` fields with an initializer in the declaration.
        let fields = self.program.class(k).fields.clone();
        for v in fields {
            let var = self.program.variable(v);
            if !(var.final_ || var.const_) || var.static_ || var.late {
                continue;
            }
            let has_init = match var.node {
                VariableRef::Field { unit, member, index } => match &self.ast(unit).member(member).kind {
                    ast::MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
                    _ => None,
                },
                _ => None,
            };
            let Some((fu, fx)) = has_init else { continue };
            let Some(Constant::Value(x)) = self.variable_value(v) else { continue };
            let declared = self.outline.variables[v.0 as usize].declared_type.or(self.outline.variables[v.0 as usize].inferred).unwrap_or(self.core.dynamic_);
            let field_type = subst(self, declared);
            if !self.matches_type(&x, field_type) {
                // `hasTypeParameterReference(field.type)`: the field's declared type,
                // before the substitution (`final T x` with `C<String>`), makes the
                // mismatch an evaluation exception, reported as
                // `CONST_EVAL_THROWS_EXCEPTION` at the creation, with the field's
                // context message.
                let exception = self.mentions_parameter(declared);
                let (eu, es) = if fu == error.unit || true { (fu, self.span(fu, fx)) } else { (error.unit, error.span) };
                let mut i = self.error(eu, es, c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH);
                i.args = vec![self.format(x.ty), self.interner.resolve(var.name).to_string(), self.format(field_type)];
                i.exception = exception;
                return Constant::Invalid(Box::new(i));
            }
            the_fields.push((Field::MemberName(var.name), x));
        }
        // `_checkParameters`.
        let FunctionRef::Constructor { unit: cu, member } = self.program.function(f).node else {
            // Synthetic constructor: no parameters or initializers.
            let g = self.complete_generator(error, f, k, typ, lib, the_fields, None, Vec::new(), HashMap::new(), types_map);
            return self.finish_generator(g);
        };
        let ast::MemberKind::Constructor(ctor) = &self.ast(cu).member(member).kind else {
            return Constant::Value(self.unknown(typ));
        };
        let type_params_list = self.outline.functions.get(f.0 as usize).map(|d| d.parameters.clone()).unwrap_or_default();
        let mut lexical: HashMap<SymbolId, Value> = HashMap::new();
        let mut i_pos = 0usize;
        for (i, p) in ctor.parameters.iter().enumerate() {
            let Some(the_name) = p.name else { continue };
            let external = p.external_name().map(|n| n.sym).unwrap_or(the_name.sym);
            let type_p = type_params_list.get(i).map(|d| d.ty).unwrap_or(self.core.dynamic_);
            let type_p = subst(self, type_p);
            let (mut the_value, tgt) = if p.kind == ast::ParameterKind::Named {
                match named_args.iter().find(|(n, _, _)| *n == external) {
                    Some((_, v, a)) => (Some(v.clone()), *a),
                    None => (None, None),
                }
            } else {
                let r = (positional_args.get(i_pos).cloned(), positional_targets.get(i_pos).copied().flatten());
                i_pos += 1;
                r
            };
            // Only an optional one (`isOptional`) receives the default; a missing
            // `required` named one is left without a value.
            if the_value.is_none() && p.kind != ast::ParameterKind::Required && !p.required {
                the_value = match p.default_value {
                    // `super.x` without a default: that of the super constructor's parameter.
                    None if p.super_ => {
                        let idx = ctor.parameters[..i].iter().filter(|q| q.super_ && q.kind != ast::ParameterKind::Named).count();
                        Some(self.inherited_default(f, the_name.sym, p.kind == ast::ParameterKind::Named, idx, 0).unwrap_or_else(|| Value::null(self.core)))
                    }
                    None => Some(Value::null(self.core)),
                    Some(d) => match self.default_value(cu, lib, d) {
                        Constant::Value(v) => Some(v),
                        Constant::Invalid(_) => None,
                    },
                };
            }
            let Some(the_value) = the_value else { continue };
            let (au, aspan, astatic) = match tgt {
                Some((u, s, t)) => (u, s, Some(t)),
                None => (error.unit, error.span, None),
            };
            let invalid = matches!(the_value.state, InstanceState::Null { invalid: true });
            if !invalid && !self.matches_type(&the_value, type_p) {
                let exception = astatic.is_some_and(|t| self.assignable(t, type_p));
                let mut e = self.error(au, aspan, c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH);
                e.args = vec![self.format_without_alias(the_value.ty), self.format_without_alias(type_p)];
                e.exception = exception;
                return Constant::Invalid(Box::new(e));
            }
            if p.this_ {
                let field = self.program.class(k).fields.iter().copied().find(|v| self.program.variable(*v).name == the_name.sym && !self.program.variable(*v).static_);
                if let Some(field) = field {
                    let field_type = self.outline.variables[field.0 as usize].declared_type.or(self.outline.variables[field.0 as usize].inferred).unwrap_or(self.core.dynamic_);
                    let field_type = subst(self, field_type);
                    if field_type != type_p && !invalid && !self.matches_type(&the_value, field_type) {
                        let mut e = self.error(au, aspan, c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH);
                        e.args = vec![self.format_without_alias(the_value.ty), self.format_without_alias(field_type)];
                        return Constant::Invalid(Box::new(e));
                    }
                    if the_fields.iter().any(|(c, _)| *c == Field::MemberName(the_name.sym)) {
                        return Constant::Invalid(Box::new(self.error(error.unit, error.span, c::CONST_EVAL_THROWS_EXCEPTION)));
                    }
                    the_fields.push((Field::MemberName(the_name.sym), the_value.clone()));
                }
            }
            lexical.insert(the_name.sym, the_value);
        }
        // `super.x` parameters: implicit arguments of the super call.
        let mut implicit_supers: (Vec<Value>, Vec<(SymbolId, Value)>) = (Vec::new(), Vec::new());
        for p in ctor.parameters.iter() {
            if !p.super_ {
                continue;
            }
            let Some(n) = p.name else { continue };
            let Some(v) = lexical.get(&n.sym).cloned() else { continue };
            if p.kind == ast::ParameterKind::Named {
                implicit_supers.1.push((n.sym, v));
            } else {
                implicit_supers.0.push(v);
            }
        }
        let inits: Vec<&ast::Initializer> = ctor.initializers.iter().collect();
        let g = self
            .complete_generator(error, f, k, typ, lib, the_fields, Some((cu, inits)), implicit_supers.0, lexical, types_map)
            .con_super_named(implicit_supers.1);
        self.finish_generator(g)
    }

    /// `SuperFormalParameterElementImpl.evaluationResult` (3.6.2
    /// `element.dart:1750-1761`, used by `_checkParameters`,
    /// `evaluation.dart:2889-2900`): a `super.x` without its own default takes
    /// the default of the corresponding parameter of the super constructor (a
    /// named one by name, a positional one by position among the positional
    /// `super.` parameters), without checking the type; in a chain, if that one
    /// is also a `super.` without a default. `None`: no corresponding parameter
    /// (the value becomes `null`).
    fn inherited_default(&mut self, f: FunctionElementId, the_name: SymbolId, named_one: bool, idx: usize, prof: u32) -> Option<Value> {
        if prof > 16 {
            return None;
        }
        let FunctionRef::Constructor { unit, member } = self.program.function(f).node else { return None };
        let k = self.program.function(f).class?;
        let sup = self.program.class(k).supertype_class?;
        let ast::MemberKind::Constructor(ctor) = &self.ast(unit).member(member).kind else { return None };
        let super_name = ctor.initializers.iter().find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        });
        let the_key = super_name.flatten().or_else(|| self.interner.lookup(""))?;
        let g = *self.program.class(sup).constructors.get(&the_key)?;
        let g = self.program.public(g);
        let FunctionRef::Constructor { unit: gu, member: gm } = self.program.function(g).node else { return None };
        let ast::MemberKind::Constructor(gc) = &self.ast(gu).member(gm).kind else { return None };
        let tgt = if named_one {
            gc.parameters.iter().position(|q| q.kind == ast::ParameterKind::Named && q.name.is_some_and(|n| n.sym == the_name))
        } else {
            gc.parameters.iter().enumerate().filter(|(_, q)| q.kind != ast::ParameterKind::Named).nth(idx).map(|(j, _)| j)
        }?;
        let q = &gc.parameters[tgt];
        match q.default_value {
            Some(d) => {
                let glib = self.program.unit(gu).library;
                match self.default_value(gu, glib, d) {
                    Constant::Value(v) => Some(v),
                    Constant::Invalid(_) => None,
                }
            }
            None if q.super_ => {
                let qname = q.name?.sym;
                let qnamed = q.kind == ast::ParameterKind::Named;
                let qindex = gc.parameters[..tgt].iter().filter(|r| r.super_ && r.kind != ast::ParameterKind::Named).count();
                self.inherited_default(g, qname, qnamed, qindex, prof + 1)
            }
            None => Some(Value::null(self.core)),
        }
    }

    /// `_checkInitializers`, `_checkSuperConstructorCall` and the object.
    #[allow(clippy::too_many_arguments)]
    fn complete_generator(
        &mut self,
        error: ErrorAt,
        f: FunctionElementId,
        k: ClassId,
        typ: TypeId,
        lib: LibraryId,
        mut the_fields: Vec<(Field, Value)>,
        inits: Option<(UnitId, Vec<&ast::Initializer>)>,
        super_positional_args: Vec<Value>,
        lexical: HashMap<SymbolId, Value>,
        types_map: HashMap<TypeParamId, TypeId>,
    ) -> Generator {
        let lexical = Rc::new(lexical);
        let types = Rc::new(types_map);
        let ctor_unit = inits.as_ref().map(|(u, _)| *u).unwrap_or(error.unit);
        let cxi = Ctx { unit: ctor_unit, lib, lexical: Some(lexical.clone()), types: Some(types.clone()), report: false };
        let mut super_name: Option<SymbolId> = None;
        let mut super_args: Option<(UnitId, Vec<ast::Argument>)> = None;
        if let Some((cu, inits)) = inits {
            for init in inits {
                match init {
                    ast::Initializer::Field { name, value, .. } => {
                        let r = self.evaluate(&cxi, *value, false);
                        match r {
                            Constant::Value(v) => {
                                if the_fields.iter().any(|(c, _)| *c == Field::MemberName(name.sym)) {
                                    return Generator::ready(Constant::Invalid(Box::new(self.error(error.unit, error.span, c::CONST_EVAL_THROWS_EXCEPTION))));
                                }
                                let field = self.program.class(k).fields.iter().copied().find(|x| self.program.variable(*x).name == name.sym && !self.program.variable(*x).static_);
                                if let Some(field) = field {
                                    let tc = self.outline.variables[field.0 as usize].declared_type.or(self.outline.variables[field.0 as usize].inferred).unwrap_or(self.core.dynamic_);
                                    let tc = if types.is_empty() { tc } else { crate::ops::substitute(tc, &types, self.table) };
                                    if !self.matches_type(&v, tc) {
                                        let static_ = self.the_static(cu, *value);
                                        let exception = self.assignable(static_, tc);
                                        let (eu, es) = if exception { (cu, self.span(cu, *value)) } else { (error.unit, error.span) };
                                        let mut i = self.error(eu, es, c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH);
                                        i.args = vec![self.format(v.ty), self.interner.resolve(name.sym).to_string(), self.format(tc)];
                                        i.exception = exception;
                                        return Generator::ready(Constant::Invalid(Box::new(i)));
                                    }
                                }
                                the_fields.push((Field::MemberName(name.sym), v));
                            }
                            Constant::Invalid(i) if !i.exception => {
                                let n = self.error_in_initializer(&i, Some(f), "field initializer", error);
                                return Generator::ready(Constant::Invalid(Box::new(n)));
                            }
                            r => return Generator::ready(r),
                        }
                    }
                    ast::Initializer::Super { constructor, arguments, .. } => {
                        super_name = constructor.map(|n| n.sym);
                        super_args = Some((cu, arguments.args.to_vec()));
                    }
                    ast::Initializer::Redirect { constructor, arguments, .. } => {
                        let the_key = constructor.map(|n| n.sym).or_else(|| self.interner.lookup(""));
                        let tgt = the_key.and_then(|c| self.program.class(k).constructors.get(&c).copied());
                        if let Some(g) = tgt {
                            if self.program.function(self.program.public(g)).const_ {
                                let r = self.evaluate_call(&cxi, error, g, typ, &CallArguments::Ast(arguments), None, false);
                                return Generator::ready(r);
                            }
                        }
                    }
                    ast::Initializer::Assert { condition, message, span } => {
                        let r = self.evaluate(&cxi, *condition, false);
                        match r {
                            Constant::Value(v) => {
                                if !v.state.is_bool() || v.as_bool() == Some(false) {
                                    let mut i = None;
                                    if let Some(m) = message {
                                        if let Constant::Value(mv) = self.evaluate(&cxi, *m, false) {
                                            if let Some(t) = mv.as_text() {
                                                let mut x = self.error(cu, *span, c::CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE);
                                                x.args = vec![String::from_utf16_lossy(t)];
                                                x.exception = true;
                                                i = Some(x);
                                            }
                                        }
                                    }
                                    let mut i = i.unwrap_or_else(|| self.error(cu, *span, c::CONST_EVAL_ASSERTION_FAILURE));
                                    i.exception = true;
                                    return Generator::ready(Constant::Invalid(Box::new(i)));
                                }
                            }
                            Constant::Invalid(i) if !i.exception => {
                                let n = self.error_in_initializer(&i, Some(f), "assert initializer", error);
                                return Generator::ready(Constant::Invalid(Box::new(n)));
                            }
                            r => return Generator::ready(r),
                        }
                    }
                }
            }
        }
        Generator {
            ready_engine: None,
            f: Some(f),
            error,
            k,
            ty: typ,
            fields: the_fields,
            super_name,
            super_args,
            super_positional_args,
            super_named_args: Vec::new(),
            cxi,
        }
    }

    /// `_checkSuperConstructorCall` and the final object.
    fn finish_generator(&mut self, g: Generator) -> R {
        if let Some(r) = g.ready_engine {
            return r;
        }
        let Generator { f, error, k, ty: typ, fields: mut the_fields, super_name, super_args, super_positional_args, super_named_args, cxi, .. } = g;
        let cls = self.program.class(k);
        if let Some(sup) = cls.supertype_class {
            if Some(sup) != self.core.object_class {
                let the_key = super_name.or_else(|| self.interner.lookup(""));
                let tgt = the_key.and_then(|c| self.program.class(sup).constructors.get(&c).copied());
                if let Some(g) = tgt {
                    if self.program.function(self.program.public(g)).const_ {
                        let super_type = self.outline.classes.get(k.0 as usize).and_then(|d| d.supertype).unwrap_or(self.core.dynamic_);
                        let super_type = match cxi.types.as_ref() {
                            Some(m) if !m.is_empty() => crate::ops::substitute(super_type, m, self.table),
                            _ => super_type,
                        };
                        let r = match super_args {
                            Some((cu, args)) => {
                                let a = ast::Arguments { span: error.span, type_args: Box::new([]), args: args.into_boxed_slice() };
                                let cx = Ctx { unit: cu, ..cxi.clone() };
                                if super_positional_args.is_empty() && super_named_args.is_empty() {
                                    self.evaluate_call(&cx, error, g, super_type, &CallArguments::Ast(&a), None, false)
                                } else {
                                    // Written arguments plus those of `super.x`.
                                    let mut pos = super_positional_args.clone();
                                    let mut the_named: Vec<(SymbolId, Value)> = super_named_args.clone();
                                    for arg in a.args.iter() {
                                        match self.evaluate(&cx, arg.value, false) {
                                            Constant::Value(v) => match arg.name {
                                                Some(n) => the_named.push((n.sym, v)),
                                                None => pos.push(v),
                                            },
                                            i => return i,
                                        }
                                    }
                                    self.evaluate_call(&cx, error, g, super_type, &CallArguments::Values { positional: pos, named: the_named }, None, false)
                                }
                            }
                            None => self.evaluate_call(
                                &cxi,
                                error,
                                g,
                                super_type,
                                &CallArguments::Values { positional: super_positional_args, named: super_named_args },
                                None,
                                false,
                            ),
                        };
                        match r {
                            Constant::Value(v) => the_fields.push((Field::Super, v)),
                            Constant::Invalid(i) if !i.exception => {
                                let mut n = if i.context.is_empty() {
                                    self.error_in_initializer(&i, f, "super constructor invocation", error)
                                } else {
                                    let mut n = (*i).clone();
                                    n.unit = error.unit;
                                    n.span = error.span;
                                    n
                                };
                                if !i.context.is_empty() {
                                    if let Some(x) = f.and_then(|f| self.constructor_stack(g, f)) {
                                        n.context.push(x);
                                    }
                                }
                                return Constant::Invalid(Box::new(n));
                            }
                            Constant::Invalid(mut i) => {
                                if let Some(x) = f.and_then(|f| self.constructor_stack(g, f)) {
                                    i.context.push(x);
                                }
                                return Constant::Invalid(i);
                            }
                        }
                    }
                }
            }
        }
        if cls.kind == ClassKind::ExtensionType {
            if let Some(rep) = cls.representation {
                let the_name = self.program.variable(rep).name;
                if let Some((_, v)) = the_fields.iter().find(|(c, _)| *c == Field::MemberName(the_name)) {
                    return Constant::Value(v.clone());
                }
            }
        }
        let t = self.erase(typ);
        Constant::Value(Value::new(t, InstanceState::Generic { fields: Rc::new(the_fields), unknown: false }))
    }

    /// Default value of a parameter (cached by expression).
    fn default_value(&mut self, u: UnitId, lib: LibraryId, d: ExprId) -> Constant {
        if let Some(r) = self.patterns.get(&(u, d)) {
            return r.clone();
        }
        let cx = Ctx::simple(u, lib);
        let saved = std::mem::take(&mut self.creation_args);
        let r = self.evaluate(&cx, d, false);
        self.creation_args = saved;
        self.patterns.insert((u, d), r.clone());
        r
    }

    /// The result of evaluating the default `d` (for the verifier).
    pub fn default_result(&mut self, u: UnitId, lib: LibraryId, d: ExprId) -> Constant {
        self.default_value(u, lib, d)
    }
}

/// Where a constructor error is reported (`_errorNode`).
#[derive(Clone, Copy, Debug)]
pub struct ErrorAt {
    pub unit: UnitId,
    pub span: Span,
}

/// The arguments of a constructor call: written ones, or already evaluated
/// (implicit super, `super.x`).
enum CallArguments<'x> {
    Ast(&'x ast::Arguments),
    Values { positional: Vec<Value>, named: Vec<(SymbolId, Value)> },
}

/// The state between the initializers and the super call.
struct Generator {
    ready_engine: Option<Constant>,
    /// The evaluated constructor (`_constructor`), for the context messages.
    f: Option<FunctionElementId>,
    error: ErrorAt,
    k: ClassId,
    ty: TypeId,
    fields: Vec<(Field, Value)>,
    super_name: Option<SymbolId>,
    super_args: Option<(UnitId, Vec<ast::Argument>)>,
    super_positional_args: Vec<Value>,
    super_named_args: Vec<(SymbolId, Value)>,
    cxi: Ctx,
}

impl Generator {
    fn ready(r: Constant) -> Generator {
        Generator {
            ready_engine: Some(r),
            f: None,
            error: ErrorAt { unit: UnitId(0), span: Span { start: 0, end: 0 } },
            k: ClassId(0),
            ty: TypeId(0),
            fields: Vec::new(),
            super_name: None,
            super_args: None,
            super_positional_args: Vec::new(),
            super_named_args: Vec::new(),
            cxi: Ctx::simple(UnitId(0), LibraryId(0)),
        }
    }

    fn con_super_named(mut self, n: Vec<(SymbolId, Value)>) -> Generator {
        self.super_named_args = n;
        self
    }
}

/// Backs up from `i` over whitespace to byte `b` (inclusive), if it comes right before.
fn back_up_to(source: &[u8], i: usize, b: u8) -> usize {
    let mut j = i;
    while j > 0 && source[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    if j > 0 && source[j - 1] == b { j - 1 } else { i }
}

/// Backs up from `i` over whitespace to one of the words (the first one that
/// matches).
fn back_up_word(source: &[u8], i: usize, words: &[&str]) -> usize {
    let mut j = i;
    while j > 0 && source[j - 1].is_ascii_whitespace() {
        j -= 1;
    }
    // `if (cond)`: skips the `(`.
    if j > 0 && source[j - 1] == b'(' {
        j -= 1;
        while j > 0 && source[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
    }
    for p in words {
        let p = p.as_bytes();
        if j >= p.len() && &source[j - p.len()..j] == p {
            return j - p.len();
        }
    }
    i
}

/// The start of the `for` that encloses position `i` (the nearest `for`
/// before it).
fn for_start(source: &[u8], i: usize) -> usize {
    let mut j = i.min(source.len());
    let mut prof = 0i32;
    while j > 0 {
        j -= 1;
        match source[j] {
            b')' => prof += 1,
            b'(' => {
                if prof == 0 {
                    let mut k = j;
                    while k > 0 && source[k - 1].is_ascii_whitespace() {
                        k -= 1;
                    }
                    if k >= 3 && &source[k - 3..k] == b"for" {
                        let mut s = k - 3;
                        // `await for`
                        let mut w = s;
                        while w > 0 && source[w - 1].is_ascii_whitespace() {
                            w -= 1;
                        }
                        if w >= 5 && &source[w - 5..w] == b"await" {
                            s = w - 5;
                        }
                        return s;
                    }
                    return i;
                }
                prof -= 1;
            }
            _ => {}
        }
    }
    i
}

/// The local constants of a unit: by the name's offset, the statement
/// (`Variables` or `for`) and the index in the list.
pub(super) fn constant_locals(a: &ast::Ast) -> HashMap<usize, (ast::StmtId, usize)> {
    let mut m = HashMap::new();
    for (i, s) in a.stmts.iter().enumerate() {
        let list = match &s.kind {
            ast::StmtKind::Variables(l) => l,
            ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => continue,
        };
        if !list.const_ {
            continue;
        }
        for (j, v) in list.variables.iter().enumerate() {
            m.insert(v.name.span.start, (ast::StmtId(i as u32), j));
        }
    }
    m
}

/// The name offsets of all formal parameters of a unit (functions,
/// constructors, function types and function-typed parameters).
fn parameter_offsets(a: &ast::Ast) -> HashSet<usize> {
    fn params(ps: &[ast::Parameter], s: &mut HashSet<usize>) {
        for p in ps {
            if let Some(n) = p.name {
                s.insert(n.span.start);
            }
            if let Some(f) = &p.function_parameters {
                params(f, s);
            }
        }
    }
    let mut s = HashSet::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            params(ps, &mut s);
        }
    }
    for m in a.members.iter() {
        if let ast::MemberKind::Constructor(k) = &m.kind {
            params(&k.parameters, &mut s);
        }
    }
    s
}
