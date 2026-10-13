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
    pub properties: Vec<(String, String)>,
    /// `(name do local, key em `locals`)`.
    pub locals: Vec<(String, String)>,
}

/// Parses the value of `*dir`, where `dir` is the directive name.
pub fn parse(dir: &str, value: &str) -> Micro {
    let mut m = Micro::default();
    let value = value.trim();
    if !e_micro(value) {
        if !value.is_empty() {
            m.properties.push((dir.to_string(), value.to_string()));
        }
        return m;
    }
    for (i, part) in parts(value).into_iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(rest) = part.strip_prefix("let ") {
            let rest = rest.trim();
            match rest.split_once('=') {
                Some((name, key)) => m
                    .locals
                    .push((name.trim().to_string(), key.trim().to_string())),
                None => {
                    // Without a value, the local is the implicit item of the
                    // loop. But `let x of xs` carries the `of` in the same
                    // part: what comes after the name is a property.
                    let mut words = rest.split_whitespace();
                    let name = words.next().unwrap_or("").to_string();
                    m.locals.push((name, "$implicit".to_string()));
                    if let Some(key) = words.next() {
                        let expr = words.collect::<Vec<_>>().join(" ");
                        if !expr.is_empty() {
                            m.properties.push((property(dir, key), expr));
                        }
                    }
                }
            }
            continue;
        }
        // `key: expr` — the key is an identifier (the `:` of a ternary or of a
        // string does not count).
        if let Some((name, expr)) = part.split_once(':')
            && is_identifier(name.trim())
        {
            m.properties
                .push((property(dir, name.trim()), expr.trim().to_string()));
            continue;
        }
        if i == 0 {
            // The first part without `let` and without `:` is the value of the
            // directive itself (`*ngIf="cond"`).
            m.properties.push((dir.to_string(), part.to_string()));
        }
    }
    m
}

/// The parts separated by `;` at the top level: outside quoted strings and
/// outside parentheses, brackets and braces (the `;` of a literal does not
/// separate).
fn parts(value: &str) -> Vec<&str> {
    let mut output = Vec::new();
    let mut level = 0i32;
    let mut quote: Option<char> = None;
    let mut escape = false;
    let mut start = 0;
    for (i, c) in value.char_indices() {
        if let Some(q) = quote {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => quote = Some(c),
            '(' | '[' | '{' => level += 1,
            ')' | ']' | '}' => level -= 1,
            ';' if level == 0 => {
                output.push(&value[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    output.push(&value[start..]);
    output
}

fn is_identifier(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && cs.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// ngast's `isMicroExpression`: starts with `let` or matches `\S+[:;]` at the
/// start — a word without whitespace followed by `:` or `;`.
pub(crate) fn e_micro(value: &str) -> bool {
    if value.starts_with("let") {
        return true;
    }
    let word = value.split(char::is_whitespace).next().unwrap_or("");
    word.char_indices()
        .any(|(i, c)| i > 0 && matches!(c, ':' | ';'))
}

/// `ngFor` + `of` -> `ngForOf`.
fn property(dir: &str, suffix: &str) -> String {
    let mut s = String::with_capacity(dir.len() + suffix.len());
    s.push_str(dir);
    let mut cs = suffix.chars();
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
    fn ng_if_and_bare_expression() {
        let m = parse("ngIf", "mostrar");
        assert_eq!(m.properties, vec![("ngIf".into(), "mostrar".into())]);
        assert!(m.locals.is_empty());
    }

    /// `:` inside an expression that is not microsyntax (case i43).
    #[test]
    fn colon_in_bare_expression() {
        let m = parse("ngIf", "ativo ? mostrar : false");
        assert_eq!(
            m.properties,
            vec![("ngIf".into(), "ativo ? mostrar : false".into())]
        );
        let m = parse("ngIf", " modo == 'a:b' ");
        assert_eq!(m.properties, vec![("ngIf".into(), "modo == 'a:b'".into())]);
        assert!(parse("ngSwitchDefault", "").properties.is_empty());
    }

    #[test]
    fn ng_for_with_implicit_item() {
        let m = parse("ngFor", "let item of itens");
        assert_eq!(
            m.locals,
            vec![("item".to_string(), "$implicit".to_string())]
        );
        assert_eq!(m.properties, vec![("ngForOf".into(), "itens".into())]);
    }

    #[test]
    fn ng_for_with_index_and_track_by() {
        let m = parse("ngFor", "let item of itens; let i = index; trackBy: porId");
        assert_eq!(
            m.locals,
            vec![
                ("item".to_string(), "$implicit".to_string()),
                ("i".to_string(), "index".to_string())
            ]
        );
        assert_eq!(
            m.properties,
            vec![
                ("ngForOf".to_string(), "itens".to_string()),
                ("ngForTrackBy".to_string(), "porId".to_string())
            ]
        );
    }
}
