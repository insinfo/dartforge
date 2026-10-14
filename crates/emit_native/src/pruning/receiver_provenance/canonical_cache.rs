//! Physical return certificate for the closed, numeric-field constant-cache shape.
//!
//! A cache read alone proves nothing. Both returns, the publication order, every
//! declared cache writer and the initializer's allocation must match this shape.
//! Extra effects, ARC adapters, callbacks, opaque initializers and error paths
//! stay unknown. The non-string canonicalization branch returns the same handle.
//! LoadGlobal yields a value, not its TLS storage address. Explicit area-address
//! APIs and symbol-address exposure disable this proof; new address-bearing HIR
//! instructions need an audit before this storage boundary can cover them.

use super::*;
use crate::hir::{ICmpOp, StrongSlot};

/// One inventory pass avoids rescanning the entire SDK for every constant.
pub(super) struct Inventory<'a> {
    fixed: bool,
    declarations: HashMap<&'a str, Vec<(u32, Type)>>,
    writers: HashMap<&'a str, (usize, usize)>,
    escaped: HashSet<String>,
    reference_roots: HashMap<u32, usize>,
    area_exposed: bool,
}

impl<'a> Inventory<'a> {
    pub(super) fn new(modules: &[&'a Module]) -> Self {
        let mut inventory = Self {
            fixed: modules.iter().all(|m| m.arc_fixed_ancestry),
            declarations: HashMap::new(),
            writers: HashMap::new(),
            escaped: HashSet::new(),
            reference_roots: HashMap::new(),
            area_exposed: false,
        };
        for module in modules {
            for (root, ty, symbol) in &module.globals {
                inventory
                    .declarations
                    .entry(symbol)
                    .or_default()
                    .push((*root, *ty));
                if *ty == Type::Ref {
                    *inventory.reference_roots.entry(*root).or_default() += 1;
                }
            }
            for f in &module.functions {
                let owner = f as *const Function as usize;
                for (_, instruction, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                    match instruction {
                        I::CallRuntime { name, .. } | I::DeclaredFieldCall { name, .. } | I::CallStatic { symbol: name, .. }
                            if exposes_area(name) =>
                        {
                            inventory.area_exposed = true
                        }
                        I::StoreGlobal { symbol, .. } => {
                            let writer = inventory.writers.entry(symbol).or_insert((owner, 0));
                            if writer.0 != owner {
                                writer.0 = 0;
                            }
                            writer.1 += 1;
                        }
                        I::ArcStoreStrong {
                            slot: StrongSlot::Global { symbol },
                            ..
                        } => {
                            inventory.escaped.insert(symbol.clone());
                        }
                        I::FunctionTable(symbols) => {
                            inventory.escaped.extend(symbols.iter().cloned())
                        }
                        I::Const(Constant::Function(symbol)) => {
                            inventory.escaped.insert(symbol.clone());
                        }
                        I::Const(Constant::String(symbol)) if exposes_area(symbol) => {
                            inventory.area_exposed = true
                        }
                        I::Const(Constant::StringWtf8(bytes))
                            if std::str::from_utf8(bytes).is_ok_and(exposes_area) =>
                        {
                            inventory.area_exposed = true
                        }
                        _ => {}
                    }
                    operands(instruction, &mut |operand| {
                        if let Operand::Constant(Constant::Function(symbol)) = operand {
                            inventory.escaped.insert(symbol.clone());
                        } else if area_literal(operand) {
                            inventory.area_exposed = true;
                        }
                    });
                }
                for block in &f.blocks {
                    terminator_operands(&block.terminator, &mut |operand| {
                        if let Operand::Constant(Constant::Function(symbol)) = operand {
                            inventory.escaped.insert(symbol.clone());
                        } else if area_literal(operand) {
                            inventory.area_exposed = true;
                        }
                    });
                }
            }
        }
        inventory.area_exposed |= inventory.escaped.iter().any(|name| exposes_area(name));
        inventory
    }
}

fn area_literal(operand: &Operand) -> bool {
    match operand {
        Operand::Constant(Constant::String(symbol)) => exposes_area(symbol),
        Operand::Constant(Constant::StringWtf8(bytes)) => {
            std::str::from_utf8(bytes).is_ok_and(exposes_area)
        }
        _ => false,
    }
}

fn exposes_area(symbol: &str) -> bool {
    matches!(
        symbol,
        "dartforge_globals_area" | "dartforge_globals_area_id"
    )
}

pub(super) fn classes(
    function: &Function,
    inventory: &Inventory<'_>,
    functions: &HashMap<String, &Function>,
) -> Option<Classes> {
    if !inventory.fixed
        || inventory.area_exposed
        || functions.contains_key("dartforge_globals_area")
        || functions.contains_key("dartforge_globals_area_id")
    {
        return None;
    }
    if !function.params.is_empty() || function.return_ty != Type::Ref || function.blocks.len() != 3
    {
        return None;
    }
    let entry = &function.blocks[0];
    let [
        (
            flag_value,
            I::LoadGlobal {
                symbol: flag,
                ty: Type::I8,
            },
            Type::I8,
        ),
        (
            condition,
            I::ICmp(ICmpOp::Ne, Operand::Val(loaded_flag), Operand::Constant(Constant::Int(0))),
            Type::I1,
        ),
    ] = entry.instructions.as_slice()
    else {
        return None;
    };
    if flag_value != loaded_flag {
        return None;
    }
    let Terminator::CondBranch {
        cond: Operand::Val(branch_condition),
        then_block,
        else_block,
    } = &entry.terminator
    else {
        return None;
    };
    if condition != branch_condition || then_block == else_block {
        return None;
    }
    let read = function.blocks.iter().find(|b| b.id == *then_block)?;
    let init = function.blocks.iter().find(|b| b.id == *else_block)?;
    if read.id == entry.id || init.id == entry.id {
        return None;
    }
    let [
        (
            cached,
            I::LoadGlobal {
                symbol: cache,
                ty: Type::Ref,
            },
            Type::Ref,
        ),
    ] = read.instructions.as_slice()
    else {
        return None;
    };
    if flag != &format!("{cache}$ok")
        || !matches!(&read.terminator, Terminator::Return(Some(Operand::Val(v))) if v == cached)
    {
        return None;
    }
    let instructions = &init.instructions;
    if instructions.len() < 5 {
        return None;
    }
    let (object, allocation, Type::Ref) = &instructions[0] else {
        return None;
    };
    let (I::CallRuntime {
        name,
        args,
        ret_ty: Type::Ref,
    } | I::DeclaredFieldCall {
        name,
        args,
        ret_ty: Type::Ref, .. }) = allocation
    else {
        return None;
    };
    if name != "dartforge_object_new" || functions.contains_key(name) {
        return None;
    }
    let [
        (Operand::Constant(Constant::Int(cid)), Type::I64),
        (Operand::Constant(Constant::Int(fields)), Type::I64),
    ] = args.as_slice()
    else {
        return None;
    };
    // Runtime text::Heap::canonical_constant preserves non-string handles. Restrict
    // this proof to ordinary allocated object headers, not strings or collections.
    let cid = u32::try_from(*cid)
        .ok()
        .filter(|cid| (128..=i32::MAX as u32).contains(cid))?;
    if usize::try_from(*fields).is_err() {
        return None;
    }
    let tail = instructions.len() - 4;
    let mut integers = HashSet::new();
    for (value, instruction, ty) in &instructions[1..tail] {
        match (instruction, ty) {
            (I::Const(Constant::Int(_)), Type::I64) => {
                integers.insert(*value);
            }
            (
                I::CallRuntime {
                    name,
                    args,
                    ret_ty: Type::Void,
                } | I::DeclaredFieldCall {
                    name,
                    args,
                    ret_ty: Type::Void, .. },
                Type::Void,
            ) if name == "dartforge_object_set" && !functions.contains_key(name) => {
                let [
                    (Operand::Val(receiver), Type::Ref),
                    (Operand::Constant(Constant::Int(index)), Type::I64),
                    (number, Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I8),
                ] = args.as_slice()
                else {
                    return None;
                };
                if receiver != object
                    || *index < 0
                    || *index >= *fields
                    || !matches!(number, Operand::Constant(Constant::Int(_)))
                        && !matches!(number, Operand::Val(v) if integers.contains(v))
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let (
        canonical,
        I::CallRuntime {
            name: canonical_name,
            args: canonical_args,
            ret_ty: Type::Ref,
        },
        Type::Ref,
    ) = &instructions[tail]
    else {
        return None;
    };
    if canonical_name != "dartforge_canonical_constant"
        || functions.contains_key(canonical_name)
        || !matches!(canonical_args.as_slice(), [(Operand::Val(v), Type::Ref)] if v == object)
    {
        return None;
    }
    let (
        _,
        I::StoreGlobal {
            symbol: stored_cache,
            val: Operand::Val(stored_value),
            ty: Type::Ref,
            root: Some(root),
        },
        Type::Void,
    ) = &instructions[tail + 1]
    else {
        return None;
    };
    if stored_cache != cache || stored_value != canonical {
        return None;
    }
    let (
        _,
        I::CallRuntime {
            name: mark,
            args: mark_args,
            ret_ty: Type::Void,
        },
        Type::Void,
    ) = &instructions[tail + 2]
    else {
        return None;
    };
    if mark != "dartforge_mark_constant"
        || functions.contains_key(mark)
        || !matches!(mark_args.as_slice(), [(Operand::Val(v), Type::Ref), (Operand::Constant(Constant::Function(symbol)), Type::I64)]
            if v == canonical && symbol == &function.symbol)
    {
        return None;
    }
    let (
        _,
        I::StoreGlobal {
            symbol: stored_flag,
            val: Operand::Constant(Constant::Int(1)),
            ty: Type::I8,
            root: None,
        },
        Type::Void,
    ) = &instructions[tail + 3]
    else {
        return None;
    };
    if stored_flag != flag
        || !matches!(&init.terminator, Terminator::Return(Some(Operand::Val(v))) if v == canonical)
    {
        return None;
    }
    // A same spelling or ready flag is not a certificate: the storage declaration
    // and all writers across the audited modules must agree with this exact body.
    let declarations = inventory.declarations.get(cache.as_str())?;
    if inventory.reference_roots.get(root) != Some(&1)
        || inventory.declarations.contains_key(flag.as_str())
        || !matches!(declarations.as_slice(), [(declared_root, Type::Ref)] if declared_root == root)
        || inventory.escaped.contains(cache)
        || inventory.escaped.contains(flag)
    {
        return None;
    }
    let owner = function as *const Function as usize;
    (inventory.writers.get(cache.as_str()) == Some(&(owner, 1))
        && inventory.writers.get(flag.as_str()) == Some(&(owner, 1)))
    .then(|| Classes::from([cid]))
}

#[cfg(test)]
mod tests;
