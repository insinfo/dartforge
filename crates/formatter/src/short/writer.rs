//! Porte de `short/line_writer.dart`: divide a lista de chunks em trechos
//! independentes, roda o divisor de linhas em cada um e escreve o texto,
//! formatando os blocos filhos recursivamente (com cache por coluna).

use std::collections::HashMap;

use super::arena::{Arenas, ChunkId};
use super::splitter::LineSplitter;

/// `FormatResult`.
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub texto: String,
    pub cost: i32,
}

/// Cache de blocos formatados, por (chunk, coluna).
pub type BlockCache = HashMap<(ChunkId, i32), WriteResult>;

/// `LineWriter`.
pub struct LineWriter<'c> {
    output: String,
    chunks: Vec<ChunkId>,
    block_indent: i32,
    cache: &'c mut BlockCache,
}

/// `formatBlock(chunk, column)`.
pub fn format_block(a: &mut Arenas, cache: &mut BlockCache, chunk: ChunkId, column: i32) -> WriteResult {
    if let Some(r) = cache.get(&(chunk, column)) {
        return r.clone();
    }
    let children = a.chunks[chunk as usize].block.as_ref().expect("bloco").children.clone();
    let r = {
        let mut writer = LineWriter {
            output: String::new(),
            chunks: children,
            block_indent: column,
            cache,
        };
        writer.write_lines(a, false)
    };
    cache.insert((chunk, column), r.clone());
    r
}

impl<'c> LineWriter<'c> {
    /// O escritor de topo.
    pub fn new(chunks: Vec<ChunkId>, cache: &'c mut BlockCache) -> LineWriter<'c> {
        LineWriter {
            output: String::new(),
            chunks,
            block_indent: 0,
            cache,
        }
    }

    /// `writeLines(isCompilationUnit:)`.
    pub fn write_lines(&mut self, a: &mut Arenas, unit: bool) -> WriteResult {
        let mut total_cost = 0;
        let mut start = 0;
        for i in 0..self.chunks.len() {
            let c = self.chunks[i];
            if !a.chunks[c as usize].splittable {
                continue;
            }
            total_cost += self.complete_line(a, start, i);
            start = i;
        }
        if start < self.chunks.len() {
            total_cost += self.complete_line(a, start, self.chunks.len());
        }
        if unit {
            self.output.push_str(&a.line_ending);
        }
        WriteResult {
            texto: std::mem::take(&mut self.output),
            cost: total_cost,
        }
    }

    fn complete_line(&mut self, a: &mut Arenas, start: usize, end: usize) -> i32 {
        let chunks: Vec<ChunkId> = self.chunks[start..end].to_vec();
        let splits = LineSplitter::new(a, chunks.clone(), self.block_indent).apply(a, self.cache);
        for (i, &c) in chunks.iter().enumerate() {
            if a.chunks[c as usize].block.is_some() {
                if !splits.should_split_at(i) {
                    self.write_without_splits(a, c);
                } else {
                    self.output.push_str(&a.line_ending);
                    let block = format_block(a, self.cache, c, splits.column(i));
                    self.output.push_str(&block.texto);
                }
            }
            if splits.should_split_at(i) {
                if !self.output.is_empty() {
                    self.output.push_str(&a.line_ending);
                    if a.chunks[c as usize].double {
                        self.output.push_str(&a.line_ending);
                    }
                }
                for _ in 0..splits.column(i) {
                    self.output.push(' ');
                }
            } else if a.chunks[c as usize].space {
                self.output.push(' ');
            }
            self.output.push_str(&a.chunks[c as usize].texto);
        }
        splits.cost
    }

    fn write_without_splits(&mut self, a: &Arenas, block: ChunkId) {
        let children = a.chunks[block as usize].block.as_ref().expect("bloco").children.clone();
        for c in children {
            if a.chunks[c as usize].space {
                self.output.push(' ');
            }
            if a.chunks[c as usize].block.is_some() {
                self.write_without_splits(a, c);
            }
            self.output.push_str(&a.chunks[c as usize].texto);
        }
    }
}
