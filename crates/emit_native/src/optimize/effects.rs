//! Resumo de exceções por função: quem não pode deixar exceção pendente.
//!
//! No código gerado, uma exceção é um estado pendente do runtime que quem
//! chama confere logo depois da chamada (`dartforge_exception_pending`,
//! `FnBuilder::emit_call_with_check`). Em toda chamada o estado está limpo
//! (cada operação que pode lançar é conferida em seguida), então, depois de
//! uma chamada a uma função que não pode lançar, a conferência é sempre
//! falsa. O resumo é o maior ponto fixo: começa com todas as funções do
//! módulo sem lançar e tira as que têm uma operação que lança. Uma função
//! num ciclo de chamadas (a recursão, direta ou mútua) lança: o prólogo
//! confere a pilha e lança `StackOverflowError` (`llvm/mod.rs`,
//! `emit_stack_check`; docs/NATIVO-PROJETOS-REAIS.md C22) — sem
//! isso `void f() => f();` não tinha conferência nenhuma, nem o contexto em
//! que a da pilha mora, e estourava a pilha do sistema.

use super::cfg::successors;
use crate::hir::*;
use crate::llvm::externs::effects_of;
use std::collections::HashSet;

/// A instrução pode deixar exceção pendente?
pub fn instruction_throws(inst: &Instruction, do_not_throw: &HashSet<String>) -> bool {
    match inst {
        Instruction::CallStatic { symbol, .. } => !do_not_throw.contains(symbol),
        Instruction::CallRuntime { name, .. } => effects_of(name).throws,
        Instruction::CallInterface { .. }
        | Instruction::CallDynamic { .. }
        | Instruction::CallClosure { .. }
        | Instruction::CallSelector { .. }
        | Instruction::CallSelectorForward { .. }
        | Instruction::CallClosureForward { .. }
        | Instruction::NativeCall { .. }
        | Instruction::TypedCall { .. }
        | Instruction::CompositeNativeCall { .. } => true,
        // Caixa de tipo errado ou nula, null, divisão por zero, `toInt` de
        // NaN ou infinito.
        Instruction::Unbox { to, .. } => !to.is_vector(),
        Instruction::CheckNotNull(_) | Instruction::SDiv(..) | Instruction::SRem(..) | Instruction::DoubleToInt(_) => true,
        _ => false,
    }
}

/// A função pode lançar, dado o conjunto corrente das que não lançam?
fn function_throws(func: &Function, do_not_throw: &HashSet<String>) -> bool {
    func.blocks.iter().any(|b| {
        matches!(b.terminator, Terminator::Throw(_)) || b.instructions.iter().any(|(_, i, _)| instruction_throws(i, do_not_throw))
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
pub fn do_not_throw(module: &Module) -> HashSet<String> {
    use std::collections::HashMap;
    let all: HashSet<String> = module.functions.iter().map(|f| f.symbol.clone()).collect();
    let mut callers: HashMap<&str, Vec<usize>> = HashMap::new();
    for (k, f) in module.functions.iter().enumerate() {
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                if let Instruction::CallStatic { symbol, .. } = i {
                    callers.entry(symbol.as_str()).or_default().push(k);
                }
            }
        }
    }
    let mut throws: HashSet<&str> = HashSet::new();
    let recursive = in_cycle(module);
    let mut queue: Vec<usize> = module
        .functions
        .iter()
        .enumerate()
        .filter(|(k, f)| recursive[*k] || function_throws(f, &all))
        .map(|(k, _)| k)
        .collect();
    while let Some(k) = queue.pop() {
        let s = module.functions[k].symbol.as_str();
        if !throws.insert(s) {
            continue;
        }
        if let Some(cs) = callers.get(s) {
            queue.extend(cs.iter().copied().filter(|&c| !throws.contains(module.functions[c].symbol.as_str())));
        }
    }
    all.into_iter().filter(|s| !throws.contains(s.as_str())).collect()
}

/// As funções do módulo que estão num ciclo de chamadas diretas
/// (`CallStatic`): os componentes fortemente conexos com mais de uma função,
/// ou a função que chama a si mesma (Tarjan, iterativo).
fn in_cycle(module: &Module) -> Vec<bool> {
    use std::collections::HashMap;
    let n = module.functions.len();
    let index: HashMap<&str, usize> = module.functions.iter().enumerate().map(|(k, f)| (f.symbol.as_str(), k)).collect();
    let edges: Vec<Vec<usize>> = module
        .functions
        .iter()
        .map(|f| {
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter())
                .filter_map(|(_, i, _)| match i {
                    Instruction::CallStatic { symbol, .. } => index.get(symbol.as_str()).copied(),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let mut cycle = vec![false; n];
    for component in super::scc::components(&edges) {
        if component.len() > 1 || edges[component[0]].contains(&component[0]) {
            for v in component {
                cycle[v] = true;
            }
        }
    }
    cycle
}

/// Toda operação da função que pode lançar é conferida logo em seguida? (As
/// externas do SDK casadas pelo nome são conferidas no fim do comando; com
/// elas, uma conferência intermediária pode ser a que pega a exceção.)
pub fn checks_all(func: &Function, do_not_throw: &HashSet<String>) -> bool {
    func.blocks.iter().all(|b| {
        let n = b.instructions.len();
        (0..n).all(|i| {
            !instruction_throws(&b.instructions[i].1, do_not_throw)
                || (i + 1 < n && is_check(&b.instructions[i + 1].1))
                || (i + 1 == n && successors(&b.terminator).is_empty())
        })
    })
}

pub fn is_check(inst: &Instruction) -> bool {
    matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending")
}

#[cfg(test)]
mod scc_tests {
    use super::*;

    #[test]
    fn recursion_requires_stack_and_propagates_failure_without_contaminating_leaf() {
        let mut m = Module::new();
        for (symbol, dests) in [
            ("folha", vec![]), ("auto", vec!["auto"]),
            ("a", vec!["b", "folha"]), ("b", vec!["a"]),
            ("chamador", vec!["a"]), ("externo", vec!["ausente"]),
        ] {
            m.functions.push(Function {
                symbol: symbol.into(), name: symbol.into(), debug_info: None,
                params: vec![], return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: dests.into_iter().enumerate().map(|(i, dest)| (
                        ValueId(i as u32), Instruction::CallStatic {
                            symbol: dest.into(), args: vec![], ret_ty: Type::Void,
                        }, Type::Void,
                    )).collect(),
                    terminator: Terminator::Return(None),
                }],
            });
        }
        assert_eq!(in_cycle(&m), vec![false, true, true, true, false, false]);
        assert_eq!(do_not_throw(&m), HashSet::from(["folha".into()]));
    }
}
