//! Addresses derived from lists: the data of a typed list
//! (`@df.typed_data`) and the storage of a core list
//! (`@df.core_storage`). Each access through them gets, right after,
//! `@df.keep_alive(list)`: the list stays borrowed and alive up to there, and ARC
//! puts the drop after the last access. The data of a typed list does not
//! change while it lives; the storage of a core list changes when the list
//! grows, so no path from its read to the access goes through a
//! barrier. Proves neither bounds nor the shape of the elements (the lowering checks).

use super::super::cfg::Cfg;
use super::super::operands::{largest_ids, operands, terminator_operands};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// The use that holds the list after an access through the derived address.
pub(crate) const KEEP_ALIVE: &str = "df.keep_alive";

#[derive(Clone, Copy, PartialEq)]
struct Origin {
    /// The list the address derives from.
    list: ValueId,
    /// The read of the address (the helper).
    root: ValueId,
    /// Core storage (changes when the list grows).
    core: bool,
}

fn origins(f: &Function) -> HashMap<ValueId, Origin> {
    let mut der = HashMap::new();
    loop {
        let before = der.len();
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if der.contains_key(v) {
                continue;
            }
            let o = match inst {
                Instruction::CallRuntime { name, args, .. }
                    if matches!(name.as_str(), "df.typed_data" | "df.core_storage") =>
                {
                    match args.as_slice() {
                        [(Operand::Val(list), Type::Ref)] => Some(Origin {
                            list: *list,
                            root: *v,
                            core: name == "df.core_storage",
                        }),
                        _ => None,
                    }
                }
                Instruction::Add(a, b) => [a, b].into_iter().find_map(|o| match o {
                    Operand::Val(x) => der.get(x).copied(),
                    _ => None,
                }),
                Instruction::Sub(Operand::Val(a), _) => der.get(a).copied(),
                _ => None,
            };
            if let Some(o) = o {
                der.insert(*v, o);
            }
        }
        if der.len() == before {
            return der;
        }
    }
}

/// The values that are addresses derived from lists.
pub(super) fn addresses(f: &Function) -> HashSet<ValueId> {
    origins(f).into_keys().collect()
}

/// The address read or written by `inst`, if it is a memory access.
fn address(inst: &Instruction) -> Option<&Operand> {
    match inst {
        Instruction::NativeLoad { address, .. } | Instruction::NativeWrite { address, .. } => {
            Some(address)
        }
        Instruction::Simd {
            op: OpSimd::Loads | OpSimd::Writes,
            args,
        } => args.first(),
        Instruction::CallRuntime { name, args, .. } if name == "df.core_shape" => {
            args.first().map(|(o, _)| o)
        }
        _ => None,
    }
}

fn holds(inst: Option<&Instruction>, list: ValueId) -> bool {
    matches!(inst, Some(Instruction::CallRuntime { name, args, ret_ty: Type::Void })
        if name == KEEP_ALIVE && matches!(args.as_slice(), [(Operand::Val(l), Type::Ref)] if *l == list))
}

fn keep_alive(list: ValueId) -> Instruction {
    Instruction::CallRuntime {
        name: KEEP_ALIVE.into(),
        args: vec![(Operand::Val(list), Type::Ref)],
        ret_ty: Type::Void,
    }
}

/// May replace the storage of a core list (calls Dart or the
/// runtime without a contract that excludes it). The safepoint does not: the
/// control messages (pause, ping, kill) do not run the program's Dart, and
/// kill leaves the loop through the pending state, before the next access. Clearing the
/// pending state only releases the exception: the list is still held by `keep_alive`.
fn barrier(inst: &Instruction) -> bool {
    match inst {
        Instruction::CallRuntime { name, .. } => {
            let base = name
                .strip_prefix(super::NATIVE_SCOPE_PREFIX)
                .unwrap_or(name);
            name != KEEP_ALIVE
                && !matches!(base, "dartforge_safepoint" | "dartforge_exception_clear")
                && super::contract(name).map_or(true, |c| c.invalidates_borrows || c.calls_dart)
        }
        Instruction::CallStatic { .. }
        | Instruction::CallSelector { .. }
        | Instruction::CallSelectorForward { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CallClosureForward { .. }
        | Instruction::TypedCall { .. }
        | Instruction::NativeCall { .. }
        | Instruction::CompositeNativeCall { .. }
        | Instruction::SetListElement { .. } => true,
        _ => false,
    }
}

/// Puts `@df.keep_alive(list)` after each access through a derived address
/// that does not have it yet (repeating changes nothing) and checks the resulting shape
/// ([`verify`]).
pub(super) fn keep_live(f: &mut Function) -> Result<(), String> {
    let der = origins(f);
    if der.is_empty() {
        return Ok(());
    }
    let mut next_item = largest_ids(f).0;
    for b in &mut f.blocks {
        let mut new_ones = Vec::with_capacity(b.instructions.len());
        let old_ones = std::mem::take(&mut b.instructions);
        for (k, (v, inst, ty)) in old_ones.iter().enumerate() {
            let list = match address(inst) {
                Some(Operand::Val(d)) => der.get(d).map(|o| o.list),
                _ => None,
            }
            .filter(|l| !holds(old_ones.get(k + 1).map(|(_, i, _)| i), *l));
            new_ones.push((*v, inst.clone(), *ty));
            if let Some(list) = list {
                next_item = next_item
                    .checked_add(1)
                    .ok_or("SSA IDs exhausted while keeping lists alive")?;
                new_ones.push((ValueId(next_item), keep_alive(list), Type::Void));
            }
        }
        b.instructions = new_ones;
    }
    verify(f)
}

/// The reads of core storage still valid at the entry of each
/// block: valid on every path since the read, with no barrier in between.
fn valid_storages(f: &Function, der: &HashMap<ValueId, Origin>) -> Vec<HashSet<ValueId>> {
    let roots: HashSet<ValueId> = der.values().filter(|o| o.core).map(|o| o.root).collect();
    let cfg = Cfg::new(f);
    let transfer = |bi: usize, mut state: HashSet<ValueId>| {
        for (v, inst, _) in &f.blocks[bi].instructions {
            if barrier(inst) {
                state.clear();
            }
            if roots.contains(v) {
                state.insert(*v);
            }
        }
        state
    };
    let mut entries = vec![roots.clone(); f.blocks.len()];
    let mut exits = vec![roots.clone(); f.blocks.len()];
    if let Some(&e) = cfg.rpo.first() {
        entries[e].clear();
    }
    loop {
        let mut changed = false;
        for (k, &bi) in cfg.rpo.iter().enumerate() {
            let mut state = if k == 0 {
                HashSet::new()
            } else {
                roots.clone()
            };
            for &p in &cfg.predecessors[bi] {
                if cfg.idom[p] != usize::MAX {
                    state.retain(|r| exits[p].contains(r));
                }
            }
            let exit = transfer(bi, state.clone());
            changed |= entries[bi] != state || exits[bi] != exit;
            entries[bi] = state;
            exits[bi] = exit;
        }
        if !changed {
            return entries;
        }
    }
}

/// Checks that the derived address only serves derivations and accesses, that
/// each access is followed by `@df.keep_alive` of its list, and that no
/// path from the read of core storage to the access has a barrier.
pub(super) fn verify(f: &Function) -> Result<(), String> {
    let der = origins(f);
    if der.is_empty() {
        return Ok(());
    }
    let valid_set = valid_storages(f, &der);
    for (bi, b) in f.blocks.iter().enumerate() {
        let mut state = valid_set[bi].clone();
        for (i, (v, inst, _)) in b.instructions.iter().enumerate() {
            let access = address(inst).and_then(|o| match o {
                Operand::Val(d) => der.get(d).copied(),
                _ => None,
            });
            let mut error = None;
            let derivation = der.get(v).is_some_and(|o| o.root != *v);
            let mut occurrences = 0;
            operands(inst, &mut |o| {
                if let Operand::Val(x) = o
                    && der.contains_key(x)
                {
                    occurrences += 1;
                    if !derivation && access.is_none() {
                        error = Some(*x);
                    }
                }
            });
            // In an access, the address is the only derived operand.
            if access.is_some() && occurrences != 1 {
                error = Some(*v);
            }
            if let Some(x) = error {
                return Err(format!(
                    "v{}: address derived from list v{} escapes in {}",
                    v.0, x.0, f.symbol
                ));
            }
            if barrier(inst) {
                state.clear();
            }
            if der.get(v).is_some_and(|o| o.core && o.root == *v) {
                state.insert(*v);
            }
            let Some(o) = access else { continue };
            let following = b.instructions.get(i + 1).map(|(_, inst, _)| inst);
            if !holds(following, o.list) {
                return Err(format!(
                    "v{}: access through a derived address without keeping v{} alive in {}",
                    v.0, o.list.0, f.symbol
                ));
            }
            if o.core && !state.contains(&o.root) {
                let barriers: Vec<String> = f
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .filter(|(_, inst, _)| barrier(inst))
                    .map(|(x, inst, _)| {
                        format!(
                            "v{} {}",
                            x.0,
                            match inst {
                                Instruction::CallRuntime { name, .. } => name.clone(),
                                Instruction::CallStatic { symbol, .. } => symbol.clone(),
                                another => format!("{another:?}").chars().take(40).collect(),
                            }
                        )
                    })
                    .collect();
                return Err(format!(
                    "v{}: barrier between the read of the storage of v{} and the access in {} (barriers: {barriers:?})",
                    v.0, o.list.0, f.symbol
                ));
            }
        }
        let mut escapes = None;
        terminator_operands(&b.terminator, &mut |o| {
            if let Operand::Val(x) = o
                && der.contains_key(x)
            {
                escapes = Some(*x);
            }
        });
        if let Some(x) = escapes {
            return Err(format!(
                "address derived from list v{} escapes in the terminator of b{} in {}",
                x.0, b.id.0, f.symbol
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(name: &str, args: Vec<(Operand, Type)>, ret_ty: Type) -> Instruction {
        Instruction::CallRuntime {
            name: name.into(),
            args,
            ret_ty,
        }
    }

    fn function(instructions: Vec<(ValueId, Instruction, Type)>) -> Function {
        Function {
            symbol: "f".into(),
            name: "f".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "l".into(), Type::Ref),
                (ValueId(1), "i".into(), Type::I64),
            ],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(None),
            }],
        }
    }

    fn val(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }

    #[test]
    fn typed_access_gets_list_keep_alive() {
        let mut f = function(vec![
            (
                ValueId(2),
                rt("df.typed_data", vec![(val(0), Type::Ref)], Type::I64),
                Type::I64,
            ),
            (
                ValueId(3),
                Instruction::NativeLoad {
                    address: val(2),
                    index: val(1),
                    ty: CType::F64,
                },
                Type::F64,
            ),
            (
                ValueId(4),
                Instruction::Add(val(2), Operand::Constant(Constant::Int(8))),
                Type::I64,
            ),
            (
                ValueId(5),
                Instruction::NativeWrite {
                    address: val(4),
                    index: val(1),
                    ty: CType::F64,
                    value: val(3),
                },
                Type::Void,
            ),
        ]);
        keep_live(&mut f).unwrap();
        let names: Vec<_> = f.blocks[0]
            .instructions
            .iter()
            .map(|(v, i, _)| match i {
                Instruction::CallRuntime { name, args, .. } if name == KEEP_ALIVE => {
                    assert_eq!(args, &vec![(val(0), Type::Ref)]);
                    format!("manter{}", v.0)
                }
                _ => format!("v{}", v.0),
            })
            .collect();
        assert_eq!(names, ["v2", "v3", "manter6", "v4", "v5", "manter7"]);
        // Repeating changes nothing.
        let before = format!("{f:?}");
        keep_live(&mut f).unwrap();
        assert_eq!(format!("{f:?}"), before);
        // Without the keep_alive after the access, it is rejected.
        f.blocks[0].instructions.remove(2);
        assert!(verify(&f).unwrap_err().contains("without keeping"));
    }

    #[test]
    fn derived_address_does_not_escape_or_cross_core_barrier() {
        // Passing the address to a call.
        let mut f = function(vec![
            (
                ValueId(2),
                rt("df.typed_data", vec![(val(0), Type::Ref)], Type::I64),
                Type::I64,
            ),
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "g".into(),
                    args: vec![val(2)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ]);
        assert!(keep_live(&mut f).unwrap_err().contains("escapes"));
        // A call between the read of the storage and the access.
        let mut f = function(vec![
            (
                ValueId(2),
                rt("df.core_storage", vec![(val(0), Type::Ref)], Type::I64),
                Type::I64,
            ),
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "g".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
            (
                ValueId(4),
                Instruction::NativeLoad {
                    address: val(2),
                    index: val(1),
                    ty: CType::I64,
                },
                Type::I64,
            ),
        ]);
        assert!(keep_live(&mut f).unwrap_err().contains("barrier"));
        // Without a barrier, it passes; the shape is also an access.
        let mut f = function(vec![
            (
                ValueId(2),
                rt("df.core_storage", vec![(val(0), Type::Ref)], Type::I64),
                Type::I64,
            ),
            (
                ValueId(3),
                rt("df.core_shape", vec![(val(2), Type::I64)], Type::I64),
                Type::I64,
            ),
            (
                ValueId(4),
                Instruction::Add(val(2), Operand::Constant(Constant::Int(22))),
                Type::I64,
            ),
            (
                ValueId(5),
                Instruction::NativeLoad {
                    address: val(4),
                    index: val(1),
                    ty: CType::I64,
                },
                Type::I64,
            ),
        ]);
        keep_live(&mut f).unwrap();
        assert_eq!(f.blocks[0].instructions.len(), 6);
    }

    /// The storage read before the loop iterations (N13): the safepoint in the body
    /// does not invalidate it; a call in the body does (through the back edge).
    #[test]
    fn storage_read_before_loop_crosses_safepoint() {
        let loop_fn = |body: Instruction| Function {
            symbol: "f".into(),
            name: "f".into(),
            debug_info: None,
            params: vec![
                (ValueId(0), "l".into(), Type::Ref),
                (ValueId(1), "i".into(), Type::I64),
            ],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(2),
                        rt("df.core_storage", vec![(val(0), Type::Ref)], Type::I64),
                        Type::I64,
                    )],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::NativeLoad {
                                address: val(2),
                                index: val(1),
                                ty: CType::I64,
                            },
                            Type::I64,
                        ),
                        (ValueId(4), body, Type::Void),
                        (
                            ValueId(5),
                            Instruction::ICmp(ICmpOp::Slt, val(3), val(1)),
                            Type::I1,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: val(5),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        };
        let mut f = loop_fn(rt("df.arc.scope.dartforge_safepoint", vec![], Type::Void));
        keep_live(&mut f).unwrap();
        let mut f = loop_fn(Instruction::CallStatic {
            symbol: "g".into(),
            args: vec![],
            ret_ty: Type::Void,
        });
        assert!(keep_live(&mut f).unwrap_err().contains("barrier"));
    }
}
