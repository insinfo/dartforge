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
//! `emitir_conferencia_da_pilha`; docs/NATIVO-PROJETOS-REAIS.md C22) — sem
//! isso `void f() => f();` não tinha conferência nenhuma, nem o contexto em
//! que a da pilha mora, e estourava a pilha do sistema.

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
    let recursivas = em_ciclo(module);
    let mut fila: Vec<usize> = module
        .functions
        .iter()
        .enumerate()
        .filter(|(k, f)| recursivas[*k] || funcao_lanca(f, &todos))
        .map(|(k, _)| k)
        .collect();
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

/// As funções do módulo que estão num ciclo de chamadas diretas
/// (`CallStatic`): os componentes fortemente conexos com mais de uma função,
/// ou a função que chama a si mesma (Tarjan, iterativo).
fn em_ciclo(module: &Module) -> Vec<bool> {
    use std::collections::HashMap;
    let n = module.functions.len();
    let indice: HashMap<&str, usize> = module.functions.iter().enumerate().map(|(k, f)| (f.symbol.as_str(), k)).collect();
    let arestas: Vec<Vec<usize>> = module
        .functions
        .iter()
        .map(|f| {
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter())
                .filter_map(|(_, i, _)| match i {
                    Instruction::CallStatic { symbol, .. } => indice.get(symbol.as_str()).copied(),
                    _ => None,
                })
                .collect()
        })
        .collect();
    let mut ciclo = vec![false; n];
    for componente in super::scc::componentes(&arestas) {
        if componente.len() > 1 || arestas[componente[0]].contains(&componente[0]) {
            for v in componente {
                ciclo[v] = true;
            }
        }
    }
    ciclo
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

#[cfg(test)]
mod testes_scc {
    use super::*;

    #[test]
    fn recursao_exige_pilha_e_propaga_falha_sem_contaminar_folha() {
        let mut m = Module::new();
        for (simbolo, destinos) in [
            ("folha", vec![]), ("auto", vec!["auto"]),
            ("a", vec!["b", "folha"]), ("b", vec!["a"]),
            ("chamador", vec!["a"]), ("externo", vec!["ausente"]),
        ] {
            m.functions.push(Function {
                symbol: simbolo.into(), name: simbolo.into(), depuracao: None,
                params: vec![], return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: destinos.into_iter().enumerate().map(|(i, destino)| (
                        ValueId(i as u32), Instruction::CallStatic {
                            symbol: destino.into(), args: vec![], ret_ty: Type::Void,
                        }, Type::Void,
                    )).collect(),
                    terminator: Terminator::Return(None),
                }],
            });
        }
        assert_eq!(em_ciclo(&m), vec![false, true, true, true, false, false]);
        assert_eq!(nao_lancam(&m), HashSet::from(["folha".into()]));
    }
}
