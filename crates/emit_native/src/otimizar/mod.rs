//! Otimizações sobre a HIR, antes do LLVM (docs/NATIVO-PLANO.md §8).
//!
//! O LLVM só otimiza o que recebe: um objeto criado pelo runtime e lido por
//! chamadas opacas não some lá. Aqui, com a semântica do Dart à mão, os
//! passes eliminam o trabalho antes de ele virar chamada:
//!
//! 1. `mem2reg`: locais viram valores SSA;
//! 2. resumo de exceções (`efeitos`) e a conferência depois de uma chamada
//!    que não lança dobrada (`simplificar`);
//! 3. inlining das funções pequenas (`inline`) — construtores, getters,
//!    operadores —, também das que lançam (a conferência depois da chamada
//!    leva a exceção ao tratador de quem chama);
//! 4. substituição escalar dos objetos que não escapam (`escape`), e de
//!    novo `mem2reg` e as limpezas.
//!
//! `DARTFORGE_OTIMIZAR_HIR=0` desliga tudo (para comparar).

mod cfg;
mod efeitos;
mod escape;
mod inline;
mod mem2reg;
mod operandos;
mod simplificar;

#[cfg(test)]
mod testes;

use crate::hir::*;
use std::collections::HashMap;

/// Os passes desligados pelo ambiente?
fn desligado() -> bool {
    std::env::var("DARTFORGE_OTIMIZAR_HIR").is_ok_and(|v| v == "0")
}

fn limpar(func: &mut Function, nao_lancam: &std::collections::HashSet<String>) {
    // Numa função que não lança, a exceção nunca está pendente: o
    // `dartforge_exception_clear` do `return` (que descarta a pendente ao
    // sair de um `finally`) não tem o que limpar.
    if nao_lancam.contains(&func.symbol) {
        for b in &mut func.blocks {
            b.instructions.retain(|(_, i, _)| !matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear"));
        }
    }
    simplificar::conferencias_mortas(func, nao_lancam);
    simplificar::dobrar_constantes(func);
    simplificar::tirar_inalcancaveis(func);
    simplificar::juntar_blocos(func);
    simplificar::tirar_mortos(func);
}

/// Otimiza as funções do módulo.
pub fn otimizar(module: &mut Module) {
    if desligado() {
        return;
    }
    // A entrada é o primeiro bloco em todo o resto do compilador.
    let validas = |f: &Function| f.blocks.first().is_some_and(|b| b.id.0 == 0);
    for f in module.functions.iter_mut().filter(|f| validas(f)) {
        mem2reg::promover(f);
    }
    for _ in 0..3 {
        let nao_lancam = efeitos::nao_lancam(module);
        for f in module.functions.iter_mut().filter(|f| validas(f)) {
            limpar(f, &nao_lancam);
        }
        let copias: HashMap<String, (Function, bool)> = module
            .functions
            .iter()
            // J05: a função compilada com informação de depuração não é
            // embutida — o corpo copiado perderia a posição, e o breakpoint
            // nela nunca pararia.
            .filter(|f| f.depuracao.is_none() && inline::copiavel(f))
            .map(|f| (f.symbol.clone(), (f.clone(), !nao_lancam.contains(&f.symbol))))
            .collect();
        let mut mudou = false;
        for f in module.functions.iter_mut().filter(|f| validas(f)) {
            if inline::inlining(f, &copias) {
                mudou = true;
                limpar(f, &nao_lancam);
            }
        }
        if !mudou {
            break;
        }
    }
    let nao_lancam = efeitos::nao_lancam(module);
    for f in module.functions.iter_mut().filter(|f| validas(f)) {
        if escape::substituir_objetos(f) {
            mem2reg::promover(f);
            limpar(f, &nao_lancam);
        }
    }
}
