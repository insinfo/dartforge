//! Consulta dos caminhos de contexto implícitos para análise ARC.
//! Compartilha a preparação e a análise de raízes com a emissão nativa.

use super::{LlvmEmitter, roots};
use crate::hir::*;

impl LlvmEmitter<'_> {
    // Prepara a mesma representação usada nas coerções e pontos de coleta.
    pub(super) fn prepare_types_and_phis(&mut self, func: &Function) {
        // Tabela de tipos da funcao: sem ela o emissor nao sabe se %v8 e um
        // i1 (resultado de icmp) ou um i64, e imprime "ret i64 %v8" para um
        // valor i1 — modulo inteiro recusado pelo Clang.
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

        // Uma entrada de phi nao pode ser convertida onde o phi esta: phi tem de
        // ser a primeira instrucao do bloco. A conversao pertence ao bloco de
        // ORIGEM daquela entrada, emitida logo antes do terminador dele. Aqui
        // so planejamos; a emissao acontece bloco a bloco, mais abaixo.
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

    /// Pode exigir quadro de raízes e, portanto, conferência de pilha.
    /// Usa slots da pilha-sombra antes da redução por mapas: cobre também
    /// fallback por orçamento e conferência de raízes. Não certifica opções
    /// de instrumentação, helpers, SDK ou ausência de outros efeitos.
    pub(crate) fn may_require_context_for_roots(func: &Function) -> bool {
        // A classificação de coleta usa tipos/apontados e o catálogo extern;
        // não consulta funções, layouts ou opções deste módulo vazio.
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
