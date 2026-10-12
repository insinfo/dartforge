//! Edições mínimas entre o código original e o formatado: porte do
//! `_MinimalEditComputer` do analysis server 3.6.2
//! (`pkg/analysis_server/lib/src/lsp/source_edits.dart`), que o
//! `textDocument/formatting` do `dart language-server` usa para devolver só
//! as trocas de brancos entre os tokens em vez do documento inteiro.
//!
//! O original percorre o fluxo de tokens do scanner do analyzer (com os
//! comentários) dos dois textos. Aqui o fluxo é o de [`Tokens`]; as
//! diferenças de tokenização (`>>` e `[]` separados) não mudam as edições,
//! porque o formatador nunca põe brancos dentro desses pares e as trocas
//! permitidas pelo original (`>>` por `>` `>`, `[]` por `[` `]`) dão a mesma
//! faixa de brancos.

use crate::syntax::token::{TokenKind, Tokens};
use dartforge_frontend::token::Op;

/// Uma edição: substituir `source[start..end]` (bytes do original) por `texto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edicao {
    pub inicio: usize,
    pub fim: usize,
    pub texto: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    Comment { line: bool },
    Token(TokenKind),
}

#[derive(Debug, Clone, Copy)]
struct Item {
    tipo: Type,
    start: usize,
    end: usize,
}

/// `_iterateAllTokens`: comentários e tokens em ordem, sem o EOF (nem os
/// comentários antes dele).
fn items(t: &Tokens) -> Vec<Item> {
    let mut v = Vec::new();
    for (i, k) in t.list.iter().enumerate() {
        if k.tipo == TokenKind::Eof {
            break;
        }
        for c in t.comments_of(i as u32) {
            v.push(Item {
                tipo: Type::Comment { line: c.is_line },
                start: c.start,
                end: c.end,
            });
        }
        v.push(Item {
            tipo: Type::Token(k.tipo),
            start: k.start,
            end: k.end,
        });
    }
    v
}

fn optional(i: &Item) -> bool {
    matches!(i.tipo, Type::Token(TokenKind::Op(Op::Comma)) | Type::Token(TokenKind::Op(Op::Semicolon)))
}

fn valid_change(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace() || matches!(c, ',' | ';' | '<' | '>'))
}

/// As edições mínimas que levam `original` a `formatted`.
///
/// Com `range` (bytes do original), só as edições inteiramente dentro dela,
/// como na formatação de trecho. Se os fluxos de tokens divergirem de um jeito
/// inesperado, devolve uma edição do documento inteiro (sem faixa) ou nenhuma
/// (com faixa), como o original.
///
/// ```
/// use dartforge_formatter::edicoes::edicoes_minimas;
/// let e = edicoes_minimas("main( ){}", "main() {}\n", None);
/// assert_eq!(e.len(), 3);
/// assert_eq!((e[0].inicio, e[0].fim, e[0].texto.as_str()), (5, 6, ""));
/// ```
pub fn edicoes_minimas(original: &str, formatted: &str, range: Option<(usize, usize)>) -> Vec<Edicao> {
    let complete = || {
        if range.is_some() {
            Vec::new()
        } else {
            vec![Edicao {
                inicio: 0,
                fim: original.len(),
                texto: formatted.to_string(),
            }]
        }
    };
    let (Ok(ta), Ok(tb)) = (Tokens::ler(original), Tokens::ler(formatted)) else {
        return complete();
    };
    let a = items(&ta);
    let b = items(&tb);
    let text_a = |i: &Item| &original[i.start..i.end];
    let text_b = |i: &Item| &formatted[i.start..i.end];
    let mut c = EditComputer {
        original,
        formatted,
        range,
        edicoes: Vec::new(),
    };
    let (mut ia, mut ib) = (0usize, 0usize);
    let mut off_a = 0usize;
    let mut off_b = 0usize;
    while ia < a.len() && ib < b.len() {
        let mut tok_a = a[ia];
        let mut tok_b = b[ib];
        let start_a = off_a;
        let mut end_a = tok_a.start;
        let start_b = off_b;
        let mut end_b = tok_b.start;
        let mut any = false;
        let mut has_a = true;
        let mut has_b = true;
        loop {
            if !(has_a && has_b) {
                break;
            }
            let same = tok_a.tipo == tok_b.tipo;
            if !same {
                if optional(&tok_b) {
                    end_b = tok_b.end;
                    ib += 1;
                    has_b = ib < b.len();
                    if has_b {
                        tok_b = b[ib];
                        end_b = tok_b.start;
                    }
                    continue;
                }
                if optional(&tok_a) {
                    end_a = tok_a.end;
                    ia += 1;
                    has_a = ia < a.len();
                    if has_a {
                        tok_a = a[ia];
                        end_a = tok_a.start;
                    }
                    continue;
                }
            } else if matches!(tok_a.tipo, Type::Comment { .. }) && text_a(&tok_a) != text_b(&tok_b) {
                end_a = tok_a.end;
                ia += 1;
                has_a = ia < a.len();
                if has_a {
                    tok_a = a[ia];
                    end_a = tok_a.start;
                }
                end_b = tok_b.end;
                ib += 1;
                has_b = ib < b.len();
                if has_b {
                    tok_b = b[ib];
                    end_b = tok_b.start;
                }
                any = true;
                continue;
            }
            break;
        }
        if !(has_a && has_b) {
            // O original não confere os lexemas aqui; o laço externo termina e
            // a sobra cai no recurso de edição completa.
            if has_a != has_b {
                return complete();
            }
            break;
        }
        if text_a(&tok_a) != text_b(&tok_b) {
            return complete();
        }
        c.add(start_a, end_a, start_b, end_b, any);
        off_a = tok_a.end;
        off_b = tok_b.end;
        ia += 1;
        ib += 1;
        if let Some((_, end)) = range
            && off_a > end
        {
            return c.edicoes;
        }
    }
    if ia < a.len() || ib < b.len() {
        return complete();
    }
    c.add(off_a, original.len(), off_b, formatted.len(), false);
    c.edicoes
}

struct EditComputer<'a> {
    original: &'a str,
    formatted: &'a str,
    range: Option<(usize, usize)>,
    edicoes: Vec<Edicao>,
}

impl EditComputer<'_> {
    /// `_addEditFor`.
    fn add(&mut self, start_a: usize, end_a: usize, start_b: usize, end_b: usize, any: bool) {
        let blank_a = &self.original[start_a..end_a];
        let blank_b = &self.formatted[start_b..end_b];
        if let Some((range_start, range_end)) = self.range {
            if start_a < range_start && end_a > range_start && blank_a.contains('\n') && blank_b.contains('\n') && !blank_a.ends_with('\n') {
                let oa = blank_a.rfind('\n').expect("quebra") + 1;
                let ob = blank_b.rfind('\n').expect("quebra") + 1;
                self.add(start_a, start_a + oa, start_b, start_b + ob, false);
                self.add(start_a + oa, end_a, start_b + ob, end_b, false);
                return;
            }
            if start_a < range_start || end_a > range_end {
                return;
            }
        }
        if blank_a == blank_b {
            return;
        }
        // Prefixo e sufixo comuns, em unidades UTF-16 como no original; nos
        // brancos e comentários comparar por `char` dá o mesmo corte, exceto
        // em pares substitutos parciais, que não ocorrem em texto válido.
        let va: Vec<char> = blank_a.chars().collect();
        let vb: Vec<char> = blank_b.chars().collect();
        let mut p = 0;
        while p < va.len() && p < vb.len() && va[p] == vb[p] {
            p += 1;
        }
        let mut s = 0;
        while s < va.len() - p && s < vb.len() - p && va[va.len() - 1 - s] == vb[vb.len() - 1 - s] {
            s += 1;
        }
        let old: String = va[p..va.len() - s].iter().collect();
        let new: String = vb[p..vb.len() - s].iter().collect();
        let start = start_a + va[..p].iter().map(|c| c.len_utf8()).sum::<usize>();
        let end = end_a - va[va.len() - s..].iter().map(|c| c.len_utf8()).sum::<usize>();
        if !any && (!valid_change(&old) || !valid_change(&new)) {
            return;
        }
        self.edicoes.push(Edicao { inicio: start, fim: end, texto: new });
    }
}
