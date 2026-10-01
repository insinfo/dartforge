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
    let mut ordem = vec![usize::MAX; n];
    let mut baixo = vec![0usize; n];
    let mut na_pilha = vec![false; n];
    let mut pilha: Vec<usize> = Vec::new();
    let mut proximo = 0usize;
    for raiz in 0..n {
        if ordem[raiz] != usize::MAX {
            continue;
        }
        // (nó, próxima aresta a visitar)
        let mut chamadas: Vec<(usize, usize)> = vec![(raiz, 0)];
        ordem[raiz] = proximo;
        baixo[raiz] = proximo;
        proximo += 1;
        pilha.push(raiz);
        na_pilha[raiz] = true;
        while let Some(topo) = chamadas.last_mut() {
            let v = topo.0;
            if topo.1 < arestas[v].len() {
                let w = arestas[v][topo.1];
                topo.1 += 1;
                if w == v {
                    ciclo[v] = true;
                }
                if ordem[w] == usize::MAX {
                    ordem[w] = proximo;
                    baixo[w] = proximo;
                    proximo += 1;
                    pilha.push(w);
                    na_pilha[w] = true;
                    chamadas.push((w, 0));
                } else if na_pilha[w] {
                    baixo[v] = baixo[v].min(ordem[w]);
                }
                continue;
            }
            chamadas.pop();
            if let Some(&(pai, _)) = chamadas.last() {
                baixo[pai] = baixo[pai].min(baixo[v]);
            }
            if baixo[v] == ordem[v] {
                let mut componente = Vec::new();
                loop {
                    let w = pilha.pop().expect("componente na pilha");
                    na_pilha[w] = false;
                    componente.push(w);
                    if w == v {
                        break;
                    }
                }
                if componente.len() > 1 {
                    for w in componente {
                        ciclo[w] = true;
                    }
                }
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
