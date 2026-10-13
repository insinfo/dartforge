//! A colocação das raízes do coletor (G1, docs/NATIVO-PLANO.md §6.5) por
//! vivacidade: só um valor `Ref` vivo num ponto de coleta precisa estar no
//! quadro de raízes, e dois valores que nunca estão vivos juntos dividem o
//! mesmo slot (o passe de colocação de raízes do Julia faz o mesmo).
//!
//! * **Ponto de coleta** é toda instrução que pode alocar (chamada Dart, do
//!   runtime que aloca, caixa, conversão com caixa, constante de texto…),
//!   decidido pelo emissor (`may_collect`), mais o `throw` e o fim de um
//!   bloco que converte a entrada de um `phi` (a conversão pode encaixotar).
//! * **Enraizado** é o valor vivo na ENTRADA de um ponto de coleta —
//!   inclusive os operandos dele: o runtime conta com o chamador para manter
//!   vivos os argumentos durante a chamada (`dartforge_string_concat` aloca
//!   o resultado lendo os dois textos).
//! * **Interferem** dois valores enraizados quando um está vivo na
//!   definição do outro: o `store` da definição sobrescreve o slot, e o
//!   valor vivo perderia a raiz. Parâmetros interferem entre si e com o que
//!   está vivo na entrada; `phi` de um bloco, entre si e com o que está vivo
//!   no começo dele.
//! * Os `alloca` de tipo `Ref` (locais mutáveis, G2) continuam com slot
//!   próprio: o slot acompanha cada gravação.
//!
//! Um slot compartilhado guarda, entre usos, um valor já morto — que o
//! coletor marca (continua um handle válido, porque estava enraizado): só
//! atrasa a coleta dele até o slot ser reusado ou o quadro fechar.

use crate::hir::*;
use crate::lower::async_sm::{successors, uses_of, terminator_uses};
use std::collections::HashMap;

/// Um conjunto de valores `Ref` (índices densos).
#[derive(Clone, PartialEq, Eq)]
struct Set(Vec<u64>);

impl Set {
    fn empty(n: usize) -> Self {
        Set(vec![0; n.div_ceil(64)])
    }
    fn has(&self, i: usize) -> bool {
        self.0[i / 64] >> (i % 64) & 1 == 1
    }
    fn by(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
    fn take_out(&mut self, i: usize) {
        self.0[i / 64] &= !(1 << (i % 64));
    }
    fn join_set(&mut self, o: &Set) {
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a |= b;
        }
    }
    fn elements(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| {
            (0..64).filter(move |b| w >> b & 1 == 1).map(move |b| k * 64 + b)
        })
    }
}

/// O resultado da análise de raízes de uma função.
#[derive(Default)]
pub struct Roots {
    /// Os slots do quadro: `alloca`s `Ref` com slot próprio, depois os
    /// valores SSA enraizados, com slot compartilhado quando não interferem.
    pub slots: HashMap<ValueId, usize>,
    /// Os valores SSA enraizados (vivos na entrada de algum ponto de
    /// coleta), na ordem de definição.
    pub rooted_set: Vec<ValueId>,
    /// Para cada instrução que é ponto de coleta, os valores SSA `Ref` vivos
    /// na entrada dela (os operandos inclusive: o runtime conta com quem
    /// chama para mantê-los vivos durante a chamada).
    pub live_at: HashMap<ValueId, Vec<ValueId>>,
    /// O mesmo para o ponto de coleta do fim de um bloco (o `throw`, as
    /// conversões das entradas de `phi`).
    pub live_at_end: HashMap<BlockId, Vec<ValueId>>,
    /// Os valores enraizados vivos na entrada de cada bloco (depois dos
    /// `phi` dele); só os blocos com algum.
    pub live_at_entry_set: HashMap<BlockId, Vec<ValueId>>,
}

/// A análise completa: os slots (a pilha-sombra) e a vivacidade em cada
/// ponto de coleta (as raízes por mapas,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8).
pub fn analyze(
    func: &Function,
    types: &HashMap<ValueId, Type>,
    may_collect: &dyn Fn(&Instruction) -> bool,
    phi_converting_blocks: &dyn Fn(BlockId) -> bool,
) -> Roots {
    analyze_with(func, types, may_collect, phi_converting_blocks, &|_| false)
}

/// [`analyze`] com a chamada que enraíza os próprios argumentos:
/// `callee_roots(i)` diz que o ponto de coleta `i` é uma chamada a uma
/// função Dart com quadro de raízes próprio, sem conversão que aloque entre
/// os operandos e a chamada. A função chamada põe no quadro dela o
/// parâmetro que ainda usa depois de um ponto de coleta seu; o que só
/// vive até a chamada não precisa de raiz em quem chama (o coletor não move
/// objetos). Ali só fica enraizado o que vive DEPOIS da chamada. O runtime
/// continua contando com quem chama ([`analyze`]): as funções dele leem os
/// argumentos depois de alocar.
pub fn analyze_with(
    func: &Function,
    types: &HashMap<ValueId, Type>,
    may_collect: &dyn Fn(&Instruction) -> bool,
    phi_converting_blocks: &dyn Fn(BlockId) -> bool,
    callee_roots: &dyn Fn(&Instruction) -> bool,
) -> Roots {
    let mut slots = HashMap::new();
    for block in &func.blocks {
        for (vid, inst, _) in &block.instructions {
            if matches!(inst, Instruction::Alloca(Type::Ref)) {
                let n = slots.len();
                slots.insert(*vid, n);
            }
        }
    }
    let base = slots.len();

    // Os valores SSA `Ref` candidatos, na ordem de definição (o IR sai
    // determinístico): parâmetros, depois as instruções.
    let mut candidates: Vec<ValueId> = Vec::new();
    for (vid, _, ty) in &func.params {
        if *ty == Type::Ref {
            candidates.push(*vid);
        }
    }
    for block in &func.blocks {
        for (vid, inst, _) in &block.instructions {
            if types.get(vid) == Some(&Type::Ref) && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_)) {
                candidates.push(*vid);
            }
        }
    }
    if candidates.is_empty() {
        return Roots { slots, ..Roots::default() };
    }
    let index: HashMap<ValueId, usize> = candidates.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let n = candidates.len();
    let idx = |v: &ValueId| index.get(v).copied();

    let position: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    // O que cada bloco lê de cada predecessor pelos `phi` dele.
    let mut phi_uses: HashMap<(BlockId, BlockId), Vec<usize>> = HashMap::new();
    let mut phi_defs: Vec<Vec<usize>> = vec![Vec::new(); func.blocks.len()];
    for (bi, block) in func.blocks.iter().enumerate() {
        for (vid, inst, _) in &block.instructions {
            let Instruction::Phi { incoming, .. } = inst else { continue };
            if let Some(i) = idx(vid) {
                phi_defs[bi].push(i);
            }
            for (origin, op) in incoming {
                if let Operand::Val(v) = op
                    && let Some(i) = idx(v)
                {
                    phi_uses.entry((*origin, block.id)).or_default().push(i);
                }
            }
        }
    }

    // Vivacidade por bloco (ponto fixo), de trás para a frente.
    let live_at_exit_set = |b: usize, entry: &[Set]| -> Set {
        let mut s = Set::empty(n);
        let block = &func.blocks[b];
        for succ in successors(&block.terminator) {
            let Some(&p) = position.get(&succ) else { continue };
            let mut e = entry[p].clone();
            for &d in &phi_defs[p] {
                e.take_out(d);
            }
            s.join_set(&e);
            if let Some(u) = phi_uses.get(&(block.id, succ)) {
                for &i in u {
                    s.by(i);
                }
            }
        }
        s
    };
    // A passada de um bloco: a partir dos vivos na saída, visita cada
    // instrução (os vivos depois dela, a definição, os usos) e devolve os
    // vivos na entrada do bloco (depois dos `phi`, que não são usos).
    let passar = |b: usize,
                  exit: Set,
                  visit: &mut dyn FnMut(Option<&Instruction>, Option<ValueId>, Option<usize>, &Set, &Set)|
     -> Set {
        let block = &func.blocks[b];
        let mut live_set = exit;
        // O terminador: usos; `throw` e as conversões de `phi` no fim do
        // bloco são pontos de coleta.
        let mut before = live_set.clone();
        for v in terminator_uses(&block.terminator) {
            if let Some(i) = idx(&v) {
                before.by(i);
            }
        }
        if matches!(block.terminator, Terminator::Throw(_)) || phi_converting_blocks(block.id) {
            visit(None, None, None, &live_set, &before);
        }
        live_set = before;
        for (vid, inst, _) in block.instructions.iter().rev() {
            if matches!(inst, Instruction::Phi { .. }) {
                continue;
            }
            let d = idx(vid);
            let mut before = live_set.clone();
            if let Some(d) = d {
                before.take_out(d);
            }
            for v in uses_of(inst) {
                if let Some(i) = idx(&v) {
                    before.by(i);
                }
            }
            visit(Some(inst), Some(*vid), d, &live_set, &before);
            live_set = before;
        }
        live_set
    };

    let nb = func.blocks.len();
    let mut entry = vec![Set::empty(n); nb];
    loop {
        let mut changed = false;
        for b in (0..nb).rev() {
            let exit = live_at_exit_set(b, &entry);
            let e = passar(b, exit, &mut |_, _, _, _, _| {});
            if e != entry[b] {
                entry[b] = e;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // Os enraizados e as interferências.
    let mut rooted = Set::empty(n);
    let mut interfere: Vec<Set> = vec![Set::empty(n); n];
    let mark = |a: usize, live_set: &Set, interfere: &mut Vec<Set>| {
        for b in live_set.elements() {
            if b != a {
                interfere[a].by(b);
                interfere[b].by(a);
            }
        }
    };
    // Os pontos de coleta, com o que está vivo na entrada de cada um:
    // (a instrução, ou `None` no fim do bloco; o bloco; os vivos).
    let mut points: Vec<(Option<ValueId>, usize, Set)> = Vec::new();
    for b in 0..nb {
        let exit = live_at_exit_set(b, &entry);
        let e = passar(b, exit, &mut |inst, who, d, after, before| {
            let collection = inst.is_none_or(|i| may_collect(i));
            if collection {
                let roots = match inst {
                    Some(i) if callee_roots(i) => {
                        let mut r = after.clone();
                        if let Some(d) = d {
                            r.take_out(d);
                        }
                        r
                    }
                    _ => before.clone(),
                };
                rooted.join_set(&roots);
                points.push((who, b, roots));
            }
            if let Some(d) = d {
                mark(d, after, &mut interfere);
            }
        });
        // Os `phi` do bloco: definidos juntos no começo dele.
        let mut live_set = e;
        for &d in &phi_defs[b] {
            live_set.by(d);
        }
        for &d in &phi_defs[b] {
            mark(d, &live_set, &mut interfere);
        }
    }
    // Os parâmetros: definidos juntos na entrada.
    if let Some(first_one) = func.blocks.first() {
        let _ = first_one;
        let mut live_set = entry[0].clone();
        let params: Vec<usize> = func.params.iter().filter_map(|(v, _, _)| idx(v)).collect();
        for &p in &params {
            live_set.by(p);
        }
        for &p in &params {
            mark(p, &live_set, &mut interfere);
        }
    }

    // Cores: o menor slot livre entre os vizinhos já coloridos.
    let mut color: Vec<Option<usize>> = vec![None; n];
    for a in 0..n {
        if !rooted.has(a) {
            continue;
        }
        let mut occupied: Vec<bool> = Vec::new();
        for b in interfere[a].elements() {
            if let Some(c) = color[b] {
                if occupied.len() <= c {
                    occupied.resize(c + 1, false);
                }
                occupied[c] = true;
            }
        }
        let c = occupied.iter().position(|&o| !o).unwrap_or(occupied.len());
        color[a] = Some(c);
        slots.insert(candidates[a], base + c);
    }
    let values = |c: &Set| -> Vec<ValueId> { c.elements().filter(|&i| rooted.has(i)).map(|i| candidates[i]).collect() };
    let mut r = Roots { slots, rooted_set: values(&rooted), ..Roots::default() };
    for (who, b, live_set) in &points {
        let v = values(live_set);
        match who {
            Some(inst) => {
                r.live_at.insert(*inst, v);
            }
            None => {
                r.live_at_end.insert(func.blocks[*b].id, v);
            }
        }
    }
    for (b, e) in entry.iter().enumerate() {
        let v = values(e);
        if !v.is_empty() {
            r.live_at_entry_set.insert(func.blocks[b].id, v);
        }
    }
    r
}
