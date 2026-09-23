// Runtime nativo: natives de `int` e `double` do SDK da fonte (P5b).
//
// Cada função é o native de mesmo nome da VM (`runtime/lib/integers.cc`,
// `double.cc`), com o símbolo `dartforge_nativo_<Nome>` e a assinatura na
// representação da HIR (R1): o receptor de `_IntegerImplementation`/`_Smi`/
// `_Mint` é o `i64`, o de `_Double` o `f64`; `bool` volta como `u8`. A tabela
// que o lowering consulta é `crates/emit_native/src/nativos.rs`.
//
// Os `*FromInteger` são chamados com os operandos TROCADOS, como na VM:
// `int operator -(num other) => other._subFromInteger(this)` — o receptor do
// native é o operando da DIREITA e o argumento o da esquerda. A divisão por
// zero é conferida em Dart antes da chamada (`integers.dart`), como na VM.

/// `Integer_addFromInteger`: `outro + this` com estouro modular.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_addFromInteger(this: i64, outro: i64) -> i64 {
    outro.wrapping_add(this)
}

/// `Integer_subFromInteger`: `outro - this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_subFromInteger(this: i64, outro: i64) -> i64 {
    outro.wrapping_sub(this)
}

/// `Integer_mulFromInteger`: `outro * this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_mulFromInteger(this: i64, outro: i64) -> i64 {
    outro.wrapping_mul(this)
}

/// `Integer_truncDivFromInteger`: `outro ~/ this`. `MIN ~/ -1` é `MIN`
/// (`Integer::ArithmeticOp`, kTRUNCDIV). `this == 0` não chega aqui.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_truncDivFromInteger(this: i64, outro: i64) -> i64 {
    if this == 0 {
        lancar_divisao_por_zero();
        return 0;
    }
    outro.wrapping_div(this)
}

/// `Integer_moduloFromInteger`: `outro % this`, sempre não negativo
/// (`Integer::ArithmeticOp`, kMOD: resto negativo soma `|this|`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_moduloFromInteger(this: i64, outro: i64) -> i64 {
    if this == 0 {
        lancar_divisao_por_zero();
        return 0;
    }
    let resto = outro.wrapping_rem(this);
    if resto < 0 {
        if this < 0 { resto.wrapping_sub(this) } else { resto.wrapping_add(this) }
    } else {
        resto
    }
}

/// A divisão por zero não chega ao native: `integers.dart` lança
/// `IntegerDivisionByZeroException` antes (como na VM, onde o native só tem
/// um `ASSERT`). Chegar aqui é bug do compilador (N4), não exceção Dart.
fn lancar_divisao_por_zero() {
    panic!("bug do compilador: divisão inteira por zero chegou ao native (o Dart confere antes)");
}

/// `Integer_bitAndFromInteger`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_bitAndFromInteger(this: i64, outro: i64) -> i64 {
    outro & this
}

/// `Integer_bitOrFromInteger`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_bitOrFromInteger(this: i64, outro: i64) -> i64 {
    outro | this
}

/// `Integer_bitXorFromInteger`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_bitXorFromInteger(this: i64, outro: i64) -> i64 {
    outro ^ this
}

/// Deslocamento com a regra da VM (`ShiftOperationHelper` e
/// `Integer::ShiftOp`): quantidade negativa lança `ArgumentError`;
/// quantidade ≥ 64 dá 0 (`<<`, `>>>`) ou o sinal (`>>`).
fn deslocar(valor: i64, quantidade: i64, tipo: u8) -> i64 {
    if quantidade < 0 {
        let err = dartforge_argument_error_value(quantidade, 0, 0, 0);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let grande = quantidade >= 64;
    match tipo {
        0 => if grande { 0 } else { valor.wrapping_shl(quantidade as u32) },
        1 => if grande { if valor < 0 { -1 } else { 0 } } else { valor >> quantidade },
        _ => if grande { 0 } else { ((valor as u64) >> quantidade) as i64 },
    }
}

/// `Integer_shlFromInteger`: `outro << this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_shlFromInteger(this: i64, outro: i64) -> i64 {
    deslocar(outro, this, 0)
}

/// `Integer_shrFromInteger`: `outro >> this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_shrFromInteger(this: i64, outro: i64) -> i64 {
    deslocar(outro, this, 1)
}

/// `Integer_ushrFromInteger`: `outro >>> this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_ushrFromInteger(this: i64, outro: i64) -> i64 {
    deslocar(outro, this, 2)
}

/// `Integer_greaterThanFromInteger`: `outro > this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_greaterThanFromInteger(this: i64, outro: i64) -> u8 {
    u8::from(outro > this)
}

/// `Integer_equalToInteger`: `this == outro`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_equalToInteger(this: i64, outro: i64) -> u8 {
    u8::from(this == outro)
}

/// `Smi_bitNegate` e `Mint_bitNegate`: `~this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Smi_bitNegate(this: i64) -> i64 {
    !this
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Mint_bitNegate(this: i64) -> i64 {
    !this
}

/// `Smi_bitLength` e `Mint_bitLength` (`Utils::BitLength`): os bits
/// significativos, contando o de `~v` para negativos.
fn comprimento_em_bits(v: i64) -> i64 {
    let v = if v < 0 { !v } else { v };
    64 - i64::from(v.leading_zeros())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Smi_bitLength(this: i64) -> i64 {
    comprimento_em_bits(this)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Mint_bitLength(this: i64) -> i64 {
    comprimento_em_bits(this)
}

/// `Double_add` e família: aritmética IEEE com o receptor à esquerda
/// (`_Double.+` chama `_add(other.toDouble())`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_add(this: f64, outro: f64) -> f64 {
    this + outro
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_sub(this: f64, outro: f64) -> f64 {
    this - outro
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_mul(this: f64, outro: f64) -> f64 {
    this * outro
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_div(this: f64, outro: f64) -> f64 {
    this / outro
}

/// `Double_doubleFromInteger`: `_Double.fromInteger(int)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_doubleFromInteger(valor: i64) -> f64 {
    valor as f64
}

/// `Double_equal`: `==` IEEE (NaN diferente de tudo).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_equal(this: f64, outro: f64) -> u8 {
    u8::from(this == outro)
}

/// `Double_equalToInteger`: `this == outro.toDouble()`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_equalToInteger(this: f64, outro: i64) -> u8 {
    u8::from(this == outro as f64)
}

/// `Double_greaterThan`: `this > outro`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_greaterThan(this: f64, outro: f64) -> u8 {
    u8::from(this > outro)
}

/// `Double_greaterThanFromInteger`: `outro > this` (operandos trocados,
/// como os `*FromInteger` de `int`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_greaterThanFromInteger(this: f64, outro: i64) -> u8 {
    u8::from((outro as f64) > this)
}

/// `Double_flipSignBit`: `-this` (inclusive para 0.0 e NaN).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_flipSignBit(this: f64) -> f64 {
    f64::from_bits(this.to_bits() ^ (1 << 63))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_getIsNaN(this: f64) -> u8 {
    u8::from(this.is_nan())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_getIsInfinite(this: f64) -> u8 {
    u8::from(this.is_infinite())
}

/// `Double_getIsNegative`: bit de sinal, exceto NaN (`-0.0` é negativo).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_getIsNegative(this: f64) -> u8 {
    u8::from(!this.is_nan() && this.is_sign_negative())
}

/// `double.toString` como a VM (`DoubleToCString`,
/// `runtime/vm/double_conversion.cc`): o `ToShortest` do double-conversion
/// com `decimal_in_shortest_low = -6`, `high = 21`, `EMIT_POSITIVE_EXPONENT_SIGN`,
/// `EMIT_TRAILING_DECIMAL_POINT` e `EMIT_TRAILING_ZERO_AFTER_POINT`.
/// Os dígitos mais curtos que voltam ao mesmo `double` vêm da formatação
/// `{:e}` do Rust (o mesmo critério do `ToShortest`).
pub fn texto_de_double_da_vm(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_string();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if d == 0.0 {
        return if d.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let e = format!("{:e}", d.abs());
    let (mantissa, expoente) = e.split_once('e').expect("formato {:e}");
    let expoente: i32 = expoente.parse().expect("expoente");
    let digitos: String = mantissa.chars().filter(|c| *c != '.').collect();
    let n = digitos.len() as i32;
    let ponto = expoente + 1; // posição do ponto decimal nos dígitos
    let mut saida = String::new();
    if d < 0.0 {
        saida.push('-');
    }
    if (-6..21).contains(&expoente) {
        if ponto <= 0 {
            saida.push_str("0.");
            for _ in 0..(-ponto) {
                saida.push('0');
            }
            saida.push_str(&digitos);
        } else if ponto >= n {
            saida.push_str(&digitos);
            for _ in 0..(ponto - n) {
                saida.push('0');
            }
            saida.push_str(".0");
        } else {
            saida.push_str(&digitos[..ponto as usize]);
            saida.push('.');
            saida.push_str(&digitos[ponto as usize..]);
        }
    } else {
        saida.push_str(&digitos[..1]);
        if n > 1 {
            saida.push('.');
            saida.push_str(&digitos[1..]);
        }
        saida.push('e');
        saida.push(if expoente < 0 { '-' } else { '+' });
        saida.push_str(&expoente.abs().to_string());
    }
    saida
}

/// `Double_toString`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toString(this: f64) -> i64 {
    alocar_str(&texto_de_double_da_vm(this))
}

#[cfg(test)]
mod testes_nativos_numeros {
    //! Os valores esperados são os da VM 3.6.2 (`print`, medidos).
    use super::*;

    #[test]
    fn double_to_string_e_o_da_vm() {
        let casos: [(f64, &str); 27] = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (100.0, "100.0"),
            (1e20, "100000000000000000000.0"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (123456789012345680000.0, "123456789012345680000.0"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (0.000001234, "0.000001234"),
            (3.141592653589793, "3.141592653589793"),
            (2.5, "2.5"),
            (1.0 / 3.0, "0.3333333333333333"),
            (2.0 / 3.0, "0.6666666666666666"),
            (1e300, "1e+300"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (0.3, "0.3"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e100, "1e+100"),
            (12345.6789, "12345.6789"),
            (-0.000001, "-0.000001"),
            (f64::NAN, "NaN"),
        ];
        for (d, esperado) in casos {
            assert_eq!(texto_de_double_da_vm(d), esperado, "{d:e}");
        }
        assert_eq!(texto_de_double_da_vm(f64::INFINITY), "Infinity");
        assert_eq!(texto_de_double_da_vm(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn inteiros_com_a_regra_da_vm() {
        for (v, bits, neg) in [
            (5i64, 3i64, -6i64),
            (-5, 3, 4),
            (7, 3, -8),
            (-7, 3, 6),
            (0, 0, -1),
            (1, 1, -2),
            (-1, 0, 0),
            (i64::MAX, 63, i64::MIN),
            (i64::MIN, 63, i64::MAX),
        ] {
            assert_eq!(dartforge_nativo_Smi_bitLength(v), bits, "{v}.bitLength");
            assert_eq!(dartforge_nativo_Mint_bitNegate(v), neg, "~{v}");
        }
        // Operandos trocados: `a op b` é `b._opFromInteger(a)`.
        let op = |f: extern "C" fn(i64, i64) -> i64, a: i64, b: i64| f(b, a);
        assert_eq!(op(dartforge_nativo_Integer_truncDivFromInteger, i64::MIN, -1), i64::MIN);
        assert_eq!(op(dartforge_nativo_Integer_moduloFromInteger, -7, 3), 2);
        assert_eq!(op(dartforge_nativo_Integer_moduloFromInteger, 7, -3), 1);
        assert_eq!(op(dartforge_nativo_Integer_moduloFromInteger, -7, -3), 2);
        assert_eq!(op(dartforge_nativo_Integer_truncDivFromInteger, -7, 2), -3);
        assert_eq!(op(dartforge_nativo_Integer_shlFromInteger, 1, 63), i64::MIN);
        assert_eq!(op(dartforge_nativo_Integer_shlFromInteger, 1, 64), 0);
        assert_eq!(op(dartforge_nativo_Integer_shrFromInteger, -1, 70), -1);
        assert_eq!(op(dartforge_nativo_Integer_ushrFromInteger, -1, 1), i64::MAX);
        assert_eq!(op(dartforge_nativo_Integer_ushrFromInteger, -1, 64), 0);
        assert_eq!(op(dartforge_nativo_Integer_subFromInteger, 10, 3), 7);
        assert_eq!(dartforge_nativo_Integer_greaterThanFromInteger(3, 10), 1, "10 > 3");
    }
}
