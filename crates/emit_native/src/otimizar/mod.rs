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
pub mod arc;
mod efeitos;
mod escape;
mod inline;
mod mem2reg;
pub(crate) mod operandos;
mod simplificar;
mod tabelas;

#[cfg(test)]
mod testes;

use crate::hir::*;
use std::collections::HashMap;

/// Prepara programa e bibliotecas do SDK pelo mesmo pipeline de emissão.
/// O modelo de memória fica definido antes dos passes; a materialização das
/// exceções permanece por último, pois seus IDs descrevem a HIR final.
/// A inserção de owners ARC ainda não faz parte deste pipeline (§20).
/// Sua integração deve ficar condicionada a `memoria_arc` (`--memoria=arc`):
/// tracing não recebe fábricas Owned, retenções ou liberações ARC.
pub(crate) fn preparar_para_emissao(module: &mut Module, memoria_arc: bool, por_tabelas: bool) {
    module.memoria_arc = memoria_arc;
    otimizar(module);
    if por_tabelas {
        let plano = tabelas::preparar(module);
        // A inserção/verificação de ownership deverá ocorrer aqui, sobre
        // o CFG excepcional preparado, antes de publicar tabelas LLVM.
        tabelas::materializar(module, plano);
    }
}

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

/// A entrada é o primeiro bloco em todo o resto do compilador.
fn valida(f: &Function) -> bool {
    f.blocks.first().is_some_and(|b| b.id.0 == 0)
}

/// Quantas threads os passes por função usam: as da máquina, no máximo 4
/// (`DARTFORGE_THREADS_HIR` escolhe; 1 = sequencial).
fn threads() -> usize {
    std::env::var("DARTFORGE_THREADS_HIR")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(4)))
        .max(1)
}

/// Aplica `passe` a cada função válida, em pedaços contíguos por thread: os
/// passes por função só leem o resto do módulo pelo que recebem pronto
/// (`nao_lancam`, as cópias do inliner), então o resultado é o mesmo da
/// ordem sequencial. Devolve se algum passe mudou alguma função. No
/// new_sali/backend (~180 mil funções) os passes levavam ~20 s numa thread.
fn em_paralelo(funcoes: &mut [Function], passe: &(dyn Fn(&mut Function) -> bool + Sync)) -> bool {
    let n = threads();
    if n == 1 || funcoes.len() < 2048 {
        let mut mudou = false;
        for f in funcoes.iter_mut().filter(|f| valida(f)) {
            mudou |= passe(f);
        }
        return mudou;
    }
    // Pedaços pequenos distribuídos sob demanda: as funções grandes (os
    // literais e as tabelas de dados) se agrupam, e um pedaço por thread
    // deixava uma thread com quase todo o trabalho.
    let pedacos: Vec<std::sync::Mutex<&mut [Function]>> =
        funcoes.chunks_mut(256).map(std::sync::Mutex::new).collect();
    let proximo = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        let tarefas: Vec<_> = (0..n)
            .map(|_| {
                let (pedacos, proximo) = (&pedacos, &proximo);
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn_scoped(s, move || {
                        let mut mudou = false;
                        loop {
                            let i = proximo.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(p) = pedacos.get(i) else { break };
                            let mut pedaco = p.lock().unwrap_or_else(|e| e.into_inner());
                            for f in pedaco.iter_mut().filter(|f| valida(f)) {
                                mudou |= passe(f);
                            }
                        }
                        mudou
                    })
                    .expect("thread dos passes da HIR")
            })
            .collect();
        tarefas.into_iter().fold(false, |acc, t| acc | t.join().expect("passe da HIR em pânico"))
    })
}

/// Passa o módulo para as exceções por tabelas (`--excecoes=tabelas`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): o último passe sobre a HIR,
/// depois de [`otimizar`] e logo antes da emissão. Sem ele o módulo fica no
/// modelo de sempre (a pendência conferida depois de cada chamada).
pub fn excecoes_por_tabelas(module: &mut Module) {
    tabelas::aplicar(module);
}

/// Otimiza as funções do módulo.
pub fn otimizar(module: &mut Module) {
    if desligado() {
        return;
    }
    em_paralelo(&mut module.functions, &|f| {
        mem2reg::promover(f);
        false
    });
    for _ in 0..3 {
        let nao_lancam = efeitos::nao_lancam(module);
        em_paralelo(&mut module.functions, &|f| {
            limpar(f, &nao_lancam);
            false
        });
        let copias: HashMap<String, (Function, bool)> = module
            .functions
            .iter()
            // J05: a função compilada com informação de depuração não é
            // embutida — o corpo copiado perderia a posição, e o breakpoint
            // nela nunca pararia.
            .filter(|f| f.depuracao.is_none() && inline::copiavel(f))
            .map(|f| (f.symbol.clone(), (f.clone(), !nao_lancam.contains(&f.symbol))))
            .collect();
        let mudou = em_paralelo(&mut module.functions, &|f| {
            if inline::inlining(f, &copias) {
                limpar(f, &nao_lancam);
                true
            } else {
                false
            }
        });
        if !mudou {
            break;
        }
    }
    let nao_lancam = efeitos::nao_lancam(module);
    em_paralelo(&mut module.functions, &|f| {
        if escape::substituir_objetos(f) {
            mem2reg::promover(f);
            limpar(f, &nao_lancam);
        }
        false
    });
}
