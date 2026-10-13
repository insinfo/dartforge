//! `ErrorVerifier._checkForOutOfRange`
//! (`an611:src/generated/error_verifier.dart:4985-5021`) and the tests of
//! `IntegerLiteralImpl` (`an611:src/dart/ast/ast.dart:10590-10645`):
//! `INTEGER_LITERAL_OUT_OF_RANGE` and `INTEGER_LITERAL_IMPRECISE_AS_DOUBLE`,
//! with a minimal arbitrary-precision natural number (Dart's `BigInt`).
//!
//! Written without compiling or running (2026-10-05).

use super::body::Body;
use super::BodyInferrer;
use crate::table::TypeId;
use dartforge_frontend::ast::ExprId;

/// A natural number in base 2^32, least significant first.
type Natural = Vec<u32>;

/// `BigInt.tryParse`: decimal or `0x`/`0X`.
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
        let mut carry = d as u64;
        for limb in n.iter_mut() {
            let v = *limb as u64 * base as u64 + carry;
            *limb = v as u32;
            carry = v >> 32;
        }
        if carry > 0 {
            n.push(carry as u32);
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

/// Bit `i` (0 = the least significant).
fn bit(n: &Natural, i: u32) -> bool {
    n.get((i / 32) as usize).is_some_and(|l| (l >> (i % 32)) & 1 == 1)
}

/// `isValidAsInteger`: the VM's `int.tryParse` (decimal up to `2^63 - 1`, or
/// `2^63` with the `-`; hexadecimal up to `0xFFFFFFFFFFFFFFFF`).
fn valid_as_integer(text: &str, the_negated: bool) -> bool {
    let Some(n) = natural_of(text) else { return false };
    let hex = text.starts_with("0x") || text.starts_with("0X");
    let b = bits(&n);
    if hex {
        // The VM's `int.tryParse`: a positive hexadecimal is valid up to 64 bits
        // (it wraps around); with the `-`, only up to `2^63`.
        if the_negated {
            return b < 64 || (b == 64 && (0..63).all(|i| !bit(&n, i)));
        }
        return b <= 64;
    }
    if b < 64 {
        return true;
    }
    // Exact `2^63` only with the `-`.
    b == 64 && the_negated && (0..63).all(|i| !bit(&n, i))
}

/// `isValidAsDouble`: fewer than 16 characters, up to 53 bits, or the bits
/// below the 53 highest all zero without exceeding `double.maxFinite`.
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

/// `BigInt.toDouble` (rounded to nearest, ties to even) capped at
/// `double.maxFinite`.
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
    // The low bits only break ties: the lowest bit of `top` is below the
    // mantissa.
    if (0..shift).any(|i| bit(n, i)) {
        top |= 1;
    }
    let v = top as f64 * 2f64.powi(shift as i32);
    if v.is_finite() { v } else { f64::MAX }
}

/// `BigInt.from(double).toString()` of an integral, finite `double`.
fn decimal_of(d: f64) -> String {
    let double_bits = d.to_bits();
    let exponent = ((double_bits >> 52) & 0x7ff) as i32;
    let fraction = double_bits & ((1u64 << 52) - 1);
    if exponent == 0 {
        return (d as u64).to_string();
    }
    let mantissa = fraction | (1u64 << 52);
    let e = exponent - 1075;
    if e <= 0 {
        return (mantissa >> (-e) as u32).to_string();
    }
    // mantissa · 2^e in base 2^32.
    let mut n: Natural = vec![mantissa as u32, (mantissa >> 32) as u32];
    for _ in 0..e {
        let mut carry = 0u32;
        for limb in n.iter_mut() {
            let new_ = (*limb << 1) | carry;
            carry = *limb >> 31;
            *limb = new_;
        }
        if carry > 0 {
            n.push(carry);
        }
    }
    // Successive divisions by 10^9.
    let mut parts: Vec<u32> = Vec::new();
    while !(n.len() == 1 && n[0] == 0) && !n.is_empty() {
        let mut rest = 0u64;
        for limb in n.iter_mut().rev() {
            let v = (rest << 32) | *limb as u64;
            *limb = (v / 1_000_000_000) as u32;
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

/// `_checkForOutOfRange` on the integer literal `e` of static type `ty`;
/// `the_negated` when the parent is the unary `-`.
pub(crate) fn literal_out_of_range(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, ty: TypeId, the_negated: bool) {
    use dartforge_diagnostics::codes::compile_time_error as c;
    let sp = inf.span_expr(cx.unit, e);
    let lexeme = &inf.program.unit(cx.unit).source[sp.start..sp.end];
    // `0x` without a digit: the scanner reports `MISSING_HEX_DIGIT` and
    // completes the token with a synthetic `0` (`tokenizeHex`,
    // `appendSyntheticSubstringToken(…, "0")`): the lexeme becomes `0x0`.
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
