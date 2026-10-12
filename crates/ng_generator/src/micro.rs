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
//!
//! Só é microssintaxe o valor que começa com `let` ou com uma palavra
//! colada a `:`/`;` (`isMicroExpression`); o resto é uma expressão só,
//! mesmo com `:` dentro (`a ? b : c`, `x == 'a:b'`).

/// O que um `*dir="..."` declara.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Micro {
    /// `(nome da propriedade, expressão)`, já com o prefixo da diretiva.
    pub propriedades: Vec<(String, String)>,
    /// `(nome do local, chave em `locals`)`.
    pub locais: Vec<(String, String)>,
}

/// Analisa o valor de `*dir`, onde `dir` é o nome da diretiva.
pub fn analisar(dir: &str, valor: &str) -> Micro {
    let mut m = Micro::default();
    let valor = valor.trim();
    if !e_micro(valor) {
        if !valor.is_empty() {
            m.propriedades.push((dir.to_string(), valor.to_string()));
        }
        return m;
    }
    for (i, parte) in partes(valor).into_iter().enumerate() {
        let parte = parte.trim();
        if parte.is_empty() {
            continue;
        }
        if let Some(resto) = parte.strip_prefix("let ") {
            let resto = resto.trim();
            match resto.split_once('=') {
                Some((nome, chave)) => m
                    .locais
                    .push((nome.trim().to_string(), chave.trim().to_string())),
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
        // `chave: expr` — a chave é um identificador (o `:` de um ternário
        // ou de um texto não conta).
        if let Some((nome, expr)) = parte.split_once(':')
            && e_identificador(nome.trim())
        {
            m.propriedades
                .push((propriedade(dir, nome.trim()), expr.trim().to_string()));
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

/// As partes separadas por `;` no nível de cima: fora de texto entre aspas
/// e de parênteses, colchetes e chaves (o `;` de um literal não separa).
fn partes(valor: &str) -> Vec<&str> {
    let mut saida = Vec::new();
    let mut nivel = 0i32;
    let mut aspa: Option<char> = None;
    let mut escape = false;
    let mut inicio = 0;
    for (i, c) in valor.char_indices() {
        if let Some(q) = aspa {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == q {
                aspa = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => aspa = Some(c),
            '(' | '[' | '{' => nivel += 1,
            ')' | ']' | '}' => nivel -= 1,
            ';' if nivel == 0 => {
                saida.push(&valor[inicio..i]);
                inicio = i + 1;
            }
            _ => {}
        }
    }
    saida.push(&valor[inicio..]);
    saida
}

fn e_identificador(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && cs.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// `isMicroExpression` do ngast: começa com `let` ou casa `\S+[:;]` no
/// início — uma palavra sem espaço seguida de `:` ou `;`.
pub(crate) fn e_micro(valor: &str) -> bool {
    if valor.starts_with("let") {
        return true;
    }
    let palavra = valor.split(char::is_whitespace).next().unwrap_or("");
    palavra
        .char_indices()
        .any(|(i, c)| i > 0 && matches!(c, ':' | ';'))
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

    /// `:` dentro de uma expressão que não é microssintaxe (caso i43).
    #[test]
    fn dois_pontos_na_expressao_solta() {
        let m = analisar("ngIf", "ativo ? mostrar : false");
        assert_eq!(
            m.propriedades,
            vec![("ngIf".into(), "ativo ? mostrar : false".into())]
        );
        let m = analisar("ngIf", " modo == 'a:b' ");
        assert_eq!(
            m.propriedades,
            vec![("ngIf".into(), "modo == 'a:b'".into())]
        );
        assert!(analisar("ngSwitchDefault", "").propriedades.is_empty());
    }

    #[test]
    fn ng_for_com_item_implicito() {
        let m = analisar("ngFor", "let item of itens");
        assert_eq!(
            m.locais,
            vec![("item".to_string(), "$implicit".to_string())]
        );
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
