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

/// A expansão decimal EXATA de `|d|` (finito, não zero): os dígitos (sem
/// zeros à esquerda) e a posição do ponto (quantos dígitos há antes dele;
/// ≤ 0 é `0.000ddd`). `|d| = m·2^e`; para `e < 0` é `m·5^k / 10^k`.
/// Inteiros grandes em base 10^9, só com multiplicação por 2 e por 5.
fn decimal_exato(d: f64) -> (Vec<u8>, i32) {
    let bits = d.abs().to_bits();
    let exp_bits = ((bits >> 52) & 0x7FF) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e) = if exp_bits == 0 { (frac, -1074) } else { (frac | (1u64 << 52), exp_bits - 1075) };
    let mut limbs: Vec<u32> = vec![(m % 1_000_000_000) as u32, ((m / 1_000_000_000) % 1_000_000_000) as u32, (m / 1_000_000_000_000_000_000) as u32];
    let mut mul = |f: u64, vezes: i32, limbs: &mut Vec<u32>| {
        for _ in 0..vezes {
            let mut vai = 0u64;
            for l in limbs.iter_mut() {
                let v = u64::from(*l) * f + vai;
                *l = (v % 1_000_000_000) as u32;
                vai = v / 1_000_000_000;
            }
            if vai > 0 {
                limbs.push(vai as u32);
            }
        }
    };
    let k = if e >= 0 {
        mul(2, e, &mut limbs);
        0
    } else {
        mul(5, -e, &mut limbs);
        -e
    };
    while limbs.len() > 1 && *limbs.last().unwrap() == 0 {
        limbs.pop();
    }
    let mut texto = limbs.last().unwrap().to_string();
    for l in limbs.iter().rev().skip(1) {
        texto.push_str(&format!("{l:09}"));
    }
    let digitos: Vec<u8> = texto.bytes().map(|b| b - b'0').collect();
    let ponto = digitos.len() as i32 - k;
    (digitos, ponto)
}

/// Mantém `n` dígitos arredondando **meio para cima** sobre a expansão
/// exata (o que o double-conversion faz em `ToFixed`/`ToExponential`/
/// `ToPrecision`: `0.125.toStringAsFixed(2)` é `0.13`, `2.5` → `3`).
/// Devolve os dígitos (exatamente `n`, ou `n + 1` com vai-um) e o ponto.
fn arredondar(digitos: &[u8], ponto: i32, n: i32) -> (Vec<u8>, i32) {
    if n < 0 {
        return (Vec::new(), ponto);
    }
    let n = n as usize;
    let mut v: Vec<u8> = digitos.iter().copied().take(n).collect();
    v.resize(n, 0);
    if digitos.get(n).is_some_and(|&x| x >= 5) {
        let mut i = n;
        loop {
            if i == 0 {
                v.insert(0, 1);
                return (v, ponto + 1);
            }
            i -= 1;
            if v[i] == 9 {
                v[i] = 0;
            } else {
                v[i] += 1;
                break;
            }
        }
    }
    (v, ponto)
}

fn texto_dos_digitos(v: &[u8]) -> String {
    v.iter().map(|d| char::from(b'0' + d)).collect()
}

/// `toStringAsFixed` (native `Double_toStringAsFixed`, o `ToFixed` da VM):
/// o Dart já conferiu `0 <= f <= 20`, NaN e `|d| >= 1e21`. O sinal vem do
/// bit de sinal (`(-0.0).toStringAsFixed(2)` é `-0.00`).
pub fn double_com_fixo(d: f64, f: i64) -> String {
    let f = f.clamp(0, 20) as i32;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    let (digitos, ponto) = if d == 0.0 { (vec![0], 1) } else { decimal_exato(d) };
    let (v, ponto) = arredondar(&digitos, ponto, ponto + f);
    // `v` tem os dígitos até a casa `f` depois do ponto; completa à esquerda.
    let inteiros = ponto.max(0) as usize;
    let mut todos: Vec<u8> = Vec::new();
    if ponto < 0 {
        todos.extend(std::iter::repeat(0).take((-ponto) as usize));
    }
    todos.extend(&v);
    todos.resize(inteiros + f as usize, 0);
    let (int, fr) = todos.split_at(inteiros);
    if int.is_empty() {
        s.push('0');
    } else {
        s.push_str(&texto_dos_digitos(int));
    }
    if f > 0 {
        s.push('.');
        s.push_str(&texto_dos_digitos(fr));
    }
    s
}

/// `toStringAsExponential(r)` (`ToExponential`): `r + 1` dígitos
/// significativos, expoente com sinal (`1.250e-1`, `0.000e+0`).
pub fn double_com_expoente(d: f64, r: i64) -> String {
    let r = r.clamp(0, 20) as i32;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    let (v, expoente) = if d == 0.0 {
        (vec![0; (r + 1) as usize], 0)
    } else {
        let (digitos, ponto) = decimal_exato(d);
        let (mut v, ponto) = arredondar(&digitos, ponto, r + 1);
        v.truncate((r + 1) as usize);
        (v, ponto - 1)
    };
    s.push(char::from(b'0' + v[0]));
    if r > 0 {
        s.push('.');
        s.push_str(&texto_dos_digitos(&v[1..]));
    }
    s.push('e');
    s.push(if expoente < 0 { '-' } else { '+' });
    s.push_str(&expoente.abs().to_string());
    s
}

/// `toStringAsPrecision(p)` (`ToPrecision`, com até 6 zeros à esquerda e
/// nenhum à direita): notação exponencial quando o expoente é `< -6` ou
/// `>= p`.
pub fn double_com_precisao(d: f64, p: i64) -> String {
    let p = p.clamp(1, 21) as i32;
    if d == 0.0 {
        let mut s = String::from(if d.is_sign_negative() { "-0" } else { "0" });
        if p > 1 {
            s.push('.');
            s.push_str(&"0".repeat((p - 1) as usize));
        }
        return s;
    }
    let (digitos, ponto) = decimal_exato(d);
    let (mut v, ponto) = arredondar(&digitos, ponto, p);
    v.truncate(p as usize);
    let expoente = ponto - 1;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    if expoente < -6 || expoente >= p {
        s.push(char::from(b'0' + v[0]));
        if p > 1 {
            s.push('.');
            s.push_str(&texto_dos_digitos(&v[1..]));
        }
        s.push('e');
        s.push(if expoente < 0 { '-' } else { '+' });
        s.push_str(&expoente.abs().to_string());
    } else if ponto <= 0 {
        s.push_str("0.");
        s.push_str(&"0".repeat((-ponto) as usize));
        s.push_str(&texto_dos_digitos(&v));
    } else if ponto >= p {
        s.push_str(&texto_dos_digitos(&v));
    } else {
        s.push_str(&texto_dos_digitos(&v[..ponto as usize]));
        s.push('.');
        s.push_str(&texto_dos_digitos(&v[ponto as usize..]));
    }
    s
}

/// `Double_toStringAsFixed`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toStringAsFixed(this: f64, digitos: i64) -> i64 {
    alocar_str(&double_com_fixo(this, digitos))
}

/// `Double_toStringAsExponential`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toStringAsExponential(this: f64, digitos: i64) -> i64 {
    alocar_str(&double_com_expoente(this, digitos))
}

/// `Double_toStringAsPrecision`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toStringAsPrecision(this: f64, precisao: i64) -> i64 {
    alocar_str(&double_com_precisao(this, precisao))
}

#[cfg(test)]
mod testes_to_string_as {
    //! Os esperados são os da VM 3.6.2 (medidos): `toStringAsFixed(0)`,
    //! `(2)`, `(5)`, `toStringAsExponential(3)`, `toStringAsPrecision(4)`.
    use super::*;

    #[test]
    fn as_tres_formas_sao_as_da_vm() {
        let casos: [(f64, [&str; 5]); 18] = [
            (0.0, ["0", "0.00", "0.00000", "0.000e+0", "0.000"]),
            (-0.0, ["-0", "-0.00", "-0.00000", "-0.000e+0", "-0.000"]),
            (1.0, ["1", "1.00", "1.00000", "1.000e+0", "1.000"]),
            (0.125, ["0", "0.13", "0.12500", "1.250e-1", "0.1250"]),
            (0.25, ["0", "0.25", "0.25000", "2.500e-1", "0.2500"]),
            (0.35, ["0", "0.35", "0.35000", "3.500e-1", "0.3500"]),
            (2.5, ["3", "2.50", "2.50000", "2.500e+0", "2.500"]),
            (-2.5, ["-3", "-2.50", "-2.50000", "-2.500e+0", "-2.500"]),
            (1.005, ["1", "1.00", "1.00500", "1.005e+0", "1.005"]),
            (123.456, ["123", "123.46", "123.45600", "1.235e+2", "123.5"]),
            (-0.001, ["-0", "-0.00", "-0.00100", "-1.000e-3", "-0.001000"]),
            (1e20, ["100000000000000000000", "100000000000000000000.00", "100000000000000000000.00000", "1.000e+20", "1.000e+20"]),
            (0.1, ["0", "0.10", "0.10000", "1.000e-1", "0.1000"]),
            (3.14159, ["3", "3.14", "3.14159", "3.142e+0", "3.142"]),
            (999.9999, ["1000", "1000.00", "999.99990", "1.000e+3", "1000"]),
            (5e-324, ["0", "0.00", "0.00000", "4.941e-324", "4.941e-324"]),
            (0.5, ["1", "0.50", "0.50000", "5.000e-1", "0.5000"]),
            (1.5, ["2", "1.50", "1.50000", "1.500e+0", "1.500"]),
        ];
        for (d, e) in casos {
            assert_eq!(double_com_fixo(d, 0), e[0], "{d}.toStringAsFixed(0)");
            assert_eq!(double_com_fixo(d, 2), e[1], "{d}.toStringAsFixed(2)");
            assert_eq!(double_com_fixo(d, 5), e[2], "{d}.toStringAsFixed(5)");
            assert_eq!(double_com_expoente(d, 3), e[3], "{d}.toStringAsExponential(3)");
            assert_eq!(double_com_precisao(d, 4), e[4], "{d}.toStringAsPrecision(4)");
        }
    }
}
