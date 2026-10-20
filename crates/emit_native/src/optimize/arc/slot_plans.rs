//! Native roles of the shared frames, derived from a verified group.
//! The contract seals the signature and the forwardings; Ref mutations require revalidating the CFG.

use super::*;
use std::sync::Arc;

#[derive(Debug)]
struct Group {
    originals: Vec<Function>,
    functions: Vec<Function>,
    borrowed_set: HashMap<String, HashMap<ValueId, ValueId>>,
    locals: HashMap<String, (ValueId, usize)>,
    symbols: HashMap<String, String>,
    identity: String,
}

/// Contract of the native roles of a closed family of Ref captures.
///
/// Created only after checking origins, every caller and the explicit lifetime
/// of the frames. It is neither a Ref owner nor an unwind certificate. Private fields
/// prevent forging borrows from arbitrary i64 parameters.
/// The signature and the forwardings must stay the same; other changes
/// still go through the current check of types, escape, CFG and cleanup.
///
/// ```
/// use dartforge_emit_native::optimize::arc::TokenPlan;
/// assert!(TokenPlan::default().shared_frames.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct SharedFramesCfgContract {
    group: Arc<Group>,
    symbol: String,
    internal: String,
    names: HashMap<String, String>,
    signature: Vec<(ValueId, Type)>,
    ret: Type,
    forwards: Vec<(ValueId, String, Vec<Operand>)>,
}

impl PartialEq for SharedFramesCfgContract {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.internal == other.internal
            && self.names == other.names
            && self.signature == other.signature
            && self.ret == other.ret
            && self.forwards == other.forwards
            && (Arc::ptr_eq(&self.group, &other.group)
                || (self.group.identity == other.group.identity
                    && self.group.borrowed_set == other.group.borrowed_set
                    && self.group.locals == other.group.locals
                    && self.group.symbols == other.group.symbols))
    }
}
impl Eq for SharedFramesCfgContract {}

fn forwards(f: &Function, group: &Group) -> Vec<(ValueId, String, Vec<Operand>)> {
    let mut found_set: Vec<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, i, _)| {
            let Instruction::CallStatic { symbol, args, .. } = i else {
                return None;
            };
            let pairs = group.borrowed_set.get(symbol)?;
            let callee = group.functions.iter().find(|f| f.symbol == *symbol)?;
            let mut positions: Vec<_> = callee
                .params
                .iter()
                .enumerate()
                .filter_map(|(p, (q, _, _))| {
                    pairs.get(q).map(|i| {
                        (
                            p,
                            callee.params.iter().position(|(v, _, _)| v == i).unwrap(),
                        )
                    })
                })
                .collect();
            positions.sort_unstable();
            let values = positions
                .into_iter()
                .flat_map(|(q, i)| [args.get(q).cloned(), args.get(i).cloned()])
                .collect::<Option<Vec<_>>>()?;
            Some((*v, symbol.clone(), values))
        })
        .collect();
    found_set.sort_by_key(|(v, _, _)| v.0);
    found_set
}

impl SharedFramesCfgContract {
    pub(super) fn check(&self, f: &Function) -> Result<(), String> {
        let mut normalized = f.clone();
        normalized.symbol = self.internal.clone();
        let name = |s: &mut String| {
            if let Some((internal, _)) = self.names.iter().find(|(_, external)| *external == s) {
                *s = internal.clone();
            }
        };
        for b in &mut normalized.blocks {
            for (v, inst, _) in &mut b.instructions {
                match inst {
                    Instruction::CallStatic { symbol, .. } => {
                        if let Some((_, internal, _)) =
                            self.forwards.iter().find(|(id, _, _)| id == v)
                        {
                            if self.names.get(internal) == Some(symbol) {
                                *symbol = internal.clone();
                            } else {
                                name(symbol);
                            }
                        } else {
                            name(symbol);
                        }
                    }
                    Instruction::Const(Constant::Function(s))
                    | Instruction::TearOff { code_symbol: s }
                    | Instruction::AllocClosure { code_symbol: s, .. } => name(s),
                    Instruction::AllocClosureTyped {
                        code_symbol, typed, ..
                    } => {
                        name(code_symbol);
                        name(typed);
                    }
                    Instruction::FunctionTable(ss) => {
                        for s in ss {
                            name(s);
                        }
                    }
                    _ => {}
                }
                super::super::operands::operands_mut(inst, &mut |o| {
                    if let Operand::Constant(Constant::Function(s)) = o {
                        name(s);
                    }
                });
            }
            super::super::operands::terminator_operands_mut(&mut b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    name(s);
                }
            });
        }
        if f.symbol != self.symbol
            || f.return_ty != self.ret
            || f.params
                .iter()
                .map(|(v, _, t)| (*v, *t))
                .collect::<Vec<_>>()
                != self.signature
            || forwards(&normalized, &self.group) != self.forwards
        {
            return Err(format!(
                "shared frames: stale signature/forwardings of {}",
                f.symbol
            ));
        }
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = self.group.originals.clone();
        let mut r = ArcRefSlotsDraft {
            functions: self.group.functions.clone(),
            borrowed_set: self.group.borrowed_set.clone(),
            locals: self.group.locals.clone(),
            symbols: self.group.symbols.clone(),
        };
        let current = r
            .functions
            .iter_mut()
            .find(|c| c.symbol == self.internal)
            .ok_or("shared frames: body outside the family")?;
        *current = normalized;
        verify_cfg_ref_slot_lives(&m, &r)
    }

    pub(super) fn pairs(&self) -> &HashMap<ValueId, ValueId> {
        static EMPTY: std::sync::LazyLock<HashMap<ValueId, ValueId>> =
            std::sync::LazyLock::new(HashMap::new);
        self.group
            .borrowed_set
            .get(&self.internal)
            .unwrap_or(&EMPTY)
    }

    pub(super) fn targets(&self) -> HashMap<String, Vec<(usize, usize)>> {
        self.group
            .borrowed_set
            .iter()
            .map(|(s, ps)| {
                let f = self
                    .group
                    .functions
                    .iter()
                    .find(|f| f.symbol == *s)
                    .unwrap();
                let positions: HashMap<_, _> = f
                    .params
                    .iter()
                    .enumerate()
                    .map(|(p, (v, _, _))| (*v, p))
                    .collect();
                let mut order: Vec<_> = ps
                    .iter()
                    .map(|(q, i)| (positions[q], positions[i]))
                    .collect();
                order.sort_unstable();
                (
                    self.names.get(s).cloned().unwrap_or_else(|| s.clone()),
                    order,
                )
            })
            .collect()
    }

    pub(super) fn remap_names(&self, map: &HashMap<String, String>) -> Self {
        let mut new = self.clone();
        if let Some(s) = map.get(&new.symbol) {
            new.symbol = s.clone();
        }
        for s in new.names.values_mut() {
            if let Some(n) = map.get(s) {
                *s = n.clone();
            }
        }
        new
    }
}

/// Atomically produces native roles and frame contracts in the plans.
///
/// Requires the intermediate bodies and their plans by new symbol. Classifies
/// only the imported frame/index as Trivial, without recording them as Dart
/// scalars. Local frames are still produced by the audited ABI.
/// The contract allows revalidating the current CFG after inserting Ref operations;
/// it does not allow changing the signature, the forwardings or publishing an address of the family.
/// Publishes no bodies, module tables nor a foreign-unwind certificate.
///
/// # Errors
/// Group failure, missing/stale plan or conflicting class. No
/// classification or contract is published on error.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.arc_memory = true;
/// let r = lower_shared_ref_slots(&m)?;
/// assert_eq!(produce_cfg_ref_slot_plans(&m, &r, &mut HashMap::new())?, 0);
/// # Ok::<(), String>(())
/// ```
pub fn produce_cfg_ref_slot_plans(
    m: &Module,
    r: &ArcRefSlotsDraft,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<usize, String> {
    verify_cfg_ref_slot_lives(m, r)?;
    let names: HashSet<_> = r.functions.iter().map(|f| f.symbol.as_str()).collect();
    if plans.len() != names.len() || plans.keys().any(|s| !names.contains(s.as_str())) {
        return Err("shared frames: incompatible plans inventory".into());
    }
    if r.borrowed_set.is_empty() {
        return Ok(0);
    }
    let members: HashSet<_> = r
        .borrowed_set
        .keys()
        .chain(r.locals.keys())
        .cloned()
        .collect();
    let originals: Vec<_> = m
        .functions
        .iter()
        .filter(|f| members.contains(r.symbols.get(&f.symbol).unwrap_or(&f.symbol)))
        .cloned()
        .collect();
    let functions: Vec<_> = r
        .functions
        .iter()
        .filter(|f| members.contains(&f.symbol))
        .cloned()
        .collect();
    let identity = format!("{originals:?}");
    let group = Arc::new(Group {
        originals,
        functions,
        borrowed_set: r.borrowed_set.clone(),
        locals: r.locals.clone(),
        symbols: r.symbols.clone(),
        identity,
    });
    let mut new_set = plans.clone();
    let mut total = 0;
    for f in &group.functions {
        let contract = SharedFramesCfgContract {
            group: group.clone(),
            symbol: f.symbol.clone(),
            internal: f.symbol.clone(),
            names: group
                .functions
                .iter()
                .map(|f| (f.symbol.clone(), f.symbol.clone()))
                .collect(),
            signature: f.params.iter().map(|(v, _, t)| (*v, *t)).collect(),
            ret: f.return_ty,
            forwards: forwards(f, &group),
        };
        contract.check(f)?;
        let p = new_set
            .get_mut(&f.symbol)
            .ok_or("shared frames: missing plan")?;
        for v in contract.pairs().iter().flat_map(|(q, i)| [q, i]) {
            if p.classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                return Err(format!(
                    "shared frames: conflicting class in {} v{}",
                    f.symbol, v.0
                ));
            }
            if p.classes.insert(*v, Ownership::Trivial).is_none() {
                total += 1;
            }
        }
        p.tables.shared_frames = Some(contract.clone());
        p.tokens.shared_frames = Some(contract);
    }
    *plans = new_set;
    Ok(total)
}

pub(super) fn remap_plans(
    plans: &mut HashMap<String, DartFunctionPlan>,
    map: &HashMap<String, String>,
) {
    if map.is_empty() {
        return;
    }
    *plans = std::mem::take(plans)
        .into_iter()
        .map(|(s, p)| (map.get(&s).cloned().unwrap_or(s), p))
        .collect();
}

pub(super) fn remap_facts(m: &mut Module, map: &HashMap<String, String>) {
    if map.is_empty() {
        return;
    }
    fn keys<T>(xs: &mut HashMap<String, T>, map: &HashMap<String, String>) {
        *xs = std::mem::take(xs)
            .into_iter()
            .map(|(s, v)| (map.get(&s).cloned().unwrap_or(s), v))
            .collect();
    }
    fn names(xs: &mut HashSet<String>, map: &HashMap<String, String>) {
        *xs = std::mem::take(xs)
            .into_iter()
            .map(|s| map.get(&s).cloned().unwrap_or(s))
            .collect();
    }
    keys(&mut m.inline_policies, map);
    keys(&mut m.dart_scalar_parameters, map);
    keys(&mut m.dart_rti_parameters, map);
    keys(&mut m.dart_vector_parameters, map);
    keys(&mut m.arc_sites, map);
    names(&mut m.dart_ref_returns, map);
    names(&mut m.arc_uniform_returns, map);
    names(&mut m.arc_shared_functions, map);
    names(&mut m.arc_direct_local_functions, map);
    if !map.is_empty() {
        m.arc_aot_ports = None;
    }
}

pub(super) fn carry_contract(p: &mut DartFunctionPlan, map: &HashMap<String, String>) {
    if let Some(c) = &p.tokens.shared_frames {
        let new = c.remap_names(map);
        p.tables.shared_frames = Some(new.clone());
        p.tokens.shared_frames = Some(new);
    }
}

pub(super) fn remap_function(f: &mut Function, map: &HashMap<String, String>) {
    let name = |s: &mut String| {
        if let Some(n) = map.get(s) {
            *s = n.clone();
        }
    };
    name(&mut f.symbol);
    for b in &mut f.blocks {
        for (_, i, _) in &mut b.instructions {
            match i {
                Instruction::CallStatic { symbol, .. } => name(symbol),
                Instruction::Const(Constant::Function(s))
                | Instruction::TearOff { code_symbol: s }
                | Instruction::AllocClosure { code_symbol: s, .. } => name(s),
                Instruction::AllocClosureTyped {
                    code_symbol, typed, ..
                } => {
                    name(code_symbol);
                    name(typed);
                }
                Instruction::FunctionTable(ss) => {
                    for s in ss {
                        name(s);
                    }
                }
                _ => {}
            }
            super::super::operands::operands_mut(i, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    name(s);
                }
            });
        }
        super::super::operands::terminator_operands_mut(&mut b.terminator, &mut |o| {
            if let Operand::Constant(Constant::Function(s)) = o {
                name(s);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepare() -> (Module, ArcRefSlotsDraft, HashMap<String, DartFunctionPlan>) {
        let m = super::super::slot_lives::tests::origin();
        let r = lower_shared_ref_slots(&m).unwrap();
        let p = r
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
            .collect();
        (m, r, p)
    }

    #[test]
    fn native_roles_are_produced_without_dart_scalar_facts() {
        let (m, r, mut ps) = prepare();
        assert_eq!(produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap(), 8);
        let previous_ones = ps.clone();
        assert_eq!(produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap(), 0);
        assert_eq!(ps, previous_ones);
        for f in &r.functions {
            let p = &ps[&f.symbol];
            super::super::frames::verify_with_plan(f, &p.tokens).unwrap();
            assert!(p.tokens.shared_frames.is_some());
            assert_eq!(p.tokens.shared_frames, p.tables.shared_frames);
        }
        assert!(m.dart_scalar_parameters.is_empty());
    }

    #[test]
    fn conflicts_and_stale_facts_do_not_publish_part_of_group() {
        for case in 0..3 {
            let (m, mut r, mut ps) = prepare();
            let s = r.symbols["a"].clone();
            match case {
                0 => {
                    ps.get_mut(&s)
                        .unwrap()
                        .classes
                        .insert(ValueId(0), Ownership::Owned);
                }
                1 => {
                    ps.remove(&s);
                }
                _ => {
                    r.locals.get_mut("raiz").unwrap().1 = 1;
                }
            }
            let before = ps.clone();
            assert!(produce_cfg_ref_slot_plans(&m, &r, &mut ps).is_err());
            assert_eq!(ps, before, "caso {case}");
        }
    }

    #[test]
    fn contract_admits_ref_operations_but_does_not_change_native_ports() {
        let (m, r, mut ps) = prepare();
        produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap();
        let mut a = r.functions[1].clone();
        let c = ps[&a.symbol].tokens.shared_frames.as_ref().unwrap();
        a.blocks[0].instructions.insert(
            1,
            (
                ValueId(100),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(2)),
                },
                Type::Ref,
            ),
        );
        a.blocks[0].instructions.push((
            ValueId(101),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(100)),
            },
            Type::Void,
        ));
        c.check(&a).unwrap();
        let Instruction::CallStatic { args, .. } = &mut a.blocks[0]
            .instructions
            .iter_mut()
            .find(|(_, i, _)| matches!(i, Instruction::CallStatic { .. }))
            .unwrap()
            .1
        else {
            unreachable!()
        };
        args[1] = args[3].clone();
        assert!(c.check(&a).unwrap_err().contains("stale"));
    }

    #[test]
    fn token_production_checks_borrowed_frames_and_rejects_divergent_markers() {
        let (m, r, mut ps) = prepare();
        produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap();
        let f = &r.functions[1];
        let p = ps.get_mut(&f.symbol).unwrap();
        // The ordinary CallStatic instruction needs the call producer of
        // the set. Here we check the leaf that only reads/copies the slot.
        let mut leaf = f.clone();
        leaf.blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        assert!(
            produce_and_verify_dart_tokens(
                &leaf,
                &mut p.classes,
                &mut p.tokens,
                &p.tables,
                &p.scopes
            )
            .is_err()
        );
        assert!(
            verify_tokens(f, &p.classes, &FunctionTables::default(), &p.tokens)
                .unwrap_err()
                .contains("diverge")
        );
    }

    #[test]
    fn leaf_produces_ref_tokens_with_certified_native_pairs() {
        let mut m = super::super::slot_lives::tests::origin();
        m.functions.pop();
        m.functions[1].blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        let r = lower_shared_ref_slots(&m).unwrap();
        let mut ps = r
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
            .collect();
        produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap();
        let mut leaf = r.functions[1].clone();
        leaf.blocks[0].instructions.push((
            ValueId(100),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(2)),
            },
            Type::Void,
        ));
        let p = ps.get_mut(&leaf.symbol).unwrap();
        produce_and_verify_dart_tokens(&leaf, &mut p.classes, &mut p.tokens, &p.tables, &p.scopes)
            .unwrap();
        assert_eq!(p.classes[&ValueId(2)], Ownership::Owned);
        assert!(p.tables.foreign_cleanup.is_none());
    }

    #[test]
    fn set_prepares_bodies_with_strong_slots_without_reopening_signatures() {
        let mut m = super::super::slot_lives::tests::origin();
        m.functions.pop();
        m.functions[1].blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        let r = lower_shared_ref_slots(&m).unwrap();
        let mut ps = r
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
            .collect();
        produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap();
        let mut fs = r.functions.clone();
        prepare_arc_dart_functions(&mut fs, &mut ps).unwrap();
        for f in &fs {
            let p = &ps[&f.symbol];
            verify_dart_function_contract(f, &p.classes, &p.tokens, &p.tables, &p.scopes).unwrap();
        }
    }

    #[test]
    fn resume_and_cleanup_preserve_caller_frame() {
        for resume in [false, true] {
            let (m, r, mut ps) = prepare();
            produce_cfg_ref_slot_plans(&m, &r, &mut ps).unwrap();
            let mut a = r.functions[1].clone();
            a.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            };
            for id in [1, 2] {
                a.blocks.push(BasicBlock {
                    id: BlockId(id),
                    instructions: vec![(
                        ValueId(100 + id),
                        Instruction::ArcDrop {
                            value: Operand::Val(ValueId(2)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                });
            }
            let p = ps.get_mut(&a.symbol).unwrap();
            p.classes.insert(ValueId(4), Ownership::Trivial);
            p.tokens.instructions.insert(
                ValueId(4),
                TokenEffect {
                    may_fail: true,
                    ..Default::default()
                },
            );
            p.tables.invocations.insert(ValueId(4), BlockId(1));
            p.tables.landing_pads.insert(BlockId(1));
            if resume {
                p.tables.exits.insert(BlockId(1), ExceptionExit::Resume);
            }
            produce_and_verify_dart_tokens(&a, &mut p.classes, &mut p.tokens, &p.tables, &p.scopes)
                .unwrap();
            super::super::check_resumes(&a, &p.tables).unwrap();
            let frames = super::super::frames::at_landing_pad_entry(&a, &p.tables).unwrap();
            assert!(
                frames[&BlockId(1)].is_empty(),
                "borrow became a cleanup owner"
            );
            p.owners_at_landing_pad =
                tokens_at_landing_pad_entry(&a, &p.classes, &p.tables, &p.tokens).unwrap();
            let certificate = ForeignCleanup::new(&a, p).unwrap();
            if resume {
                assert!(certificate.is_none());
            } else {
                let c = certificate.unwrap();
                c.check(&a, &p.tables).unwrap();
                assert!(c.frames(BlockId(1)).is_empty());
            }
        }
    }
}

#[cfg(test)]
mod inline_policy_tests {
    use super::*;

    #[test]
    fn physical_body_names_preserve_inline_policy() {
        let mut module = Module::new();
        module
            .inline_policies
            .insert("body".into(), crate::hir::InlinePolicy::Never);
        remap_facts(
            &mut module,
            &HashMap::from([("body".into(), "body$arcslots_v1".into())]),
        );
        assert!(!module.inline_policies.contains_key("body"));
        assert_eq!(
            module.inline_policies["body$arcslots_v1"],
            crate::hir::InlinePolicy::Never
        );
    }
}
