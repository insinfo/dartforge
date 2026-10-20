//! Physical class provenance from allocation bodies, independent of Dart types.
//!
//! Facts concern only normal returns and remain unknown across loads, parameters,
//! dynamic calls, and recursive return summaries. They are not ownership/effect proofs.

mod canonical_cache;
mod dominators;
pub(crate) mod allocation_only_exports;
pub(crate) mod allocation_certificates;
mod selector_returns;
pub(crate) mod registry;
#[cfg(test)]
mod field_provenance;

use crate::hir::{
    BlockId, Constant, Function, Instruction as I, Module, Operand, Terminator, Type, ValueId,
};
use crate::optimize::operands::{operands, terminator_operands};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Physical class IDs, including Null when a normal path returns null.
pub(crate) type Classes = BTreeSet<u32>;

/// Checks bounded structural SSA and edge dominance, not general typing or effects.
pub(crate) fn has_structural_ssa_shape(function: &Function) -> bool {
    Shape::new(function).is_ok()
}

/// Compilation-only ablation; this does not change runtime execution policies.
pub(crate) fn enabled() -> bool {
    std::env::var("DARTFORGE_FACTORY_RECEIVER_PROVENANCE").as_deref() != Ok("0")
}

/// Facts derived from the exact bodies supplied to this analysis.
#[derive(Default, Debug)]
pub(crate) struct Provenance<'a> {
    returns: HashMap<String, Classes>,
    values: HashMap<String, HashMap<ValueId, Classes>>,
    bodies: HashMap<String, &'a Function>,
}

impl<'a> Provenance<'a> {
    /// Only facts for the very same immutable body are usable. A same-name
    /// replacement, including a clone with different instructions, is unknown.
    pub(crate) fn values_for(&self, function: &Function) -> Option<&HashMap<ValueId, Classes>> {
        self.bodies
            .get(&function.symbol)
            .filter(|body| std::ptr::eq(**body, function))
            .and_then(|_| self.values.get(&function.symbol))
    }

    /// Copies the facts for one module without detaching their body witnesses.
    pub(crate) fn for_module(&self, module: &'a Module) -> Self {
        let mut subset = Self::default();
        for function in &module.functions {
            if let Some(values) = self.values_for(function) {
                subset
                    .values
                    .insert(function.symbol.clone(), values.clone());
                subset.bodies.insert(function.symbol.clone(), function);
            }
        }
        subset
    }
}

/// Audits bodies jointly. Duplicate symbols and recursive return summaries stay unknown.
/// No external summary or nominal return declaration is trusted by this entry.
pub(crate) fn analyze<'a>(modules: &[&'a Module]) -> Provenance<'a> {
    let mut functions = HashMap::new();
    let mut duplicates = HashSet::new();
    for module in modules {
        for f in &module.functions {
            if functions.insert(f.symbol.clone(), f).is_some() {
                duplicates.insert(f.symbol.clone());
            }
        }
    }
    let shapes: HashMap<_, _> = functions
        .iter()
        .filter_map(|(symbol, f)| Shape::new(f).ok().map(|shape| (symbol.clone(), shape)))
        .collect();
    let imported = if modules.len() == 1 { modules[0].imported_allocation_returns.as_ref()
        .map(|imports| imports.reconcile(modules)).unwrap_or_default() } else { allocation_certificates::Imports::default() };
    let getters = selector_returns::Inventory::new(modules, &functions, &duplicates, &shapes, &imported);
    let dependencies: HashMap<_, Vec<String>> = functions
        .iter()
        .map(|(symbol, f)| {
            let calls = shapes
                .get(symbol)
                .map(|shape| {
                    shape
                        .reachable
                        .iter()
                        .flat_map(|&b| {
                            f.blocks[b]
                                .instructions
                                .iter()
                                .flat_map(|(_, instruction, _)| {
                                    if let I::CallStatic { symbol, .. } = instruction {
                                        vec![symbol.clone()]
                                    } else if let Some((selector, _)) = selector_returns::getter(instruction) {
                                        getters.dependencies(selector).cloned().collect::<Vec<_>>()
                                    } else { Vec::new() }
                                })
                        })
                        .collect()
                })
                .unwrap_or_default();
            (symbol.clone(), calls)
        })
        .collect();
    let mut symbols: Vec<_> = functions.keys().cloned().collect();
    symbols.sort();
    let positions: HashMap<_, _> = symbols
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let edges: Vec<Vec<usize>> = symbols
        .iter()
        .map(|s| {
            dependencies[s]
                .iter()
                .filter_map(|callee| positions.get(callee.as_str()).copied())
                .collect()
        })
        .collect();
    let mut result = Provenance::default();
    result.returns.extend(imported.exports.iter().map(|(symbol, export)| (symbol.clone(), export.classes.clone())));
    if std::env::var("DARTFORGE_NO_CANONICAL_CACHE_PROVENANCE").as_deref() != Ok("1") {
        let caches = canonical_cache::Inventory::new(modules);
        for (symbol, function) in &functions {
            if !duplicates.contains(symbol) && shapes.contains_key(symbol)
                && let Some(classes) = canonical_cache::classes(function, &caches, &functions) {
                result.returns.insert(symbol.clone(), classes);
            }
        }
    }
    // SCCs are emitted after their dependencies. Recursive components have no
    // body-return summary, even if a particular recursive path happens to throw.
    // Their local allocations and calls to independently proved factories still
    // have physical class facts; intra-component calls remain unknown.
    for component in crate::optimize::scc::components(&edges) {
        let recursive = component.len() != 1 || edges[component[0]].contains(&component[0]);
        for index in component {
            let symbol = &symbols[index];
            if duplicates.contains(symbol) {
                continue;
            }
            let f = functions[symbol];
            let Some(shape) = shapes.get(symbol) else {
                continue;
            };
            let values = value_classes(f, shape, &functions, &result.returns, &getters, &imported);
            if !recursive && f.return_ty == Type::Ref {
                let mut classes = Classes::new();
                let mut normal_returns = 0;
                let mut known = true;
                for &b in &shape.reachable {
                    if let Terminator::Return(value) = &f.blocks[b].terminator {
                        normal_returns += 1;
                        match value.as_ref().and_then(|v| operand_classes(v, &values)) {
                            Some(c) => classes.extend(c),
                            None => {
                                known = false;
                                break;
                            }
                        }
                    }
                }
                // Throw-only bodies are not allocation factories.
                if known && normal_returns > 0 && !classes.is_empty() {
                    result.returns.insert(symbol.clone(), classes);
                }
            }
            result.values.insert(symbol.clone(), values);
            result.bodies.insert(symbol.clone(), f);
        }
    }
    result
}

fn operand_classes(op: &Operand, values: &HashMap<ValueId, Classes>) -> Option<Classes> {
    match op {
        Operand::Constant(Constant::Null) => Some(BTreeSet::from([1])),
        Operand::Val(v) => values.get(v).cloned(),
        _ => None,
    }
}

fn value_classes(
    f: &Function,
    shape: &Shape,
    functions: &HashMap<String, &Function>,
    returns: &HashMap<String, Classes>,
    getters: &selector_returns::Inventory,
    imported: &allocation_certificates::Imports,
) -> HashMap<ValueId, Classes> {
    let mut values = HashMap::new();
    loop {
        let mut changed = false;
        for &b in &shape.reachable {
            for (v, instruction, ty) in &f.blocks[b].instructions {
                if *ty != Type::Ref || values.contains_key(v) {
                    continue;
                }
                let classes = match instruction {
                    I::Const(Constant::Null) => Some(BTreeSet::from([1])),
                    // ir_lists.rs always emits cid::GROWABLE_LIST for AllocList.
                    I::AllocList { .. } => Some(BTreeSet::from([10])),
                    I::AllocObject { class_id, .. } if i32::try_from(*class_id).is_ok() => {
                        Some(BTreeSet::from([*class_id]))
                    }
                    I::CallRuntime {
                        name,
                        args,
                        ret_ty: Type::Ref,
                    } | I::DeclaredFieldCall {
                        name,
                        args,
                        ret_ty: Type::Ref, .. } if matches!(
                        name.as_str(),
                        "dartforge_object_new" | "dartforge_arc_object_owned_v1"
                    ) && !functions.contains_key(name) =>
                    {
                        // Both ABIs allocate an instance with the supplied header CID.
                        // Literal arguments keep malformed calls and runtime-selected
                        // classes/counts outside this bounded allocation contract.
                        match args.as_slice() {
                            [
                                (Operand::Constant(Constant::Int(cid)), Type::I64),
                                (Operand::Constant(Constant::Int(count)), Type::I64),
                            ] if (0..=i64::from(i32::MAX)).contains(cid)
                                && usize::try_from(*count).is_ok()
                                && (name == "dartforge_object_new"
                                    || u16::try_from(*count).is_ok()) =>
                            {
                                Some(BTreeSet::from([*cid as u32]))
                            }
                            _ => None,
                        }
                    }
                    I::ArcCopy { value } | I::ArcMove { value } => operand_classes(value, &values),
                    I::Phi {
                        incoming,
                        ty: Type::Ref,
                    } => {
                        let mut union = Classes::new();
                        let mut known = true;
                        for (predecessor, op) in incoming {
                            if !shape.reachable.contains(&shape.positions[predecessor]) {
                                continue;
                            }
                            match operand_classes(op, &values) {
                                Some(c) => union.extend(c),
                                None => {
                                    known = false;
                                    break;
                                }
                            }
                        }
                        (known && !union.is_empty()).then_some(union)
                    }
                    I::CallRuntime {
                        name,
                        args,
                        ret_ty: Type::Ref,
                    } | I::DeclaredFieldCall {
                        name,
                        args,
                        ret_ty: Type::Ref, .. } if matches!(
                        name.as_str(),
                        "dartforge_native_List_allocate" | "dartforge_native_GrowableList_allocate"
                    ) && !functions.contains_key(name)
                        && args.len() == 2
                        && args[0].1 == Type::Ref
                        && args[1].1 == Type::I64
                        && shape.operand_type(&args[0].0) == Some(Type::Ref)
                        && shape.operand_type(&args[1].0) == Some(Type::I64) =>
                    {
                        Some(BTreeSet::from([
                            if name == "dartforge_native_List_allocate" {
                                8
                            } else {
                                10
                            },
                        ]))
                    }
                    I::CallStatic {
                        symbol,
                        args,
                        ret_ty: Type::Ref,
                    } => {
                        if let Some(export) = imported.exports.get(symbol) {
                            (export.parameters.len() == args.len()
                                && export.parameters.iter().zip(args).all(|(ty, arg)| shape.operand_type(arg) == Some(*ty)))
                                .then(|| export.classes.clone())
                        } else { functions
                            .get(symbol)
                            .filter(|callee| {
                                callee.return_ty == Type::Ref
                                    && callee.params.len() == args.len()
                                    && callee.params.iter().zip(args).all(|((_, _, ty), arg)| {
                                        shape.operand_type(arg) == Some(*ty)
                                    })
                            })
                            .and_then(|_| returns.get(symbol))
                            .cloned() }
                    }
                    instruction if selector_returns::getter(instruction).is_some() => {
                        let (selector, receiver) = selector_returns::getter(instruction).unwrap();
                        (shape.operand_type(receiver) == Some(Type::Ref))
                            .then(|| operand_classes(receiver, &values))
                            .flatten()
                            .and_then(|classes| getters.returns(selector, &classes, returns))
                    }
                    _ => None,
                };
                if let Some(classes) = classes {
                    values.insert(*v, classes);
                    changed = true;
                }
            }
        }
        if !changed {
            return values;
        }
    }
}

/// Structural SSA witness for identity/allocation facts, not a general HIR typer.
pub(super) struct Shape {
    positions: HashMap<BlockId, usize>,
    reachable: BTreeSet<usize>,
    definitions: HashMap<ValueId, (Type, Option<(usize, usize)>)>,
}
impl Shape {
    pub(super) fn operand_type(&self, op: &Operand) -> Option<Type> {
        match op {
            Operand::Val(v) => self.definitions.get(v).map(|d| d.0),
            Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
                Some(Type::Ref)
            }
            Operand::Constant(Constant::Int(_) | Constant::Function(_)) => Some(Type::I64),
            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
            Operand::Constant(Constant::Double(_)) => Some(Type::F64),
        }
    }
    pub(super) fn new(f: &Function) -> Result<Self, String> {
        // A bounded proof budget is a conservative fallback, never a type guess.
        if f.blocks.is_empty()
            || f.blocks.len() > dominators::MAX_BLOCKS
            || f.blocks.iter().map(|b| b.instructions.len()).sum::<usize>() > 8192
        {
            return Err("empty body or structural proof budget exceeded".into());
        }
        let mut shape = Self {
            positions: HashMap::new(),
            reachable: BTreeSet::new(),
            definitions: HashMap::new(),
        };
        for (b, block) in f.blocks.iter().enumerate() {
            if shape.positions.insert(block.id, b).is_some() {
                return Err("duplicate block identity".into());
            }
        }
        let mut predecessors = vec![Vec::new(); f.blocks.len()];
        let mut successors = vec![Vec::new(); f.blocks.len()];
        for (b, block) in f.blocks.iter().enumerate() {
            let edges: Vec<_> = match &block.terminator {
                Terminator::Branch(to) => vec![*to],
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => vec![*then_block, *else_block],
                Terminator::Switch { default, cases, .. } => std::iter::once(*default)
                    .chain(cases.iter().map(|c| c.1))
                    .collect(),
                _ => vec![],
            };
            for edge in edges {
                let &to = shape
                    .positions
                    .get(&edge)
                    .ok_or_else(|| "missing CFG successor".to_string())?;
                successors[b].push(to);
                predecessors[to].push(b);
            }
        }
        let order = dominators::reverse_postorder(&successors);
        shape.reachable.extend(order.iter().copied());
        // Entry phis/edges need a more complete entry-loop model; stay unknown.
        if !predecessors[0].is_empty() {
            return Err("unsupported entry predecessor".into());
        }
        for (v, _, ty) in &f.params {
            if *ty == Type::Void || shape.definitions.insert(*v, (*ty, None)).is_some() {
                return Err("invalid or duplicate formal value".into());
            }
        }
        for &b in &shape.reachable {
            for (position, (v, _, ty)) in f.blocks[b].instructions.iter().enumerate() {
                if shape
                    .definitions
                    .insert(*v, (*ty, Some((b, position))))
                    .is_some()
                {
                    return Err("duplicate SSA definition".into());
                }
            }
        }
        let dominators = dominators::Dominators::new(&predecessors, &order);
        let available = |op: &Operand, b: usize, position: usize| match op {
            Operand::Val(v) => match shape.definitions.get(v) {
                Some((_, None)) => true,
                Some((_, Some((def_b, def_position)))) => {
                    if *def_b == b {
                        *def_position < position
                    } else {
                        dominators.dominates(*def_b, b)
                    }
                }
                None => false,
            },
            _ => true,
        };
        for &b in &shape.reachable {
            let mut non_phi = false;
            for (position, (_, i, ty)) in f.blocks[b].instructions.iter().enumerate() {
                if let I::Phi {
                    incoming,
                    ty: phi_ty,
                } = i
                {
                    if non_phi || phi_ty != ty {
                        return Err("non-leading or inconsistent Phi".into());
                    }
                    let mut edges: Vec<_> = incoming
                        .iter()
                        .map(|(p, _)| shape.positions.get(p).copied())
                        .collect::<Option<_>>()
                        .ok_or_else(|| "unknown Phi predecessor".to_string())?;
                    let mut expected = predecessors[b].clone();
                    edges.sort_unstable();
                    expected.sort_unstable();
                    if edges != expected {
                        return Err("Phi predecessors do not match CFG edges".into());
                    }
                    for (p, op) in incoming {
                        let p = shape.positions[p];
                        if shape.reachable.contains(&p)
                            && (!available(op, p, usize::MAX)
                                || shape.operand_type(op) != Some(*ty))
                        {
                            return Err("Phi type or edge dominance unavailable".into());
                        }
                    }
                } else {
                    non_phi = true;
                    let mut valid = true;
                    operands(i, &mut |op| valid &= available(op, b, position));
                    if !valid {
                        return Err("SSA operand definition/dominance unavailable".into());
                    }
                    if matches!(i, I::ArcCopy { .. } | I::ArcMove { .. }) {
                        let mut valid = *ty == Type::Ref;
                        operands(i, &mut |op| {
                            valid &= shape.operand_type(op) == Some(Type::Ref)
                        });
                        if !valid {
                            return Err("ARC identity operand is not an exact Ref".into());
                        }
                    }
                }
            }
            let mut valid = true;
            terminator_operands(&f.blocks[b].terminator, &mut |op| {
                valid &= available(op, b, usize::MAX)
            });
            if let Terminator::Return(Some(op)) = &f.blocks[b].terminator {
                valid &= shape.operand_type(op) == Some(f.return_ty);
            }
            match &f.blocks[b].terminator {
                Terminator::Return(None) => valid &= f.return_ty == Type::Void,
                Terminator::CondBranch { cond, .. } => {
                    valid &= shape.operand_type(cond) == Some(Type::I1)
                }
                Terminator::Switch { val, .. } => {
                    valid &= shape.operand_type(val) == Some(Type::I64)
                }
                Terminator::Throw(value) => valid &= shape.operand_type(value) == Some(Type::Ref),
                _ => {}
            }
            if !valid {
                return Err("terminator operand representation or dominance unavailable".into());
            }
        }
        Ok(shape)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::BasicBlock;

    fn value(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }
    fn block(n: u32, instructions: Vec<(ValueId, I, Type)>, terminator: Terminator) -> BasicBlock {
        BasicBlock {
            id: BlockId(n),
            instructions,
            terminator,
        }
    }
    fn function(name: &str, blocks: Vec<BasicBlock>) -> Function {
        Function {
            symbol: name.into(),
            name: name.into(),
            params: vec![],
            return_ty: Type::Ref,
            blocks,
            debug_info: None,
        }
    }
    fn allocated(n: u32, cid: u32) -> (ValueId, I, Type) {
        (
            ValueId(n),
            I::AllocObject {
                class_id: cid,
                fields: vec![],
            },
            Type::Ref,
        )
    }
    struct TestFacts {
        returns: HashMap<String, Classes>,
        values: HashMap<String, HashMap<ValueId, Classes>>,
    }

    fn getter_module() -> Module {
        let mut module = Module::new();
        module.arc_fixed_ancestry = true;
        for (cid, target, returned) in [(300, "getter_a", 400), (301, "getter_b", 401)] {
            module.classes.push(crate::hir::ClassDef {
                id: cid, name: target.into(), field_count: 0, vtable: vec![], to_string_symbol: None,
            });
            let table = format!("table_{cid}");
            module.table_functions.insert(cid, table.clone());
            module.method_tables.push((cid, table, vec![("g:encoder".into(), target.into())]));
            let mut getter = function(target, vec![block(0, vec![allocated(3, returned)], Terminator::Return(Some(value(3))))]);
            getter.params = vec![(ValueId(0), "this".into(), Type::Ref), (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "descriptor".into(), Type::Ptr)];
            module.functions.push(getter);
        }
        module.functions.push(function("caller", vec![block(0, vec![allocated(0, 300),
            (ValueId(1), I::CallSelector { selector: "g:encoder".into(), recv: value(0), args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Int(0)) }, Type::Ref)], Terminator::Return(Some(value(1))))]));
        module
    }

    #[test]
    fn physical_getter_result_requires_complete_registered_body_and_abi() {
        let baseline = getter_module();
        assert_eq!(analyze(&[&baseline]).returns["caller"], BTreeSet::from([400]));
        for variant in 0..7 {
            let mut module = getter_module();
            match variant {
                0 => module.method_tables[0].2.clear(),
                1 => module.method_tables.push(module.method_tables[0].clone()),
                2 => module.functions.push(module.functions[0].clone()),
                3 => module.functions[0].params[1].2 = Type::Ref,
                4 => module.arc_fixed_ancestry = false,
                5 => { module.arc_external_ancestry.nominal_classes.insert(300); }
                _ => { let pair = module.method_tables[0].2[0].clone(); module.method_tables[0].2.push(pair); }
            };
            assert!(!analyze(&[&module]).returns.contains_key("caller"), "variant {variant}");
        }
    }

    #[test]
    fn getter_result_never_bootstraps_recursive_getter_cycle() {
        let mut module = getter_module();
        module.functions[0].blocks[0].instructions[0] = allocated(3, 300);
        module.functions[0].blocks[0].instructions.push((ValueId(4), I::CallSelector {
            selector: "g:encoder".into(), recv: value(3), args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Int(0)),
        }, Type::Ref));
        module.functions[0].blocks[0].terminator = Terminator::Return(Some(value(4)));
        assert!(!analyze(&[&module]).returns.contains_key("getter_a"));
        assert!(!analyze(&[&module]).returns.contains_key("caller"));
    }

    #[test]
    fn getter_union_requires_every_receiver_and_preserves_unknown_formals() {
        let mut module = getter_module();
        let mut caller = function("caller", vec![
            block(0, vec![], Terminator::CondBranch { cond: value(0), then_block: BlockId(1), else_block: BlockId(2) }),
            block(1, vec![allocated(1, 300)], Terminator::Branch(BlockId(3))),
            block(2, vec![allocated(2, 301)], Terminator::Branch(BlockId(3))),
            block(3, vec![(ValueId(3), I::Phi { incoming: vec![(BlockId(1), value(1)), (BlockId(2), value(2))], ty: Type::Ref }, Type::Ref),
                (ValueId(4), I::CallSelector { selector: "g:encoder".into(), recv: value(3), args: vec![], names: vec![], tuple_types: Operand::Constant(Constant::Int(0)) }, Type::Ref)], Terminator::Return(Some(value(4)))),
        ]);
        caller.params = vec![(ValueId(0), "choose".into(), Type::I1)];
        module.functions[2] = caller;
        assert_eq!(analyze(&[&module]).returns["caller"], BTreeSet::from([400, 401]));
        module.functions[1].blocks[0].terminator = Terminator::Return(Some(value(0)));
        assert!(!analyze(&[&module]).returns.contains_key("caller"), "unknown candidate cannot be dropped from the union");
        let mut unknown = getter_module();
        unknown.functions[2].params = vec![(ValueId(0), "unknown".into(), Type::Ref)];
        unknown.functions[2].blocks[0].instructions.remove(0);
        assert!(!analyze(&[&unknown]).returns.contains_key("caller"), "a formal receiver is not its declared class");
    }
    fn run(functions: Vec<Function>) -> TestFacts {
        let module = Module {
            functions,
            ..Module::default()
        };
        let proof = analyze(&[&module]);
        TestFacts {
            returns: proof.returns,
            values: proof.values,
        }
    }
    fn instance_call(name: &str, cid: i64, count: i64) -> I {
        I::CallRuntime {
            name: name.into(),
            args: vec![
                (Operand::Constant(Constant::Int(cid)), Type::I64),
                (Operand::Constant(Constant::Int(count)), Type::I64),
            ],
            ret_ty: Type::Ref,
        }
    }
    #[test]
    fn instance_allocation_cids_and_factory_returns_are_physical() {
        for cid in [0, 123, 32001, i32::MAX as u32] {
            let factory = function(
                "factory",
                vec![block(
                    0,
                    vec![allocated(0, cid)],
                    Terminator::Return(Some(value(0))),
                )],
            );
            assert_eq!(run(vec![factory]).returns["factory"], BTreeSet::from([cid]));
            for name in ["dartforge_object_new", "dartforge_arc_object_owned_v1"] {
                let factory = function(
                    "factory",
                    vec![block(
                        0,
                        vec![(
                            ValueId(0),
                            instance_call(name, i64::from(cid), 2),
                            Type::Ref,
                        )],
                        Terminator::Return(Some(value(0))),
                    )],
                );
                let caller = function(
                    "caller",
                    vec![block(
                        0,
                        vec![(
                            ValueId(0),
                            I::CallStatic {
                                symbol: "factory".into(),
                                args: vec![],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        )],
                        Terminator::Return(Some(value(0))),
                    )],
                );
                let proof = run(vec![factory, caller]);
                assert_eq!(proof.returns["caller"], BTreeSet::from([cid]));
                assert_eq!(proof.values["factory"][&ValueId(0)], BTreeSet::from([cid]));
            }
        }
        let proof = run(vec![function(
            "invalid",
            vec![block(
                0,
                vec![allocated(0, i32::MAX as u32 + 1)],
                Terminator::Return(Some(value(0))),
            )],
        )]);
        assert!(!proof.returns.contains_key("invalid"));
    }
    #[test]
    fn instance_runtime_allocations_reject_unproved_abi_ranges_and_homonyms() {
        for name in ["dartforge_object_new", "dartforge_arc_object_owned_v1"] {
            let mut invalid = vec![
                instance_call(name, -1, 0),
                instance_call(name, i64::from(i32::MAX) + 1, 0),
                instance_call(name, 123, -1),
                instance_call(&format!("{name}_t"), 123, 0),
            ];
            for position in 0..2 {
                let mut declared = instance_call(name, 123, 0);
                let (I::CallRuntime { args, .. } | I::DeclaredFieldCall { args, .. }) = &mut declared else {
                    unreachable!()
                };
                args[position].1 = Type::Ref;
                invalid.push(declared);
                let mut actual = instance_call(name, 123, 0);
                let (I::CallRuntime { args, .. } | I::DeclaredFieldCall { args, .. }) = &mut actual else {
                    unreachable!()
                };
                args[position].0 = Operand::Constant(Constant::Null);
                invalid.push(actual);
            }
            let mut arity = instance_call(name, 123, 0);
            let (I::CallRuntime { args, .. } | I::DeclaredFieldCall { args, .. }) = &mut arity else {
                unreachable!()
            };
            args.pop();
            invalid.push(arity);
            let mut arity = instance_call(name, 123, 0);
            let (I::CallRuntime { args, .. } | I::DeclaredFieldCall { args, .. }) = &mut arity else {
                unreachable!()
            };
            args.push((Operand::Constant(Constant::Int(0)), Type::I64));
            invalid.push(arity);
            let mut result = instance_call(name, 123, 0);
            let (I::CallRuntime { ret_ty, .. } | I::DeclaredFieldCall { ret_ty, .. }) = &mut result else {
                unreachable!()
            };
            *ret_ty = Type::I64;
            invalid.push(result);
            if name == "dartforge_arc_object_owned_v1" {
                invalid.push(instance_call(name, 123, i64::from(u16::MAX) + 1));
            }
            for instruction in invalid {
                let proof = run(vec![function(
                    "invalid",
                    vec![block(
                        0,
                        vec![(ValueId(0), instruction, Type::Ref)],
                        Terminator::Return(Some(value(0))),
                    )],
                )]);
                assert!(!proof.returns.contains_key("invalid"));
                assert!(!proof.values["invalid"].contains_key(&ValueId(0)));
            }
            let factory = function(
                "shadowed",
                vec![block(
                    0,
                    vec![(ValueId(0), instance_call(name, 123, 0), Type::Ref)],
                    Terminator::Return(Some(value(0))),
                )],
            );
            let homonym = function(
                name,
                vec![block(
                    0,
                    vec![],
                    Terminator::Return(Some(Operand::Constant(Constant::Null))),
                )],
            );
            assert!(!run(vec![factory, homonym]).returns.contains_key("shadowed"));
            for position in 0..2 {
                let mut instruction = instance_call(name, 123, 0);
                let (I::CallRuntime { args, .. } | I::DeclaredFieldCall { args, .. }) = &mut instruction else {
                    unreachable!()
                };
                args[position].0 = value(1);
                let mut factory = function(
                    "dynamic",
                    vec![block(
                        0,
                        vec![(ValueId(0), instruction, Type::Ref)],
                        Terminator::Return(Some(value(0))),
                    )],
                );
                factory.params.push((ValueId(1), "input".into(), Type::I64));
                assert!(!run(vec![factory]).returns.contains_key("dynamic"));
            }
            let mut wrong_result = function(
                "scalar",
                vec![block(
                    0,
                    vec![(ValueId(0), instance_call(name, 123, 0), Type::I64)],
                    Terminator::Return(Some(value(0))),
                )],
            );
            wrong_result.return_ty = Type::I64;
            assert!(!run(vec![wrong_result]).values["scalar"].contains_key(&ValueId(0)));
        }
        for (name, count) in [
            ("dartforge_object_new", i64::from(u16::MAX) + 1),
            ("dartforge_arc_object_owned_v1", i64::from(u16::MAX)),
        ] {
            let proof = run(vec![function(
                "boundary",
                vec![block(
                    0,
                    vec![(ValueId(0), instance_call(name, 123, count), Type::Ref)],
                    Terminator::Return(Some(value(0))),
                )],
            )]);
            assert_eq!(proof.returns["boundary"], BTreeSet::from([123]));
        }
    }
    #[test]
    fn allocation_call_and_cfg_phi_union_are_body_proved() {
        let mut choose = function(
            "choose",
            vec![
                block(
                    0,
                    vec![],
                    Terminator::CondBranch {
                        cond: value(0),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                ),
                block(1, vec![allocated(1, 8)], Terminator::Branch(BlockId(3))),
                block(2, vec![allocated(2, 10)], Terminator::Branch(BlockId(3))),
                block(
                    3,
                    vec![(
                        ValueId(3),
                        I::Phi {
                            incoming: vec![(BlockId(1), value(1)), (BlockId(2), value(2))],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    Terminator::Return(Some(value(3))),
                ),
            ],
        );
        choose
            .params
            .push((ValueId(0), "condition".into(), Type::I1));
        let caller = function(
            "caller",
            vec![block(
                0,
                vec![(
                    ValueId(0),
                    I::CallStatic {
                        symbol: "choose".into(),
                        args: vec![Operand::Constant(Constant::Bool(false))],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Terminator::Return(Some(value(0))),
            )],
        );
        let result = run(vec![caller, choose]);
        assert_eq!(result.returns["caller"], BTreeSet::from([8, 10]));
        assert_eq!(
            result.values["choose"][&ValueId(3)],
            BTreeSet::from([8, 10])
        );
    }
    #[test]
    fn parameter_load_capture_and_opaque_returns_stay_unknown() {
        for instruction in [
            I::LoadGlobal {
                symbol: "global".into(),
                ty: Type::Ref,
            },
            I::CallStatic {
                symbol: "unavailable".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            I::CallRuntime {
                name: "unreviewed_allocator".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            I::EnvGet {
                env: Operand::Constant(Constant::Null),
                index: 0,
            },
        ] {
            let result = run(vec![function(
                "unknown",
                vec![block(
                    0,
                    vec![(ValueId(0), instruction, Type::Ref)],
                    Terminator::Return(Some(value(0))),
                )],
            )]);
            assert!(!result.returns.contains_key("unknown"));
        }
        let mut parameter = function(
            "parameter",
            vec![block(0, vec![], Terminator::Return(Some(value(0))))],
        );
        parameter
            .params
            .push((ValueId(0), "nominalList".into(), Type::Ref));
        assert!(!run(vec![parameter]).returns.contains_key("parameter"));
    }
    #[test]
    fn throws_do_not_pollute_normal_returns_and_dead_returns_are_not_evidence() {
        let mut f = function(
            "factory",
            vec![
                block(
                    0,
                    vec![],
                    Terminator::CondBranch {
                        cond: value(0),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                ),
                block(1, vec![allocated(1, 8)], Terminator::Return(Some(value(1)))),
                block(
                    2,
                    vec![],
                    Terminator::Throw(Operand::Constant(Constant::Null)),
                ),
                block(3, vec![], Terminator::Return(Some(value(999)))),
            ],
        );
        f.params.push((ValueId(0), "throw".into(), Type::I1));
        assert_eq!(run(vec![f]).returns["factory"], BTreeSet::from([8]));
        assert!(
            run(vec![function(
                "throw",
                vec![block(0, vec![], Terminator::Unreachable)]
            )])
            .returns
            .is_empty()
        );
    }
    #[test]
    fn cycles_duplicates_wrong_call_abi_and_bad_cfg_are_unknown() {
        let recursive = function(
            "recursive",
            vec![block(
                0,
                vec![
                    (
                        ValueId(0),
                        I::CallStatic {
                            symbol: "recursive".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    allocated(1, 8),
                ],
                Terminator::Return(Some(value(1))),
            )],
        );
        let recursive_facts = run(vec![recursive.clone()]);
        assert!(recursive_facts.returns.is_empty());
        assert_eq!(
            recursive_facts.values["recursive"][&ValueId(1)],
            BTreeSet::from([8])
        );
        assert!(!recursive_facts.values["recursive"].contains_key(&ValueId(0)));
        let mut peer = recursive.clone();
        peer.symbol = "peer".into();
        let mut recursive = recursive;
        recursive.blocks[0].instructions[0].1 = I::CallStatic {
            symbol: "peer".into(),
            args: vec![],
            ret_ty: Type::Ref,
        };
        let mutual = run(vec![recursive, peer]);
        assert!(mutual.returns.is_empty());
        for symbol in ["recursive", "peer"] {
            assert_eq!(mutual.values[symbol][&ValueId(1)], BTreeSet::from([8]));
            assert!(!mutual.values[symbol].contains_key(&ValueId(0)));
        }
        let f = function(
            "same",
            vec![block(
                0,
                vec![allocated(0, 8)],
                Terminator::Return(Some(value(0))),
            )],
        );
        assert!(run(vec![f.clone(), f]).returns.is_empty());
        let wrong = function(
            "wrong",
            vec![block(
                0,
                vec![(
                    ValueId(0),
                    I::CallRuntime {
                        name: "dartforge_native_List_allocate".into(),
                        args: vec![],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Terminator::Return(Some(value(0))),
            )],
        );
        assert!(run(vec![wrong]).returns.is_empty());
        assert!(
            run(vec![function(
                "bad",
                vec![block(
                    0,
                    vec![allocated(0, 8)],
                    Terminator::Branch(BlockId(9))
                )]
            )])
            .returns
            .is_empty()
        );
    }
    #[test]
    fn phi_unknown_arm_and_non_dominating_definition_do_not_narrow() {
        let mut f = function(
            "factory",
            vec![
                block(
                    0,
                    vec![],
                    Terminator::CondBranch {
                        cond: value(0),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                ),
                block(1, vec![allocated(2, 8)], Terminator::Branch(BlockId(3))),
                block(2, vec![], Terminator::Branch(BlockId(3))),
                block(
                    3,
                    vec![(
                        ValueId(3),
                        I::Phi {
                            incoming: vec![(BlockId(1), value(2)), (BlockId(2), value(1))],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    )],
                    Terminator::Return(Some(value(3))),
                ),
            ],
        );
        f.params = vec![
            (ValueId(0), "condition".into(), Type::I1),
            (ValueId(1), "other".into(), Type::Ref),
        ];
        assert!(!run(vec![f.clone()]).returns.contains_key("factory"));
        f.blocks[3].terminator = Terminator::Return(Some(value(2)));
        assert!(!run(vec![f]).returns.contains_key("factory"));
    }
    #[test]
    fn mutations_keep_physical_class_but_loaded_alias_does_not_gain_fact() {
        let f = function(
            "factory",
            vec![block(
                0,
                vec![
                    allocated(0, 10),
                    (
                        ValueId(1),
                        I::CallRuntime {
                            name: "dartforge_native_GrowableList_setLength".into(),
                            args: vec![
                                (value(0), Type::Ref),
                                (Operand::Constant(Constant::Int(3)), Type::I64),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ),
                    (ValueId(2), I::ArcCopy { value: value(0) }, Type::Ref),
                ],
                Terminator::Return(Some(value(2))),
            )],
        );
        assert_eq!(run(vec![f]).returns["factory"], BTreeSet::from([10]));
    }
    #[test]
    fn native_homonym_or_substituted_factory_body_is_not_a_certificate() {
        let call = function(
            "call",
            vec![block(
                0,
                vec![(
                    ValueId(0),
                    I::CallRuntime {
                        name: "dartforge_native_List_allocate".into(),
                        args: vec![
                            (Operand::Constant(Constant::Null), Type::Ref),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Terminator::Return(Some(value(0))),
            )],
        );
        assert_eq!(run(vec![call.clone()]).returns["call"], BTreeSet::from([8]));
        let shadow = function(
            "dartforge_native_List_allocate",
            vec![block(
                0,
                vec![],
                Terminator::Return(Some(Operand::Constant(Constant::Null))),
            )],
        );
        assert!(!run(vec![call, shadow]).returns.contains_key("call"));
        let allocating = function(
            "factory",
            vec![block(
                0,
                vec![allocated(0, 8)],
                Terminator::Return(Some(value(0))),
            )],
        );
        assert_eq!(
            run(vec![allocating]).returns["factory"],
            BTreeSet::from([8])
        );
        let mut replacement = function(
            "factory",
            vec![block(0, vec![], Terminator::Return(Some(value(0))))],
        );
        replacement
            .params
            .push((ValueId(0), "input".into(), Type::Ref));
        assert!(!run(vec![replacement]).returns.contains_key("factory"));
    }
    #[test]
    fn borrowed_witness_rejects_same_symbol_body_replacement() {
        let module = Module {
            functions: vec![function(
                "factory",
                vec![block(
                    0,
                    vec![allocated(0, 8)],
                    Terminator::Return(Some(value(0))),
                )],
            )],
            ..Module::default()
        };
        let proof = analyze(&[&module]);
        assert_eq!(
            proof.values_for(&module.functions[0]).unwrap()[&ValueId(0)],
            BTreeSet::from([8])
        );
        let replacement = module.functions[0].clone();
        assert!(proof.values_for(&replacement).is_none());
        let subset = proof.for_module(&module);
        assert_eq!(
            subset.values_for(&module.functions[0]).unwrap()[&ValueId(0)],
            BTreeSet::from([8])
        );
    }
    #[test]
    fn unreviewed_prepared_arc_allocation_abi_stays_unknown() {
        let f = function(
            "factory",
            vec![block(
                0,
                vec![(
                    ValueId(0),
                    I::CallRuntime {
                        name: "dartforge_arc_list_new_owned_v1".into(),
                        args: vec![],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Terminator::Return(Some(value(0))),
            )],
        );
        assert!(run(vec![f]).returns.is_empty());
    }
    #[test]
    fn actual_sdk_list_filled_returns_are_proved_without_trusting_captured_receivers() {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                use dartforge_types::table::{CoreTypes, TypeTable};
                let root =
                    std::env::temp_dir().join(format!("factory-provenance-{}", std::process::id()));
                std::fs::create_dir_all(&root).unwrap();
                let entry = root.join("main.dart");
                std::fs::write(
                    &entry,
                    "import 'dart:convert'; void main() { print(jsonEncode({'x': 1})); }",
                )
                .unwrap();
                let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
                let mut interner = dartforge_intern::Interner::new();
                let (program, diagnostics) =
                    dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
                assert!(diagnostics.is_empty());
                let mut table = TypeTable::new();
                let core = CoreTypes::init(&mut table, &program, &interner);
                let (mut outline, _) =
                    dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
                let libs: Vec<_> = program
                    .libraries
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| matches!(l.uri.as_str(), "dart:core" | "dart:convert"))
                    .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
                    .collect();
                let (bodies, _) = dartforge_types::infer_library_bodies(
                    &program,
                    &interner,
                    &mut table,
                    &core,
                    &mut outline,
                    &libs,
                );
                let modules: Vec<_> = libs
                    .iter()
                    .map(|&lib| {
                        let ctx = crate::context::Context::new(
                            &program, &interner, &table, &core, &outline, &bodies,
                        )
                        .with_source_sdk()
                        .only_the_library(lib);
                        let mut module = crate::lower::lower_program(&ctx);
                        module.arc_fixed_ancestry = true;
                        crate::optimize::prepare_for_emission(&mut module, false, false);
                        module
                    })
                    .collect();
                let proof = analyze(&modules.iter().collect::<Vec<_>>());
                let encoder = modules.iter().flat_map(|m| &m.functions)
                    .find(|f| f.symbol == "df.dart$3aconvert.AsciiCodec.encode$c").unwrap();
                let mut encoder_classes: Classes = modules.iter().flat_map(|m| &m.method_tables)
                    .filter(|(_, symbol, _)| symbol.ends_with(".AsciiEncoder"))
                    .map(|(cid, _, _)| *cid).collect();
                assert_eq!(encoder_classes.len(), 1);
                // The retained constant-child failure guard has an explicit
                // error return of null. This analysis conservatively includes
                // it instead of mistaking an error path for a non-null value.
                encoder_classes.insert(1);
                let encoder_calls: Vec<_> = encoder.blocks.iter().flat_map(|b| &b.instructions)
                    .filter_map(|(_, instruction, _)| match instruction {
                        I::CallSelector { selector, recv: Operand::Val(receiver), .. } if selector == "tc:convert" =>
                            Some(proof.values_for(encoder).and_then(|values| values.get(receiver))),
                        _ => None,
                    }).collect();
                assert!(!encoder_calls.is_empty());
                assert!(encoder_calls.iter().all(|classes| *classes == Some(&encoder_classes)),
                    "canonical AsciiEncoder receiver must be physically proved: {encoder_calls:?}");
                let public_factory = "df.dart$3acore.List.new$3afilled";
                if proof.returns.get(public_factory) != Some(&BTreeSet::from([1, 8, 10])) {
                    let bodies: Vec<_> = modules
                        .iter()
                        .flat_map(|m| &m.functions)
                        .filter(|f| {
                            f.symbol.contains("List.new") || f.symbol.contains("List.filled")
                        })
                        .map(|f| format!("{f:#?}"))
                        .collect();
                    std::fs::write(root.join("list-factory-hir.txt"), bodies.join("\n")).unwrap();
                }
                assert_eq!(
                    proof.returns.get(public_factory),
                    Some(&BTreeSet::from([1, 8, 10])),
                    "actual public List.filled factory must be body-proved"
                );
                let json = modules
                    .iter()
                    .flat_map(|m| &m.functions)
                    .find(|f| f.symbol == "df.dart$3aconvert._JsonPrettyPrintMixin.writeMap")
                    .unwrap();
                let definitions: HashMap<_, _> = json.blocks.iter().flat_map(|b| &b.instructions).map(|(v, i, _)| (*v, i)).collect();
                let calls: Vec<_> = json.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(_, i, _)| match i {
                    I::CallSelector { recv: Operand::Val(v), selector, .. } | I::CallSelectorForward { recv: Operand::Val(v), selector, .. }
                        if selector.ends_with("length") => Some(serde_json::json!({
                            "selector": selector, "receiver": v.0,
                            "classes": proof.values_for(json).and_then(|values| values.get(v)),
                            "origin": format!("{:?}", definitions.get(v)),
                        })),
                    _ => None,
                }).collect();
                std::fs::write(root.join("json-write-map-hir.txt"), format!("{json:#?}")).unwrap();
                std::fs::write(root.join("factory-provenance.json"), serde_json::to_string_pretty(&serde_json::json!({
                    "factory_classes": proof.returns[public_factory],
                    "json_body_structural_witness": proof.values_for(json).is_some(),
                    "json_structural_rejection": Shape::new(json).err(),
                    "json_length_calls": calls,
                    "body_proved_factories": proof.returns.len(),
                    "known_selector_receivers": modules.iter().flat_map(|m| &m.functions).flat_map(|f| {
                        f.blocks.iter().flat_map(|b| &b.instructions).filter_map(move |(_, i, _)| match i {
                            I::CallSelector { recv: Operand::Val(v), .. } | I::CallSelectorForward { recv: Operand::Val(v), .. } => Some((f, *v)), _ => None,
                        })
                    }).filter(|(f, v)| proof.values_for(f).is_some_and(|values| values.contains_key(v))).count(),
                })).unwrap()).unwrap();
                // The Map parameter stays unknown. The local factory result is
                // known even though this JSON writer belongs to a recursive SCC.
                assert!(calls.iter().any(|call| call["classes"].is_null()));
                assert!(calls.iter().any(|call| call["classes"] == serde_json::json!([1, 8, 10])));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
