//! Análises para inserir ownership ARC na HIR (§20 da especificação).
//!
//! A vivacidade recebe o inventário semântico de referências/slots; não
//! classifica externs, insere contadores ou prova consumo de tokens. Esses
//! estágios ainda precisam ser integrados ao pipeline de emissão.

use super::cfg::Cfg;
use super::operandos::{operandos, operandos_do_terminador};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Vivacidade dos valores classificados pelo chamador, só em blocos alcançáveis.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::Vivacidade;
/// let vazia = Vivacidade::default();
/// assert!(vazia.entrada.is_empty() && vazia.arestas.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct Vivacidade {
    /// Antes das definições de `Phi` na entrada do bloco.
    pub entrada: HashMap<BlockId, HashSet<ValueId>>,
    /// Depois do terminador, união das necessidades das arestas de saída.
    pub saida: HashMap<BlockId, HashSet<ValueId>>,
    /// Antes de cada instrução; entradas de `Phi` pertencem às arestas.
    pub antes: HashMap<ValueId, HashSet<ValueId>>,
    /// Depois de cada instrução.
    pub depois: HashMap<ValueId, HashSet<ValueId>>,
    /// Necessidade específica de `(predecessor, sucessor)`, incluindo apenas
    /// as entradas de `Phi` desse predecessor.
    pub arestas: HashMap<(BlockId, BlockId), HashSet<ValueId>>,
}

fn usos(o: &Operand, referencias: &HashSet<ValueId>, vivos: &mut HashSet<ValueId>) {
    if let Operand::Val(v) = o
        && referencias.contains(v)
    {
        vivos.insert(*v);
    }
}

fn na_aresta(
    f: &Function,
    origem: usize,
    destino: usize,
    entrada: &[HashSet<ValueId>],
    referencias: &HashSet<ValueId>,
) -> HashSet<ValueId> {
    let mut vivos = entrada[destino].clone();
    for (v, inst, _) in &f.blocks[destino].instructions {
        if let Instruction::Phi { incoming, .. } = inst {
            vivos.remove(v);
            for (de, op) in incoming {
                if *de == f.blocks[origem].id {
                    usos(op, referencias, &mut vivos);
                }
            }
        }
    }
    vivos
}

fn na_saida(
    f: &Function,
    cfg: &Cfg,
    b: usize,
    entrada: &[HashSet<ValueId>],
    referencias: &HashSet<ValueId>,
) -> HashSet<ValueId> {
    let mut vivos = HashSet::new();
    for &s in &cfg.sucessores[b] {
        vivos.extend(na_aresta(f, b, s, entrada, referencias));
    }
    vivos
}

fn recuar(
    v: ValueId,
    inst: &Instruction,
    referencias: &HashSet<ValueId>,
    vivos: &mut HashSet<ValueId>,
) {
    vivos.remove(&v);
    if !matches!(inst, Instruction::Phi { .. }) {
        operandos(inst, &mut |o| usos(o, referencias, vivos));
    }
}

/// Calcula vivacidade reversa em ponto fixo, com `Phi` por predecessor.
///
/// A função deve ter CFG/SSA válidos. `referencias` vem da classificação
/// semântica: não deduzimos ownership de `Type::I64` nem de `Type::Ptr`.
/// Incluir aliases com proveniência gerenciada e slots proprietários, quando
/// classificados. Dependências de borrows e extensões de vida precisam ser
/// materializadas como usos antes desta análise. No modo tabelas, executar
/// depois da preparação excepcional. Conjuntos não têm ordem de emissão;
/// ordenar IDs antes de materializar operações para manter determinismo.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::vivacidade};
/// use std::collections::HashSet;
/// let f = Function {
///     symbol: "identidade".into(), name: "identidade".into(),
///     params: vec![(ValueId(0), "x".into(), Type::Ref)],
///     return_ty: Type::Ref, depuracao: None,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
///         terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }],
/// };
/// let vivos = vivacidade(&f, &HashSet::from([ValueId(0)]));
/// assert!(vivos.entrada[&BlockId(0)].contains(&ValueId(0)));
/// assert!(vivos.saida[&BlockId(0)].is_empty());
/// ```
pub fn vivacidade(f: &Function, referencias: &HashSet<ValueId>) -> Vivacidade {
    let cfg = Cfg::novo(f);
    let mut entrada = vec![HashSet::new(); f.blocks.len()];
    let mut fila: Vec<usize> = cfg.rpo.clone();
    let mut na_fila = vec![false; f.blocks.len()];
    for &b in &fila {
        na_fila[b] = true;
    }
    while let Some(b) = fila.pop() {
        na_fila[b] = false;
        let mut vivos = na_saida(f, &cfg, b, &entrada, referencias);
        operandos_do_terminador(&f.blocks[b].terminator, &mut |o| {
            usos(o, referencias, &mut vivos)
        });
        for (v, inst, _) in f.blocks[b].instructions.iter().rev() {
            recuar(*v, inst, referencias, &mut vivos);
        }
        if vivos != entrada[b] {
            entrada[b] = vivos;
            for &p in &cfg.predecessores[b] {
                if cfg.alcancavel(p) && !na_fila[p] {
                    fila.push(p);
                    na_fila[p] = true;
                }
            }
        }
    }
    let mut resultado = Vivacidade::default();
    for &b in &cfg.rpo {
        let bloco = &f.blocks[b];
        let mut vivos = na_saida(f, &cfg, b, &entrada, referencias);
        resultado.entrada.insert(bloco.id, entrada[b].clone());
        resultado.saida.insert(bloco.id, vivos.clone());
        for &s in &cfg.sucessores[b] {
            resultado.arestas.insert(
                (bloco.id, f.blocks[s].id),
                na_aresta(f, b, s, &entrada, referencias),
            );
        }
        operandos_do_terminador(&bloco.terminator, &mut |o| usos(o, referencias, &mut vivos));
        for (v, inst, _) in bloco.instructions.iter().rev() {
            resultado.depois.insert(*v, vivos.clone());
            recuar(*v, inst, referencias, &mut vivos);
            resultado.antes.insert(*v, vivos.clone());
        }
    }
    resultado
}

#[cfg(test)]
mod testes;
