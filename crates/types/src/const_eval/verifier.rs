//! Analyzer 6.11's `ConstantVerifier`
//! (`src/dart/constant/constant_verifier.dart`): walks the units of a
//! library and reports constant errors: `const` variables, `final` fields of
//! classes with a `const` constructor, constant creations and collections,
//! records, default values, constant and relational patterns, initializers
//! of `const` constructors.

use super::evaluation::{Constant, Ctx, Engine, Invalid};
use super::exhaustiveness::{Case, Entry, Exhaustiveness};
use super::value::{InstanceState, Value};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::{
    Code, Diagnostic, Span, codes::compile_time_error as c, codes::warning as w,
};
use dartforge_elements::model::{ClassId, LibraryId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{
    self, CollectionElement, DeclKind, ExprId, ExprKind, MemberKind, PatternId, PatternKind,
    StmtId, StmtKind,
};
use std::collections::HashMap;

/// The codes more specific than the default (`_reportError`): reported as
/// they are, instead of the default code at the point of use.
const SPECIFIC: &[Code] = &[
    c::CONST_EVAL_EXTENSION_METHOD,
    c::CONST_EVAL_EXTENSION_TYPE_METHOD,
    c::CONST_EVAL_FOR_ELEMENT,
    c::CONST_EVAL_METHOD_INVOCATION,
    c::CONST_EVAL_PROPERTY_ACCESS,
    c::CONST_EVAL_THROWS_EXCEPTION,
    c::CONST_EVAL_THROWS_IDBZE,
    c::CONST_EVAL_TYPE_BOOL_NUM_STRING,
    c::CONST_EVAL_TYPE_BOOL,
    c::CONST_EVAL_TYPE_BOOL_INT,
    c::CONST_EVAL_TYPE_INT,
    c::CONST_EVAL_TYPE_NUM,
    c::CONST_EVAL_TYPE_NUM_STRING,
    c::CONST_EVAL_TYPE_STRING,
    c::RECURSIVE_COMPILE_TIME_CONSTANT,
    c::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH,
    c::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH,
    c::CONST_TYPE_PARAMETER,
    c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF,
    c::CONST_SPREAD_EXPECTED_LIST_OR_SET,
    c::CONST_SPREAD_EXPECTED_MAP,
    c::EXPRESSION_IN_MAP,
    c::VARIABLE_TYPE_MISMATCH,
    c::NON_BOOL_CONDITION,
    c::NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY,
    c::SET_ELEMENT_FROM_DEFERRED_LIBRARY,
    c::SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY,
    c::INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY,
    c::IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY,
    c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY,
    c::NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY,
    c::PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY,
    c::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
    c::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION,
];

/// The codes whose checking belongs to this module (what inference emits for
/// them is discarded by whoever measures parity).
pub const CODES: &[&str] = &[
    "const_initialized_with_non_constant_value",
    "const_eval_throws_exception",
    "const_with_non_constant_argument",
    "equal_elements_in_const_set",
    "equal_keys_in_const_map",
];

struct Verifier<'m, 'a> {
    m: &'m mut Engine<'a>,
    lib: LibraryId,
    unit: UnitId,
    output: Vec<(UnitId, Diagnostic)>,
    /// `(unit_id, mem, índice)` → variable; `(unit_id, decl, índice)` → variable.
    fields: HashMap<(UnitId, ast::MemberId, usize), VariableId>,
    top: HashMap<(UnitId, ast::DeclId, usize), VariableId>,
    /// The class (declaration) of each member.
    member_class: HashMap<(UnitId, ast::MemberId), ClassId>,
    patterns_enabled: bool,
    /// Values of the unit's constant patterns and map pattern keys
    /// (`_withConstantPatternValues`), for exhaustiveness.
    the_default_values: HashMap<PatternId, Value>,
    key_values: HashMap<ExprId, Value>,
    /// `inConstantExpression`: inside a parameter's default value or the
    /// initializer of an instance field of a class with a `const` generative
    /// constructor (a constant expression without a constant context).
    in_constant_expression: bool,
}

/// The errors that the `_ConstantAnalysisErrorListener` of the analyzer
/// 3.6.2 linter counts as a "constant error"
/// (`analyzer/lib/src/lint/linter.dart`), by unique name.
const CONSTANT_ERRORS: &[&str] = &[
    "CompileTimeErrorCode.CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY",
    "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST",
    "CompileTimeErrorCode.CONST_EVAL_EXTENSION_METHOD",
    "CompileTimeErrorCode.CONST_EVAL_EXTENSION_TYPE_METHOD",
    "CompileTimeErrorCode.CONST_EVAL_METHOD_INVOCATION",
    "CompileTimeErrorCode.CONST_EVAL_PROPERTY_ACCESS",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_INT",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_NUM_STRING",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_INT",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_NUM",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_NUM_STRING",
    "CompileTimeErrorCode.CONST_EVAL_TYPE_STRING",
    "CompileTimeErrorCode.CONST_EVAL_THROWS_EXCEPTION",
    "CompileTimeErrorCode.CONST_EVAL_THROWS_IDBZE",
    "CompileTimeErrorCode.CONST_EVAL_FOR_ELEMENT",
    "CompileTimeErrorCode.CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY",
    "CompileTimeErrorCode.CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY",
    "CompileTimeErrorCode.CONST_TYPE_PARAMETER",
    "CompileTimeErrorCode.CONST_WITH_NON_CONST",
    "CompileTimeErrorCode.CONST_WITH_NON_CONSTANT_ARGUMENT",
    "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS",
    "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF",
    "CompileTimeErrorCode.INVALID_CONSTANT",
    "CompileTimeErrorCode.MISSING_CONST_IN_LIST_LITERAL",
    "CompileTimeErrorCode.MISSING_CONST_IN_MAP_LITERAL",
    "CompileTimeErrorCode.MISSING_CONST_IN_SET_LITERAL",
    "CompileTimeErrorCode.NON_BOOL_CONDITION",
    "CompileTimeErrorCode.NON_CONSTANT_LIST_ELEMENT",
    "CompileTimeErrorCode.NON_CONSTANT_MAP_ELEMENT",
    "CompileTimeErrorCode.NON_CONSTANT_MAP_KEY",
    "CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE",
    "CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD",
    "CompileTimeErrorCode.NON_CONSTANT_SET_ELEMENT",
];

/// `canBeConst` of an instance creation (`_canBeConstInstanceCreation`,
/// `analyzer/lib/src/lint/linter.dart`): the resolved constructor is `const`
/// and the `ConstantVerifier` over the creation, as if it had `const`,
/// reports none of the `CONSTANT_ERRORS`. The constant creation here is
/// the same `visitInstanceCreationExpression` as in the regular verification
/// (`constant_creation`), with the arguments visited in a constant context.
pub fn creation_can_be_const(
    m: &mut Engine<'_>,
    lib: LibraryId,
    unit_id: UnitId,
    e: ExprId,
) -> bool {
    let program = m.program;
    let a = m.ast(unit_id);
    let args_list = match &a.expr(e).kind {
        ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => {
            &**arguments
        }
        _ => return false,
    };
    let Some(Resolved::Constructor(f)) = m.resolved(unit_id, e) else {
        return false;
    };
    if !program.function(*f).const_ {
        return false;
    }
    let patterns_enabled = program.library(lib).features.version()
        >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    let mut v = Verifier {
        m,
        lib,
        unit: unit_id,
        output: Vec::new(),
        fields: HashMap::new(),
        top: HashMap::new(),
        member_class: HashMap::new(),
        patterns_enabled,
        the_default_values: HashMap::new(),
        key_values: HashMap::new(),
        in_constant_expression: false,
    };
    v.constant_creation(a, e, args_list);
    !v.output.iter().any(|(_, d)| {
        d.code
            .is_some_and(|c| CONSTANT_ERRORS.contains(&c.info().unique))
    })
}

/// A verifier without the library's tables (the lint queries).
fn standalone_verifier<'v, 'm>(
    m: &'v mut Engine<'m>,
    lib: LibraryId,
    unit_id: UnitId,
) -> Verifier<'v, 'm> {
    let patterns_enabled = m.program.library(lib).features.version()
        >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    Verifier {
        m,
        lib,
        unit: unit_id,
        output: Vec::new(),
        fields: HashMap::new(),
        top: HashMap::new(),
        member_class: HashMap::new(),
        patterns_enabled,
        the_default_values: HashMap::new(),
        key_values: HashMap::new(),
        in_constant_expression: false,
    }
}

/// `canBeConst` of a typed literal (`_canBeConstTypedLiteral`): the
/// `ConstantVerifier` over the literal with `const` reports none of the
/// `CONSTANT_ERRORS`.
pub fn literal_can_be_const(
    m: &mut Engine<'_>,
    lib: LibraryId,
    unit_id: UnitId,
    e: ExprId,
) -> bool {
    let a = m.ast(unit_id);
    let mut v = standalone_verifier(m, lib, unit_id);
    v.expr(a, e, true);
    !v.output.iter().any(|(_, d)| {
        d.code
            .is_some_and(|c| CONSTANT_ERRORS.contains(&c.info().unique))
    })
}

/// `ConstructorDeclaration.canBeConst` (`analyzer/lib/src/lint/linter.dart`):
/// the class has no instance field that is not `final`, and the
/// `ConstantVerifier` over the constructor marked `const` reports none of the
/// `CONSTANT_ERRORS`: potentially constant initializers, instance fields
/// with a constant initializer and constant default values.
pub fn constructor_can_be_const(
    m: &mut Engine<'_>,
    lib: LibraryId,
    unit_id: UnitId,
    mid: ast::MemberId,
) -> bool {
    let a = m.ast(unit_id);
    let MemberKind::Constructor(k) = &a.member(mid).kind else {
        return false;
    };
    let mut v = standalone_verifier(m, lib, unit_id);
    let Some(cls) = v.class_of(a, mid) else {
        return false;
    };
    let program = v.m.program;
    // `hasNonFinalField`.
    let non_final = program.class(cls).fields.iter().any(|f| {
        let x = program.variable(*f);
        !x.static_ && !x.final_ && !x.const_
    });
    if non_final {
        return false;
    }
    let cx = Ctx {
        lexical: None,
        ..v.cx()
    };
    for init in k.initializers.iter() {
        let exprs: Vec<ExprId> = match init {
            ast::Initializer::Field { value, .. } => vec![*value],
            ast::Initializer::Assert {
                condition, message, ..
            } => std::iter::once(*condition).chain(*message).collect(),
            ast::Initializer::Super { arguments, .. }
            | ast::Initializer::Redirect { arguments, .. } => {
                arguments.args.iter().map(|x| x.value).collect()
            }
        };
        for x in exprs {
            let mut nodes = Vec::new();
            super::potential::collect_into(v.m, &cx, x, true, false, &mut nodes);
            if !nodes.is_empty() {
                return false;
            }
        }
    }
    if !k.factory {
        for fm in v.class_members(a, cls) {
            let MemberKind::Field(l) = &a.member(fm).kind else {
                continue;
            };
            if l.static_ {
                continue;
            }
            for var in l.variables.iter() {
                let Some(init) = var.initializer else {
                    continue;
                };
                let cx = v.cx();
                if matches!(v.m.evaluate(&cx, init, l.const_), Constant::Invalid(_)) {
                    return false;
                }
            }
        }
    }
    v.default_values(a, &k.parameters);
    !v.output.iter().any(|(_, d)| {
        d.code
            .is_some_and(|c| CONSTANT_ERRORS.contains(&c.info().unique))
    })
}

/// The constant errors of the units of `lib`.
pub fn the_verify(m: &mut Engine<'_>, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    // The cycles come from the dependency graph, before any evaluation
    // (`computeConstants`).
    super::cycles::compute(m, lib);
    let program = m.program;
    let mut the_fields = HashMap::new();
    let mut top = HashMap::new();
    let mut member_class = HashMap::new();
    for (i, v) in program.index().library_variables[lib.0 as usize]
        .iter()
        .map(|&x| (x.0 as usize, program.variable(x)))
    {
        if v.library != lib {
            continue;
        }
        match v.node {
            VariableRef::Field {
                unit,
                member,
                index,
            } => {
                the_fields.insert((unit, member, index), VariableId(i as u32));
                if let Some(k) = v.class {
                    member_class.insert((unit, member), k);
                }
            }
            VariableRef::TopLevel { unit, decl, index } => {
                top.insert((unit, decl, index), VariableId(i as u32));
            }
            _ => {}
        }
    }
    let patterns_enabled = program.library(lib).features.version()
        >= dartforge_frontend::features::LanguageVersion::new(3, 0);
    let mut v = Verifier {
        m,
        lib,
        unit: UnitId(0),
        output: Vec::new(),
        fields: the_fields,
        top,
        member_class,
        patterns_enabled,
        the_default_values: HashMap::new(),
        key_values: HashMap::new(),
        in_constant_expression: false,
    };
    for &u in &program.library(lib).units {
        if program.unit(u).role == dartforge_elements::model::UnitRole::Patch {
            continue;
        }
        v.unit = u;
        v.the_default_values.clear();
        v.key_values.clear();
        let a = v.m.ast(u);
        for &d in &program.unit(u).unit.declarations {
            v.declaration(a, d);
        }
        // `visitAnnotation`: the arguments of an annotation that creates with a
        // `const` constructor (`_validateConstantArguments`).
        for m in dartforge_frontend::parents::all_annotations(a, &program.unit(u).unit) {
            v.annotation(m);
        }
    }
    v.output
}

impl Verifier<'_, '_> {
    /// `const C.the_name()` with `C` a mixin application (`class C = S with M;`): the
    /// forwarded constructor is `const` only if the superclass's is and no
    /// mixin declares a field (`ConstructorElementImpl.isConst` of forwarded
    /// constructors, `analyzer/lib/src/dart/element/element.dart:612-614`);
    /// otherwise, `CONST_WITH_NON_CONST` at the `const` keyword.
    fn non_const_forwarded(
        &mut self,
        ty: ast::TypeId,
        mut the_constructor: Option<ast::Name>,
        span: Span,
    ) {
        let program = self.m.program;
        let u = self.unit;
        let class = match self
            .annotation_type(ty)
            .map(|t| self.m.table.get(t).clone())
        {
            Some(Type::Interface { class, .. }) => class,
            _ => {
                // The creation's type is not recorded: the written name.
                let ast::TypeKind::Named { name, .. } = &program.unit(u).ast.ty(ty).kind else {
                    return;
                };
                // `C.the_name` comes out of the parser as a two-part name (`p.C` or
                // `C.the_name`): the first part as a class is `C`, with the constructor.
                let (el, ctor) = match &name[..] {
                    [n] => (
                        program.lookup_in_unit(u, n.sym).and_then(|b| b.getter),
                        the_constructor,
                    ),
                    [a1, b1] => match program.lookup_in_unit(u, a1.sym).and_then(|b| b.getter) {
                        Some(el @ dartforge_elements::model::Element::Class(_)) => {
                            (Some(el), Some(*b1))
                        }
                        _ => (
                            program
                                .lookup_prefixed_in_unit(u, a1.sym, b1.sym)
                                .and_then(|b| b.getter),
                            the_constructor,
                        ),
                    },
                    [p, c, n] => (
                        program
                            .lookup_prefixed_in_unit(u, p.sym, c.sym)
                            .and_then(|b| b.getter),
                        Some(*n),
                    ),
                    _ => (None, None),
                };
                the_constructor = ctor;
                match el {
                    Some(dartforge_elements::model::Element::Class(c)) => c,
                    _ => return,
                }
            }
        };
        let class = &class;
        if program.class(*class).kind != dartforge_elements::model::ClassKind::MixinApplication {
            return;
        }
        let interner = self.m.interner;
        let the_key = match the_constructor {
            Some(n) if interner.resolve(n.sym) != "new" => Some(n.sym),
            _ => interner.lookup(""),
        };
        let Some(the_key) = the_key else { return };
        let Some(is_const) = self.const_forwarded_constructor(*class, the_key, 0) else {
            return;
        };
        if !is_const {
            self.report(
                c::CONST_WITH_NON_CONST,
                Span {
                    start: span.start,
                    end: span.start + 5,
                },
                Vec::new(),
            );
        }
    }

    /// `isConst` of constructor `the_key` as seen by class `c` (forwarded in mixin
    /// applications); `None` without a constructor.
    fn const_forwarded_constructor(
        &self,
        c: ClassId,
        the_key: dartforge_intern::SymbolId,
        prof: u32,
    ) -> Option<bool> {
        if prof > 32 {
            return None;
        }
        let program = self.m.program;
        let k = program.class(c);
        if k.kind != dartforge_elements::model::ClassKind::MixinApplication {
            let f = *k.constructors.get(&the_key)?;
            let f = program.public(f);
            if program.function(f).factory {
                return None;
            }
            return Some(program.function(f).const_);
        }
        let s = k.supertype_class?;
        let from_super = self.const_forwarded_constructor(s, the_key, prof + 1)?;
        let mixin_with_field = k
            .mixin_classes
            .iter()
            .any(|&m| !program.class(m).fields.is_empty());
        Some(from_super && !mixin_with_field)
    }

    /// `ConstantVerifier.visitAnnotation`: the element is a `const` constructor
    /// with an argument list; each argument is evaluated, and one that is not
    /// constant is `CONST_WITH_NON_CONSTANT_ARGUMENT`.
    fn annotation(&mut self, m: &ast::Annotation) {
        let Some(args) = &m.arguments else { return };
        let program = self.m.program;
        let interner = self.m.interner;
        let Some(el) = crate::annotations::annotation_element(program, interner, self.unit, m)
        else {
            return;
        };
        let cls = match el {
            dartforge_elements::model::Element::Class(c) => c,
            dartforge_elements::model::Element::Typedef(td) => match self
                .m
                .table
                .get(self.m.outline.typedefs[td.0 as usize].target_type)
            {
                Type::Interface { class, .. } => *class,
                _ => return,
            },
            _ => return,
        };
        // The constructor: the last name, if it is a constructor of the class;
        // otherwise the unnamed one.
        let last = m.name.last().map(|n| n.sym);
        let named_one = (m.name.len() >= 2)
            .then_some(last)
            .flatten()
            .filter(|&s| {
                interner.resolve(s) != "new"
                    && interner.resolve(s) != interner.resolve(program.class(cls).name)
            })
            .and_then(|s| program.class(cls).constructors.get(&s).copied());
        let f = named_one.or_else(|| {
            interner
                .lookup("")
                .and_then(|k| program.class(cls).constructors.get(&k).copied())
        });
        let Some(f) = f else { return };
        if !program.function(program.public(f)).const_ {
            return;
        }
        for x in args.args.iter() {
            self.evaluate_and_report(x.value, true, c::CONST_WITH_NON_CONSTANT_ARGUMENT);
        }
    }

    fn cx(&self) -> Ctx {
        Ctx::simple(self.unit, self.lib)
    }

    fn report(&mut self, code: Code, span: Span, args: Vec<String>) {
        self.output
            .push((self.unit, Diagnostic::with_code(code, span, args)));
    }

    /// The error with its code and the `contextMessages` (in each one's file,
    /// when it is not this unit's).
    fn report_with_context(&mut self, i: &Invalid) {
        let mut d = Diagnostic::with_code(i.code, i.span, i.args.clone());
        for (u, span, text) in &i.context {
            let file = (*u != self.unit).then(|| self.m.program.unit_path(*u).into());
            d.context.push(dartforge_diagnostics::ContextMessage {
                file,
                span: *span,
                message: text.as_str().into(),
            });
        }
        self.output.push((self.unit, d));
    }

    /// `_reportError`: the specific code carries the context messages; the
    /// default one does not.
    fn report_invalid(&mut self, i: &Invalid, the_pattern: Option<Code>) {
        if i.avoid_report || i.unit != self.unit {
            return;
        }
        if SPECIFIC.contains(&i.code) {
            self.report_with_context(i);
        } else if let Some(p) = the_pattern {
            self.report(p, i.span, Vec::new());
        }
    }

    /// `_evaluateAndReportError`.
    fn evaluate_and_report(&mut self, e: ExprId, in_const: bool, the_pattern: Code) -> Constant {
        let cx = self.cx();
        let r = self.m.evaluate(&cx, e, in_const);
        if let Constant::Invalid(i) = &r {
            let i = (**i).clone();
            self.report_invalid(&i, Some(the_pattern));
        }
        r
    }

    // -- Declarations -------------------------------------------------------------

    fn declaration(&mut self, a: &ast::Ast, d: ast::DeclId) {
        let decl = a.decl(d);
        match &decl.kind {
            DeclKind::Variables(l) => {
                for (i, var) in l.variables.iter().enumerate() {
                    if let Some(init) = var.initializer {
                        self.expr(a, init, l.const_);
                        if l.const_
                            && let Some(&v) = self.top.get(&(self.unit, d, i))
                        {
                            self.variable_result(v, true);
                        }
                    }
                }
            }
            DeclKind::Function(f) => self.function(a, *f),
            DeclKind::Class(k) => self.members(a, &k.members),
            DeclKind::Mixin(k) => self.members(a, &k.members),
            DeclKind::Enum(k) => {
                let unit_id = self.unit;
                let cls = self
                    .m
                    .program
                    .class_of_decl(unit_id, d)
                    .map(|c| c.0 as usize);
                for (i, cst) in k.constants.iter().enumerate() {
                    if let Some(args) = &cst.arguments {
                        for arg in args.args.iter() {
                            self.expr(a, arg.value, true);
                        }
                        // `visitEnumConstantDeclaration`: `_validateConstantArguments`.
                        for arg in args.args.iter() {
                            self.evaluate_and_report(
                                arg.value,
                                true,
                                c::CONST_WITH_NON_CONSTANT_ARGUMENT,
                            );
                        }
                    }
                    // `visitEnumConstantDeclaration` (`constant_verifier.dart:204-217`):
                    // the constant's invalid `evaluationResult`, without a default
                    // code (the evaluation exception becomes
                    // `CONST_EVAL_THROWS_EXCEPTION` at the constant).
                    if let Some(ci) = cls
                        && let Some(&v) = self.m.program.classes[ci].enum_constants.get(i)
                    {
                        self.variable_result(v, false);
                    }
                }
                self.members(a, &k.members);
            }
            DeclKind::Extension(k) => self.members(a, &k.members),
            DeclKind::ExtensionType(k) => self.members(a, &k.members),
            DeclKind::Typedef(_) => {}
        }
    }

    fn variable_result(&mut self, v: VariableId, const_: bool) {
        if let Some(Constant::Invalid(i)) = self.m.variable_value(v) {
            let i = (*i).clone();
            let the_pattern = if const_ {
                Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE)
            } else {
                None
            };
            self.report_invalid(&i, the_pattern);
        }
    }

    fn members(&mut self, a: &ast::Ast, members: &[ast::MemberId]) {
        for &mid in members {
            let mem = a.member(mid);
            match &mem.kind {
                MemberKind::Field(l) => {
                    for (i, var) in l.variables.iter().enumerate() {
                        let Some(init) = var.initializer else {
                            continue;
                        };
                        let const_class = !l.static_
                            && self
                                .member_class
                                .get(&(self.unit, mid))
                                .copied()
                                .is_some_and(|k| {
                                    self.m.program.class(k).kind
                                        == dartforge_elements::model::ClassKind::Class
                                        && self.has_const_generative_constructor(k)
                                });
                        let before =
                            std::mem::replace(&mut self.in_constant_expression, const_class);
                        self.expr(a, init, l.const_);
                        self.in_constant_expression = before;
                        if !(l.const_ || l.final_) {
                            continue;
                        }
                        // A class with a primary constructor (3.13.4): a field whose
                        // initializer is not potentially constant has no evaluated
                        // value (its error is the field's,
                        // `CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST`,
                        // at the `const`); the evaluation error only comes from the
                        // potentially constant one (measured on the 3.13.4 oracle,
                        // `primary_constructors/const/potentially_constant_error_test`).
                        if !l.static_ && Self::class_member_with_primary(a, mid) {
                            let cx = self.cx();
                            let mut nodes = Vec::new();
                            super::potential::collect_into(
                                self.m, &cx, init, true, false, &mut nodes,
                            );
                            if !nodes.is_empty() {
                                continue;
                            }
                        }
                        let Some(&v) = self.fields.get(&(self.unit, mid, i)) else {
                            continue;
                        };
                        if !l.static_ {
                            let cls = self.member_class.get(&(self.unit, mid)).copied();
                            let has = cls.is_some_and(|k| self.has_const_generative_constructor(k));
                            let is_class = cls.is_some_and(|k| {
                                self.m.program.class(k).kind
                                    == dartforge_elements::model::ClassKind::Class
                            });
                            if is_class && !has {
                                continue;
                            }
                            if !has {
                                continue;
                            }
                        } else if !l.const_ {
                            continue;
                        }
                        self.variable_result(v, l.const_);
                    }
                }
                MemberKind::Method(f) => self.function(a, *f),
                MemberKind::Constructor(k) => self.constructor(a, mid, k),
            }
        }
    }

    /// Member `mid` belongs to a class with a primary constructor. Enums are left
    /// out: their constants (`e(1)`) evaluate the constructor, and the field
    /// initializer's error comes from that evaluation (the `CONST_EVAL_METHOD_INVOCATION`
    /// of `final int x = fn(p)` in `enum const E(int p)` on the 3.13.4 oracle).
    fn class_member_with_primary(a: &ast::Ast, mid: ast::MemberId) -> bool {
        a.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(x) => x.primary_constructor.is_some() && x.members.contains(&mid),
            _ => false,
        })
    }

    fn has_const_generative_constructor(&self, k: ClassId) -> bool {
        self.m
            .program
            .class(k)
            .constructors
            .values()
            .any(|f| self.m.program.function(*f).const_ && !self.m.program.function(*f).factory)
    }

    /// `visitConstructorDeclaration`.
    /// The `ErrorVerifier` rules on a constructor's header
    /// (docs/ANALYZER-ESPECIFICACAO.md §D): `non_const_generative_enum_constructor`
    /// (`error_verifier.dart:4710-4719`); and, for a `const` generative
    /// constructor, in this order and stopping at the first report,
    /// `const_constructor_with_mixin_with_field` (`:2807-2856`),
    /// `const_constructor_with_non_const_super` (`:2858-2893`) and
    /// `const_constructor_with_non_final_field` (`:2898-2916`); plus
    /// `const_constructor_throws_exception` (`:2936-2943`) on each `throw` of the
    /// initializers. Written without compiling or running (2026-10-04).
    /// The constructor is the primary one (3.13) of the class or the enum, or
    /// its body part.
    fn is_primary(&self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) -> bool {
        k.primary_body_part
            || a.decls.iter().any(|d| match &d.kind {
                ast::DeclKind::Class(x) => x.primary_constructor == Some(mid),
                ast::DeclKind::Enum(x) => x.primary_constructor == Some(mid),
                _ => false,
            })
    }

    fn constructor_rules(&mut self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) {
        use dartforge_elements::model::ClassKind;
        let Some(cls) = self.class_of(a, mid) else {
            return;
        };
        let program = self.m.program;
        let interner = self.m.interner;
        let data = program.class(cls);
        // `atConstructorDeclaration`: from the class name to the end of the
        // constructor name, when there is one.
        let header = Span {
            start: k.class_name.span.start,
            end: k.name.map_or(k.class_name.span.end, |n| n.span.end),
        };
        // Only in `visitConstructorDeclaration`: a leftover `this` part (repeated
        // or without a primary header) is a `PrimaryConstructorBody`, not a
        // constructor declaration (3.13.4).
        if data.kind == ClassKind::Enum && !k.const_ && !k.factory && !k.primary_body_part {
            self.report(c::NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR, header, Vec::new());
        }
        if !k.const_ || k.factory {
            return;
        }
        for init in k.initializers.iter() {
            let sp = match init {
                ast::Initializer::Field { span, .. }
                | ast::Initializer::Super { span, .. }
                | ast::Initializer::Redirect { span, .. }
                | ast::Initializer::Assert { span, .. } => *span,
            };
            let throws: Vec<Span> = a
                .exprs
                .iter()
                .filter(|x| {
                    matches!(x.kind, ExprKind::Throw(_))
                        && x.span.start >= sp.start
                        && x.span.end <= sp.end
                })
                .map(|x| x.span)
                .collect();
            for s in throws {
                self.report(c::CONST_CONSTRUCTOR_THROWS_EXCEPTION, s, Vec::new());
            }
        }
        // Instance fields of the mixins in the `with` clause: every field that
        // is neither static nor (`abstract` and `final`).
        let mut mixin_fields: Vec<String> = Vec::new();
        for &mx in data.mixin_classes.iter() {
            for &v in program.class(mx).fields.iter() {
                let var = program.variable(v);
                if var.static_ {
                    continue;
                }
                let the_abstract_ = match var.node {
                    VariableRef::Field { unit, member, .. } => {
                        matches!(&program.unit(unit).ast.member(member).kind, MemberKind::Field(l) if l.abstract_)
                    }
                    _ => false,
                };
                if the_abstract_ && var.final_ {
                    continue;
                }
                mixin_fields.push(format!(
                    "'{}.{}'",
                    interner.resolve(program.class(mx).name),
                    interner.resolve(var.name)
                ));
            }
        }
        // `implicitErrorRange`: for a primary constructor (3.13) it is the
        // `errorRange` of `PrimaryConstructorDeclaration` (`ast.dart:45420-45424`),
        // from the first token (the `const`, when written) to the constructor
        // name, or only the first token.
        let primary = self.is_primary(a, mid, k);
        let range = if primary {
            let source = &program.unit(self.unit).source;
            let start = if k.const_ {
                let before = source[..k.class_name.span.start].trim_end();
                if before.ends_with("const") {
                    before.len() - "const".len()
                } else {
                    k.class_name.span.start
                }
            } else {
                k.class_name.span.start
            };
            let end = match k.name {
                Some(n) => n.span.end,
                None if start < k.class_name.span.start => start + "const".len(),
                None => k.class_name.span.end,
            };
            Span { start, end }
        } else {
            k.class_name.span
        };
        if mixin_fields.len() == 1 {
            self.report(
                c::CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD,
                range,
                mixin_fields,
            );
            return;
        }
        if mixin_fields.len() > 1 {
            self.report(
                c::CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS,
                range,
                vec![mixin_fields.join(", ")],
            );
            return;
        }
        // The superclass constructor that this one invokes (the written one, or
        // the implicit unnamed one) is not `const`. An enum always calls a
        // `const` one; a redirecting one is checked elsewhere.
        let redirects = k
            .initializers
            .iter()
            .any(|i| matches!(i, ast::Initializer::Redirect { .. }));
        if data.kind != ClassKind::Enum && !redirects {
            let written = k.initializers.iter().find_map(|i| match i {
                ast::Initializer::Super {
                    span, constructor, ..
                } => Some((*span, constructor.map(|n| n.sym))),
                _ => None,
            });
            if let Some(sup) = super::cycles::declared_superclass(program, cls)
                && Some(sup) != self.m.core.object_class
            {
                let super_name = written.and_then(|(_, n)| n);
                let the_key = super_name.or_else(|| interner.lookup(""));
                let tgt = the_key.and_then(|ch| program.class(sup).constructors.get(&ch).copied());
                let non_const = match tgt {
                    Some(g) => !program.function(program.public(g)).const_,
                    // A superclass without a declared constructor: the implicit
                    // default one, which is not `const`.
                    None => program.class(sup).constructors.is_empty() && super_name.is_none(),
                };
                if non_const {
                    // Without a written `super(…)`, the `implicitErrorRange`: for
                    // the primary, the header's range (`class const B()`: the
                    // `const`).
                    let implicit = if primary { range } else { k.class_name.span };
                    let span = written.map_or(implicit, |(s, _)| s);
                    // In 3.6.2 the argument is the CONSTRUCTOR's class; in
                    // 3.13.4, the superclass (the second argument, which only
                    // the 3.13 variant of the code uses: T2, case c04).
                    let the_name = interner.resolve(data.name).to_string();
                    let upper = interner.resolve(program.class(sup).name).to_string();
                    self.report(
                        c::CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER,
                        span,
                        vec![the_name, upper],
                    );
                    return;
                }
            }
        }
        // `hasNonFinalField`: the class, the mixins and the superclasses,
        // breadth-first: some instance field that is neither `final` nor
        // `const`. In 3.13.4 (`link.dart:284-323`, `_computeHasNonFinalField`)
        // only a field with storage counts: `abstract int x;` does not (in
        // 3.6.2, `element.dart:329-356`, it does).
        let reference_313 =
            program.reference(self.unit) == dartforge_diagnostics::AnalyzerReference::V3_13;
        if data.kind == ClassKind::Class {
            let mut queue: Vec<ClassId> = vec![cls];
            let mut seen: Vec<ClassId> = Vec::new();
            let mut found = false;
            while let Some(x) = queue.pop() {
                if seen.contains(&x) {
                    continue;
                }
                seen.push(x);
                let dx = program.class(x);
                if dx.fields.iter().any(|&v| {
                    let var = program.variable(v);
                    let the_abstract_ = var.getter.is_some_and(|g| program.function(g).abstract_);
                    !var.final_ && !var.const_ && !var.static_ && !(reference_313 && the_abstract_)
                }) {
                    found = true;
                    break;
                }
                queue.extend(dx.mixin_classes.iter().copied());
                queue.extend(dx.supertype_class);
            }
            if found {
                // For the primary, the header's `errorRange`
                // (`class const A()`: `const`; `class const A.n()`:
                // `const A.n`).
                let span = if primary { range } else { header };
                self.report(c::CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD, span, Vec::new());
            }
        }
    }

    /// `_validateConstructorInitializers`: every initializer of a `const`
    /// constructor is potentially constant (`INVALID_CONSTANT` at each node
    /// that is not).
    fn potentially_constant_initializers(&mut self, k: &ast::Constructor) {
        let cx = Ctx {
            lexical: None,
            ..self.cx()
        };
        for init in k.initializers.iter() {
            let exprs: Vec<ExprId> = match init {
                ast::Initializer::Field { value, .. } => vec![*value],
                ast::Initializer::Assert {
                    condition, message, ..
                } => std::iter::once(*condition).chain(*message).collect(),
                ast::Initializer::Super { arguments, .. }
                | ast::Initializer::Redirect { arguments, .. } => {
                    arguments.args.iter().map(|x| x.value).collect()
                }
            };
            for x in exprs {
                let mut nodes = Vec::new();
                super::potential::collect_into(self.m, &cx, x, true, false, &mut nodes);
                for n in nodes {
                    self.report(c::INVALID_CONSTANT, n, Vec::new());
                }
            }
        }
    }

    fn constructor(&mut self, a: &ast::Ast, mid: ast::MemberId, k: &ast::Constructor) {
        self.constructor_rules(a, mid, k);
        if k.const_ {
            // The generative constructor of a cycle (the factory ones belong to
            // `ErrorVerifier`): at the name of the header's class.
            let in_cycle = self
                .m
                .graph
                .constructor_of(self.unit, mid)
                .is_some_and(|f| self.m.graph.constructors_in_cycle.contains(&f));
            // The primary constructor (3.13) is a `PrimaryConstructorDeclaration`:
            // `visitConstructorDeclaration` does not go through it. In 3.13.4 the
            // report goes from the type name (or the `new`) to the end of the name
            // (`errorRange`); in 3.6.2, only the `returnType`.
            let primary = self.is_primary(a, mid, k);
            if in_cycle && !k.factory && !primary {
                let span = match k.name {
                    Some(n)
                        if self.m.program.reference(self.unit)
                            == dartforge_diagnostics::AnalyzerReference::V3_13 =>
                    {
                        Span {
                            start: k.class_name.span.start,
                            end: n.span.end,
                        }
                    }
                    _ => k.class_name.span,
                };
                self.report(c::RECURSIVE_CONSTANT_CONSTRUCTOR, span, Vec::new());
            }
            self.potentially_constant_initializers(k);
            if !k.factory {
                let primary = self.is_primary(a, mid, k);
                self.field_initializers(a, mid, primary);
            }
        }
        // The `this : …` part of a `const` extension type's primary constructor
        // (3.13): its initializers belong to that `const` constructor.
        if !k.const_
            && k.primary_body_part
            && a.decls.iter().any(|d| matches!(&d.kind, ast::DeclKind::ExtensionType(x) if x.const_ && x.members.contains(&mid)))
        {
            self.potentially_constant_initializers(k);
        }
        self.default_values(a, &k.parameters);
        for init in k.initializers.iter() {
            match init {
                ast::Initializer::Field { value, .. } => self.expr(a, *value, false),
                ast::Initializer::Assert {
                    condition, message, ..
                } => {
                    self.expr(a, *condition, false);
                    if let Some(m) = message {
                        self.expr(a, *m, false);
                    }
                }
                ast::Initializer::Super { arguments, .. }
                | ast::Initializer::Redirect { arguments, .. } => {
                    for x in arguments.args.iter() {
                        self.expr(a, x.value, false);
                    }
                }
            }
        }
        self.body(a, &k.body);
    }

    /// `_validateFieldInitializers`: instance fields with an initializer that is
    /// not constant, reported at the constructor's `const`. With a primary
    /// constructor (3.13.4, `_validatePrimaryFieldInitializers`), an initializer
    /// that has a node that is not potentially constant (the primary parameters
    /// count), without evaluating: `final int i = d.length` is not reported here
    /// (the evaluation error comes from the field), `final int x = fn(p)` is.
    fn field_initializers(&mut self, a: &ast::Ast, mid: ast::MemberId, primary: bool) {
        let Some(k) = self.class_of(a, mid) else {
            return;
        };
        let is_enum = self.m.program.class(k).kind == dartforge_elements::model::ClassKind::Enum;
        let Some(the_keyword) = self.const_keyword(a, mid) else {
            return;
        };
        let members: Vec<ast::MemberId> = self.class_members(a, k);
        for fm in members {
            let MemberKind::Field(l) = &a.member(fm).kind else {
                continue;
            };
            if l.static_ {
                continue;
            }
            for var in l.variables.iter() {
                if is_enum && self.m.interner.resolve(var.name.sym) == "values" {
                    continue;
                }
                let Some(init) = var.initializer else {
                    continue;
                };
                let cx = self.cx();
                let invalid = if primary {
                    let mut nodes = Vec::new();
                    super::potential::collect_into(self.m, &cx, init, true, false, &mut nodes);
                    !nodes.is_empty()
                } else {
                    matches!(self.m.evaluate(&cx, init, l.const_), Constant::Invalid(_))
                };
                if invalid {
                    let the_name = self.m.interner.resolve(var.name.sym).to_string();
                    self.report(
                        c::CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST,
                        the_keyword,
                        vec![the_name],
                    );
                }
            }
        }
    }

    /// The declared class that contains member `mid` (through the constructors).
    fn class_of(&self, _a: &ast::Ast, mid: ast::MemberId) -> Option<ClassId> {
        for (i, f) in self.m.program.functions.iter().enumerate() {
            let _ = i;
            if let dartforge_elements::model::FunctionRef::Constructor { unit, member } = f.node
                && unit == self.unit
                && member == mid
            {
                return f.class;
            }
        }
        None
    }

    fn class_members(&self, a: &ast::Ast, k: ClassId) -> Vec<ast::MemberId> {
        let Some(d) = self.m.program.class(k).decl else {
            return Vec::new();
        };
        if d.unit != self.unit {
            return Vec::new();
        }
        match &a.decl(d.decl).kind {
            DeclKind::Class(x) => x.members.clone(),
            DeclKind::Enum(x) => x.members.clone(),
            DeclKind::Mixin(x) => x.members.clone(),
            DeclKind::ExtensionType(x) => x.members.clone(),
            _ => Vec::new(),
        }
    }

    /// The constructor's `const` token.
    fn const_keyword(&self, a: &ast::Ast, mid: ast::MemberId) -> Option<Span> {
        let s = a.member(mid).span;
        let source = &self.m.program.unit(self.unit).source;
        let t = source.get(s.start..s.end)?;
        let mut i = 0;
        let b = t.as_bytes();
        while i + 5 <= b.len() {
            if &b[i..i + 5] == b"const"
                && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
                && b.get(i + 5)
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_')
            {
                return Some(Span {
                    start: s.start + i,
                    end: s.start + i + 5,
                });
            }
            i += 1;
        }
        None
    }

    fn function(&mut self, a: &ast::Ast, f: ast::FunctionId) {
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.default_values(a, ps);
        }
        self.body(a, &func.body);
    }

    /// `_validateDefaultValues`.
    fn default_values(&mut self, a: &ast::Ast, ps: &[ast::Parameter]) {
        for p in ps {
            let Some(d) = p.default_value else { continue };
            let before = std::mem::replace(&mut self.in_constant_expression, true);
            self.expr(a, d, false);
            self.in_constant_expression = before;
            // `defaultValue.typeOrThrow is InvalidType` (3.6.2
            // `constant_verifier.dart:825-826`): the error was already reported
            // (the undefined name, for example).
            if self.m.invalid_type(self.unit, d) {
                continue;
            }
            let r = self.m.default_result(self.unit, self.lib, d);
            if let Constant::Invalid(i) = r {
                self.report_invalid(&i, Some(c::NON_CONSTANT_DEFAULT_VALUE));
            }
        }
    }

    fn body(&mut self, a: &ast::Ast, b: &ast::FunctionBody) {
        match b {
            ast::FunctionBody::Block(s) => self.stmt(a, *s),
            ast::FunctionBody::Expression(e) => self.expr(a, *e, false),
            _ => {}
        }
    }

    // -- Comandos --------------------------------------------------------------

    fn stmt(&mut self, a: &ast::Ast, s: StmtId) {
        match &a.stmt(s).kind {
            StmtKind::Block(xs) => {
                for x in xs.iter() {
                    self.stmt(a, *x);
                }
            }
            StmtKind::Variables(l) => {
                for (i, var) in l.variables.iter().enumerate() {
                    if let Some(init) = var.initializer {
                        self.expr(a, init, l.const_);
                        if l.const_ {
                            let r = self.m.declared_local_value(self.unit, self.lib, s, i);
                            if let Constant::Invalid(x) = r {
                                self.report_invalid(
                                    &x,
                                    Some(c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE),
                                );
                            }
                        }
                    }
                }
            }
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(a, *value, false);
                self.pattern(a, *pattern);
            }
            StmtKind::Function(f) => self.function(a, *f),
            StmtKind::Expression(e) => self.expr(a, *e, false),
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(a, *condition, false);
                if let Some(p) = case_pattern {
                    self.pattern(a, *p);
                }
                if let Some(g) = guard {
                    self.expr(a, *g, false);
                }
                self.stmt(a, *then);
                if let Some(e) = else_ {
                    self.stmt(a, *e);
                }
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                match init {
                    Some(ast::ForInit::Variables(l)) => {
                        for var in l.variables.iter() {
                            if let Some(i) = var.initializer {
                                self.expr(a, i, l.const_);
                            }
                        }
                    }
                    Some(ast::ForInit::Expression(e)) => self.expr(a, *e, false),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        self.expr(a, *value, false);
                        self.pattern(a, *pattern);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(a, *c, false);
                }
                for u in updates.iter() {
                    self.expr(a, *u, false);
                }
                self.stmt(a, *body);
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                if let ast::ForInTarget::Pattern { pattern, .. } = target {
                    self.pattern(a, *pattern);
                }
                self.expr(a, *iterable, false);
                self.stmt(a, *body);
            }
            StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
                self.expr(a, *condition, false);
                self.stmt(a, *body);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(a, *value, false);
                for case in cases.iter() {
                    if let Some(p) = case.pattern {
                        if self.patterns_enabled {
                            self.pattern(a, p);
                        } else if let PatternKind::Variable { name, .. } = &a.pattern(p).kind
                            && self.m.body.units[self.unit.0 as usize]
                                .non_constant_name_patterns
                                .contains(&p)
                        {
                            // The old `case the_name:` (the expression of the `SwitchCase`).
                            self.report(c::NON_CONSTANT_CASE_EXPRESSION, name.span, Vec::new());
                        } else if let PatternKind::Constant(e) = &a.pattern(p).kind {
                            // `_validateSwitchStatement_nullSafety`.
                            let e = unparenthesize(a, *e);
                            self.expr(a, e, true);
                            // Without patterns, the value needs primitive equality:
                            // `CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS` with the value's type
                            // (`constant_verifier.dart:1009-1030`).
                            if let Constant::Value(v) =
                                self.evaluate_and_report(e, true, c::NON_CONSTANT_CASE_EXPRESSION)
                                && !v.actually_unknown()
                                && !self.m.primitive_equality(&v, self.lib)
                            {
                                let sp = self.m.span(self.unit, e);
                                let typ = self.m.format(v.ty);
                                self.report(
                                    c::CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS,
                                    sp,
                                    vec![typ],
                                );
                            }
                        }
                    }
                    if let Some(g) = case.guard {
                        self.expr(a, g, false);
                    }
                    for x in case.body.iter() {
                        self.stmt(a, *x);
                    }
                }
                if self.patterns_enabled {
                    let the_cases: Vec<Case> = cases
                        .iter()
                        .map(|k| Case {
                            pattern: k.pattern,
                            stored: k.guard.is_some(),
                        })
                        .collect();
                    let start = a.stmt(s).span.start;
                    let pos: Vec<usize> =
                        cases.iter().map(|k| self.case_keyword_start(k)).collect();
                    self.exhaustiveness(a, *value, start, &the_cases, &pos, false);
                }
            }
            StmtKind::Return(Some(e)) | StmtKind::Yield { value: e, .. } => self.expr(a, *e, false),
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.stmt(a, *body);
                for c in catches.iter() {
                    self.stmt(a, c.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(a, *f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(a, *body),
            StmtKind::Assert { condition, message } => {
                self.expr(a, *condition, false);
                if let Some(m) = message {
                    self.expr(a, *m, false);
                }
            }
            _ => {}
        }
    }

    // -- Patterns -----------------------------------------------------------------

    /// `extensionTypeErasure`.
    fn erase_extension(&mut self, t: TypeId) -> TypeId {
        let outline = self.m.outline;
        let program = self.m.program;
        let rep = |decl: ClassId,
                   args: &[TypeId],
                   table: &mut crate::table::TypeTable|
         -> Option<TypeId> {
            let v = program.class(decl).representation?;
            let t = outline.variables[v.0 as usize].declared_type?;
            let params = &outline.classes[decl.0 as usize].type_params;
            if params.is_empty() || params.len() != args.len() {
                return Some(t);
            }
            let map: HashMap<crate::table::TypeParamId, TypeId> =
                params.iter().copied().zip(args.iter().copied()).collect();
            Some(crate::ops::substitute(t, &map, table))
        };
        crate::ops::erase_extension_type(t, self.m.table, &rep)
    }

    /// `_canBeEqual(constantType, valueType)`
    /// (an611:src/dart/constant/constant_verifier.dart:517-546). `Null` is an
    /// `InterfaceType` in the analyzer (`NullTypeImpl`).
    fn can_be_equal(&mut self, c: TypeId, v: TypeId, prof: u32) -> bool {
        if prof > 16 {
            return true;
        }
        let interface = |t: &Type| matches!(t, Type::Interface { .. } | Type::Null);
        let tc = self.m.table.get(c).clone();
        if !interface(&tc) {
            return true;
        }
        let tv = self.m.table.get(v).clone();
        match tv {
            Type::Interface { .. } | Type::Null => {
                let c_int = matches!(tc, Type::Interface { class, .. } if Some(class) == self.m.core.int_class);
                let v_double = matches!(tv, Type::Interface { class, .. } if Some(class) == self.m.core.double_class);
                if c_int && v_double {
                    return true;
                }
                let greater = self.greatest_closure_of(v, true, &[]);
                let mut env = crate::subtyping::SubtypeEnv::new(
                    &mut *self.m.table,
                    &self.m.outline.hierarchy,
                    self.m.core,
                );
                crate::subtyping::is_subtype(c, greater, &mut env)
            }
            Type::TypeParameter { param, nullable } => {
                let data = self.m.table.param(param).clone();
                if !data.explicit || parameter_reference(self.m.table, data.bound) {
                    return true;
                }
                let b = if nullable {
                    crate::ops::nullable(data.bound, self.m.table)
                } else {
                    data.bound
                };
                self.can_be_equal(c, b, prof + 1)
            }
            // `promotedBound`: the promoted bound (the nullability is the
            // parameter's, without `?`).
            Type::Intersection { bound, .. } => {
                if parameter_reference(self.m.table, bound) {
                    return true;
                }
                self.can_be_equal(c, bound, prof + 1)
            }
            Type::Function { .. } => {
                if matches!(tc, Type::Null) {
                    self.m.table.get(v).is_declared_nullable()
                } else {
                    false
                }
            }
            _ => true,
        }
    }

    /// `PatternGreatestClosureHelper.eliminateToGreatest`: each type parameter
    /// becomes `Object?` in covariant positions and `Never` in contravariant ones
    /// (with the nullability joined); the formals of the function type itself
    /// stay.
    fn greatest_closure_of(
        &mut self,
        t: TypeId,
        co: bool,
        formals: &[crate::table::TypeParamId],
    ) -> TypeId {
        let the_table = &mut *self.m.table;
        match the_table.get(t).clone() {
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. }
                if !formals.contains(&param) =>
            {
                let is_nullable =
                    matches!(the_table.get(t), Type::TypeParameter { nullable: true, .. });
                if co {
                    self.m.core.object_nullable
                } else if is_nullable {
                    self.m.core.null
                } else {
                    self.m.core.never
                }
            }
            Type::Interface {
                class,
                args,
                nullable,
            } if !args.is_empty() => {
                let new_: Vec<TypeId> = args
                    .iter()
                    .map(|&x| self.greatest_closure_of(x, co, formals))
                    .collect();
                self.m.table.intern(Type::Interface {
                    class,
                    args: new_.into_boxed_slice(),
                    nullable,
                })
            }
            Type::ExtensionType {
                decl,
                args,
                nullable,
            } if !args.is_empty() => {
                let new_: Vec<TypeId> = args
                    .iter()
                    .map(|&x| self.greatest_closure_of(x, co, formals))
                    .collect();
                self.m.table.intern(Type::ExtensionType {
                    decl,
                    args: new_.into_boxed_slice(),
                    nullable,
                })
            }
            Type::FutureOr { arg, nullable } => {
                let arg = self.greatest_closure_of(arg, co, formals);
                self.m.table.intern(Type::FutureOr { arg, nullable })
            }
            Type::Record {
                positional,
                named,
                nullable,
            } => {
                let pos: Vec<TypeId> = positional
                    .iter()
                    .map(|&x| self.greatest_closure_of(x, co, formals))
                    .collect();
                let nm: Vec<(dartforge_intern::SymbolId, TypeId)> = named
                    .iter()
                    .map(|&(n, x)| (n, self.greatest_closure_of(x, co, formals)))
                    .collect();
                self.m.table.intern(Type::Record {
                    positional: pos.into_boxed_slice(),
                    named: nm.into_boxed_slice(),
                    nullable,
                })
            }
            Type::Function {
                type_params,
                ret,
                positional,
                optional,
                named,
                nullable,
            } => {
                let mut inside: Vec<crate::table::TypeParamId> = formals.to_vec();
                inside.extend(type_params.iter().copied());
                let ret = self.greatest_closure_of(ret, co, &inside);
                let pos: Vec<TypeId> = positional
                    .iter()
                    .map(|&x| self.greatest_closure_of(x, !co, &inside))
                    .collect();
                let opt: Vec<TypeId> = optional
                    .iter()
                    .map(|&x| self.greatest_closure_of(x, !co, &inside))
                    .collect();
                let nm: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named
                    .iter()
                    .map(|&(n, x, r)| (n, self.greatest_closure_of(x, !co, &inside), r))
                    .collect();
                self.m.table.intern(Type::Function {
                    type_params,
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: nm.into_boxed_slice(),
                    nullable,
                })
            }
            _ => t,
        }
    }

    fn pattern(&mut self, a: &ast::Ast, p: PatternId) {
        match &a.pattern(p).kind {
            PatternKind::Constant(e) => {
                // `const (List<T>)`: the type literal right inside the
                // parentheses of the constant pattern
                // (`isTypeLiteralInConstantPattern`, 3.6.2
                // `evaluation.dart:1066-1072`, `:3197-3201`) that mentions a
                // type parameter is `CONST_TYPE_PARAMETER` at the type.
                if let ExprKind::Parenthesized(x) = a.expr(*e).kind
                    && let ExprKind::TypeArguments { target, type_args } = &a.expr(x).kind
                    && matches!(
                        self.m.body.units[self.unit.0 as usize].get_resolved(*target),
                        Some(crate::resolved::Resolved::Element(
                            dartforge_elements::model::Element::Class(_)
                                | dartforge_elements::model::Element::Typedef(_)
                        ))
                    )
                    && type_args.iter().any(|&t| {
                        self.annotation_type(t)
                            .is_some_and(|r| self.m.mentions_parameter(r))
                    })
                {
                    self.report(c::CONST_TYPE_PARAMETER, a.expr(x).span, Vec::new());
                    return;
                }
                let e = unparenthesize(a, *e);
                // `expression.typeOrThrow is InvalidType` (3.6.2
                // `constant_verifier.dart:133-135`).
                if self.m.invalid_type(self.unit, e) {
                    return;
                }
                let r = self.evaluate_and_report(
                    e,
                    false,
                    c::CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION,
                );
                if let Constant::Value(v) = r {
                    // `CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE`
                    // (an611:src/dart/constant/constant_verifier.dart:141-158):
                    // a primitive-equality value whose type cannot be equal to
                    // a value of the matched type (without extension
                    // types).
                    if self.patterns_enabled
                        && self.m.primitive_equality(&v, self.lib)
                        && let Some(&matched) = self.m.body.units[self.unit.0 as usize]
                            .matched_types
                            .get(&p)
                    {
                        let matched = self.erase_extension(matched);
                        if !self.can_be_equal(v.ty, matched, 0) {
                            let displayer = crate::display::Displayer {
                                table: &*self.m.table,
                                interner: self.m.interner,
                                program: self.m.program,
                            };
                            let (texts, context) = displayer.arguments_and_context(&[
                                crate::display::Arg::Type_(matched),
                                crate::display::Arg::Type_(v.ty),
                            ]);
                            let mut d = Diagnostic::with_code(dartforge_diagnostics::codes::warning::CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE, a.pattern(p).span, texts);
                            d.context.extend(context);
                            self.output.push((self.unit, d));
                            self.the_default_values.insert(p, v);
                            return;
                        }
                    }
                    self.the_default_values.insert(p, v);
                    self.expr(a, e, false);
                }
            }
            // `case the_name:` (the parser stores it as a variable): the top-level
            // constant with that name, for exhaustiveness.
            PatternKind::Variable {
                final_: false,
                var_: false,
                ty: None,
                name,
            } if self.patterns_enabled => {
                if self.m.body.units[self.unit.0 as usize]
                    .non_constant_name_patterns
                    .contains(&p)
                {
                    self.report(
                        c::CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION,
                        name.span,
                        Vec::new(),
                    );
                    return;
                }
                let program = self.m.program;
                if let Some(dartforge_elements::model::Element::Variable(v)) = program
                    .lookup_in_unit(self.unit, name.sym)
                    .and_then(|b| b.getter)
                    && let Some(Constant::Value(val)) = self.m.variable_value(v)
                {
                    self.the_default_values.insert(p, val);
                }
            }
            PatternKind::Relational { value, .. } => {
                self.expr(a, *value, false);
                self.evaluate_and_report(
                    *value,
                    false,
                    c::NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION,
                );
            }
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.pattern(a, *x);
                self.pattern(a, *y);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x) => self.pattern(a, *x),
            PatternKind::Cast { pattern, .. } => self.pattern(a, *pattern),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x)
                        | ast::ListPatternElement::Rest(Some(x)) => self.pattern(a, *x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                // `visitMapPattern` (`constant_verifier.dart:311-358`): each key
                // evaluated; one equal (identical, or `==` with primitive equality)
                // to a previous one is `EQUAL_KEYS_IN_MAP_PATTERN`, at the repeated
                // key, after all of them. A record's `isIdentical` is never true
                // (`RecordState.isIdentical`: false or unknown), so two records are
                // only equal with primitive equality on both: a field with its own
                // `==` removes the repetition.
                let mut unique: Vec<(Value, Span)> = Vec::new();
                let mut the_repeated: Vec<(Span, Span)> = Vec::new();
                for en in entries.iter() {
                    self.pattern(a, en.value);
                    if let Constant::Value(v) =
                        self.evaluate_and_report(en.key, false, c::NON_CONSTANT_MAP_PATTERN_KEY)
                    {
                        let known = !(v.actually_unknown()
                            || matches!(v.state, InstanceState::Null { invalid: true }));
                        if known {
                            let span = a.expr(en.key).span;
                            let lib = self.lib;
                            let record =
                                |x: &Value| matches!(x.state, InstanceState::Record { .. });
                            let primitive_v = !record(&v) || self.m.primitive_equality(&v, lib);
                            let m = &mut *self.m;
                            let the_found = unique.iter().find(|(x, _)| {
                                (!record(x) || (primitive_v && m.primitive_equality(x, lib)))
                                    && m.equal(x, &v)
                            });
                            match the_found {
                                Some(&(_, original)) => the_repeated.push((span, original)),
                                None => unique.push((v.clone(), span)),
                            }
                        }
                        self.key_values.insert(en.key, v);
                    }
                }
                // `equalKeysInMapPattern`: the context is the first key.
                for (sp, original) in the_repeated {
                    let d = Diagnostic::with_code(
                        c::EQUAL_KEYS_IN_MAP_PATTERN,
                        sp,
                        Vec::<String>::new(),
                    )
                    .with_context(original, "The first key with this value.");
                    self.output.push((self.unit, d));
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.pattern(a, f.pattern);
                }
            }
            _ => {}
        }
    }

    // -- Expressions --------------------------------------------------------------

    fn expr(&mut self, a: &ast::Ast, e: ExprId, in_const: bool) {
        let u = self.unit;
        match &a.expr(e).kind {
            ExprKind::InstanceCreation {
                keyword,
                arguments,
                ty,
                constructor,
            } => {
                let const_ = matches!(keyword, Some(ast::CreationKeyword::Const));
                // `_checkForConstWithNonConst` with the forwarded constructor of a
                // mixin application (inference does not resolve it).
                if const_ && !matches!(self.m.resolved(u, e), Some(Resolved::Constructor(_))) {
                    self.non_const_forwarded(*ty, *constructor, a.expr(e).span);
                }
                // `visitInstanceCreationExpression` (`isConst`: `const`, or
                // without a keyword in a constant context).
                if const_ || (keyword.is_none() && in_const) {
                    self.without_type_parameters(a, *ty, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                }
                if const_ {
                    self.constant_creation(a, e, arguments);
                } else {
                    for x in arguments.args.iter() {
                        self.expr(a, x.value, in_const);
                    }
                }
            }
            ExprKind::Call { target, arguments } => {
                if in_const && matches!(self.m.resolved(u, e), Some(Resolved::Constructor(_))) {
                    // The `NamedType` of the creation without `new`: the call's type
                    // arguments or those of the `C<T>` before `.the_name`.
                    let mut written_ones: Vec<ast::TypeId> = arguments.type_args.to_vec();
                    if let ExprKind::Property { target: r, .. } = &a.expr(*target).kind
                        && let ExprKind::TypeArguments { type_args, .. } = &a.expr(*r).kind
                    {
                        written_ones.extend(type_args.iter().copied());
                    }
                    for x in written_ones {
                        self.without_type_parameters(a, x, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                    }
                    self.constant_creation(a, e, arguments);
                    return;
                }
                self.expr(a, *target, in_const);
                for x in arguments.args.iter() {
                    self.expr(a, x.value, in_const);
                }
            }
            ExprKind::List {
                const_,
                elements,
                type_args,
            } => {
                let c = *const_ || in_const;
                for el in elements.iter() {
                    self.child_element(a, el, c);
                }
                if c {
                    for &x in type_args.iter() {
                        self.const_type_argument(a, x, c::INVALID_TYPE_ARGUMENT_IN_CONST_LIST);
                    }
                }
                if c {
                    let typ = self.m.the_static(u, e);
                    let element = match self.m.table.get(typ) {
                        Type::Interface { args, .. } if args.len() == 1 => args[0],
                        _ => self.m.core.dynamic_,
                    };
                    let mut lv = Literal {
                        ty: LiteralType::List(element),
                        the_unique: Vec::new(),
                        duplicates: Vec::new(),
                    };
                    for el in elements.iter() {
                        self.verify_element(a, &mut lv, el);
                    }
                }
            }
            ExprKind::SetOrMap {
                const_,
                elements,
                type_args,
            } => {
                let c = *const_ || in_const;
                for el in elements.iter() {
                    self.child_element(a, el, c);
                }
                if c {
                    let typ = self.m.the_static(u, e);
                    let (cls, args) = match self.m.table.get(typ) {
                        Type::Interface { class, args, .. } => (Some(*class), args.to_vec()),
                        _ => (None, Vec::new()),
                    };
                    // `TypeArgumentsVerifier.checkMapLiteral`/`checkSetLiteral`:
                    // only with the shape decided.
                    let code = if cls.is_some() && cls == self.m.core.map_class {
                        Some(c::INVALID_TYPE_ARGUMENT_IN_CONST_MAP)
                    } else if cls.is_some() && cls == self.m.core.set_class {
                        Some(c::INVALID_TYPE_ARGUMENT_IN_CONST_SET)
                    } else {
                        None
                    };
                    if let Some(code) = code {
                        for &x in type_args.iter() {
                            self.const_type_argument(a, x, code);
                        }
                    }
                    let tl = if cls.is_some() && cls == self.m.core.set_class && args.len() == 1 {
                        Some(LiteralType::Set(args[0]))
                    } else if cls.is_some() && cls == self.m.core.map_class && args.len() == 2 {
                        Some(LiteralType::Map(args[0], args[1]))
                    } else {
                        None
                    };
                    if let Some(tl) = tl {
                        let mut lv = Literal {
                            ty: tl,
                            the_unique: Vec::new(),
                            duplicates: Vec::new(),
                        };
                        for el in elements.iter() {
                            self.verify_element(a, &mut lv, el);
                        }
                        // `equalKeysInConstMap`/`equalElementsInConstSet`: the context
                        // is the first one with the same value.
                        let (code, context) = if matches!(lv.ty, LiteralType::Map(..)) {
                            (c::EQUAL_KEYS_IN_CONST_MAP, "The first key with this value.")
                        } else {
                            (
                                c::EQUAL_ELEMENTS_IN_CONST_SET,
                                "The first element with this value.",
                            )
                        };
                        for (dup, original) in std::mem::take(&mut lv.duplicates) {
                            let d = Diagnostic::with_code(code, dup, Vec::<String>::new())
                                .with_context(original, context);
                            self.output.push((self.unit, d));
                        }
                    }
                } else {
                    self.duplicates_in_non_constant_literal(a, e, elements);
                }
            }
            ExprKind::Record {
                const_,
                positional,
                named,
            } => {
                let c = *const_ || in_const;
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(a, *x, c);
                }
                if *const_ {
                    for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                        self.evaluate_and_report(*x, true, c::NON_CONSTANT_RECORD_FIELD);
                    }
                }
            }
            ExprKind::FunctionExpression(f) => self.function(a, *f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(a, *x, in_const);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => {
                self.expr(a, *x, in_const)
            }
            ExprKind::Property { target, name, .. } => {
                // `visitConstructorReference`: `C<T>.the_name` of a constructor.
                if (in_const || self.in_constant_expression)
                    && let ExprKind::TypeArguments {
                        target: base,
                        type_args,
                    } = &a.expr(*target).kind
                    && self.referenced_constructor(a, *base, *name)
                {
                    for &x in type_args.iter() {
                        self.without_type_parameters(
                            a,
                            x,
                            c::CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF,
                            &[],
                        );
                    }
                    self.expr(a, *base, in_const);
                    return;
                }
                self.expr(a, *target, in_const)
            }
            ExprKind::Index { target, index, .. } => {
                self.expr(a, *target, in_const);
                self.expr(a, *index, in_const);
            }
            ExprKind::TypeArguments { target, type_args } => {
                // `visitFunctionReference`: the instantiation of a function (not
                // the type literal, of type `Type`).
                if (in_const || self.in_constant_expression)
                    && self.m.the_static(u, e) != self.m.core.type_
                {
                    for &x in type_args.iter() {
                        self.without_type_parameters(
                            a,
                            x,
                            c::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF,
                            &[],
                        );
                    }
                }
                self.expr(a, *target, in_const)
            }
            ExprKind::Unary { operand, .. } => self.expr(a, *operand, in_const),
            ExprKind::Binary { left, right, .. } => {
                self.expr(a, *left, in_const);
                self.expr(a, *right, in_const);
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                self.expr(a, *condition, in_const);
                self.expr(a, *then, in_const);
                self.expr(a, *else_, in_const);
            }
            ExprKind::Is { value, ty, .. } | ExprKind::As { value, ty } => {
                // `visitGenericFunctionType` under `is`/`as` in a constant
                // context.
                if in_const && matches!(a.ty(*ty).kind, ast::TypeKind::Function { .. }) {
                    self.without_type_parameters(a, *ty, c::CONST_WITH_TYPE_PARAMETERS, &[]);
                }
                self.expr(a, *value, in_const)
            }
            ExprKind::Assign { target, value, .. } => {
                self.expr(a, *target, in_const);
                self.expr(a, *value, in_const);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.pattern(a, *pattern);
                self.expr(a, *value, in_const);
            }
            ExprKind::Cascade {
                target, sections, ..
            } => {
                self.expr(a, *target, in_const);
                for s in sections.iter() {
                    self.expr(a, *s, in_const);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(a, *value, in_const);
                for case in cases.iter() {
                    self.pattern(a, case.pattern);
                    if let Some(g) = case.guard {
                        self.expr(a, g, false);
                    }
                    self.expr(a, case.body, in_const);
                }
                let the_cases: Vec<Case> = cases
                    .iter()
                    .map(|k| Case {
                        pattern: Some(k.pattern),
                        stored: k.guard.is_some(),
                    })
                    .collect();
                let start = a.expr(e).span.start;
                let pos: Vec<usize> = cases.iter().map(|k| self.case_arrow(a, k)).collect();
                self.exhaustiveness(a, *value, start, &the_cases, &pos, true);
            }
            _ => {}
        }
    }

    // -- Exaustividade (`_validateSwitchExhaustiveness`, `:883-1000`) ----------

    /// Reports `UNREACHABLE_SWITCH_CASE` (at the `case` keyword or at the
    /// arrow), `NON_EXHAUSTIVE_SWITCH_*` (at the `switch` keyword) and
    /// `UNREACHABLE_SWITCH_DEFAULT` (at the `default` keyword). `pos` is the
    /// start of each case's token.
    fn exhaustiveness(
        &mut self,
        a: &ast::Ast,
        the_value: ExprId,
        switch_start: usize,
        the_cases: &[Case],
        pos: &[usize],
        expression: bool,
    ) {
        let u = self.unit;
        let body = self.m.body;
        let the_body = &body.units[u.0 as usize];
        if the_body.invalid_types.contains(&the_value) {
            return;
        }
        let Some(t) = the_body.get_type(the_value) else {
            return;
        };
        let program = self.m.program;
        let source = program.unit(u).source.as_str();
        let version_3_3 = program.library(self.lib).features.version()
            >= dartforge_frontend::features::LanguageVersion::new(3, 3);
        let the_default_values = std::mem::take(&mut self.the_default_values);
        let key_values = std::mem::take(&mut self.key_values);
        let entry = Entry {
            ast: a,
            source,
            pattern_types: &the_body.pattern_types,
            extension_fields: &the_body.extension_fields,
            invalid_patterns: &the_body.invalid_patterns,
            the_default_values: &the_default_values,
            key_values: &key_values,
            version_3_3,
        };
        let collect = self.m.witnesses.is_some();
        let mut ex = Exhaustiveness::new(self.m, self.lib);
        ex.collect_parts = collect;
        let must = expression || ex.always_exhaustive(t, 0);
        let r = ex.verify(&entry, t, the_cases);
        drop(ex);
        self.the_default_values = the_default_values;
        self.key_values = key_values;
        let Some(r) = r else { return };
        let mut parts = r.parts;
        for i in r.unreachable {
            let n = if expression { 2 } else { 4 };
            self.report(
                w::UNREACHABLE_SWITCH_CASE,
                Span {
                    start: pos[i],
                    end: pos[i] + n,
                },
                Vec::new(),
            );
        }
        let default = the_cases.iter().position(|k| k.pattern.is_none());
        match (r.witness, r.fix) {
            (Some(tw), Some(co)) => {
                if must && default.is_none() {
                    let code = if expression {
                        c::NON_EXHAUSTIVE_SWITCH_EXPRESSION
                    } else {
                        c::NON_EXHAUSTIVE_SWITCH_STATEMENT
                    };
                    let typ = self.m.format(t);
                    // The fourth argument only exists for the "private" variant of
                    // 3.13.4 (`constant_verifier.dart` from main, case c29 of
                    // `corpus/specification/t2/v`): the value is from an enum of
                    // another library and what is missing is private. The original
                    // looks at all the witnesses; here, the first one.
                    let private = match self.m.table.get(t) {
                        Type::Interface { class, .. } => {
                            let k = self.m.program.class(*class);
                            k.kind == dartforge_elements::model::ClassKind::Enum
                                && k.library != self.lib
                                && (self.m.interner.resolve(k.name).starts_with('_')
                                    || tw.contains("._"))
                        }
                        _ => false,
                    };
                    let mut args = vec![typ, tw, co];
                    if private {
                        args.push(String::new());
                    }
                    self.report(
                        code,
                        Span {
                            start: switch_start,
                            end: switch_start + 6,
                        },
                        args,
                    );
                    let u = self.unit;
                    if let Some(map) = self.m.witnesses.as_mut() {
                        map.insert((u, switch_start), std::mem::take(&mut parts));
                    }
                }
            }
            _ => {
                if let Some(d) = default
                    && must
                {
                    self.report(
                        w::UNREACHABLE_SWITCH_DEFAULT,
                        Span {
                            start: pos[d],
                            end: pos[d] + 7,
                        },
                        Vec::new(),
                    );
                }
            }
        }
    }

    /// The start of the `case`/`default` keyword of a `switch` case (after the
    /// labels).
    fn case_keyword_start(&self, k: &ast::SwitchCase) -> usize {
        let source = self.m.program.unit(self.unit).source.as_bytes();
        let mut i = k.span.start;
        if let Some(l) = k.labels.last() {
            i = skip_blanks(source, l.span.end);
            if source.get(i) == Some(&b':') {
                i += 1;
            }
            i = skip_blanks(source, i);
        }
        i
    }

    /// The start of the `=>` arrow of a `switch` expression case.
    fn case_arrow(&self, a: &ast::Ast, k: &ast::SwitchExprCase) -> usize {
        let source = self.m.program.unit(self.unit).source.as_bytes();
        let end = match k.guard {
            Some(g) => a.expr(g).span.end,
            None => a.pattern(k.pattern).span.end,
        };
        let bnd = a.expr(k.body).span.start;
        let mut i = end;
        while i + 1 < source.len() && i < bnd && !(source[i] == b'=' && source[i + 1] == b'>') {
            let j = skip_blanks(source, i);
            i = if j > i { j } else { i + 1 };
        }
        i
    }

    fn child_element(&mut self, a: &ast::Ast, el: &CollectionElement, in_const: bool) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                self.expr(a, *x, in_const)
            }
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(a, *key, in_const);
                self.expr(a, *value, in_const);
            }
            CollectionElement::Spread { value, .. } => self.expr(a, *value, in_const),
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(a, *condition, in_const);
                if let Some(p) = case_pattern {
                    self.pattern(a, *p);
                }
                if let Some(g) = guard {
                    self.expr(a, *g, in_const);
                }
                self.child_element(a, then, in_const);
                if let Some(x) = else_ {
                    self.child_element(a, x, in_const);
                }
            }
            CollectionElement::For {
                condition,
                updates,
                body,
                ..
            } => {
                if let Some(c) = condition {
                    self.expr(a, *c, in_const);
                }
                for u in updates.iter() {
                    self.expr(a, *u, in_const);
                }
                self.child_element(a, body, in_const);
            }
            CollectionElement::ForIn { iterable, body, .. } => {
                self.expr(a, *iterable, in_const);
                self.child_element(a, body, in_const);
            }
        }
    }

    /// `visitInstanceCreationExpression` of a constant creation: evaluates and
    /// reports (with the real reporter: unresolved arguments too); with a value,
    /// continues into the arguments.
    fn constant_creation(&mut self, a: &ast::Ast, e: ExprId, arguments: &ast::Arguments) {
        if !matches!(
            self.m.resolved(self.unit, e),
            Some(Resolved::Constructor(_))
        ) {
            return;
        }
        let cx = Ctx {
            report: true,
            ..self.cx()
        };
        let before = self.m.reports.len();
        let r = self.m.evaluate(&cx, e, true);
        let reports: Vec<Invalid> = self.m.reports.drain(before..).collect();
        for i in reports {
            if i.unit == self.unit && !i.avoid_report {
                self.report_with_context(&i);
            }
        }
        match r {
            Constant::Invalid(i) => {
                if !i.avoid_report && i.unit == self.unit {
                    self.report_with_context(&i);
                }
            }
            Constant::Value(_) => {
                for x in arguments.args.iter() {
                    self.expr(a, x.value, true);
                }
            }
        }
    }

    // -- Constant literals (`_ConstLiteralVerifier`) ----------------------------

    fn literal_code(t: &LiteralType) -> Code {
        match t {
            LiteralType::List(_) => c::NON_CONSTANT_LIST_ELEMENT,
            LiteralType::Set(_) => c::NON_CONSTANT_SET_ELEMENT,
            LiteralType::Map(..) => c::NON_CONSTANT_MAP_ELEMENT,
        }
    }

    fn verify_element(&mut self, a: &ast::Ast, lv: &mut Literal, el: &CollectionElement) -> bool {
        let u = self.unit;
        let code = Self::literal_code(&lv.ty);
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => {
                let Constant::Value(v) = self.evaluate_and_report(*x, true, code) else {
                    return false;
                };
                let span = self.m.span(u, *x);
                match lv.ty {
                    LiteralType::List(t) => {
                        if !self.m.matches_type(&v, t) {
                            let args = vec![self.m.format(v.ty), self.m.format(t)];
                            self.report(c::LIST_ELEMENT_TYPE_NOT_ASSIGNABLE, span, args);
                            return false;
                        }
                        true
                    }
                    LiteralType::Set(t) => {
                        if !self.m.matches_type(&v, t) {
                            let args = vec![self.m.format(v.ty), self.m.format(t)];
                            self.report(c::SET_ELEMENT_TYPE_NOT_ASSIGNABLE, span, args);
                            return false;
                        }
                        if !self.m.primitive_equality(&v, self.lib) {
                            let args = vec![self.m.format(v.ty)];
                            self.report(c::CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY, span, args);
                            return false;
                        }
                        self.record_unique(lv, v, span);
                        true
                    }
                    LiteralType::Map(..) => true,
                }
            }
            CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                let s = self.m.element_span(u, el);
                self.report(c::CONST_EVAL_FOR_ELEMENT, s, Vec::new());
                false
            }
            CollectionElement::If {
                condition,
                then,
                else_,
                case_pattern,
                ..
            } => {
                if case_pattern.is_some() {
                    return false;
                }
                let Constant::Value(cond) = self.evaluate_and_report(*condition, true, code) else {
                    return false;
                };
                if !cond.state.is_bool() {
                    return false;
                }
                match cond.as_bool() {
                    None => {
                        let a1 = self.potential_in_literal(a, lv, then);
                        let a2 = else_
                            .as_ref()
                            .is_none_or(|x| self.potential_in_literal(a, lv, x));
                        a1 && a2
                    }
                    Some(true) => {
                        let a1 = self.verify_element(a, lv, then);
                        let a2 = else_
                            .as_ref()
                            .is_none_or(|x| self.potential_in_literal(a, lv, x));
                        a1 && a2
                    }
                    Some(false) => {
                        let a1 = self.potential_in_literal(a, lv, then);
                        let a2 = else_.as_ref().is_none_or(|x| self.verify_element(a, lv, x));
                        a1 && a2
                    }
                }
            }
            CollectionElement::MapEntry { key, value, .. } => {
                let LiteralType::Map(tk, tv) = lv.ty else {
                    return false;
                };
                let k = self.evaluate_and_report(*key, true, c::NON_CONSTANT_MAP_KEY);
                let v = self.evaluate_and_report(*value, true, c::NON_CONSTANT_MAP_VALUE);
                if let Constant::Value(k) = k {
                    let span = self.m.span(u, *key);
                    if !self.m.matches_type(&k, tk) {
                        let args = vec![self.m.format(k.ty), self.m.format(tk)];
                        self.report(c::MAP_KEY_TYPE_NOT_ASSIGNABLE, span, args);
                    }
                    if !self.m.primitive_equality(&k, self.lib) {
                        let args = vec![self.m.format(k.ty)];
                        self.report(c::CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY, span, args);
                    }
                    self.record_unique(lv, k, span);
                }
                if let Constant::Value(v) = v
                    && !self.m.matches_type(&v, tv)
                {
                    let span = self.m.span(u, *value);
                    let args = vec![self.m.format(v.ty), self.m.format(tv)];
                    self.report(c::MAP_VALUE_TYPE_NOT_ASSIGNABLE, span, args);
                }
                true
            }
            CollectionElement::Spread { value, null_aware } => {
                let Constant::Value(v) = self.evaluate_and_report(*value, true, code) else {
                    return false;
                };
                let span = self.m.span(u, *value);
                match lv.ty {
                    LiteralType::List(_) | LiteralType::Set(_) => {
                        let list = v.as_list().cloned();
                        let set = v.as_set().cloned();
                        let Some(items) = list.clone().or(set) else {
                            if v.state.is_null() && *null_aware {
                                return true;
                            }
                            self.report(c::CONST_SPREAD_EXPECTED_LIST_OR_SET, span, Vec::new());
                            return false;
                        };
                        if matches!(lv.ty, LiteralType::List(_)) {
                            return true;
                        }
                        if list.is_some()
                            && !items.iter().all(|x| self.m.primitive_equality(x, self.lib))
                        {
                            let s = self.m.element_span(u, el);
                            let args = vec![self.m.format(v.ty)];
                            self.report(c::CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY, s, args);
                            return false;
                        }
                        for x in items.iter() {
                            self.record_unique(lv, x.clone(), span);
                        }
                        true
                    }
                    LiteralType::Map(..) => {
                        if v.state.is_null() && *null_aware {
                            return true;
                        }
                        match v.as_map().cloned() {
                            Some(m) => {
                                for (k, _) in m.iter() {
                                    self.record_unique(lv, k.clone(), span);
                                }
                                true
                            }
                            None => {
                                self.report(c::CONST_SPREAD_EXPECTED_MAP, span, Vec::new());
                                false
                            }
                        }
                    }
                }
            }
        }
    }

    /// The resolved type of a type annotation of the unit.
    /// `_checkForConstWithTypeParameters`
    /// (an611:src/dart/constant/constant_verifier.dart:554-605): a `NamedType`
    /// that names an outer type parameter (at the node, without descending), and
    /// its arguments; in a function type, its own formals are allowed, and the
    /// bounds, the return and the simple parameters are checked.
    fn without_type_parameters(
        &mut self,
        a: &ast::Ast,
        x: ast::TypeId,
        code: Code,
        allowed: &[crate::table::TypeParamId],
    ) {
        let annotation = a.ty(x);
        match &annotation.kind {
            ast::TypeKind::Named { args, .. } => {
                let parameter = self.annotation_type(x).and_then(|r| {
                    if matches!(
                        self.m.table.decoration(r),
                        Some(crate::table::DisplayDecoration::Alias { .. })
                    ) {
                        return None;
                    }
                    match self.m.table.get(r) {
                        Type::TypeParameter { param, .. } => Some(*param),
                        _ => None,
                    }
                });
                if let Some(p) = parameter
                    && !allowed.contains(&p)
                {
                    self.report(code, annotation.span, Vec::new());
                    return;
                }
                for &y in args.iter() {
                    self.without_type_parameters(a, y, code, allowed);
                }
            }
            ast::TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let mut inside: Vec<crate::table::TypeParamId> = allowed.to_vec();
                if let Some(r) = self.annotation_type(x)
                    && let Type::Function {
                        type_params: formals,
                        ..
                    } = self.m.table.get(r)
                {
                    inside.extend(formals.iter().copied());
                }
                for tp in type_params.iter() {
                    if let Some(b) = tp.bound {
                        self.without_type_parameters(a, b, code, &inside);
                    }
                }
                if let Some(r) = return_type {
                    self.without_type_parameters(a, *r, code, &inside);
                }
                for p in parameters.iter() {
                    if p.function_parameters.is_none()
                        && let Some(y) = p.ty
                    {
                        self.without_type_parameters(a, y, code, &inside);
                    }
                }
            }
            ast::TypeKind::Record { .. } | ast::TypeKind::Void => {}
        }
    }

    /// `base.the_name` designates a constructor (`C.new`, `C.the_name`, `p.C.the_name`, also
    /// through an alias).
    fn referenced_constructor(&self, a: &ast::Ast, base: ExprId, the_name: ast::Name) -> bool {
        let program = self.m.program;
        let u = self.unit;
        let el = match &a.expr(base).kind {
            ExprKind::Identifier(n) => program.lookup_in_unit(u, n.sym).and_then(|b| b.getter),
            ExprKind::Property {
                target: p, name: n, ..
            } => match &a.expr(*p).kind {
                ExprKind::Identifier(p) => program
                    .lookup_prefixed_in_unit(u, p.sym, n.sym)
                    .and_then(|b| b.getter),
                _ => None,
            },
            _ => None,
        };
        let cls = match el {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            Some(dartforge_elements::model::Element::Typedef(td)) => match self
                .m
                .table
                .get(self.m.outline.typedefs[td.0 as usize].target_type)
            {
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                    Some(*class)
                }
                _ => None,
            },
            _ => None,
        };
        let Some(c) = cls else { return false };
        let text = self.m.interner.resolve(the_name.sym);
        let the_key = if text == "new" {
            self.m.interner.lookup("")
        } else {
            Some(the_name.sym)
        };
        the_key.is_some_and(|k| program.class(c).constructors.contains_key(&k))
    }

    fn annotation_type(&self, x: ast::TypeId) -> Option<TypeId> {
        let u = self.unit;
        self.m
            .body
            .units
            .get(u.0 as usize)
            .and_then(|b| b.annotation_types.get(&x).copied())
            .or_else(|| self.m.outline.written_types.get(&(u, x)).copied())
    }

    /// `_checkTypeArgumentConst` (`type_arguments_verifier.dart:501-544`): a type
    /// argument of a constant literal that is a type parameter, at the type's
    /// node; for a named type with the lexeme, for function and record types
    /// with the displayed type. Descends into the arguments, the simple
    /// parameters and the return of a function type, and into the fields of a
    /// record.
    fn const_type_argument(&mut self, a: &ast::Ast, x: ast::TypeId, code: Code) {
        let is_parameter = |s: &Self, t: ast::TypeId| {
            s.annotation_type(t)
                .filter(|r| matches!(s.m.table.get(*r), Type::TypeParameter { .. }))
        };
        let annotation = a.ty(x);
        match &annotation.kind {
            ast::TypeKind::Named { name, args } => {
                if is_parameter(self, x).is_some() {
                    let lexeme = name
                        .last()
                        .map(|n| self.m.interner.resolve(n.sym).to_string())
                        .unwrap_or_default();
                    self.report(code, annotation.span, vec![lexeme]);
                } else {
                    for &y in args.iter() {
                        self.const_type_argument(a, y, code);
                    }
                }
            }
            ast::TypeKind::Function {
                return_type,
                parameters,
                ..
            } => {
                let mut children: Vec<ast::TypeId> = parameters
                    .iter()
                    .filter(|p| p.function_parameters.is_none())
                    .filter_map(|p| p.ty)
                    .collect();
                children.extend(return_type.iter().copied());
                for y in children {
                    self.const_type_field(a, y, code);
                }
            }
            ast::TypeKind::Record { positional, named } => {
                let children: Vec<ast::TypeId> = positional
                    .iter()
                    .copied()
                    .chain(named.iter().map(|(_, t)| *t))
                    .collect();
                for y in children {
                    self.const_type_field(a, y, code);
                }
            }
            ast::TypeKind::Void => {}
        }
    }

    /// A parameter, return or record field: a type parameter reports with the
    /// displayed type; otherwise it descends.
    fn const_type_field(&mut self, a: &ast::Ast, y: ast::TypeId, code: Code) {
        match self.annotation_type(y) {
            Some(r) if matches!(self.m.table.get(r), Type::TypeParameter { .. }) => {
                let displayed = self.m.format(r);
                self.report(code, a.ty(y).span, vec![displayed]);
            }
            _ => self.const_type_argument(a, y, code),
        }
    }

    /// `BestPracticesVerifier._checkForDuplications`
    /// (`best_practices_verifier.dart:851-875`): in a non-constant literal, the
    /// first-level elements (set) or keys (map) that constant evaluation gives
    /// without an error, equal to a previous one:
    /// `EQUAL_ELEMENTS_IN_SET`/`EQUAL_KEYS_IN_MAP` at the repeated one.
    fn duplicates_in_non_constant_literal(
        &mut self,
        _a: &ast::Ast,
        e: ExprId,
        elements: &[CollectionElement],
    ) {
        let u = self.unit;
        let typ = self.m.the_static(u, e);
        let cls = match self.m.table.get(typ) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let (map, code) = if cls.is_some() && cls == self.m.core.set_class {
            (false, w::EQUAL_ELEMENTS_IN_SET)
        } else if cls.is_some() && cls == self.m.core.map_class {
            (true, w::EQUAL_KEYS_IN_MAP)
        } else {
            return;
        };
        let expressions: Vec<ExprId> = elements
            .iter()
            .filter_map(|el| match el {
                CollectionElement::Expression(x) if !map => Some(*x),
                CollectionElement::MapEntry { key, .. } if map => Some(*key),
                _ => None,
            })
            .collect();
        let mut seen: Vec<Value> = Vec::new();
        for x in expressions {
            let cx = self.cx();
            let Constant::Value(v) = self.m.evaluate(&cx, x, false) else {
                continue;
            };
            if v.actually_unknown() {
                continue;
            }
            let repeated = seen.iter().any(|y| self.m.equal(y, &v));
            if repeated {
                let sp = self.m.span(u, x);
                self.report(code, sp, Vec::new());
            } else {
                seen.push(v);
            }
        }
    }

    fn record_unique(&mut self, lv: &mut Literal, v: Value, span: Span) {
        if v.actually_unknown() || matches!(v.state, InstanceState::Null { invalid: true }) {
            return;
        }
        for (x, s) in lv.the_unique.iter() {
            if self.m.equal(x, &v) {
                let original = *s;
                if !lv.duplicates.iter().any(|(d, _)| *d == span) {
                    lv.duplicates.push((span, original));
                }
                return;
            }
        }
        lv.the_unique.push((v, span));
    }

    /// `_reportNotPotentialConstants` of the literal verifier.
    fn potential_in_literal(&mut self, a: &ast::Ast, lv: &Literal, el: &CollectionElement) -> bool {
        let _ = a;
        let cx = self.cx();
        let mut nodes: Vec<(Span, Code)> = Vec::new();
        let base = Self::literal_code(&lv.ty);
        self.collect_element(&cx, el, base, &mut nodes);
        if nodes.is_empty() {
            return true;
        }
        for (s, code) in nodes {
            self.report(code, s, Vec::new());
        }
        false
    }

    fn collect_element(
        &mut self,
        cx: &Ctx,
        el: &CollectionElement,
        base: Code,
        nodes: &mut Vec<(Span, Code)>,
    ) {
        let map = base == c::NON_CONSTANT_MAP_ELEMENT;
        let one = |m: &Engine<'_>, x: ExprId, code: Code, nodes: &mut Vec<(Span, Code)>| {
            let mut v = Vec::new();
            super::potential::collect_into(m, cx, x, false, true, &mut v);
            nodes.extend(v.into_iter().map(|s| (s, code)));
        };
        match el {
            CollectionElement::Expression(x)
            | CollectionElement::NullAwareExpression(x)
            | CollectionElement::Spread { value: x, .. } => one(self.m, *x, base, nodes),
            CollectionElement::MapEntry { key, value, .. } => {
                let (ck, cv) = if map {
                    (c::NON_CONSTANT_MAP_KEY, c::NON_CONSTANT_MAP_VALUE)
                } else {
                    (base, base)
                };
                one(self.m, *key, ck, nodes);
                one(self.m, *value, cv, nodes);
            }
            CollectionElement::If {
                condition,
                then,
                else_,
                ..
            } => {
                one(self.m, *condition, base, nodes);
                self.collect_element(cx, then, base, nodes);
                if let Some(x) = else_ {
                    self.collect_element(cx, x, base, nodes);
                }
            }
            CollectionElement::For { .. } | CollectionElement::ForIn { .. } => {
                nodes.push((self.m.element_span(cx.unit, el), base));
            }
        }
    }
}

enum LiteralType {
    List(TypeId),
    Set(TypeId),
    Map(TypeId, TypeId),
}

struct Literal {
    ty: LiteralType,
    the_unique: Vec<(Value, Span)>,
    duplicates: Vec<(Span, Span)>,
}

/// Skips whitespace and comments starting at `i`.
fn skip_blanks(f: &[u8], mut i: usize) -> usize {
    loop {
        while i < f.len() && f[i].is_ascii_whitespace() {
            i += 1;
        }
        if f.get(i) == Some(&b'/') && f.get(i + 1) == Some(&b'/') {
            while i < f.len() && f[i] != b'\n' {
                i += 1;
            }
        } else if f.get(i) == Some(&b'/') && f.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < f.len() && !(f[i] == b'*' && f[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(f.len());
        } else {
            return i;
        }
    }
}

fn unparenthesize(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// `hasTypeParameterReference`.
fn parameter_reference(table: &crate::table::TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::TypeParameter { .. } | Type::Intersection { .. } => true,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().any(|&a| parameter_reference(table, a))
        }
        Type::FutureOr { arg, .. } => parameter_reference(table, *arg),
        Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } => {
            parameter_reference(table, *ret)
                || positional
                    .iter()
                    .chain(optional.iter())
                    .any(|&a| parameter_reference(table, a))
                || named.iter().any(|(_, a, _)| parameter_reference(table, *a))
        }
        Type::Record {
            positional, named, ..
        } => {
            positional.iter().any(|&a| parameter_reference(table, a))
                || named.iter().any(|(_, a)| parameter_reference(table, *a))
        }
        _ => false,
    }
}
