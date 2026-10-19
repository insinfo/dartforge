//! Joint preparation of separate bodies for emission in distinct modules.

use super::*;

/// Certifies the AOT ports of each module of a set prepared by
/// [`prepare_arc_dart_modules`]. The checked writes carry the layouts
/// of the set (the union the preparation checked, without conflicts), and each
/// module is checked against that union. Publishes
/// only if all are certified; returns the total number of ports.
///
/// # Errors
/// Incompatible number of plans or a failure of any certification.
pub fn certify_arc_module_ports(
    modules: &mut [Module],
    plans: &[HashMap<String, DartFunctionPlan>],
) -> Result<usize, String> {
    if modules.len() != plans.len() {
        return Err("ARC modules: incompatible number of plans".into());
    }
    let mut union_set: HashMap<u32, Vec<Type>> = HashMap::new();
    for m in modules.iter() {
        for (cid, layout) in &m.arc_field_layouts {
            if union_set.insert(*cid, layout.clone()).is_some_and(|previous| previous != *layout) {
                return Err(format!("ARC modules: conflicting layout of class {cid}"));
            }
        }
    }
    let mut new_set = modules.to_vec();
    // The idempotence of the whole set (the callees of other modules are
    // part of the proof), once; each module checks the rest on its own.
    let mut again = new_set.clone();
    let mut plans_again = plans.to_vec();
    let changes = prepare_arc_dart_modules(&mut again, &mut plans_again)?;
    let equal = again.iter().zip(&new_set).all(|(a, b)| {
        a.functions.len() == b.functions.len()
            && a.functions.iter().zip(&b.functions).all(|(x, y)| format!("{x:?}") == format!("{y:?}"))
    });
    if changes != (0, 0) || !equal || plans_again != plans {
        return Err("ARC ports: set not prepared yet or divergent".into());
    }
    let mut total = 0;
    for (m, p) in new_set.iter_mut().zip(plans) {
        total += super::ports::certify(m, p, Some(&union_set))?;
    }
    modules.clone_from_slice(&new_set);
    Ok(total)
}

/// Atomically prepares a closed set of Dart modules with the same ARC ABI.
///
/// Calls between modules use the bodies of this transaction. No summary of an
/// already emitted SDK is presumed Owned: the prepared bodies must be the emitted ones.
/// Preserves the distribution of functions, globals and other metadata of each
/// module. Publishes bodies, verified tables and plans only after the success of the
/// whole set. Each shared variant is verified separately,
/// with the same interface and distinct global areas. Failures of any variant
/// propagate to the callers. Does not prove ODR functional equivalence. Shared
/// layouts must coincide. Tracing is an operation without changes.
/// Does not integrate the pipeline nor certify indirect dispatch, callbacks or reload.
///
/// # Errors
/// Incompatible counts/modes, facts assigned to the wrong module, repeated
/// symbols, conflicting layouts or any failure of the ARC preparation.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let leaf = Function { symbol: "leaf".into(), name: "leaf".into(),
///     debug_info: None, params: vec![], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(None) }] };
/// let mut caller = leaf.clone(); caller.symbol = "caller".into();
/// caller.blocks[0].instructions.push((ValueId(0), Instruction::CallStatic {
///     symbol: "leaf".into(), args: vec![], ret_ty: Type::Void }, Type::Void));
/// let mut a = Module::new(); a.arc_memory = true; a.functions.push(caller);
/// let mut b = Module::new(); b.arc_memory = true; b.functions.push(leaf);
/// let mut modules = [a, b];
/// let mut plans = [HashMap::from([("caller".into(), DartFunctionPlan::default())]),
///     HashMap::from([("leaf".into(), DartFunctionPlan::default())])];
/// assert_eq!(prepare_arc_dart_modules(&mut modules, &mut plans)?, (0, 0));
/// # Ok::<(), String>(())
/// ```
pub fn prepare_arc_dart_modules(
    modules: &mut [Module],
    plans: &mut [HashMap<String, DartFunctionPlan>],
) -> Result<(usize, usize), String> {
    if modules.len() != plans.len() {
        return Err("ARC modules: incompatible number of plans".into());
    }
    let Some(first_one) = modules.first() else {
        return Ok((0, 0));
    };
    if modules.iter().any(|m| {
        m.arc_memory != first_one.arc_memory
            || m.table_exceptions != first_one.table_exceptions
    }) {
        return Err("ARC modules: incompatible memory/exception modes".into());
    }
    if !first_one.arc_memory {
        return Ok((0, 0));
    }
    let mut set = Module::new();
    set.arc_memory = true;
    set.table_exceptions = first_one.table_exceptions;
    let mut new_set = HashMap::new();
    let mut definitions: HashMap<String, (usize, usize)> = HashMap::new();
    let mut variants: HashMap<String, Vec<String>> = HashMap::new();
    let mut distribution = Vec::new();
    let mut variant_origins = Vec::new();
    let mut classes = HashMap::<u32, usize>::new();
    let mut used_set: HashSet<_> = modules
        .iter()
        .flat_map(|m| {
            m.functions.iter().map(|f| f.symbol.clone()).chain(
                m.globals
                    .iter()
                    .flat_map(|g| [g.2.clone(), format!("{}$ok", g.2)]),
            )
        })
        .collect();
    let mut next_item = 0u64;
    for (module_index, (m, p)) in modules.iter().zip(plans.iter()).enumerate() {
        // Native entries stay open when inferring the arguments of the group.
        set.helpers.extend(m.helpers.iter().cloned());
        set.method_tables.extend(m.method_tables.iter().cloned());
        let locals: HashSet<_> = m.functions.iter().map(|f| &f.symbol).collect();
        if locals.len() != m.functions.len()
            || locals.len() != p.len()
            || p.keys().any(|s| !locals.contains(s))
        {
            return Err("ARC modules: incompatible local functions/plans".into());
        }
        for s in m
            .dart_scalar_parameters
            .keys()
            .chain(m.dart_rti_parameters.keys())
            .chain(m.dart_vector_parameters.keys())
            .chain(m.dart_ref_returns.iter())
            .chain(m.arc_uniform_returns.iter())
            .chain(m.arc_shared_functions.iter())
            .chain(m.arc_sites.keys())
        {
            if !locals.contains(s) {
                return Err(format!("ARC modules: fact outside the module of {s}"));
            }
        }
        if set.table_exceptions && m.functions.len() != m.tables.len() {
            return Err("ARC modules: incomplete local exceptional inventory".into());
        }
        let mut area = HashMap::new();
        for (root, ty, name) in &m.globals {
            let private = private_name(&mut used_set, &mut next_item, "global")?;
            if area.insert(name.clone(), private.clone()).is_some()
                || area
                    .insert(format!("{name}$ok"), format!("{private}$ok"))
                    .is_some()
            {
                return Err(format!("ARC modules: overlapping local global {name}"));
            }
            set.globals.push((*root, *ty, private));
        }
        for s in &m.arc_rti_caches {
            let name = area
                .get(s)
                .ok_or_else(|| format!("ARC modules: cache outside the module of {s}"))?;
            set.arc_rti_caches.insert(name.clone());
        }
        for s in &m.dart_scalar_globals {
            let name = area
                .get(s)
                .ok_or_else(|| format!("ARC modules: scalar global outside the module of {s}"))?;
            set.dart_scalar_globals.insert(name.clone());
        }
        super::layouts::verify(m)?;
        let mut local_ids = HashSet::new();
        for c in &m.classes {
            if !local_ids.insert(c.id) {
                return Err(format!("ARC modules: repeated local class {}", c.id));
            }
            if let Some(&index) = classes.get(&c.id) {
                let previous = &set.classes[index];
                if previous.name != c.name || previous.field_count != c.field_count {
                    return Err(format!("ARC modules: conflicting class {}", c.id));
                }
            } else {
                classes.insert(c.id, set.classes.len());
                set.classes.push(c.clone());
            }
        }
        for (cid, layout) in &m.arc_field_layouts {
            if set
                .arc_field_layouts
                .get(cid)
                .is_some_and(|l| l != layout)
            {
                return Err(format!("ARC modules: conflicting layout of class {cid}"));
            }
            set.arc_field_layouts.insert(*cid, layout.clone());
        }
        let mut indices = Vec::new();
        for (function_index, original) in m.functions.iter().enumerate() {
            let symbol = &original.symbol;
            let private = if let Some(&(am, af)) = definitions.get(symbol) {
                let previous = &modules[am];
                if !m.sdk_mode
                    || !previous.sdk_mode
                    || !m.arc_shared_functions.contains(symbol)
                    || !previous.arc_shared_functions.contains(symbol)
                {
                    return Err(format!("ARC modules: repeated function {symbol}"));
                }
                if protocol(previous, &previous.functions[af], &plans[am][symbol])
                    != protocol(m, original, &p[symbol])
                {
                    return Err(format!(
                        "ARC modules: divergent shared ABI {symbol}"
                    ));
                }
                let name = private_name(&mut used_set, &mut next_item, "funcao")?;
                variants
                    .entry(symbol.clone())
                    .or_default()
                    .push(name.clone());
                name
            } else {
                definitions.insert(symbol.clone(), (module_index, function_index));
                symbol.clone()
            };
            if private != *symbol {
                variant_origins.push((private.clone(), symbol.clone(), module_index));
            }
            let mut f = original.clone();
            f.symbol = private.clone();
            if let Some(policy) = m.inline_policies.get(symbol) {
                set.inline_policies.insert(private.clone(), *policy);
            }
            remap_area(&mut f, &area)?;
            let mut plan = p[symbol].clone();
            if set.table_exceptions {
                let t = &m.tables[function_index];
                if plan.tables != FunctionTables::default() && plan.tables != *t {
                    return Err(format!(
                        "ARC modules: conflicting local tables {symbol}"
                    ));
                }
                plan.tables = t.clone();
            }

            if let Some(c) = &plan.tables.foreign_cleanup {
                c.check(original, &plan.tables)?;
            }
            plan.tables.foreign_cleanup = ForeignCleanup::new(&f, &plan)?;
            if let Some(facts) = m.dart_scalar_parameters.get(symbol) {
                set
                    .dart_scalar_parameters
                    .insert(private.clone(), facts.clone());
            }
            if let Some(facts) = m.dart_rti_parameters.get(symbol) {
                set
                    .dart_rti_parameters
                    .insert(private.clone(), facts.clone());
            }
            if let Some(facts) = m.dart_vector_parameters.get(symbol) {
                set
                    .dart_vector_parameters
                    .insert(private.clone(), facts.clone());
            }
            if let Some(facts) = m.arc_sites.get(symbol) {
                set.arc_sites.insert(private.clone(), facts.clone());
            }
            if m.dart_ref_returns.contains(symbol) {
                set.dart_ref_returns.insert(private.clone());
            }
            if m.arc_direct_local_functions.contains(symbol) {
                set.arc_direct_local_functions.insert(private.clone());
            }
            if m.arc_uniform_returns.contains(symbol) {
                set.arc_uniform_returns.insert(private.clone());
            }
            new_set.insert(private, plan);
            indices.push(set.functions.len());
            set.functions.push(f);
            if set.table_exceptions {
                set.tables.push(
                    new_set[&set.functions.last().unwrap().symbol]
                        .tables
                        .clone(),
                );
            }
        }
        distribution.push((indices, area));
    }
    if set.table_exceptions {
        produce_module_arc_tables(&set, &mut new_set)?;
    }
    let names_before: Vec<_> = set.functions.iter().map(|f| f.symbol.clone()).collect();
    let result = super::functions::prepare_with_variants(&mut set, &mut new_set, &variants)
        .map_err(|mut error| {
        variant_origins.sort_by_key(|(s, _, _)| std::cmp::Reverse(s.len()));
        for (private, original, module) in &variant_origins {
            if error.contains(private) {
                error = error.replace(private, original);
                error.push_str(&format!(" (variant of module {module})"));
            }
        }
        error
    })?;
    // Restores the physical names and rebinds the certificates still in
    // private copies. Any failure preserves all the input modules.
    let mut physical_names = HashMap::new();
    let mut physical_facts = Vec::new();
    for (m, (indices, _)) in modules.iter().zip(&distribution) {
        let mut map = HashMap::new();
        for (original, &index) in m.functions.iter().zip(indices) {
            let before = &names_before[index];
            let current = &set.functions[index].symbol;
            let physical = if current == before { original.symbol.clone() }
                else if current == &format!("{before}$arcslots_v1") { format!("{}$arcslots_v1", original.symbol) }
                else { return Err("ARC modules: unrecognized signature/symbol change".into()); };
            physical_names.insert(current.clone(), physical.clone());
            map.insert(original.symbol.clone(), physical);
        }
        physical_facts.push(map);
    }
    let mut publications = Vec::new();
    for (module_index, ((m, (indices, area)), facts)) in modules.iter().zip(distribution).zip(physical_facts).enumerate() {
        let inverse: HashMap<_, _> = area.into_iter().map(|(a, b)| (b, a)).collect();
        let mut functions = Vec::new();
        let mut ps = plans[module_index].clone();
        for (original, index) in m.functions.iter().zip(indices) {
            let mut f = set.functions[index].clone();
            let mut plan = new_set[&f.symbol].clone();
            super::slot_plans::remap_function(&mut f, &physical_names);
            remap_area(&mut f, &inverse)?;
            super::slot_plans::carry_contract(&mut plan, &physical_names);
            plan.tables.foreign_cleanup = ForeignCleanup::new(&f, &plan)?;
            ps.remove(&original.symbol);
            ps.insert(f.symbol.clone(), plan);
            functions.push(f);
        }
        publications.push((functions, ps, facts));
    }
    for ((m, p), (fs, ps, facts)) in modules.iter_mut().zip(plans.iter_mut()).zip(publications) {
        m.functions = fs;
        m.arc_prepared = true;
        super::slot_plans::remap_facts(m, &facts);
        if m.table_exceptions {
            m.tables = m
                .functions
                .iter()
                .map(|f| ps[&f.symbol].tables.clone())
                .collect();
        }
        *p = ps;
    }
    Ok(result)
}

fn private_name(
    used_set: &mut HashSet<String>,
    next_item: &mut u64,
    role: &str,
) -> Result<String, String> {
    loop {
        let name = format!("df.arc.private.{role}.{next_item}");
        *next_item = next_item
            .checked_add(1)
            .ok_or("nomes privados ARC esgotados")?;
        if used_set.insert(name.clone()) {
            return Ok(name);
        }
    }
}

/// Parameter representation, scalar/RTI certificates and optional entry vector.
type ParameterProtocol = (Type, bool, bool, Option<DartEntryVector>);
/// Return representation/ownership and the ordered parameter protocol.
type FunctionProtocol = (Type, TokenReturn, Vec<ParameterProtocol>);

// A verified interface, not equality of the body nor a promise of functional ODR.
// SSA IDs are local: the semantic roles belong to the positions of the signature.
fn protocol(
    m: &Module,
    f: &Function,
    p: &DartFunctionPlan,
) -> FunctionProtocol {
    let ret = if m.dart_ref_returns.contains(&f.symbol)
        || m.arc_uniform_returns.contains(&f.symbol)
    {
        TokenReturn::Owned
    } else {
        p.tokens.ret
    };
    let parameters = f
        .params
        .iter()
        .map(|(v, _, t)| {
            (
                *t,
                m.dart_scalar_parameters
                    .get(&f.symbol)
                    .is_some_and(|s| s.contains(v)),
                m.dart_rti_parameters
                    .get(&f.symbol)
                    .is_some_and(|s| s.contains(v)),
                m.dart_vector_parameters
                    .get(&f.symbol)
                    .and_then(|s| s.get(v))
                    .copied(),
            )
        })
        .collect();
    (f.return_ty, ret, parameters)
}

fn remap_area(f: &mut Function, area: &HashMap<String, String>) -> Result<(), String> {
    for b in &f.blocks {
        let mut address = false;
        for (_, i, _) in &b.instructions {
            address |=
                matches!(i, Instruction::Const(Constant::Function(s)) if area.contains_key(s));
            super::super::operands::operands(i, &mut |o| {
                address |=
                    matches!(o, Operand::Constant(Constant::Function(s)) if area.contains_key(s));
            });
        }
        super::super::operands::terminator_operands(&b.terminator, &mut |o| {
            address |= matches!(o, Operand::Constant(Constant::Function(s)) if area.contains_key(s));
        });
        if address {
            return Err(format!("address of the global area published in {}", f.symbol));
        }
    }
    for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::LoadGlobal { symbol, .. } | Instruction::StoreGlobal { symbol, .. }
            | Instruction::ArcLoadStrong { slot: StrongSlot::Global { symbol } }
            | Instruction::ArcStoreStrong { slot: StrongSlot::Global { symbol }, .. } =
            i
        {
            *symbol = area
                .get(symbol)
                .ok_or_else(|| format!("global outside the local area: {symbol}"))?
                .clone();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_module_shared_slots_publish_abi_and_contracts_together() {
        let mut origin = super::super::slot_lives::tests::origin();
        origin.functions.pop();
        origin.functions[1].blocks[0].instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        let leaf = origin.functions.pop().unwrap();
        let mut library = Module::new();
        library.arc_memory = true;
        library.functions.push(leaf);
        let mut ms = [origin, library];
        let mut ps: Vec<HashMap<String, DartFunctionPlan>> = ms.iter().map(|m| {
            m.functions.iter().map(|f| (f.symbol.clone(), DartFunctionPlan::default())).collect()
        }).collect();
        prepare_arc_dart_modules(&mut ms, &mut ps).unwrap();
        assert_eq!(ms[1].functions[0].symbol, "a$arcslots_v1");
        assert!(!ps[1].contains_key("a"));
        assert_eq!(ms[1].functions[0].params.len(), 4);
        assert!(ms[0].functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| {
            matches!(i, Instruction::CallStatic { symbol, args, .. }
                if symbol == "a$arcslots_v1" && args.len() == 4)
        }));
        for (m, plans) in ms.iter().zip(&ps) {
            for f in &m.functions {
                let p = &plans[&f.symbol];
                assert!(p.tokens.shared_frames.is_some());
                verify_dart_function_contract(f, &p.classes, &p.tokens, &p.tables, &p.scopes).unwrap();
            }
        }
    }

    #[test]
    fn fallible_variant_propagates_cleanup_to_public_symbol_and_caller() {
        for resume in [false, true] {
            let (mut ms, mut ps) = entry();
            let mut copy_inst = Module::new();
            copy_inst.arc_memory = true;
            copy_inst.functions.push(ms[1].functions[0].clone());
            copy_inst.dart_ref_returns.insert("biblioteca".into());
            ms.push(copy_inst);
            ps.push(ps[1].clone());
            for m in &mut ms {
                m.sdk_mode = true;
            }
            for m in &mut ms[1..] {
                m.arc_shared_functions.insert("biblioteca".into());
            }
            // Only the second definition checks the stack in the prologue. The linker
            // may choose it; the caller cannot presume the first, pure one.
            ps[2].get_mut("biblioteca").unwrap().tables.checks_stack = true;
            // Exercises both exit shapes, regardless of the CI host.
            // The call still depends on the union of the failures of the variants.
            let f = &mut ms[0].functions[0];
            f.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            };
            for id in [1, 2] {
                f.blocks.push(BasicBlock {
                    id: BlockId(id),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                });
            }
            let t = &mut ps[0].get_mut("programa").unwrap().tables;
            t.invocations.insert(ValueId(1), BlockId(1));
            t.landing_pads.insert(BlockId(1));
            t.exits.insert(
                BlockId(1),
                if resume {
                    ExceptionExit::Resume
                } else {
                    ExceptionExit::Throws
                },
            );
            prepare_arc_dart_modules(&mut ms, &mut ps).unwrap();
            let p = &ps[0]["programa"];
            assert!(p.tokens.instructions[&ValueId(1)].may_fail);
            let landing_pad = p.tables.invocations[&ValueId(1)];
            assert_eq!(p.owners_at_landing_pad[&landing_pad], vec![ValueId(0)]);
            if p.tables.exits.get(&landing_pad) == Some(&ExceptionExit::Resume) {
                // Itanium runs the explicit cleanup before the resume. The additional
                // certificate only serves landing pads that the emitter still needs to clean.
                assert!(p.tables.foreign_cleanup.is_none());
                let block = ms[0].functions[0]
                    .blocks
                    .iter()
                    .find(|b| b.id == landing_pad)
                    .unwrap();
                assert!(block.instructions.iter().any(|(_, i, _)| matches!(
                    i,
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(0))
                    }
                )));
            } else {
                p.tables
                    .foreign_cleanup
                    .as_ref()
                    .unwrap()
                    .check(&ms[0].functions[0], &p.tables)
                    .unwrap();
            }
            assert!(
                ms[1].functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
            assert!(
                ms[2].functions[0]
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
            );
        }
    }

    #[test]
    fn equal_caches_in_distinct_areas_preserve_bodies_and_physical_names() {
        let mut ms = Vec::new();
        for recipe in ["C201", "C350"] {
            let mut m = Module::new();
            m.arc_memory = true;
            m.sdk_mode = true;
            m.globals.push((0, Type::I64, "cache".into()));
            m.arc_rti_caches.insert("cache".into());
            m.arc_shared_functions.insert("getter".into());
            m.functions.push(Function {
                symbol: "getter".into(),
                name: "getter".into(),
                params: vec![],
                return_ty: Type::I64,
                debug_info: None,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(0),
                            Instruction::Const(Constant::String(recipe.into())),
                            Type::Ref,
                        ),
                        (
                            ValueId(1),
                            Instruction::CallRuntime {
                                name: "dartforge_rti_recipe".into(),
                                args: vec![(Operand::Val(ValueId(0)), Type::Ref)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(2),
                            Instruction::StoreGlobal {
                                symbol: "cache".into(),
                                val: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                                root: None,
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::LoadGlobal {
                                symbol: "cache".into(),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
                }],
            });
            ms.push(m);
        }
        let mut ps = vec![HashMap::from([("getter".into(), DartFunctionPlan::default())]); 2];
        let originals: Vec<_> = ms.iter().map(|m| format!("{m:?}")).collect();
        prepare_arc_dart_modules(&mut ms, &mut ps).unwrap();
        // This proof is about ownership/ABI, not about the equality of ODR values.
        // Apart from the published preparation mark, nothing changes.
        for (m, original) in ms.iter().zip(originals) {
            assert!(m.arc_prepared);
            let mut without_mark = m.clone();
            without_mark.arc_prepared = false;
            assert_eq!(format!("{without_mark:?}"), original);
        }
        for p in &ps {
            assert_eq!(p["getter"].classes[&ValueId(3)], Ownership::Trivial);
        }
        ms[1].functions[0].blocks[0].instructions[2].1 = Instruction::StoreGlobal {
            symbol: "cache".into(),
            val: Operand::Val(ValueId(0)),
            ty: Type::I64,
            root: None,
        };
        let before = format!("{ms:?}/{ps:?}");
        assert!(prepare_arc_dart_modules(&mut ms, &mut ps).is_err());
        assert_eq!(format!("{ms:?}/{ps:?}"), before);
    }

    #[test]
    fn sharing_requires_compatible_declaration_body_contracts_and_dependencies() {
        for case in 0..11 {
            let (mut ms, mut ps) = entry();
            let mut copy_inst = Module::new();
            copy_inst.arc_memory = true;
            copy_inst.functions.push(ms[1].functions[0].clone());
            copy_inst.dart_ref_returns.insert("biblioteca".into());
            ms.push(copy_inst);
            ps.push(ps[1].clone());
            for m in &mut ms {
                m.sdk_mode = true;
                if m.functions[0].symbol == "biblioteca" {
                    m.arc_shared_functions.insert("biblioteca".into());
                }
            }
            match case {
                1 => ms[2].arc_shared_functions.clear(),
                2 => ms[1].sdk_mode = false,
                3 => {
                    ms[2].functions[0].blocks[0].terminator =
                        Terminator::Return(Some(Operand::Constant(Constant::Int(42))))
                }
                4 => ps[2].get_mut("biblioteca").unwrap().tokens.ret = TokenReturn::Borrowed,
                5 => {
                    for m in &mut ms[1..] {
                        m.functions[0].blocks[0].instructions.push((
                            ValueId(1),
                            Instruction::LoadGlobal {
                                symbol: "cache".into(),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ));
                    }
                }
                6 => {
                    for m in &mut ms {
                        m.arc_memory = false;
                        m.arc_shared_functions.insert("obsoleto".into());
                    }
                }
                7 => {
                    ms[2].arc_shared_functions.insert("obsoleto".into());
                }
                8 => ms[2].dart_ref_returns.clear(),
                9 => {
                    ps[2]
                        .get_mut("biblioteca")
                        .unwrap()
                        .classes
                        .insert(ValueId(0), Ownership::Trivial);
                }
                10 => {
                    for m in &mut ms {
                        m.table_exceptions = true;
                        m.tables.push(FunctionTables::default());
                    }
                }
                _ => {}
            }
            let before = format!("{ms:?}/{ps:?}");
            let r = prepare_arc_dart_modules(&mut ms, &mut ps);
            if matches!(case, 0 | 6 | 10) {
                assert_eq!(r.unwrap(), if case == 6 { (0, 0) } else { (2, 2) });
                if case == 6 {
                    assert_eq!(format!("{ms:?}/{ps:?}"), before);
                    continue;
                }
                assert_eq!(
                    format!("{:?}", ms[1].functions),
                    format!("{:?}", ms[2].functions)
                );
                assert_eq!(ps[1], ps[2]);
                let after = format!("{ms:?}/{ps:?}");
                assert_eq!(prepare_arc_dart_modules(&mut ms, &mut ps).unwrap(), (0, 0));
                assert_eq!(format!("{ms:?}/{ps:?}"), after);
            } else {
                assert!(r.is_err(), "caso {case}: {r:?}");
                assert_eq!(format!("{ms:?}/{ps:?}"), before, "caso {case}");
            }
        }
    }

    #[test]
    fn arc_link_uses_declaration_instead_of_symbol_name() {
        for arc in [false, true] {
            let mut m = Module::new();
            m.sdk_mode = true;
            m.arc_memory = arc;
            for s in ["dfc.nome_nao_certifica", "nome_sem_prefixo"] {
                m.functions.push(Function {
                    symbol: s.into(),
                    name: s.into(),
                    params: vec![],
                    return_ty: Type::Void,
                    debug_info: None,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions: vec![],
                        terminator: Terminator::Return(None),
                    }],
                });
            }
            m.arc_shared_functions
                .insert("nome_sem_prefixo".into());
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert_eq!(
                ir.contains("define linkonce_odr void @nome_sem_prefixo("),
                arc,
                "{ir}"
            );
            assert_eq!(
                ir.contains("define linkonce_odr void @dfc.nome_nao_certifica("),
                !arc,
                "{ir}"
            );
        }
    }

    #[test]
    fn uniform_entry_forwards_descriptor_between_modules_without_publishing_it() {
        for invalid in [false, true] {
            let mut modules = Vec::new();
            for symbol in ["caller", "callee"] {
                let mut m = Module::new();
                m.arc_memory = true;
                let params = vec![
                    (ValueId(0), "ctx".into(), Type::Ref),
                    (ValueId(1), "args".into(), Type::Ptr),
                    (ValueId(2), "desc".into(), Type::Ptr),
                ];
                let (instructions, value) = if symbol == "caller" {
                    (
                        vec![(
                            ValueId(3),
                            Instruction::CallStatic {
                                symbol: "callee".into(),
                                args: vec![
                                    Operand::Val(ValueId(0)),
                                    Operand::Val(ValueId(1)),
                                    Operand::Val(ValueId(2)),
                                ],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        )],
                        ValueId(3),
                    )
                } else {
                    (vec![], ValueId(0))
                };
                m.functions.push(Function {
                    symbol: symbol.into(),
                    name: symbol.into(),
                    params,
                    return_ty: Type::Ref,
                    debug_info: None,
                    blocks: vec![BasicBlock {
                        id: BlockId(0),
                        instructions,
                        terminator: Terminator::Return(Some(Operand::Val(value))),
                    }],
                });
                m.dart_vector_parameters.insert(
                    symbol.into(),
                    HashMap::from([
                        (ValueId(1), DartEntryVector::Arguments),
                        (ValueId(2), DartEntryVector::Descriptor),
                    ]),
                );
                m.arc_uniform_returns.insert(symbol.into());
                modules.push(m);
            }
            let mut ms = modules;
            if invalid {
                ms[1].dart_vector_parameters.clear();
            }
            let mut plans: Vec<HashMap<_, _>> = ms
                .iter()
                .map(|m| {
                    m.functions
                        .iter()
                        .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
                        .collect()
                })
                .collect();
            let before = format!("{ms:?}/{plans:?}");
            let r = prepare_arc_dart_modules(&mut ms, &mut plans);
            if invalid {
                assert!(r.is_err());
                assert_eq!(format!("{ms:?}/{plans:?}"), before);
            } else {
                assert_eq!(r.unwrap(), (1, 0));
                assert_eq!(plans[0]["caller"].classes[&ValueId(3)], Ownership::Owned);
                assert_eq!(plans[0]["caller"].classes[&ValueId(2)], Ownership::Trivial);
            }
        }
    }

    fn entry() -> (Vec<Module>, Vec<HashMap<String, DartFunctionPlan>>) {
        let mut program = Module::new();
        program.arc_memory = true;
        program.functions.push(Function {
            symbol: "programa".into(),
            name: "programa".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::Box {
                            op: Operand::Constant(Constant::Int(i64::MAX)),
                            from: Type::I64,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::CallStatic {
                            symbol: "biblioteca".into(),
                            args: vec![Operand::Val(ValueId(0))],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        let mut library = Module::new();
        library.arc_memory = true;
        library.functions.push(Function {
            symbol: "biblioteca".into(),
            name: "biblioteca".into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        library.dart_ref_returns.insert("biblioteca".into());
        let modules = vec![program, library];
        let plans = modules
            .iter()
            .map(|m| {
                m.functions
                    .iter()
                    .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
                    .collect()
            })
            .collect();
        (modules, plans)
    }

    #[test]
    fn cross_module_calls_require_joint_verification_and_atomic_publication() {
        for case in 0..20 {
            let (mut modules, mut plans) = entry();
            match case {
                1 => modules[1].dart_ref_returns.clear(),
                2 => {
                    modules.pop();
                    plans.pop();
                }
                3 => modules[1].functions[0].params[0].2 = Type::I64,
                4 => modules[1].functions[0].symbol = "programa".into(),
                5 => {
                    modules[0]
                        .dart_rti_parameters
                        .insert("biblioteca".into(), HashSet::from([ValueId(0)]));
                }
                6 | 7 => {
                    for m in &mut modules {
                        m.classes.push(ClassDef {
                            id: 42,
                            name: "C".into(),
                            field_count: 1,
                            vtable: vec![],
                            to_string_symbol: None,
                        });
                    }
                    modules[0].arc_field_layouts.insert(42, vec![Type::Ref]);
                    modules[1]
                        .arc_field_layouts
                        .insert(42, vec![if case == 6 { Type::I64 } else { Type::Ref }]);
                }
                8 => {
                    for m in &mut modules {
                        m.globals.push((0, Type::I64, "sobreposto".into()));
                    }
                }
                9 => modules[1].arc_memory = false,
                10 => {
                    for m in &mut modules {
                        m.arc_memory = false;
                        m.arc_uniform_returns.insert("obsoleto".into());
                    }
                }
                11 => {
                    modules[1].functions[0].blocks[0].terminator =
                        Terminator::Return(Some(Operand::Constant(Constant::Int(42))))
                }
                12 => modules[1].table_exceptions = true,
                13 | 14 => {
                    for m in &mut modules {
                        m.table_exceptions = true;
                        m.tables.push(FunctionTables::default());
                    }
                    if case == 14 {
                        modules[1].tables.clear();
                    }
                }
                15 => {
                    modules.reverse();
                    plans.reverse();
                }
                16 => {
                    let mut empty = Module::new();
                    empty.arc_memory = true;
                    modules.push(empty);
                    plans.push(HashMap::new());
                }
                17 => {
                    let shifted = plans[1].remove("biblioteca").unwrap();
                    plans[0].insert("biblioteca".into(), shifted);
                }
                18 => {
                    plans[1].get_mut("biblioteca").unwrap().tokens.ret =
                        TokenReturn::Borrowed
                }
                19 => {
                    modules[0].arc_rti_caches.insert("da_biblioteca".into());
                    modules[1]
                        .globals
                        .push((0, Type::I64, "da_biblioteca".into()));
                }
                _ => {}
            }
            let before = format!("{modules:?}/{plans:?}");
            let result = prepare_arc_dart_modules(&mut modules, &mut plans);
            if matches!(case, 0 | 7 | 8 | 10 | 13 | 15 | 16) {
                if case == 10 {
                    assert_eq!(result.unwrap(), (0, 0));
                    assert_eq!(format!("{modules:?}/{plans:?}"), before);
                    continue;
                }
                assert_eq!(
                    result.unwrap_or_else(|e| panic!("caso {case}: {e}")),
                    (1, 2)
                );
                let library = modules
                    .iter()
                    .find(|m| m.functions.iter().any(|f| f.symbol == "biblioteca"))
                    .unwrap();
                assert!(
                    library.functions[0]
                        .blocks
                        .iter()
                        .flat_map(|b| &b.instructions)
                        .any(|(_, i, _)| matches!(i, Instruction::ArcCopy { .. }))
                );
                let program = modules
                    .iter()
                    .position(|m| m.functions.iter().any(|f| f.symbol == "programa"))
                    .unwrap();
                assert_eq!(
                    plans[program]["programa"].classes[&ValueId(1)],
                    Ownership::Owned
                );
                if case == 13 {
                    for (m, p) in modules.iter().zip(&plans) {
                        assert_eq!(
                            m.tables,
                            m.functions
                                .iter()
                                .map(|f| p[&f.symbol].tables.clone())
                                .collect::<Vec<_>>()
                        );
                    }
                }
                let after = format!("{modules:?}/{plans:?}");
                assert_eq!(
                    prepare_arc_dart_modules(&mut modules, &mut plans).unwrap(),
                    (0, 0)
                );
                assert_eq!(format!("{modules:?}/{plans:?}"), after, "caso {case}");
            } else {
                assert!(result.is_err(), "caso {case}: {result:?}");
                assert_eq!(format!("{modules:?}/{plans:?}"), before, "caso {case}");
            }
        }
        let (mut modules, mut plans) = entry();
        plans.pop();
        let before = format!("{modules:?}/{plans:?}");
        assert!(prepare_arc_dart_modules(&mut modules, &mut plans).is_err());
        assert_eq!(format!("{modules:?}/{plans:?}"), before);
        assert_eq!(prepare_arc_dart_modules(&mut [], &mut []).unwrap(), (0, 0));
    }
}
