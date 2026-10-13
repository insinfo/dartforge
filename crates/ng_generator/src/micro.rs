//! The microsyntax of `*dir="..."`.
//!
//! It is what turns the shorthand into the `<template>` with attributes that
//! directive matching sees:
//!
//! ```text
//! *ngIf="cond"              <template [ngIf]="cond">
//! *ngFor="let x of xs"      <template ngFor let-x [ngForOf]="xs">
//! ```
//!
//! The rules came from `expression/micro/parser.dart` of `ngast`, which is the
//! parser `ngcompiler` uses:
//!
//! | shape | effect |
//! |---|---|
//! | bare expression (only in the first position) | property `dir` |
//! | `let x` | local `x` bound to `$implicit` |
//! | `let x = y` | local `x` bound to `locals['y']` |
//! | `name: expr` | property `dir` + `Name` |
//!
//! Only a value that starts with `let` or with a word glued to `:`/`;`
//! (`isMicroExpression`) is microsyntax; the rest is a single expression, even
//! with `:` inside (`a ? b : c`, `x == 'a:b'`).

/// What a `*dir="..."` declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Micro {
    /// `(property name, expression)`, already with the directive prefix.
    pub propriedades: Vec<(String, String)>,
    /// `(nome do local, chave em `locals`)`.
    pub locais: Vec<(String, String)>,
}

/// Parses the value of `*dir`, where `dir` is the directive name.
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
                    // Without a value, the local is the implicit item of the
                    // loop. But `let x of xs` carries the `of` in the same
                    // part: what comes after the name is a property.
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
        // `key: expr` — the key is an identifier (the `:` of a ternary or of a
        // string does not count).
        if let Some((nome, expr)) = parte.split_once(':')
            && e_identificador(nome.trim())
        {
            m.propriedades
                .push((propriedade(dir, nome.trim()), expr.trim().to_string()));
            continue;
        }
        if i == 0 {
            // The first part without `let` and without `:` is the value of the
            // directive itself (`*ngIf="cond"`).
            m.propriedades.push((dir.to_string(), parte.to_string()));
        }
    }
    m
}

/// The parts separated by `;` at the top level: outside quoted strings and
/// outside parentheses, brackets and braces (the `;` of a literal does not
/// separate).
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

/// ngast's `isMicroExpression`: starts with `let` or matches `\S+[:;]` at the
/// start — a word without whitespace followed by `:` or `;`.
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

    /// `:` inside an expression that is not microsyntax (case i43).
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
