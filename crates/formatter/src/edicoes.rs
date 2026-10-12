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

use crate::syntax::token::{TipoToken, Tokens};
use dartforge_frontend::token::Op;

/// Uma edição: substituir `fonte[inicio..fim]` (bytes do original) por `texto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edicao {
    pub inicio: usize,
    pub fim: usize,
    pub texto: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tipo {
    Comentario { linha: bool },
    Token(TipoToken),
}

#[derive(Debug, Clone, Copy)]
struct Item {
    tipo: Tipo,
    inicio: usize,
    fim: usize,
}

/// `_iterateAllTokens`: comentários e tokens em ordem, sem o EOF (nem os
/// comentários antes dele).
fn itens(t: &Tokens) -> Vec<Item> {
    let mut v = Vec::new();
    for (i, k) in t.lista.iter().enumerate() {
        if k.tipo == TipoToken::Eof {
            break;
        }
        for c in t.comentarios_de(i as u32) {
            v.push(Item {
                tipo: Tipo::Comentario { linha: c.de_linha },
                inicio: c.inicio,
                fim: c.fim,
            });
        }
        v.push(Item {
            tipo: Tipo::Token(k.tipo),
            inicio: k.inicio,
            fim: k.fim,
        });
    }
    v
}

fn opcional(i: &Item) -> bool {
    matches!(i.tipo, Tipo::Token(TipoToken::Op(Op::Comma)) | Tipo::Token(TipoToken::Op(Op::Semicolon)))
}

fn mudanca_valida(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace() || matches!(c, ',' | ';' | '<' | '>'))
}

/// As edições mínimas que levam `original` a `formatado`.
///
/// Com `faixa` (bytes do original), só as edições inteiramente dentro dela,
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
pub fn edicoes_minimas(original: &str, formatado: &str, faixa: Option<(usize, usize)>) -> Vec<Edicao> {
    let completa = || {
        if faixa.is_some() {
            Vec::new()
        } else {
            vec![Edicao {
                inicio: 0,
                fim: original.len(),
                texto: formatado.to_string(),
            }]
        }
    };
    let (Ok(ta), Ok(tb)) = (Tokens::ler(original), Tokens::ler(formatado)) else {
        return completa();
    };
    let a = itens(&ta);
    let b = itens(&tb);
    let texto_a = |i: &Item| &original[i.inicio..i.fim];
    let texto_b = |i: &Item| &formatado[i.inicio..i.fim];
    let mut c = Computador {
        original,
        formatado,
        faixa,
        edicoes: Vec::new(),
    };
    let (mut ia, mut ib) = (0usize, 0usize);
    let mut off_a = 0usize;
    let mut off_b = 0usize;
    while ia < a.len() && ib < b.len() {
        let mut tok_a = a[ia];
        let mut tok_b = b[ib];
        let ini_a = off_a;
        let mut fim_a = tok_a.inicio;
        let ini_b = off_b;
        let mut fim_b = tok_b.inicio;
        let mut qualquer = false;
        let mut tem_a = true;
        let mut tem_b = true;
        loop {
            if !(tem_a && tem_b) {
                break;
            }
            let mesmo = tok_a.tipo == tok_b.tipo;
            if !mesmo {
                if opcional(&tok_b) {
                    fim_b = tok_b.fim;
                    ib += 1;
                    tem_b = ib < b.len();
                    if tem_b {
                        tok_b = b[ib];
                        fim_b = tok_b.inicio;
                    }
                    continue;
                }
                if opcional(&tok_a) {
                    fim_a = tok_a.fim;
                    ia += 1;
                    tem_a = ia < a.len();
                    if tem_a {
                        tok_a = a[ia];
                        fim_a = tok_a.inicio;
                    }
                    continue;
                }
            } else if matches!(tok_a.tipo, Tipo::Comentario { .. }) && texto_a(&tok_a) != texto_b(&tok_b) {
                fim_a = tok_a.fim;
                ia += 1;
                tem_a = ia < a.len();
                if tem_a {
                    tok_a = a[ia];
                    fim_a = tok_a.inicio;
                }
                fim_b = tok_b.fim;
                ib += 1;
                tem_b = ib < b.len();
                if tem_b {
                    tok_b = b[ib];
                    fim_b = tok_b.inicio;
                }
                qualquer = true;
                continue;
            }
            break;
        }
        if !(tem_a && tem_b) {
            // O original não confere os lexemas aqui; o laço externo termina e
            // a sobra cai no recurso de edição completa.
            if tem_a != tem_b {
                return completa();
            }
            break;
        }
        if texto_a(&tok_a) != texto_b(&tok_b) {
            return completa();
        }
        c.adicionar(ini_a, fim_a, ini_b, fim_b, qualquer);
        off_a = tok_a.fim;
        off_b = tok_b.fim;
        ia += 1;
        ib += 1;
        if let Some((_, fim)) = faixa
            && off_a > fim
        {
            return c.edicoes;
        }
    }
    if ia < a.len() || ib < b.len() {
        return completa();
    }
    c.adicionar(off_a, original.len(), off_b, formatado.len(), false);
    c.edicoes
}

struct Computador<'a> {
    original: &'a str,
    formatado: &'a str,
    faixa: Option<(usize, usize)>,
    edicoes: Vec<Edicao>,
}

impl Computador<'_> {
    /// `_addEditFor`.
    fn adicionar(&mut self, ini_a: usize, fim_a: usize, ini_b: usize, fim_b: usize, qualquer: bool) {
        let branco_a = &self.original[ini_a..fim_a];
        let branco_b = &self.formatado[ini_b..fim_b];
        if let Some((fx_ini, fx_fim)) = self.faixa {
            if ini_a < fx_ini && fim_a > fx_ini && branco_a.contains('\n') && branco_b.contains('\n') && !branco_a.ends_with('\n') {
                let oa = branco_a.rfind('\n').expect("quebra") + 1;
                let ob = branco_b.rfind('\n').expect("quebra") + 1;
                self.adicionar(ini_a, ini_a + oa, ini_b, ini_b + ob, false);
                self.adicionar(ini_a + oa, fim_a, ini_b + ob, fim_b, false);
                return;
            }
            if ini_a < fx_ini || fim_a > fx_fim {
                return;
            }
        }
        if branco_a == branco_b {
            return;
        }
        // Prefixo e sufixo comuns, em unidades UTF-16 como no original; nos
        // brancos e comentários comparar por `char` dá o mesmo corte, exceto
        // em pares substitutos parciais, que não ocorrem em texto válido.
        let va: Vec<char> = branco_a.chars().collect();
        let vb: Vec<char> = branco_b.chars().collect();
        let mut p = 0;
        while p < va.len() && p < vb.len() && va[p] == vb[p] {
            p += 1;
        }
        let mut s = 0;
        while s < va.len() - p && s < vb.len() - p && va[va.len() - 1 - s] == vb[vb.len() - 1 - s] {
            s += 1;
        }
        let velho: String = va[p..va.len() - s].iter().collect();
        let novo: String = vb[p..vb.len() - s].iter().collect();
        let inicio = ini_a + va[..p].iter().map(|c| c.len_utf8()).sum::<usize>();
        let fim = fim_a - va[va.len() - s..].iter().map(|c| c.len_utf8()).sum::<usize>();
        if !qualquer && (!mudanca_valida(&velho) || !mudanca_valida(&novo)) {
            return;
        }
        self.edicoes.push(Edicao { inicio, fim, texto: novo });
    }
}
