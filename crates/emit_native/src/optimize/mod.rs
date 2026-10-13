//! Otimizações sobre a HIR, antes do LLVM (docs/NATIVO-PLANO.md §8).
//!
//! O LLVM só otimiza o que recebe: um objeto criado pelo runtime e lido por
//! chamadas opacas não some lá. Aqui, com a semântica do Dart à mão, os
//! passes eliminam o trabalho antes de ele virar chamada:
//!
//! 1. `mem2reg`: locais viram valores SSA;
//! 2. resumo de exceções (`effects`) e a conferência depois de uma chamada
//!    que não lança dobrada (`simplify`);
//! 3. inlining das funções pequenas (`inline`) — construtores, getters,
//!    operadores —, também das que lançam (a conferência depois da chamada
//!    leva a exceção ao tratador de quem chama);
//! 4. substituição escalar dos objetos que não escapam (`escape`), e de
//!    novo `mem2reg` e as limpezas.
//!
//! `DARTFORGE_OTIMIZAR_HIR=0` desliga tudo (para comparar).

mod cfg;
pub mod arc;
mod effects;
pub(crate) mod scc;
mod escape;
mod inline;
mod direct_locals;
mod mem2reg;
pub(crate) mod operands;
mod simplify;
mod tables;

#[cfg(test)]
mod tests;

use crate::hir::*;
use std::collections::HashMap;

/// Prepara programa e bibliotecas do SDK pelo mesmo pipeline de emissão.
/// O modelo de memória fica definido antes dos passes; a materialização das
/// exceções permanece por último, pois seus IDs descrevem a HIR final.
/// A inserção de owners ARC ainda não faz parte deste pipeline (§20).
/// Sua integração deve ficar condicionada a `arc_memory` (`--memoria=arc`):
/// tracing não recebe fábricas Owned, retenções ou liberações ARC.
pub(crate) fn prepare_for_emission(module: &mut Module, arc_memory: bool, by_tables: bool) {
    module.arc_memory = arc_memory;
    optimize(module);
    if by_tables {
        let plan = tables::prepare(module);
        // A inserção/verificação de ownership deverá ocorrer aqui, sobre
        // o CFG excepcional preparado, antes de publicar tabelas LLVM.
        tables::materialize(module, plan);
    }
}

/// Os passes desligados pelo ambiente?
fn disabled() -> bool {
    std::env::var("DARTFORGE_OTIMIZAR_HIR").is_ok_and(|v| v == "0")
}

fn clear(func: &mut Function, do_not_throw: &std::collections::HashSet<String>) {
    // Numa função que não lança, a exceção nunca está pendente: o
    // `dartforge_exception_clear` do `return` (que descarta a pendente ao
    // sair de um `finally`) não tem o que limpar.
    if do_not_throw.contains(&func.symbol) {
        for b in &mut func.blocks {
            b.instructions.retain(|(_, i, _)| !matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear"));
        }
    }
    simplify::dead_checks(func, do_not_throw);
    simplify::fold_constants(func);
    simplify::remove_unreachable(func);
    simplify::join_blocks(func);
    simplify::remove_dead(func);
}

/// A entrada é o primeiro bloco em todo o resto do compilador.
fn valid(f: &Function) -> bool {
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

/// Aplica `pass` a cada função válida, em pedaços contíguos por thread: os
/// passes por função só leem o resto do módulo pelo que recebem pronto
/// (`do_not_throw`, as cópias do inliner), então o resultado é o mesmo da
/// ordem sequencial. Devolve se algum passe mudou alguma função. No
/// new_sali/backend (~180 mil funções) os passes levavam ~20 s numa thread.
fn in_parallel(functions: &mut [Function], pass: &(dyn Fn(&mut Function) -> bool + Sync)) -> bool {
    let n = threads();
    if n == 1 || functions.len() < 2048 {
        let mut changed = false;
        for f in functions.iter_mut().filter(|f| valid(f)) {
            changed |= pass(f);
        }
        return changed;
    }
    // Pedaços pequenos distribuídos sob demanda: as funções grandes (os
    // literais e as tabelas de dados) se agrupam, e um pedaço por thread
    // deixava uma thread com quase todo o trabalho.
    let chunks: Vec<std::sync::Mutex<&mut [Function]>> =
        functions.chunks_mut(256).map(std::sync::Mutex::new).collect();
    let next_item = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        let tasks: Vec<_> = (0..n)
            .map(|_| {
                let (chunks, next_item) = (&chunks, &next_item);
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn_scoped(s, move || {
                        let mut changed = false;
                        loop {
                            let i = next_item.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(p) = chunks.get(i) else { break };
                            let mut chunk = p.lock().unwrap_or_else(|e| e.into_inner());
                            for f in chunk.iter_mut().filter(|f| valid(f)) {
                                changed |= pass(f);
                            }
                        }
                        changed
                    })
                    .expect("thread dos passes da HIR")
            })
            .collect();
        tasks.into_iter().fold(false, |acc, t| acc | t.join().expect("passe da HIR em pânico"))
    })
}

/// Passa o módulo para as exceções por tabelas (`--excecoes=tables`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): o último passe sobre a HIR,
/// depois de [`optimize`] e logo antes da emissão. Sem ele o módulo fica no
/// modelo de sempre (a pendência conferida depois de cada chamada).
pub fn excecoes_por_tabelas(module: &mut Module) {
    tables::apply(module);
}

/// Otimiza as funções do módulo.
pub fn optimize(module: &mut Module) {
    if disabled() {
        return;
    }
    in_parallel(&mut module.functions, &|f| {
        mem2reg::promote(f);
        false
    });
    for _ in 0..3 {
        let do_not_throw = effects::do_not_throw(module);
        in_parallel(&mut module.functions, &|f| {
            clear(f, &do_not_throw);
            false
        });
        // A limpeza pode liberar IDs SSA que as próximas cópias reutilizam.
        // Não deixar uma origem morta aderir a uma instrução nova com esse ID.
        if module.arc_memory { arc::analysis::origins::prune(module); }
        let copies: HashMap<String, (Function, bool)> = module
            .functions
            .iter()
            // J05: a função compilada com informação de depuração não é
            // embutida — o corpo copiado perderia a posição, e o breakpoint
            // nela nunca pararia.
            .filter(|f| f.debug_info.is_none() && inline::copyable(f))
            .map(|f| (f.symbol.clone(), (f.clone(), !do_not_throw.contains(&f.symbol))))
            .collect();
        let origins = &module.arc_sites;
        let updated = std::sync::Mutex::new(HashMap::new());
        let arc_memory = module.arc_memory;
        let changed = in_parallel(&mut module.functions, &|f| {
            let changed = if arc_memory {
                let mut sites = origins.get(&f.symbol).cloned().unwrap_or_default();
                let changed = inline::inlining_with_origins(f, &copies, origins, &mut sites);
                if !sites.is_empty() {
                    updated.lock().unwrap().insert(f.symbol.clone(), sites);
                }
                changed
            } else {
                inline::inlining(f, &copies)
            };
            // O corpo embutido de uma função local direta deixa o local que
            // ela recebia por endereço só com `Load`/`Store`: promovível.
            if changed {
                mem2reg::promote(f);
                clear(f, &do_not_throw);
            }
            changed
        });
        if arc_memory { module.arc_sites.extend(updated.into_inner().unwrap()); }
        if !changed {
            break;
        }
    }
    let do_not_throw = effects::do_not_throw(module);
    in_parallel(&mut module.functions, &|f| {
        if escape::replace_objects(f) {
            mem2reg::promote(f);
            clear(f, &do_not_throw);
        }
        false
    });
    if module.arc_memory {
        direct_locals::prune(module);
        arc::analysis::origins::prune(module);
    }
}
