//! `ErrorVerifier._checkForOutOfRange`
//! (`an611:src/generated/error_verifier.dart:4985-5021`) e os testes do
//! `IntegerLiteralImpl` (`an611:src/dart/ast/ast.dart:10590-10645`):
//! `INTEGER_LITERAL_OUT_OF_RANGE` e `INTEGER_LITERAL_IMPRECISE_AS_DOUBLE`,
//! com um natural de precisão arbitrária mínimo (o `BigInt` do Dart).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::body::Body;
use super::BodyInferrer;
use crate::table::TypeId;
use dartforge_frontend::ast::ExprId;

/// Um natural em base 2^32, o menos significativo primeiro.
type Natural = Vec<u32>;

/// `BigInt.tryParse`: decimal ou `0x`/`0X`.
fn natural_of(text: &str) -> Option<Natural> {
    let (digits, base) = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(h) => (h, 16u32),
        None => (text, 10u32),
    };
    if digits.is_empty() {
        return None;
    }
    let mut n: Natural = vec![0];
    for c in digits.chars() {
        let d = c.to_digit(base)?;
        let mut vai = d as u64;
        for limbo in n.iter_mut() {
            let v = *limbo as u64 * base as u64 + vai;
            *limbo = v as u32;
            vai = v >> 32;
        }
        if vai > 0 {
            n.push(vai as u32);
        }
    }
    while n.len() > 1 && n.last() == Some(&0) {
        n.pop();
    }
    Some(n)
}

/// `bitLength`.
fn bits(n: &Natural) -> u32 {
    match n.last() {
        Some(&top) if !(n.len() == 1 && top == 0) => (n.len() as u32 - 1) * 32 + (32 - top.leading_zeros()),
        _ => 0,
    }
}

/// O bit `i` (0 = o menos significativo).
fn bit(n: &Natural, i: u32) -> bool {
    n.get((i / 32) as usize).is_some_and(|l| (l >> (i % 32)) & 1 == 1)
}

/// `isValidAsInteger`: o `int.tryParse` da VM (decimal até `2^63 - 1`, ou
/// `2^63` com o `-`; hexadecimal até `0xFFFFFFFFFFFFFFFF`).
fn valid_as_integer(text: &str, the_negated: bool) -> bool {
    let Some(n) = natural_of(text) else { return false };
    let hex = text.starts_with("0x") || text.starts_with("0X");
    let b = bits(&n);
    if hex {
        // `int.tryParse` da VM: o hexadecimal positivo vale até 64 bits (dá a
        // volta); com o `-`, só até `2^63`.
        if the_negated {
            return b < 64 || (b == 64 && (0..63).all(|i| !bit(&n, i)));
        }
        return b <= 64;
    }
    if b < 64 {
        return true;
    }
    // `2^63` exato só com o `-`.
    b == 64 && the_negated && (0..63).all(|i| !bit(&n, i))
}

/// `isValidAsDouble`: menos de 16 caracteres, até 53 bits, ou os bits abaixo
/// dos 53 mais altos todos zero sem passar de `double.maxFinite`.
fn valid_as_double(text: &str) -> bool {
    if text.len() < 16 {
        return true;
    }
    let Some(n) = natural_of(text) else { return false };
    let b = bits(&n);
    if b <= 53 {
        return true;
    }
    if b > 1024 {
        return false;
    }
    (0..b - 53).all(|i| !bit(&n, i))
}

/// `BigInt.toDouble` (arredondado ao mais próximo, empate para o par) com o
/// teto de `double.maxFinite`.
fn nearest(n: &Natural) -> f64 {
    let b = bits(n);
    if b <= 64 {
        let v = n.first().copied().unwrap_or(0) as u64 | (n.get(1).copied().unwrap_or(0) as u64) << 32;
        return v as f64;
    }
    let shift = b - 64;
    let mut top: u64 = 0;
    for i in 0..64 {
        if bit(n, shift + i) {
            top |= 1 << i;
        }
    }
    // Os bits de baixo só desempatam: o bit mais baixo de `topo` fica
    // abaixo da mantissa.
    if (0..shift).any(|i| bit(n, i)) {
        top |= 1;
    }
    let v = top as f64 * 2f64.powi(shift as i32);
    if v.is_finite() { v } else { f64::MAX }
}

/// `BigInt.from(double).toString()` de um `double` inteiro e finito.
fn decimal_of(d: f64) -> String {
    let bits_do_double = d.to_bits();
    let exponent = ((bits_do_double >> 52) & 0x7ff) as i32;
    let fraction = bits_do_double & ((1u64 << 52) - 1);
    if exponent == 0 {
        return (d as u64).to_string();
    }
    let mantissa = fraction | (1u64 << 52);
    let e = exponent - 1075;
    if e <= 0 {
        return (mantissa >> (-e) as u32).to_string();
    }
    // mantissa · 2^e em base 2^32.
    let mut n: Natural = vec![mantissa as u32, (mantissa >> 32) as u32];
    for _ in 0..e {
        let mut vai = 0u32;
        for limbo in n.iter_mut() {
            let new_ = (*limbo << 1) | vai;
            vai = *limbo >> 31;
            *limbo = new_;
        }
        if vai > 0 {
            n.push(vai);
        }
    }
    // Divisões sucessivas por 10^9.
    let mut parts: Vec<u32> = Vec::new();
    while !(n.len() == 1 && n[0] == 0) && !n.is_empty() {
        let mut rest = 0u64;
        for limbo in n.iter_mut().rev() {
            let v = (rest << 32) | *limbo as u64;
            *limbo = (v / 1_000_000_000) as u32;
            rest = v % 1_000_000_000;
        }
        parts.push(rest as u32);
        while n.len() > 1 && n.last() == Some(&0) {
            n.pop();
        }
    }
    let mut s = parts.last().map(|p| p.to_string()).unwrap_or_else(|| "0".to_string());
    for p in parts.iter().rev().skip(1) {
        s.push_str(&format!("{p:09}"));
    }
    s
}

/// `_checkForOutOfRange` no literal inteiro `e` de tipo estático `tipo`;
/// `negado` quando o pai é o `-` unário.
pub(crate) fn literal_out_of_range(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, ty: TypeId, the_negated: bool) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let sp = inf.span_expr(cx.unit, e);
    let lexeme = &inf.program.unit(cx.unit).source[sp.start..sp.end];
    // `0x` sem dígito: o scanner relata `MISSING_HEX_DIGIT` e completa o
    // token com um `0` sintético (`tokenizeHex`,
    // `appendSyntheticSubstringToken(…, "0")`): o lexema vira `0x0`.
    if lexeme.eq_ignore_ascii_case("0x") {
        return;
    }
    let source: String = lexeme.chars().filter(|ch| *ch != '_').collect();
    let as_double = ty == inf.core.double;
    let valid = if as_double { valid_as_double(&source) } else { valid_as_integer(&source, the_negated) };
    if valid {
        return;
    }
    let first = if the_negated { format!("-{lexeme}") } else { lexeme.to_string() };
    if as_double {
        let next_ = natural_of(&source).map(|n| decimal_of(nearest(&n))).unwrap_or_default();
        inf.warning_with_code(c::INTEGER_LITERAL_IMPRECISE_AS_DOUBLE, sp, &[&first, &next_]);
    } else {
        inf.warning_with_code(c::INTEGER_LITERAL_OUT_OF_RANGE, sp, &[&first]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_range() {
        assert!(valid_as_integer("9223372036854775807", false));
        assert!(!valid_as_integer("9223372036854775808", false));
        assert!(valid_as_integer("9223372036854775808", true));
        assert!(!valid_as_integer("9223372036854775809", true));
        assert!(valid_as_integer("0xFFFFFFFFFFFFFFFF", false));
        assert!(!valid_as_integer("0x10000000000000000", false));
    }

    #[test]
    fn exact_doubles() {
        assert!(valid_as_double("9007199254740992"));
        assert!(!valid_as_double("9007199254740993"));
        assert_eq!(decimal_of(9007199254740992.0), "9007199254740992");
        assert_eq!(decimal_of(nearest(&natural_of("9007199254740993").unwrap())), "9007199254740992");
        assert_eq!(decimal_of(1e20), "100000000000000000000");
    }
}
