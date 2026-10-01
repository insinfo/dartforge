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
///
/// O maior ponto fixo de cima, calculado por lista de trabalho: uma função
/// lança se tem uma operação que lança por si (com todas as do módulo
/// supostas sem lançar) ou se chama (`CallStatic`) uma que lança. Parte das
/// que lançam por si e sobe pelas arestas inversas de chamada. Era uma
/// varredura do módulo inteiro por rodada, uma rodada por nível da cadeia de
/// chamadas — quadrática num programa real (o new_sali/backend).
pub fn nao_lancam(module: &Module) -> HashSet<String> {
    use std::collections::HashMap;
    let todos: HashSet<String> = module.functions.iter().map(|f| f.symbol.clone()).collect();
    let mut chamadores: HashMap<&str, Vec<usize>> = HashMap::new();
    for (k, f) in module.functions.iter().enumerate() {
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                if let Instruction::CallStatic { symbol, .. } = i {
                    chamadores.entry(symbol.as_str()).or_default().push(k);
                }
            }
        }
    }
    let mut lanca: HashSet<&str> = HashSet::new();
    let mut fila: Vec<usize> =
        module.functions.iter().enumerate().filter(|(_, f)| funcao_lanca(f, &todos)).map(|(k, _)| k).collect();
    while let Some(k) = fila.pop() {
        let s = module.functions[k].symbol.as_str();
        if !lanca.insert(s) {
            continue;
        }
        if let Some(cs) = chamadores.get(s) {
            fila.extend(cs.iter().copied().filter(|&c| !lanca.contains(module.functions[c].symbol.as_str())));
        }
    }
    todos.into_iter().filter(|s| !lanca.contains(s.as_str())).collect()
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
