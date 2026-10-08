//! `ErrorVerifier._checkForOutOfRange`
//! (`an611:src/generated/error_verifier.dart:4985-5021`) e os testes do
//! `IntegerLiteralImpl` (`an611:src/dart/ast/ast.dart:10590-10645`):
//! `INTEGER_LITERAL_OUT_OF_RANGE` e `INTEGER_LITERAL_IMPRECISE_AS_DOUBLE`,
//! com um natural de precisão arbitrária mínimo (o `BigInt` do Dart).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::corpo::Corpo;
use super::BodyInferrer;
use crate::table::TypeId;
use dartforge_frontend::ast::ExprId;

/// Um natural em base 2^32, o menos significativo primeiro.
type Natural = Vec<u32>;

/// `BigInt.tryParse`: decimal ou `0x`/`0X`.
fn natural_de(texto: &str) -> Option<Natural> {
    let (digitos, base) = match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
        Some(h) => (h, 16u32),
        None => (texto, 10u32),
    };
    if digitos.is_empty() {
        return None;
    }
    let mut n: Natural = vec![0];
    for c in digitos.chars() {
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
        Some(&topo) if !(n.len() == 1 && topo == 0) => (n.len() as u32 - 1) * 32 + (32 - topo.leading_zeros()),
        _ => 0,
    }
}

/// O bit `i` (0 = o menos significativo).
fn bit(n: &Natural, i: u32) -> bool {
    n.get((i / 32) as usize).is_some_and(|l| (l >> (i % 32)) & 1 == 1)
}

/// `isValidAsInteger`: o `int.tryParse` da VM (decimal até `2^63 - 1`, ou
/// `2^63` com o `-`; hexadecimal até `0xFFFFFFFFFFFFFFFF`).
fn valido_como_inteiro(texto: &str, negado: bool) -> bool {
    let Some(n) = natural_de(texto) else { return false };
    let hex = texto.starts_with("0x") || texto.starts_with("0X");
    let b = bits(&n);
    if hex {
        return b <= 64;
    }
    if b < 64 {
        return true;
    }
    // `2^63` exato só com o `-`.
    b == 64 && negado && (0..63).all(|i| !bit(&n, i))
}

/// `isValidAsDouble`: menos de 16 caracteres, até 53 bits, ou os bits abaixo
/// dos 53 mais altos todos zero sem passar de `double.maxFinite`.
fn valido_como_double(texto: &str) -> bool {
    if texto.len() < 16 {
        return true;
    }
    let Some(n) = natural_de(texto) else { return false };
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
fn mais_proximo(n: &Natural) -> f64 {
    let b = bits(n);
    if b <= 64 {
        let v = n.first().copied().unwrap_or(0) as u64 | (n.get(1).copied().unwrap_or(0) as u64) << 32;
        return v as f64;
    }
    let deslocamento = b - 64;
    let mut topo: u64 = 0;
    for i in 0..64 {
        if bit(n, deslocamento + i) {
            topo |= 1 << i;
        }
    }
    // Os bits de baixo só desempatam: o bit mais baixo de `topo` fica
    // abaixo da mantissa.
    if (0..deslocamento).any(|i| bit(n, i)) {
        topo |= 1;
    }
    let v = topo as f64 * 2f64.powi(deslocamento as i32);
    if v.is_finite() { v } else { f64::MAX }
}

/// `BigInt.from(double).toString()` de um `double` inteiro e finito.
fn decimal_de(d: f64) -> String {
    let bits_do_double = d.to_bits();
    let expoente = ((bits_do_double >> 52) & 0x7ff) as i32;
    let fracao = bits_do_double & ((1u64 << 52) - 1);
    if expoente == 0 {
        return (d as u64).to_string();
    }
    let mantissa = fracao | (1u64 << 52);
    let e = expoente - 1075;
    if e <= 0 {
        return (mantissa >> (-e) as u32).to_string();
    }
    // mantissa · 2^e em base 2^32.
    let mut n: Natural = vec![mantissa as u32, (mantissa >> 32) as u32];
    for _ in 0..e {
        let mut vai = 0u32;
        for limbo in n.iter_mut() {
            let novo = (*limbo << 1) | vai;
            vai = *limbo >> 31;
            *limbo = novo;
        }
        if vai > 0 {
            n.push(vai);
        }
    }
    // Divisões sucessivas por 10^9.
    let mut partes: Vec<u32> = Vec::new();
    while !(n.len() == 1 && n[0] == 0) && !n.is_empty() {
        let mut resto = 0u64;
        for limbo in n.iter_mut().rev() {
            let v = (resto << 32) | *limbo as u64;
            *limbo = (v / 1_000_000_000) as u32;
            resto = v % 1_000_000_000;
        }
        partes.push(resto as u32);
        while n.len() > 1 && n.last() == Some(&0) {
            n.pop();
        }
    }
    let mut s = partes.last().map(|p| p.to_string()).unwrap_or_else(|| "0".to_string());
    for p in partes.iter().rev().skip(1) {
        s.push_str(&format!("{p:09}"));
    }
    s
}

/// `_checkForOutOfRange` no literal inteiro `e` de tipo estático `tipo`;
/// `negado` quando o pai é o `-` unário.
pub(crate) fn literal_fora_do_alcance(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, tipo: TypeId, negado: bool) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let sp = inf.span_expr(cx.unit, e);
    let lexema = &inf.program.unit(cx.unit).source[sp.start..sp.end];
    // `0x` sem dígito: o scanner relata `MISSING_HEX_DIGIT` e completa o
    // token com um `0` sintético (`tokenizeHex`,
    // `appendSyntheticSubstringToken(…, "0")`): o lexema vira `0x0`.
    if lexema.eq_ignore_ascii_case("0x") {
        return;
    }
    let fonte: String = lexema.chars().filter(|ch| *ch != '_').collect();
    let como_double = tipo == inf.core.double;
    let valido = if como_double { valido_como_double(&fonte) } else { valido_como_inteiro(&fonte, negado) };
    if valido {
        return;
    }
    let primeiro = if negado { format!("-{lexema}") } else { lexema.to_string() };
    if como_double {
        let proximo = natural_de(&fonte).map(|n| decimal_de(mais_proximo(&n))).unwrap_or_default();
        inf.aviso_com_codigo(c::INTEGER_LITERAL_IMPRECISE_AS_DOUBLE, sp, &[&primeiro, &proximo]);
    } else {
        inf.aviso_com_codigo(c::INTEGER_LITERAL_OUT_OF_RANGE, sp, &[&primeiro]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alcance_dos_inteiros() {
        assert!(valido_como_inteiro("9223372036854775807", false));
        assert!(!valido_como_inteiro("9223372036854775808", false));
        assert!(valido_como_inteiro("9223372036854775808", true));
        assert!(!valido_como_inteiro("9223372036854775809", true));
        assert!(valido_como_inteiro("0xFFFFFFFFFFFFFFFF", false));
        assert!(!valido_como_inteiro("0x10000000000000000", false));
    }

    #[test]
    fn doubles_exatos() {
        assert!(valido_como_double("9007199254740992"));
        assert!(!valido_como_double("9007199254740993"));
        assert_eq!(decimal_de(9007199254740992.0), "9007199254740992");
        assert_eq!(decimal_de(mais_proximo(&natural_de("9007199254740993").unwrap())), "9007199254740992");
        assert_eq!(decimal_de(1e20), "100000000000000000000");
    }
}
