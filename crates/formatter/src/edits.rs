//! Minimal edits between the original and the formatted code: a port of the
//! `_MinimalEditComputer` of analysis server 3.6.2
//! (`pkg/analysis_server/lib/src/lsp/source_edits.dart`), which
//! `textDocument/formatting` in `dart language-server` uses to return only the
//! whitespace changes between tokens instead of the whole document.
//!
//! The original walks the analyzer scanner's token stream (with comments) of
//! both texts. Here the stream is that of [`Tokens`]; the tokenization
//! differences (`>>` and `[]` split apart) do not change the edits, because
//! the formatter never puts whitespace inside those pairs, and the swaps the
//! original allows (`>>` for `>` `>`, `[]` for `[` `]`) give the same
//! whitespace range.

use crate::syntax::token::{TokenKind, Tokens};
use dartforge_frontend::token::Op;

/// One edit: replace `source[start..end]` (bytes of the original) with `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Type {
    Comment { line: bool },
    Token(TokenKind),
}

#[derive(Debug, Clone, Copy)]
struct Item {
    kind_: Type,
    start: usize,
    end: usize,
}

/// `_iterateAllTokens`: comments and tokens in order, without the EOF (or the
/// comments before it).
fn items(t: &Tokens) -> Vec<Item> {
    let mut v = Vec::new();
    for (i, k) in t.list.iter().enumerate() {
        if k.kind_ == TokenKind::Eof {
            break;
        }
        for c in t.comments_of(i as u32) {
            v.push(Item {
                kind_: Type::Comment { line: c.is_line },
                start: c.start,
                end: c.end,
            });
        }
        v.push(Item {
            kind_: Type::Token(k.kind_),
            start: k.start,
            end: k.end,
        });
    }
    v
}

fn optional(i: &Item) -> bool {
    matches!(
        i.kind_,
        Type::Token(TokenKind::Op(Op::Comma)) | Type::Token(TokenKind::Op(Op::Semicolon))
    )
}

fn valid_change(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_whitespace() || matches!(c, ',' | ';' | '<' | '>'))
}

/// The minimal edits that turn `original` into `formatted`.
///
/// With `range` (bytes of the original), only the edits entirely inside it,
/// as in range formatting. If the token streams diverge unexpectedly, returns
/// one edit of the whole document (without a range) or none (with a range),
/// like the original.
///
/// ```
/// use dartforge_formatter::edits::minimal_edits;
/// let e = minimal_edits("main( ){}", "main() {}\n", None);
/// assert_eq!(e.len(), 3);
/// assert_eq!((e[0].start, e[0].end, e[0].text.as_str()), (5, 6, ""));
/// ```
pub fn minimal_edits(original: &str, formatted: &str, range: Option<(usize, usize)>) -> Vec<Edit> {
    let complete = || {
        if range.is_some() {
            Vec::new()
        } else {
            vec![Edit {
                start: 0,
                end: original.len(),
                text: formatted.to_string(),
            }]
        }
    };
    let (Ok(ta), Ok(tb)) = (Tokens::read(original), Tokens::read(formatted)) else {
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
        edits: Vec::new(),
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
            let same = tok_a.kind_ == tok_b.kind_;
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
            } else if matches!(tok_a.kind_, Type::Comment { .. })
                && text_a(&tok_a) != text_b(&tok_b)
            {
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
            // The original does not compare the lexemes here; the outer loop ends and
            // the leftover falls back to the full-edit path.
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
            return c.edits;
        }
    }
    if ia < a.len() || ib < b.len() {
        return complete();
    }
    c.add(off_a, original.len(), off_b, formatted.len(), false);
    c.edits
}

struct EditComputer<'a> {
    original: &'a str,
    formatted: &'a str,
    range: Option<(usize, usize)>,
    edits: Vec<Edit>,
}

impl EditComputer<'_> {
    /// `_addEditFor`.
    fn add(&mut self, start_a: usize, end_a: usize, start_b: usize, end_b: usize, any: bool) {
        let blank_a = &self.original[start_a..end_a];
        let blank_b = &self.formatted[start_b..end_b];
        if let Some((range_start, range_end)) = self.range {
            if start_a < range_start
                && end_a > range_start
                && blank_a.contains('\n')
                && blank_b.contains('\n')
                && !blank_a.ends_with('\n')
            {
                let oa = blank_a.rfind('\n').expect("line break") + 1;
                let ob = blank_b.rfind('\n').expect("line break") + 1;
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
        // Common prefix and suffix, in UTF-16 units as in the original; on
        // whitespace and comments, comparing by `char` gives the same cut, except
        // for partial surrogate pairs, which do not occur in valid text.
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
        let end = end_a
            - va[va.len() - s..]
                .iter()
                .map(|c| c.len_utf8())
                .sum::<usize>();
        if !any && (!valid_change(&old) || !valid_change(&new)) {
            return;
        }
        self.edits.push(Edit {
            start,
            end,
            text: new,
        });
    }
}
