//! Resumo de exceções por função: quem não pode deixar exceção pendente.
//!
//! No código gerado, uma exceção é um estado pendente do runtime que quem
//! chama confere logo depois da chamada (`dartforge_exception_pending`,
//! `FnBuilder::emit_call_with_check`). Em toda chamada o estado está limpo
//! (cada operação que pode lançar é conferida em seguida), então, depois de
//! uma chamada a uma função que não pode lançar, a conferência é sempre
//! falsa. O resumo é o maior ponto fixo: começa com todas as funções do
//! módulo sem lançar e tira as que têm uma operação que lança — uma
//! recursão sem outra operação que lance continua sem lançar.

use super::cfg::sucessores;
use crate::hir::*;
use crate::llvm::externs::efeitos_de;
use std::collections::HashSet;

/// A instrução pode deixar exceção pendente?
pub fn instrucao_lanca(inst: &Instruction, nao_lancam: &HashSet<String>) -> bool {
    match inst {
        Instruction::CallStatic { symbol, .. } => !nao_lancam.contains(symbol),
        Instruction::CallRuntime { name, .. } => efeitos_de(name).lanca,
        Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CallSeletor { .. }
        | Instruction::CallSeletorRepasse { .. }
        | Instruction::CallClosureRepasse { .. }
        | Instruction::ChamadaNativa { .. }
        | Instruction::ChamadaTipada { .. }
        | Instruction::ChamadaNativaComposta { .. } => true,
        // Caixa de tipo errado ou nula, null, divisão por zero, `toInt` de
        // NaN ou infinito.
        Instruction::Unbox { to, .. } => !to.e_vetor(),
        Instruction::CheckNotNull(_) | Instruction::SDiv(..) | Instruction::SRem(..) | Instruction::DoubleToInt(_) => true,
        _ => false,
    }
}

/// A função pode lançar, dado o conjunto corrente das que não lançam?
fn funcao_lanca(func: &Function, nao_lancam: &HashSet<String>) -> bool {
    func.blocks.iter().any(|b| {
        matches!(b.terminator, Terminator::Throw(_)) || b.instructions.iter().any(|(_, i, _)| instrucao_lanca(i, nao_lancam))
    })
}

/// As funções do módulo que não lançam.
pub fn nao_lancam(module: &Module) -> HashSet<String> {
    let mut conjunto: HashSet<String> = module.functions.iter().map(|f| f.symbol.clone()).collect();
    loop {
        let lancam: Vec<String> =
            module.functions.iter().filter(|f| conjunto.contains(&f.symbol) && funcao_lanca(f, &conjunto)).map(|f| f.symbol.clone()).collect();
        if lancam.is_empty() {
            return conjunto;
        }
        for s in lancam {
            conjunto.remove(&s);
        }
    }
}

/// Toda operação da função que pode lançar é conferida logo em seguida? (As
/// externas do SDK casadas pelo nome são conferidas no fim do comando; com
/// elas, uma conferência intermediária pode ser a que pega a exceção.)
pub fn confere_tudo(func: &Function, nao_lancam: &HashSet<String>) -> bool {
    func.blocks.iter().all(|b| {
        let n = b.instructions.len();
        (0..n).all(|i| {
            !instrucao_lanca(&b.instructions[i].1, nao_lancam)
                || (i + 1 < n && e_conferencia(&b.instructions[i + 1].1))
                || (i + 1 == n && sucessores(&b.terminator).is_empty())
        })
    })
}

pub fn e_conferencia(inst: &Instruction) -> bool {
    matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")
}
