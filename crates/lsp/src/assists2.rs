//! Quatro assistências só sintáticas do servidor do Dart 3.6.2, pelas
//! regras de docs/LSP-ESPECIFICACAO.md §13.8.2 e §13.10:
//!
//! | Assistência | Espécie |
//! | --- | --- |
//! | `Add digit separators` | `refactor.add.digitSeparators` |
//! | `Remove digit separators` | `refactor.remove.digitSeparators` |
//! | `Convert to is!` | `refactor.convert.isNot` |
//! | `Exchange operands` | `refactor.exchangeOperands` |
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::actions::CodeAction;
use crate::project::Project;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, UnaryOp};

fn action(uri: &str, title: &str, kind: &str, edits: Vec<(Span, String)>) -> CodeAction {
    CodeAction {
        title: title.into(),
        kind: kind.into(),
        edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
        diagnostic: None,
        create_file: None, linked: Default::default(),
    }
}

/// `addSep` (`add_digit_separators.dart:169-185`): os dígitos em grupos de
/// `group`, alinhados à direita; `None` com menos de `min` dígitos.
fn separate(digits: &str, group: usize, min: usize) -> Option<String> {
    let n = digits.len();
    if n < min || !digits.is_ascii() {
        return None;
    }
    let mut k = n % group;
    if k == 0 {
        k = group;
    }
    let mut output = digits[..k].to_string();
    while k + group <= n {
        output.push('_');
        output.push_str(&digits[k..k + group]);
        k += group;
    }
    Some(output)
}

/// O literal numérico `lexeme` com os separadores refeitos, ou `None` se a
/// assistência não se aplica (poucos dígitos, ou o resultado é o próprio
/// lexema).
fn with_separators(lexeme: &str, floating_point: bool) -> Option<String> {
    let source: String = lexeme.chars().filter(|c| *c != '_').collect();
    let result = if source.starts_with("0x") || source.starts_with("0X") {
        // Hexadecimal: de dois em dois, com pelo menos quatro dígitos.
        format!("{}{}", &source[..2], separate(&source[2..], 2, 4)?)
    } else if !floating_point {
        separate(&source, 3, 5)?
    } else {
        let exponent_at = source.find('e').or_else(|| source.find('E'));
        let dot = source.find('.');
        let mantissa_end = exponent_at.unwrap_or(source.len());
        let (integer, fracao) = match dot {
            Some(p) => (&source[..p], Some(&source[p + 1..mantissa_end])),
            None => (&source[..mantissa_end], None),
        };
        let mut r = separate(integer, 3, 5).unwrap_or_else(|| integer.to_string());
        if let Some(f) = fracao {
            // Na fração os grupos se alinham à esquerda.
            let inverted: String = f.chars().rev().collect();
            let separate = separate(&inverted, 3, 5).map(|s| s.chars().rev().collect::<String>());
            r.push('.');
            r.push_str(&separate.unwrap_or_else(|| f.to_string()));
        }
        if let Some(e) = exponent_at {
            let after = &source[e + 1..];
            let (negative, exponent) = match after.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, after),
            };
            r.push_str(&source[e..e + 1]);
            if negative {
                r.push('-');
            }
            r.push_str(&separate(exponent, 3, 5).unwrap_or_else(|| exponent.to_string()));
        }
        r
    };
    (result != lexeme).then_some(result)
}

/// `getExpressionParentPrecedence(child) >= Precedence.relational`: o pai
/// de `child` é uma expressão que liga pelo menos tão forte quanto um
/// operador relacional.
fn relational_parent_or_higher(a: &ast::Ast, child: ExprId) -> bool {
    a.exprs.iter().any(|e| match &e.kind {
        ExprKind::Binary { op, left, right } if *left == child || *right == child => {
            !matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull)
        }
        ExprKind::Is { value, .. } | ExprKind::As { value, .. } => *value == child,
        ExprKind::Unary { operand, .. } => *operand == child,
        ExprKind::Await(x) => *x == child,
        ExprKind::Property { target, .. } | ExprKind::Call { target, .. } | ExprKind::TypeArguments { target, .. } => *target == child,
        // O alvo de um índice é pós-fixo; o índice em si conta como atribuição.
        ExprKind::Index { target, .. } => *target == child,
        _ => false,
    })
}

impl Project {
    /// As assistências desta lista com o cursor em `offset` de `unit`.
    pub(crate) fn syntactic_assists(&self, uri: &str, unit: UnitId, offset: usize) -> Vec<CodeAction> {
        let mut output = Vec::new();
        let u = self.program().unit(unit);
        let a = &u.ast;
        let source = u.source.as_str();
        let contains = |s: Span| s.start <= offset && offset <= s.end;

        // Os separadores de dígitos: o literal numérico sob o cursor.
        for e in a.exprs.iter() {
            let (span, floating) = match &e.kind {
                ExprKind::Int(s) => (*s, false),
                ExprKind::Double(s) => (*s, true),
                _ => continue,
            };
            if !contains(span) {
                continue;
            }
            let Some(lexeme) = source.get(span.start..span.end) else { continue };
            if let Some(new) = with_separators(lexeme, floating) {
                output.push(action(uri, "Add digit separators", "refactor.add.digitSeparators", vec![(span, new)]));
            }
            if lexeme.contains('_') {
                let sem: String = lexeme.chars().filter(|c| *c != '_').collect();
                output.push(action(uri, "Remove digit separators", "refactor.remove.digitSeparators", vec![(span, sem)]));
            }
            break;
        }

        // `Convert to is!`: `!(x is T)` com o cursor no `!`, num parêntese
        // ou dentro do `is`.
        for (i, e) in a.exprs.iter().enumerate() {
            let ExprKind::Unary { op: UnaryOp::Not, operand } = &e.kind else { continue };
            let between = a.expr(*operand);
            let ExprKind::Parenthesized(inside) = &between.kind else { continue };
            let test_ = a.expr(*inside);
            let ExprKind::Is { value, negated: false, .. } = &test_.kind else { continue };
            let test_node = contains(test_.span);
            let prefix_node = offset == e.span.start || offset == between.span.start || offset == between.span.end;
            if !(test_node || prefix_node) {
                continue;
            }
            // O `is` fica depois do valor: a palavra, e o ponto logo depois dela.
            let after_value = a.expr(*value).span.end;
            let Some(k) = source.get(after_value..test_.span.end).and_then(|t| t.find("is")) else { continue };
            let is_end = after_value + k + 2;
            let mut edits: Vec<(Span, String)> = Vec::new();
            if relational_parent_or_higher(a, ExprId(i as u32)) {
                // Os parênteses ficam: sai só o `!`.
                edits.push((Span { start: e.span.start, end: e.span.start + 1 }, String::new()));
            } else {
                // Saem o `!(` e o `)`.
                edits.push((Span { start: e.span.start, end: between.span.start + 1 }, String::new()));
                edits.push((Span { start: between.span.end.saturating_sub(1), end: e.span.end }, String::new()));
            }
            edits.push((Span { start: is_end, end: is_end }, "!".to_string()));
            output.push(action(uri, "Convert to is!", "refactor.convert.isNot", edits));
            break;
        }

        // `Exchange operands`: a expressão binária com o cursor no operador.
        let selected = a.exprs.iter().enumerate().find_map(|(i, e)| match &e.kind {
            ExprKind::Binary { op, left, right } => {
                let (de, until) = (a.expr(*left).span.end, a.expr(*right).span.start);
                let text = source.get(de..until)?;
                let start = de + (text.len() - text.trim_start().len());
                let end = de + text.trim_end().len();
                (start <= offset && offset <= end).then_some((ExprId(i as u32), *op, *left, *right, Span { start: start, end: end }))
            }
            _ => None,
        });
        if let Some((id, op, left_, right_, operator)) = selected {
            // Alarga pelos pais de mesmo operador.
            let mut wide = id;
            loop {
                let parent = a.exprs.iter().position(|e| matches!(&e.kind, ExprKind::Binary { op: o, left, right } if *o == op && (*left == wide || *right == wide)));
                match parent {
                    Some(p) => wide = ExprId(p as u32),
                    None => break,
                }
            }
            let wide = a.expr(wide).span;
            let (left_end, right_start) = (a.expr(left_).span.end, a.expr(right_).span.start);
            if let (Some(left_text), Some(right_text)) = (source.get(wide.start..left_end), source.get(right_start..wide.end)) {
                let mut edits = vec![
                    (Span { start: wide.start, end: left_end }, right_text.to_string()),
                    (Span { start: right_start, end: wide.end }, left_text.to_string()),
                ];
                // Um operador relacional troca de lado.
                let inverted = match op {
                    BinaryOp::Lt => Some(">"),
                    BinaryOp::LtEq => Some(">="),
                    BinaryOp::Gt => Some("<"),
                    BinaryOp::GtEq => Some("<="),
                    _ => None,
                };
                if let Some(new) = inverted {
                    edits.push((operator, new.to_string()));
                }
                edits.sort_by_key(|(s, _)| s.start);
                output.push(action(uri, "Exchange operands", "refactor.exchangeOperands", edits));
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os exemplos da especificação (§13.8.2).
    #[test]
    fn digit_separators() {
        assert_eq!(with_separators("1234567", false).as_deref(), Some("1_234_567"));
        assert_eq!(with_separators("1234", false), None);
        assert_eq!(with_separators("12_34567", false).as_deref(), Some("1_234_567"));
        assert_eq!(with_separators("0xFFFFFF", false).as_deref(), Some("0xFF_FF_FF"));
        assert_eq!(with_separators("0xFFF", false), None);
        assert_eq!(with_separators("12345.67891", true).as_deref(), Some("12_345.678_91"));
        assert_eq!(with_separators("1_000_000", false), None);
    }
}
