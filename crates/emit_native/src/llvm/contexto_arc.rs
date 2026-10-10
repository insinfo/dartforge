//! Consulta dos caminhos de contexto implícitos para análise ARC.
//! Compartilha a preparação e a análise de raízes com a emissão nativa.

use super::{LlvmEmitter, raizes};
use crate::hir::*;

impl LlvmEmitter<'_> {
    // Prepara a mesma representação usada nas coerções e pontos de coleta.
    pub(super) fn preparar_tipos_e_phis(&mut self, func: &Function) {
        // Tabela de tipos da funcao: sem ela o emissor nao sabe se %v8 e um
        // i1 (resultado de icmp) ou um i64, e imprime "ret i64 %v8" para um
        // valor i1 — modulo inteiro recusado pelo Clang.
        self.tipos.clear();
        self.apontado.clear();
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                if let Instruction::Alloca(t) = inst {
                    self.apontado.insert(*vid, *t);
                }
            }
        }
        self.prox_coercao = 0;
        for (vid, _, ty) in &func.params {
            self.tipos.insert(*vid, *ty);
        }
        for block in &func.blocks {
            for (vid, inst, ty) in &block.instructions {
                self.tipos.insert(*vid, Self::tipo_do_resultado(inst, *ty));
            }
        }

        // Uma entrada de phi nao pode ser convertida onde o phi esta: phi tem de
        // ser a primeira instrucao do bloco. A conversao pertence ao bloco de
        // ORIGEM daquela entrada, emitida logo antes do terminador dele. Aqui
        // so planejamos; a emissao acontece bloco a bloco, mais abaixo.
        self.conv_phi.clear();
        let blocos_existentes: std::collections::HashSet<u32> =
            func.blocks.iter().map(|b| b.id.0).collect();
        for block in &func.blocks {
            for (_, inst, _) in &block.instructions {
                let Instruction::Phi { incoming, ty } = inst else {
                    continue;
                };
                for (origem, op) in incoming {
                    let Operand::Val(v) = op else { continue };
                    if !blocos_existentes.contains(&origem.0) {
                        continue;
                    }
                    let de = self.tipos.get(v).copied().unwrap_or(Type::I64);
                    let igual =
                        de.llvm_ir() == ty.llvm_ir() && (de == Type::F64) == (*ty == Type::F64);
                    if igual {
                        continue;
                    }
                    let ja = self.conv_phi.iter().any(|(b, _, _, vv, para)| {
                        *b == origem.0 && vv == v && para.llvm_ir() == ty.llvm_ir()
                    });
                    if ja {
                        continue;
                    }
                    let nome = format!("%p{}", self.prox_coercao);
                    self.prox_coercao += 1;
                    self.conv_phi.push((origem.0, nome, de, *v, *ty));
                }
            }
        }
    }

    /// Pode exigir quadro de raízes e, portanto, conferência de pilha.
    /// Usa slots da pilha-sombra antes da redução por mapas: cobre também
    /// fallback por orçamento e conferência de raízes. Não certifica opções
    /// de instrumentação, helpers, SDK ou ausência de outros efeitos.
    pub(crate) fn pode_exigir_contexto_por_raizes(func: &Function) -> bool {
        // A classificação de coleta usa tipos/apontados e o catálogo extern;
        // não consulta funções, layouts ou opções deste módulo vazio.
        let modulo = Module::new();
        let mut emissor = LlvmEmitter::new(&modulo);
        emissor.preparar_tipos_e_phis(func);
        let blocos: std::collections::HashSet<_> =
            emissor.conv_phi.iter().map(|(b, ..)| *b).collect();
        let analise = raizes::analisar(func, &emissor.tipos, &|i| emissor.pode_coletar(i), &|b| {
            blocos.contains(&b.0)
        });
        !analise.slots.is_empty()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn alloca_ref_exige_quadro_mesmo_sem_valores_ssa_vivos() {
        for t in [Type::Ref, Type::I64] {
            let f = Function {
                symbol: "f".into(),
                name: "f".into(),
                depuracao: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(0), Instruction::Alloca(t), Type::Ptr)],
                    terminator: Terminator::Return(None),
                }],
            };
            assert!(!LlvmEmitter::exige_contexto_explicito(&f, None));
            assert_eq!(
                LlvmEmitter::pode_exigir_contexto_por_raizes(&f),
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
