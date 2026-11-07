//! Semantic declaration and use analysis for static weak tear-off references.
use crate::const_eval::{
    evaluation::{Constant, Ctx, Engine},
    value::{Field, Function, InstanceState},
};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::{CoreTypes, Type, TypeTable};
use dartforge_elements::model::{
    Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId,
};
use dartforge_frontend::ast::{self, ExprId, ExprKind, ParameterKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// Frontend rules for the constant target of a static weak reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TargetPolicy {
    /// dart2js permits zero-argument, non-generic constructor wrappers.
    #[default]
    JavaScript,
    /// The VM frontend permits only top-level or static methods, including in AOT.
    Native,
}

/// Validated intrinsic identities, call targets and constant-local dependencies.
///
/// The identifiers belong to the supplied program. Reject `issues` before using
/// the call map to weaken reachability. An empty map preserves ordinary calls.
#[derive(Clone, Default, Debug)]
pub struct WeakReferences {
    /// Declarations whose resolved core pragma and signature designate an intrinsic.
    pub declarations: HashSet<FunctionElementId>,
    /// Direct intrinsic call expressions and their constant static targets.
    pub calls: HashMap<(UnitId, ExprId), FunctionElementId>,
    /// Proven constant local initializers with no ordinary runtime reads.
    pub elided_const_initializers: HashSet<(UnitId, ExprId)>,
    /// Unit, byte offset and diagnostic code for each invalid declaration or call.
    pub issues: Vec<(UnitId, usize, String)>,
}

/// Inspect weak references using dart2js target rules, without changing liveness.
///
/// All semantic inputs must describe the same program. Bodies must include local
/// declaration bindings, and `inferred` identifies libraries with available
/// initializer bodies. Calling an alias of an intrinsic remains an ordinary call.
/// Native callers select their frontend rules with [`normalize_for`].
///
/// ```
/// use dartforge_elements::model::Program;
/// use dartforge_intern::Interner;
/// use dartforge_types::{CoreTypes, TypeTable, resolve_outline};
/// use dartforge_types::resolved::BodyTypes;
/// use dartforge_types::static_weak_references::normalize;
/// let program = Program::default();
/// let mut names = Interner::new();
/// names.intern("_");
/// let mut table = TypeTable::new();
/// let core = CoreTypes::init(&mut table, &program, &names);
/// let (outline, _) = resolve_outline(&program, &names, &mut table, &core);
/// let references = normalize(&program, &names, &mut table, &core,
///     &outline, &BodyTypes::default(), &Default::default());
/// assert!(references.calls.is_empty() && references.issues.is_empty());
/// ```
///
/// # Panics
///
/// Panics if element or type identifiers do not belong to the supplied tables.
pub fn normalize(
    program: &Program,
    names: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    inferred: &HashSet<LibraryId>,
) -> WeakReferences {
    normalize_for(
        program,
        names,
        table,
        core,
        outline,
        bodies,
        inferred,
        TargetPolicy::JavaScript,
    )
}

/// Inspect weak declarations and calls using the selected frontend's target rules.
///
/// This validates declarations even when their owners are unreachable. Callers
/// must preserve ordinary resolution and constant errors, and reject `issues`
/// before applying these edges. A JIT validates the same native declarations
/// but must retain ordinary strong argument edges and execute the wrapper body.
///
/// ```
/// use dartforge_types::static_weak_references::{TargetPolicy, WeakReferences};
/// let references = WeakReferences::default();
/// assert!(references.calls.is_empty());
/// assert_ne!(TargetPolicy::Native, TargetPolicy::JavaScript);
/// ```
///
/// # Panics
///
/// Panics if the semantic tables violate the preconditions of [`normalize`].
#[allow(clippy::too_many_arguments)]
pub fn normalize_for(
    program: &Program,
    names: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    inferred: &HashSet<LibraryId>,
    policy: TargetPolicy,
) -> WeakReferences {
    let mut result = WeakReferences::default();
    let functions: HashMap<_, _> = program
        .functions
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.node {
            FunctionRef::Function { unit, function } => {
                Some(((unit, function), FunctionElementId(i as u32)))
            }
            _ => None,
        })
        .collect();
    let constructors: HashMap<_, _> = program
        .functions
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.node {
            FunctionRef::Constructor { unit, member } => {
                Some(((unit, member), FunctionElementId(i as u32)))
            }
            _ => None,
        })
        .collect();
    let mut engine = Engine::new(program, names, table, core, outline, bodies, inferred);
    for (i, unit) in program.units.iter().enumerate() {
        if program.library(unit.library).is_sdk {
            continue;
        }
        let id = UnitId(i as u32);
        let mut owners = HashMap::new();
        for d in &unit.ast.decls {
            if let ast::DeclKind::Function(f) = &d.kind {
                for m in &d.metadata {
                    if let Some(f) = functions.get(&(id, *f)) {
                        owners.insert(m.span.start, *f);
                    }
                }
            }
        }
        for (j, m) in unit.ast.members.iter().enumerate() {
            let owner = match &m.kind {
                ast::MemberKind::Method(f) => functions.get(&(id, *f)),
                ast::MemberKind::Constructor(_) => constructors.get(&(id, ast::MemberId(j as u32))),
                _ => None,
            };
            if let Some(f) = owner {
                for metadata in &m.metadata {
                    owners.insert(metadata.span.start, *f);
                }
            }
        }
        for metadata in dartforge_frontend::parents::all_annotations(&unit.ast, &unit.unit) {
            let Some(value) = engine.evaluate_annotation(id, metadata) else {
                continue;
            };
            let Type::Interface { class, .. } = engine.table.get(value.ty) else {
                continue;
            };
            let class = program.class(*class);
            if program.library(class.library).uri != "dart:core"
                || names.resolve(class.name) != "pragma"
            {
                continue;
            }
            let InstanceState::Generic { fields, .. } = &value.state else {
                continue;
            };
            let recognized=fields.iter().any(|(key,v)|matches!(key,Field::MemberName(name) if names.resolve(*name)=="name")
                && matches!(&v.state,InstanceState::Str(Some(t)) if String::from_utf16_lossy(t)=="weak-tearoff-reference"));
            if !recognized {
                continue;
            }
            let fail = |r: &mut WeakReferences, code: &str| {
                r.issues.push((id, metadata.span.start, code.into()))
            };
            let Some(&f) = owners.get(&metadata.span.start) else {
                fail(&mut result, "weak_reference_not_static");
                continue;
            };
            let function = program.function(f);
            if function.kind != FunctionKind::Function
                || !(function.static_ || function.class.is_none() && function.extension.is_none())
            {
                fail(&mut result, "weak_reference_not_static");
                continue;
            }
            let signature = &outline.functions[f.0 as usize];
            if signature.parameters.len() != 1
                || signature.parameters[0].kind != ParameterKind::Required
            {
                fail(&mut result, "weak_reference_not_one_argument");
                continue;
            }
            let returned = engine.table.get(signature.return_type);
            if !returned.is_declared_nullable()
                && !matches!(returned, Type::Dynamic | Type::Null | Type::Void)
            {
                fail(&mut result, "weak_reference_return_type_not_nullable");
                continue;
            }
            if signature.return_type != signature.parameters[0].ty {
                fail(
                    &mut result,
                    "weak_reference_mismatch_return_and_argument_types",
                );
                continue;
            }
            result.declarations.insert(f);
        }
    }
    for (i, unit) in program.units.iter().enumerate() {
        if program.library(unit.library).is_sdk {
            continue;
        }
        let id = UnitId(i as u32);
        let Some(body) = bodies.units.get(i) else {
            continue;
        };
        for (j, expr) in unit.ast.exprs.iter().enumerate() {
            let ExprKind::Call { target, arguments } = &expr.kind else {
                continue;
            };
            let mut callee = *target;
            while let ExprKind::TypeArguments { target, .. } | ExprKind::Parenthesized(target) =
                unit.ast.expr(callee).kind
            {
                callee = target;
            }
            let f = match body.get_resolved(callee) {
                Some(Resolved::Element(Element::Function(f))) => Some(*f),
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                }) if program.function(*f).static_ => Some(*f),
                Some(Resolved::ExtensionMember { member, .. })
                    if program.function(*member).static_ =>
                {
                    Some(*member)
                }
                _ => None,
            };
            let Some(f) = f.filter(|f| result.declarations.contains(f)) else {
                continue;
            };
            let _ = f;
            let fail = |r: &mut WeakReferences, code: &str| {
                r.issues.push((id, expr.span.start, code.into()))
            };
            if arguments.args.len() != 1 || arguments.args[0].name.is_some() {
                fail(&mut result, "weak_reference_not_one_argument");
                continue;
            }
            let mut argument = arguments.args[0].value;
            while let ExprKind::Parenthesized(inner) = unit.ast.expr(argument).kind {
                argument = inner;
            }
            // CFE folds constant aliases, but an explicit cast or conditional
            // expression is not a static-tear-off constant at this boundary.
            if !matches!(
                unit.ast.expr(argument).kind,
                ExprKind::Identifier(_) | ExprKind::Property { .. }
            ) {
                fail(&mut result, "weak_reference_target_not_static_tearoff");
                continue;
            }
            let cx = Ctx::simple(id, unit.library);
            let value = engine.evaluate(&cx, arguments.args[0].value, true);
            let Constant::Value(value) = value else {
                fail(&mut result, "weak_reference_target_not_static_tearoff");
                continue;
            };
            let InstanceState::Function {
                element: Function::Element(target),
                args,
            } = value.state
            else {
                fail(&mut result, "weak_reference_target_not_static_tearoff");
                continue;
            };
            let function = program.function(target);
            let signature = &outline.functions[target.0 as usize];
            // The JS frontend represents a constructor tear-off through a
            // static wrapper; AOT's normalizer uses its stricter target policy.
            let constructor = matches!(
                function.kind,
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor
            );
            let static_method = function.kind == FunctionKind::Function
                && (function.static_ || function.class.is_none() && function.extension.is_none());
            if !static_method && !(constructor && policy == TargetPolicy::JavaScript) {
                fail(&mut result, "weak_reference_target_not_static_tearoff");
                continue;
            }
            if constructor
                && function
                    .class
                    .is_none_or(|c| !program.class(c).type_params.is_empty())
            {
                fail(&mut result, "weak_reference_target_has_parameters");
                continue;
            }
            if !signature.parameters.is_empty()
                || !signature.type_params.is_empty()
                || args.is_some()
            {
                fail(&mut result, "weak_reference_target_has_parameters");
                continue;
            }
            result.calls.insert((id, ExprId(j as u32)), target);
        }
    }
    // A constant local used only as a weak argument must not create a
    // separate strong tear-off. Resolve reads semantically, and keep a
    // constant initializer only when a real runtime read reaches it.
    if !result.calls.is_empty() {
        for (i, unit) in program.units.iter().enumerate() {
            if program.library(unit.library).is_sdk {
                continue;
            }
            let id = UnitId(i as u32);
            let Some(body) = bodies.units.get(i) else {
                continue;
            };
            if unit.ast.exprs.iter().enumerate().any(|(j, _)| {
                let e = ExprId(j as u32);
                matches!(body.get_resolved(e), Some(Resolved::Local(_)))
                    && body.local_declaration(e).is_none()
            }) {
                continue;
            }
            let mut candidates = HashMap::new();
            let cx = Ctx::simple(id, unit.library);
            for statement in &unit.ast.stmts {
                let list = match &statement.kind {
                    ast::StmtKind::Variables(list) => Some(list),
                    ast::StmtKind::For {
                        init: Some(ast::ForInit::Variables(list)),
                        ..
                    } => Some(list),
                    _ => None,
                };
                if let Some(list) = list.filter(|list| list.const_) {
                    for variable in &list.variables {
                        if let Some(initializer) = variable.initializer
                            && let Constant::Value(value) = engine.evaluate(&cx, initializer, true)
                            && !value.state.unknown()
                            && !matches!(value.state, InstanceState::Null { invalid: true })
                        {
                            candidates.insert(variable.name.span.start, initializer);
                        }
                    }
                }
            }
            if candidates.is_empty() {
                continue;
            }
            let initializers: HashMap<_, _> = candidates
                .iter()
                .map(|(&offset, &expr)| (expr, offset))
                .collect();
            let arguments: HashSet<_> = result
                .calls
                .keys()
                .filter(|(u, _)| *u == id)
                .filter_map(|(_, expr)| match &unit.ast.expr(*expr).kind {
                    ExprKind::Call { arguments, .. } => arguments.args.first().map(|a| a.value),
                    _ => None,
                })
                .collect();
            let parents = dartforge_frontend::parents::Parents::new(
                &unit.ast,
                &unit.unit,
                &unit.source,
                false,
            );
            let mut live = HashSet::new();
            let mut dependencies: HashMap<usize, Vec<usize>> = HashMap::new();
            for (j, _) in unit.ast.exprs.iter().enumerate() {
                let mut expr = ExprId(j as u32);
                let Some(offset) = body
                    .local_declaration(expr)
                    .filter(|o| candidates.contains_key(o))
                else {
                    continue;
                };
                loop {
                    if arguments.contains(&expr) {
                        break;
                    }
                    if let Some(&owner) = initializers.get(&expr) {
                        dependencies.entry(owner).or_default().push(offset);
                        break;
                    }
                    if let dartforge_frontend::parents::Parent::Expr(parent) = parents.parent(expr)
                    {
                        expr = parent;
                    } else {
                        live.insert(offset);
                        break;
                    }
                }
            }
            let mut pending: Vec<_> = live.iter().copied().collect();
            while let Some(local) = pending.pop() {
                for dependency in dependencies.get(&local).into_iter().flatten() {
                    if live.insert(*dependency) {
                        pending.push(*dependency);
                    }
                }
            }
            for (local, initializer) in candidates {
                if !live.contains(&local) {
                    result.elided_const_initializers.insert((id, initializer));
                }
            }
        }
    }
    result
}
