//! Query of the implicit context paths for ARC analysis.
//! Shares the preparation and the roots analysis with native emission.

use super::{LlvmEmitter, roots};
use crate::hir::*;

impl LlvmEmitter<'_> {
    // Prepares the same representation used in coercions and collection points.
    pub(super) fn prepare_types_and_phis(&mut self, func: &Function) {
        // Type table of the function: without it the emitter does not know whether %v8 is an
        // i1 (result of an icmp) or an i64, and prints "ret i64 %v8" for an
        // i1 value — a whole module rejected by Clang.
        self.types.clear();
        self.pointee.clear();
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                if let Instruction::Alloca(t) = inst {
                    self.pointee.insert(*vid, *t);
                }
            }
        }
        self.next_coercion = 0;
        for (vid, _, ty) in &func.params {
            self.types.insert(*vid, *ty);
        }
        for block in &func.blocks {
            for (vid, inst, ty) in &block.instructions {
                self.types.insert(*vid, Self::result_type(inst, *ty));
            }
        }

        // A phi entry cannot be converted where the phi is: a phi has to
        // be the first instruction of the block. The conversion belongs to the
        // ORIGIN block of that entry, emitted right before its terminator. Here
        // we only plan; the emission happens block by block, further below.
        self.conv_phi.clear();
        let existing_blocks: std::collections::HashSet<u32> =
            func.blocks.iter().map(|b| b.id.0).collect();
        for block in &func.blocks {
            for (_, inst, _) in &block.instructions {
                let Instruction::Phi { incoming, ty } = inst else {
                    continue;
                };
                for (origin, op) in incoming {
                    let Operand::Val(v) = op else { continue };
                    if !existing_blocks.contains(&origin.0) {
                        continue;
                    }
                    let de = self.types.get(v).copied().unwrap_or(Type::I64);
                    let equal =
                        de.llvm_ir() == ty.llvm_ir() && (de == Type::F64) == (*ty == Type::F64);
                    if equal {
                        continue;
                    }
                    let already = self.conv_phi.iter().any(|(b, _, _, vv, to)| {
                        *b == origin.0 && vv == v && to.llvm_ir() == ty.llvm_ir()
                    });
                    if already {
                        continue;
                    }
                    let name = format!("%p{}", self.next_coercion);
                    self.next_coercion += 1;
                    self.conv_phi.push((origin.0, name, de, *v, *ty));
                }
            }
        }
    }

    /// May require a roots frame and, therefore, a stack check.
    /// Uses shadow-stack slots before the reduction by maps: also covers the
    /// fallback by budget and the roots check. Does not certify instrumentation
    /// options, helpers, the SDK nor the absence of other effects.
    pub(crate) fn may_require_context_for_roots(func: &Function) -> bool {
        // The collection classification uses types/pointees and the extern catalog;
        // it does not consult functions, layouts or options of this empty module.
        let module = Module::new();
        let mut emitter = LlvmEmitter::new(&module);
        emitter.prepare_types_and_phis(func);
        let blocks: std::collections::HashSet<_> =
            emitter.conv_phi.iter().map(|(b, ..)| *b).collect();
        let analysis = roots::analyze(func, &emitter.types, &|i| emitter.may_collect(i), &|b| {
            blocks.contains(&b.0)
        });
        !analysis.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_alloca_requires_frame_even_without_live_ssa_values() {
        for t in [Type::Ref, Type::I64] {
            let f = Function {
                symbol: "f".into(),
                name: "f".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(0), Instruction::Alloca(t), Type::Ptr)],
                    terminator: Terminator::Return(None),
                }],
            };
            assert!(!LlvmEmitter::requires_explicit_context(&f, None));
            assert_eq!(
                LlvmEmitter::may_require_context_for_roots(&f),
                t == Type::Ref
            );
            let mut m = Module::new();
            m.functions.push(f);
            let ir = LlvmEmitter::new(&m).emit_all();
            assert_eq!(ir.contains("%gcq = alloca"), t == Type::Ref);
            assert_eq!(ir.contains("pilha.estouro:"), t == Type::Ref);
        }
    }
}
