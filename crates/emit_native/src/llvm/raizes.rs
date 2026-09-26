//! A colocação das raízes do coletor (G1, docs/NATIVO-PLANO.md §6.5) por
//! vivacidade: só um valor `Ref` vivo num ponto de coleta precisa estar no
//! quadro de raízes, e dois valores que nunca estão vivos juntos dividem o
//! mesmo slot (o passe de colocação de raízes do Julia faz o mesmo).
//!
//! * **Ponto de coleta** é toda instrução que pode alocar (chamada Dart, do
//!   runtime que aloca, caixa, conversão com caixa, constante de texto…),
//!   decidido pelo emissor (`pode_coletar`), mais o `throw` e o fim de um
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
use crate::lower::async_sm::{sucessores, usos_de, usos_do_terminador};
use std::collections::HashMap;

/// Um conjunto de valores `Ref` (índices densos).
#[derive(Clone, PartialEq, Eq)]
struct Conjunto(Vec<u64>);

impl Conjunto {
    fn vazio(n: usize) -> Self {
        Conjunto(vec![0; n.div_ceil(64)])
    }
    fn tem(&self, i: usize) -> bool {
        self.0[i / 64] >> (i % 64) & 1 == 1
    }
    fn por(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
    fn tirar(&mut self, i: usize) {
        self.0[i / 64] &= !(1 << (i % 64));
    }
    fn unir(&mut self, o: &Conjunto) {
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a |= b;
        }
    }
    fn elementos(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| {
            (0..64).filter(move |b| w >> b & 1 == 1).map(move |b| k * 64 + b)
        })
    }
}

/// Os slots do quadro: `alloca`s `Ref` com slot próprio, depois os valores
/// SSA enraizados, com slot compartilhado quando não interferem.
pub fn atribuir_slots(
    func: &Function,
    tipos: &HashMap<ValueId, Type>,
    pode_coletar: &dyn Fn(&Instruction) -> bool,
    blocos_que_convertem_phi: &dyn Fn(BlockId) -> bool,
) -> HashMap<ValueId, usize> {
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
    let mut candidatos: Vec<ValueId> = Vec::new();
    for (vid, _, ty) in &func.params {
        if *ty == Type::Ref {
            candidatos.push(*vid);
        }
    }
    for block in &func.blocks {
        for (vid, inst, _) in &block.instructions {
            if tipos.get(vid) == Some(&Type::Ref) && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_)) {
                candidatos.push(*vid);
            }
        }
    }
    if candidatos.is_empty() {
        return slots;
    }
    let indice: HashMap<ValueId, usize> = candidatos.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let n = candidatos.len();
    let idx = |v: &ValueId| indice.get(v).copied();

    let posicao: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    // O que cada bloco lê de cada predecessor pelos `phi` dele.
    let mut usos_de_phi: HashMap<(BlockId, BlockId), Vec<usize>> = HashMap::new();
    let mut defs_de_phi: Vec<Vec<usize>> = vec![Vec::new(); func.blocks.len()];
    for (bi, block) in func.blocks.iter().enumerate() {
        for (vid, inst, _) in &block.instructions {
            let Instruction::Phi { incoming, .. } = inst else { continue };
            if let Some(i) = idx(vid) {
                defs_de_phi[bi].push(i);
            }
            for (origem, op) in incoming {
                if let Operand::Val(v) = op
                    && let Some(i) = idx(v)
                {
                    usos_de_phi.entry((*origem, block.id)).or_default().push(i);
                }
            }
        }
    }

    // Vivacidade por bloco (ponto fixo), de trás para a frente.
    let vivos_na_saida = |b: usize, entrada: &[Conjunto]| -> Conjunto {
        let mut s = Conjunto::vazio(n);
        let block = &func.blocks[b];
        for suc in sucessores(&block.terminator) {
            let Some(&p) = posicao.get(&suc) else { continue };
            let mut e = entrada[p].clone();
            for &d in &defs_de_phi[p] {
                e.tirar(d);
            }
            s.unir(&e);
            if let Some(u) = usos_de_phi.get(&(block.id, suc)) {
                for &i in u {
                    s.por(i);
                }
            }
        }
        s
    };
    // A passada de um bloco: a partir dos vivos na saída, visita cada
    // instrução (os vivos depois dela, a definição, os usos) e devolve os
    // vivos na entrada do bloco (depois dos `phi`, que não são usos).
    let passar = |b: usize, saida: Conjunto, visitar: &mut dyn FnMut(Option<&Instruction>, Option<usize>, &Conjunto, &Conjunto)| -> Conjunto {
        let block = &func.blocks[b];
        let mut vivos = saida;
        // O terminador: usos; `throw` e as conversões de `phi` no fim do
        // bloco são pontos de coleta.
        let mut antes = vivos.clone();
        for v in usos_do_terminador(&block.terminator) {
            if let Some(i) = idx(&v) {
                antes.por(i);
            }
        }
        if matches!(block.terminator, Terminator::Throw(_)) || blocos_que_convertem_phi(block.id) {
            visitar(None, None, &vivos, &antes);
        }
        vivos = antes;
        for (vid, inst, _) in block.instructions.iter().rev() {
            if matches!(inst, Instruction::Phi { .. }) {
                continue;
            }
            let d = idx(vid);
            let mut antes = vivos.clone();
            if let Some(d) = d {
                antes.tirar(d);
            }
            for v in usos_de(inst) {
                if let Some(i) = idx(&v) {
                    antes.por(i);
                }
            }
            visitar(Some(inst), d, &vivos, &antes);
            vivos = antes;
        }
        vivos
    };

    let nb = func.blocks.len();
    let mut entrada = vec![Conjunto::vazio(n); nb];
    loop {
        let mut mudou = false;
        for b in (0..nb).rev() {
            let saida = vivos_na_saida(b, &entrada);
            let e = passar(b, saida, &mut |_, _, _, _| {});
            if e != entrada[b] {
                entrada[b] = e;
                mudou = true;
            }
        }
        if !mudou {
            break;
        }
    }

    // Os enraizados e as interferências.
    let mut enraizado = Conjunto::vazio(n);
    let mut interfere: Vec<Conjunto> = vec![Conjunto::vazio(n); n];
    let marcar = |a: usize, vivos: &Conjunto, interfere: &mut Vec<Conjunto>| {
        for b in vivos.elementos() {
            if b != a {
                interfere[a].por(b);
                interfere[b].por(a);
            }
        }
    };
    for b in 0..nb {
        let saida = vivos_na_saida(b, &entrada);
        let e = passar(b, saida, &mut |inst, d, depois, antes| {
            let coleta = inst.is_none_or(|i| pode_coletar(i));
            if coleta {
                enraizado.unir(antes);
            }
            if let Some(d) = d {
                marcar(d, depois, &mut interfere);
            }
        });
        // Os `phi` do bloco: definidos juntos no começo dele.
        let mut vivos = e;
        for &d in &defs_de_phi[b] {
            vivos.por(d);
        }
        for &d in &defs_de_phi[b] {
            marcar(d, &vivos, &mut interfere);
        }
    }
    // Os parâmetros: definidos juntos na entrada.
    if let Some(primeiro) = func.blocks.first() {
        let _ = primeiro;
        let mut vivos = entrada[0].clone();
        let params: Vec<usize> = func.params.iter().filter_map(|(v, _, _)| idx(v)).collect();
        for &p in &params {
            vivos.por(p);
        }
        for &p in &params {
            marcar(p, &vivos, &mut interfere);
        }
    }

    // Cores: o menor slot livre entre os vizinhos já coloridos.
    let mut cor: Vec<Option<usize>> = vec![None; n];
    for a in 0..n {
        if !enraizado.tem(a) {
            continue;
        }
        let mut ocupados: Vec<bool> = Vec::new();
        for b in interfere[a].elementos() {
            if let Some(c) = cor[b] {
                if ocupados.len() <= c {
                    ocupados.resize(c + 1, false);
                }
                ocupados[c] = true;
            }
        }
        let c = ocupados.iter().position(|&o| !o).unwrap_or(ocupados.len());
        cor[a] = Some(c);
        slots.insert(candidatos[a], base + c);
    }
    slots
}
