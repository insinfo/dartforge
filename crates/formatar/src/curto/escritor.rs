//! Porte de `short/line_writer.dart`: divide a lista de chunks em trechos
//! independentes, roda o divisor de linhas em cada um e escreve o texto,
//! formatando os blocos filhos recursivamente (com cache por coluna).

use std::collections::HashMap;

use super::arena::{Arenas, ChunkId};
use super::divisor::DivisorDeLinhas;

/// `FormatResult`.
#[derive(Debug, Clone)]
pub struct Resultado {
    pub texto: String,
    pub custo: i32,
}

/// Cache de blocos formatados, por (chunk, coluna).
pub type CacheDeBlocos = HashMap<(ChunkId, i32), Resultado>;

/// `LineWriter`.
pub struct EscritorDeLinhas<'c> {
    saida: String,
    chunks: Vec<ChunkId>,
    recuo_do_bloco: i32,
    cache: &'c mut CacheDeBlocos,
}

/// `formatBlock(chunk, column)`.
pub fn formatar_bloco(a: &mut Arenas, cache: &mut CacheDeBlocos, chunk: ChunkId, coluna: i32) -> Resultado {
    if let Some(r) = cache.get(&(chunk, coluna)) {
        return r.clone();
    }
    let filhos = a.chunks[chunk as usize].bloco.as_ref().expect("bloco").filhos.clone();
    let r = {
        let mut escritor = EscritorDeLinhas {
            saida: String::new(),
            chunks: filhos,
            recuo_do_bloco: coluna,
            cache,
        };
        escritor.escrever_linhas(a, false)
    };
    cache.insert((chunk, coluna), r.clone());
    r
}

impl<'c> EscritorDeLinhas<'c> {
    /// O escritor de topo.
    pub fn novo(chunks: Vec<ChunkId>, cache: &'c mut CacheDeBlocos) -> EscritorDeLinhas<'c> {
        EscritorDeLinhas {
            saida: String::new(),
            chunks,
            recuo_do_bloco: 0,
            cache,
        }
    }

    /// `writeLines(isCompilationUnit:)`.
    pub fn escrever_linhas(&mut self, a: &mut Arenas, unidade: bool) -> Resultado {
        let mut custo_total = 0;
        let mut inicio = 0;
        for i in 0..self.chunks.len() {
            let c = self.chunks[i];
            if !a.chunks[c as usize].divisivel {
                continue;
            }
            custo_total += self.completar_linha(a, inicio, i);
            inicio = i;
        }
        if inicio < self.chunks.len() {
            custo_total += self.completar_linha(a, inicio, self.chunks.len());
        }
        if unidade {
            self.saida.push_str(&a.fim_de_linha);
        }
        Resultado {
            texto: std::mem::take(&mut self.saida),
            custo: custo_total,
        }
    }

    fn completar_linha(&mut self, a: &mut Arenas, inicio: usize, fim: usize) -> i32 {
        let chunks: Vec<ChunkId> = self.chunks[inicio..fim].to_vec();
        let quebras = DivisorDeLinhas::novo(a, chunks.clone(), self.recuo_do_bloco).aplicar(a, self.cache);
        for (i, &c) in chunks.iter().enumerate() {
            if a.chunks[c as usize].bloco.is_some() {
                if !quebras.quebra_em(i) {
                    self.escrever_sem_quebras(a, c);
                } else {
                    self.saida.push_str(&a.fim_de_linha);
                    let bloco = formatar_bloco(a, self.cache, c, quebras.coluna(i));
                    self.saida.push_str(&bloco.texto);
                }
            }
            if quebras.quebra_em(i) {
                if !self.saida.is_empty() {
                    self.saida.push_str(&a.fim_de_linha);
                    if a.chunks[c as usize].duplo {
                        self.saida.push_str(&a.fim_de_linha);
                    }
                }
                for _ in 0..quebras.coluna(i) {
                    self.saida.push(' ');
                }
            } else if a.chunks[c as usize].espaco {
                self.saida.push(' ');
            }
            self.saida.push_str(&a.chunks[c as usize].texto);
        }
        quebras.custo
    }

    fn escrever_sem_quebras(&mut self, a: &Arenas, bloco: ChunkId) {
        let filhos = a.chunks[bloco as usize].bloco.as_ref().expect("bloco").filhos.clone();
        for c in filhos {
            if a.chunks[c as usize].espaco {
                self.saida.push(' ');
            }
            if a.chunks[c as usize].bloco.is_some() {
                self.escrever_sem_quebras(a, c);
            }
            self.saida.push_str(&a.chunks[c as usize].texto);
        }
    }
}
