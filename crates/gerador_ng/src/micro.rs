//! A microssintaxe de `*dir="..."`.
//!
//! É o que transforma a forma abreviada no `<template>` com atributos que o
//! casamento de diretivas enxerga:
//!
//! ```text
//! *ngIf="cond"              <template [ngIf]="cond">
//! *ngFor="let x of xs"      <template ngFor let-x [ngForOf]="xs">
//! ```
//!
//! As regras vieram de `expression/micro/parser.dart` do `ngast`, que é o
//! parser que o `ngcompiler` usa:
//!
//! | forma | efeito |
//! |---|---|
//! | expressão solta (só na primeira posição) | propriedade `dir` |
//! | `let x` | local `x` ligado a `$implicit` |
//! | `let x = y` | local `x` ligado a `locals['y']` |
//! | `nome: expr` | propriedade `dir` + `Nome` |

/// O que um `*dir="..."` declara.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Micro {
    /// `(nome da propriedade, expressão)`, já com o prefixo da diretiva.
    pub propriedades: Vec<(String, String)>,
    /// `(nome do local, chave em `locals`)`.
    pub locais: Vec<(String, String)>,
}

/// Analisa o valor de `*dir`, onde `dir` é o nome da diretiva.
pub fn analisar(dir: &str, valor: &str) -> Micro {
    let mut m = Micro::default();
    for (i, parte) in valor.split(';').enumerate() {
        let parte = parte.trim();
        if parte.is_empty() {
            continue;
        }
        if let Some(resto) = parte.strip_prefix("let ") {
            let resto = resto.trim();
            match resto.split_once('=') {
                Some((nome, chave)) => {
                    m.locais.push((nome.trim().to_string(), chave.trim().to_string()))
                }
                None => {
                    // Sem valor, o local é o item implícito do laço. Mas
                    // `let x of xs` traz o `of` na mesma parte: o que vem
                    // depois do nome é uma propriedade.
                    let mut palavras = resto.split_whitespace();
                    let nome = palavras.next().unwrap_or("").to_string();
                    m.locais.push((nome, "$implicit".to_string()));
                    if let Some(chave) = palavras.next() {
                        let expr = palavras.collect::<Vec<_>>().join(" ");
                        if !expr.is_empty() {
                            m.propriedades.push((propriedade(dir, chave), expr));
                        }
                    }
                }
            }
            continue;
        }
        if let Some((nome, expr)) = parte.split_once(':') {
            m.propriedades.push((propriedade(dir, nome.trim()), expr.trim().to_string()));
            continue;
        }
        if i == 0 {
            // A primeira parte sem `let` e sem `:` é o valor da própria
            // diretiva (`*ngIf="cond"`).
            m.propriedades.push((dir.to_string(), parte.to_string()));
        }
    }
    m
}

/// `ngFor` + `of` -> `ngForOf`.
fn propriedade(dir: &str, sufixo: &str) -> String {
    let mut s = String::with_capacity(dir.len() + sufixo.len());
    s.push_str(dir);
    let mut cs = sufixo.chars();
    if let Some(c) = cs.next() {
        s.extend(c.to_uppercase());
        s.push_str(cs.as_str());
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ng_if_e_expressao_solta() {
        let m = analisar("ngIf", "mostrar");
        assert_eq!(m.propriedades, vec![("ngIf".into(), "mostrar".into())]);
        assert!(m.locais.is_empty());
    }

    #[test]
    fn ng_for_com_item_implicito() {
        let m = analisar("ngFor", "let item of itens");
        assert_eq!(m.locais, vec![("item".to_string(), "$implicit".to_string())]);
        assert_eq!(m.propriedades, vec![("ngForOf".into(), "itens".into())]);
    }

    #[test]
    fn ng_for_com_indice_e_track_by() {
        let m = analisar("ngFor", "let item of itens; let i = index; trackBy: porId");
        assert_eq!(
            m.locais,
            vec![
                ("item".to_string(), "$implicit".to_string()),
                ("i".to_string(), "index".to_string())
            ]
        );
        assert_eq!(
            m.propriedades,
            vec![
                ("ngForOf".to_string(), "itens".to_string()),
                ("ngForTrackBy".to_string(), "porId".to_string())
            ]
        );
    }
}
