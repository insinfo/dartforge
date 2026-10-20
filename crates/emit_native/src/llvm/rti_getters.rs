//! Compact bodies for the lazy type getters of `lower::rti` (`df.rti.<hash>.<library>`).
//!
//! Each getter reads a flag of the isolate area, returns the cached type when it is
//! set and otherwise parses its recipe once (`dartforge_rti_recipe`) and stores the
//! result. Written out per getter, the inlined `df.get_area` plus the two paths cost
//! about a hundred bytes each, and a large program has thousands of them (3,490 in the
//! File Explorer). In AOT with static strings the body becomes a tail call to one
//! shared helper, `@df.rti_fill`, with the two slots and the recipe as constants: the
//! same loads, stores and runtime call, only once per module.
//!
//! The rewrite applies only to the exact shape that `recipe_rti` builds; anything an
//! optimization changed is emitted as usual. `DARTFORGE_NO_COMPACT_RTI_GETTERS=1` at
//! compile time turns it off (measurement).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// The getter shape, reduced to what the compact body needs.
pub(super) struct RtiGetter {
    value_slot: usize,
    ready_slot: usize,
    recipe: Vec<u8>,
}

/// The shared helper: the getter body with the slots as parameters.
const RTI_FILL: &str = "define internal i64 @df.rti_fill(i64 %vs, i64 %rs, i64 %s) noinline {\n\
entry:\n\
  %area = call ptr @df.get_area()\n\
  %rp = getelementptr i64, ptr %area, i64 %rs\n\
  %vp = getelementptr i64, ptr %area, i64 %vs\n\
  %r = load i8, ptr %rp\n\
  %ready = icmp ne i8 %r, 0\n\
  br i1 %ready, label %hit, label %parse\n\
hit:\n\
  %v = load i64, ptr %vp\n\
  ret i64 %v\n\
parse:\n\
  %t = call i64 @dartforge_rti_recipe(i64 %s)\n\
  store i64 %t, ptr %vp\n\
  store i8 1, ptr %rp\n\
  ret i64 %t\n\
}\n";

impl LlvmEmitter<'_> {
    /// Whether `func` is a getter of `recipe_rti` that can take the compact body.
    pub(super) fn compact_rti_getter(&self, func: &Function) -> Option<RtiGetter> {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !self.static_objects
            || self.maps
            || self.module.arc_memory
            || super::externs::check_effects()
            || !func.symbol.starts_with("df.rti.")
            || !func.params.is_empty()
            || func.return_ty != Type::I64
            || func.blocks.len() != 3
            || *OFF.get_or_init(|| {
                std::env::var_os("DARTFORGE_NO_COMPACT_RTI_GETTERS")
                    .is_some_and(|v| !v.is_empty() && v != "0")
            })
        {
            return None;
        }
        let block = |id: BlockId| func.blocks.iter().find(|b| b.id == id);
        // Entry: `ok = load g$ok (i8)`, `c = ok != 0`, `br c, ready, parse`.
        let entry = &func.blocks[0];
        let [
            (
                ok,
                Instruction::LoadGlobal {
                    symbol: ready_symbol,
                    ty: Type::I8,
                },
                _,
            ),
            (
                c,
                Instruction::ICmp(
                    ICmpOp::Ne,
                    Operand::Val(ok_use),
                    Operand::Constant(Constant::Int(0)),
                ),
                _,
            ),
        ] = entry.instructions.as_slice()
        else {
            return None;
        };
        let Terminator::CondBranch {
            cond: Operand::Val(cond),
            then_block,
            else_block,
        } = &entry.terminator
        else {
            return None;
        };
        if ok_use != ok || cond != c {
            return None;
        }
        let value_symbol = ready_symbol.strip_suffix("$ok")?;
        // Ready: `v = load g (i64)`, `ret v`.
        let ready = block(*then_block)?;
        let [
            (
                v,
                Instruction::LoadGlobal {
                    symbol: read,
                    ty: Type::I64,
                },
                _,
            ),
        ] = ready.instructions.as_slice()
        else {
            return None;
        };
        if read != value_symbol
            || !matches!(&ready.terminator, Terminator::Return(Some(Operand::Val(r))) if r == v)
        {
            return None;
        }
        // Parse: `s = "recipe"`, `t = rti_recipe(s)`, `g = t`, `g$ok = 1`, `ret t`.
        let parse = block(*else_block)?;
        let [
            (s, Instruction::Const(Constant::String(recipe)), _),
            (
                t,
                Instruction::CallRuntime {
                    name,
                    args,
                    ret_ty: Type::I64,
                },
                _,
            ),
            (
                _,
                Instruction::StoreGlobal {
                    symbol: stored,
                    val: Operand::Val(stored_value),
                    ty: Type::I64,
                    root: None,
                },
                _,
            ),
            (
                _,
                Instruction::StoreGlobal {
                    symbol: flagged,
                    val: Operand::Constant(Constant::Int(1)),
                    ty: Type::I8,
                    root: None,
                },
                _,
            ),
        ] = parse.instructions.as_slice()
        else {
            return None;
        };
        if name != "dartforge_rti_recipe"
            || args.as_slice() != [(Operand::Val(*s), Type::Ref)]
            || stored != value_symbol
            || stored_value != t
            || flagged != ready_symbol
            || !matches!(&parse.terminator, Terminator::Return(Some(Operand::Val(r))) if r == t)
        {
            return None;
        }
        Some(RtiGetter {
            value_slot: *self.global_slots.get(value_symbol)?,
            ready_slot: *self.global_slots.get(ready_symbol.as_str())?,
            recipe: recipe.as_bytes().to_vec(),
        })
    }

    /// The compact body of `getter` (see the module documentation); the helper goes
    /// out with the first one.
    pub(super) fn emit_compact_rti_getter(&mut self, func: &Function, getter: &RtiGetter) {
        if !self.rti_fill_emitted {
            self.rti_fill_emitted = true;
            self.out.push_str(RTI_FILL);
        }
        let recipe = self.static_string(&getter.recipe);
        writeln!(
            self.out,
            "define i64 @{}() nounwind {{\n\
  %r = tail call i64 @df.rti_fill(i64 {}, i64 {}, i64 ptrtoint (ptr {recipe} to i64))\n\
  ret i64 %r\n\
}}\n",
            func.symbol, getter.value_slot, getter.ready_slot
        )
        .unwrap();
    }
}
