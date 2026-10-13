// Runtime nativo: natives de `int`, `double` e `dart:math` do SDK da fonte
// (P5b; espaço unificado, P2: os `DartForge_double_*`, `Double_parse`,
// `DartForge_int_hashCode` e `DartForge_math_*` vieram de `nativos_listas.rs`).
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

/// `Integer_addFromInteger`: `other_one + this` com estouro modular.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_addFromInteger(this: i64, other_one: i64) -> i64 {
    other_one.wrapping_add(this)
}

/// `Integer_subFromInteger`: `other_one - this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_subFromInteger(this: i64, other_one: i64) -> i64 {
    other_one.wrapping_sub(this)
}

/// `Integer_mulFromInteger`: `other_one * this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_mulFromInteger(this: i64, other_one: i64) -> i64 {
    other_one.wrapping_mul(this)
}

/// `Integer_truncDivFromInteger`: `outro ~/ this`. `MIN ~/ -1` é `MIN`
/// (`Integer::ArithmeticOp`, kTRUNCDIV). `this == 0` não chega aqui.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_truncDivFromInteger(this: i64, other_one: i64) -> i64 {
    if this == 0 {
        throw_division_by_zero();
        return 0;
    }
    other_one.wrapping_div(this)
}

/// `Integer_moduloFromInteger`: `other_one % this`, sempre não negativo
/// (`Integer::ArithmeticOp`, kMOD: resto negativo soma `|this|`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_moduloFromInteger(this: i64, other_one: i64) -> i64 {
    if this == 0 {
        throw_division_by_zero();
        return 0;
    }
    let rest = other_one.wrapping_rem(this);
    if rest < 0 {
        if this < 0 { rest.wrapping_sub(this) } else { rest.wrapping_add(this) }
    } else {
        rest
    }
}

/// A divisão por zero não chega ao native: `integers.dart` lança
/// `IntegerDivisionByZeroException` antes (como na VM, onde o native só tem
/// um `ASSERT`). Chegar aqui é bug do compilador (N4), não exceção Dart.
fn throw_division_by_zero() {
    panic!("bug do compilador: divisão inteira por zero chegou ao native (o Dart confere antes)");
}

/// `Integer_bitAndFromInteger`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_bitAndFromInteger(this: i64, other_one: i64) -> i64 {
    other_one & this
}

/// `Integer_bitOrFromInteger`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_bitOrFromInteger(this: i64, other_one: i64) -> i64 {
    other_one | this
}

/// `Integer_bitXorFromInteger`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_bitXorFromInteger(this: i64, other_one: i64) -> i64 {
    other_one ^ this
}

/// Deslocamento com a regra da VM (`ShiftOperationHelper` e
/// `Integer::ShiftOp`): quantidade negativa lança `ArgumentError`;
/// quantidade ≥ 64 dá 0 (`<<`, `>>>`) ou o sinal (`>>`).
fn shift(value: i64, amount: i64, tipo: u8) -> i64 {
    if amount < 0 {
        let err = dartforge_argument_error_value(amount, 0, 0, 0);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let large = amount >= 64;
    match tipo {
        0 => if large { 0 } else { value.wrapping_shl(amount as u32) },
        1 => if large { if value < 0 { -1 } else { 0 } } else { value >> amount },
        _ => if large { 0 } else { ((value as u64) >> amount) as i64 },
    }
}

/// `Integer_shlFromInteger`: `outro << this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_shlFromInteger(this: i64, other_one: i64) -> i64 {
    shift(other_one, this, 0)
}

/// `Integer_shrFromInteger`: `other_one >> this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_shrFromInteger(this: i64, other_one: i64) -> i64 {
    shift(other_one, this, 1)
}

/// `Integer_ushrFromInteger`: `other_one >>> this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_ushrFromInteger(this: i64, other_one: i64) -> i64 {
    shift(other_one, this, 2)
}

/// `Integer_greaterThanFromInteger`: `other_one > this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_greaterThanFromInteger(this: i64, other_one: i64) -> u8 {
    u8::from(other_one > this)
}

/// `Integer_equalToInteger`: `this == outro`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_equalToInteger(this: i64, other_one: i64) -> u8 {
    u8::from(this == other_one)
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
fn bit_length(v: i64) -> i64 {
    let v = if v < 0 { !v } else { v };
    64 - i64::from(v.leading_zeros())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Smi_bitLength(this: i64) -> i64 {
    bit_length(this)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Mint_bitLength(this: i64) -> i64 {
    bit_length(this)
}

/// `Double_add` e família: aritmética IEEE com o receptor à esquerda
/// (`_Double.+` chama `_add(other.toDouble())`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_add(this: f64, other_one: f64) -> f64 {
    this + other_one
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_sub(this: f64, other_one: f64) -> f64 {
    this - other_one
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_mul(this: f64, other_one: f64) -> f64 {
    this * other_one
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_div(this: f64, other_one: f64) -> f64 {
    this / other_one
}

/// `Double_doubleFromInteger`: `_Double.fromInteger(int)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_doubleFromInteger(value: i64) -> f64 {
    value as f64
}

/// `Double_equal`: `==` IEEE (NaN diferente de tudo).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_equal(this: f64, other_one: f64) -> u8 {
    u8::from(this == other_one)
}

/// `Double_equalToInteger`: `this == outro.toDouble()`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_equalToInteger(this: f64, other_one: i64) -> u8 {
    u8::from(this == other_one as f64)
}

/// `Double_greaterThan`: `this > other_one`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_greaterThan(this: f64, other_one: f64) -> u8 {
    u8::from(this > other_one)
}

/// `Double_greaterThanFromInteger`: `other_one > this` (operandos trocados,
/// como os `*FromInteger` de `int`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_greaterThanFromInteger(this: f64, other_one: i64) -> u8 {
    u8::from((other_one as f64) > this)
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
pub fn vm_double_string(d: f64) -> String {
    let mut buf = [0u8; VM_DOUBLE_SIZE];
    let n = write_vm_double(d, &mut buf);
    // Só ASCII.
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// O maior texto de [`vm_double_string`]: `-` + 17 dígitos + `.` +
/// `e-308` (ou `0.000000` + 17 dígitos) cabe com folga.
const VM_DOUBLE_SIZE: usize = 48;

/// Buffer da pilha para o `{:e}` (sem a `String` do `format!`).
struct StackBuffer<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl std::fmt::Write for StackBuffer<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.n + s.len();
        if end > self.buf.len() {
            return Err(std::fmt::Error);
        }
        self.buf[self.n..end].copy_from_slice(s.as_bytes());
        self.n = end;
        Ok(())
    }
}

/// [`vm_double_string`] escrito em `out` (ASCII), sem alocar: o
/// `Double_toString` do `jsonEncode` de uma lista de `double` alocava
/// quatro `String` por número. Devolve o comprimento.
fn write_vm_double(d: f64, out: &mut [u8; VM_DOUBLE_SIZE]) -> usize {
    let mut n = 0usize;
    let mut by = |b: &[u8], n: &mut usize| {
        out[*n..*n + b.len()].copy_from_slice(b);
        *n += b.len();
    };
    if d.is_nan() {
        by(b"NaN", &mut n);
        return n;
    }
    if d.is_infinite() {
        by(if d > 0.0 { b"Infinity" } else { b"-Infinity" }, &mut n);
        return n;
    }
    if d == 0.0 {
        by(if d.is_sign_negative() { b"-0.0" } else { b"0.0" }, &mut n);
        return n;
    }
    // Os dígitos mais curtos: `{:e}` do valor absoluto, `d[.ddd]e<exp>`.
    let mut e = [0u8; 40];
    let mut w = StackBuffer { buf: &mut e, n: 0 };
    std::fmt::Write::write_fmt(&mut w, format_args!("{:e}", d.abs())).expect("formato {:e}");
    let used = w.n;
    let e = &e[..used];
    let pos_exp = e.iter().position(|&c| c == b'e').expect("formato {:e}");
    let (mantissa, exp_txt) = (&e[..pos_exp], &e[pos_exp + 1..]);
    let exponent: i32 = std::str::from_utf8(exp_txt).ok().and_then(|t| t.parse().ok()).expect("expoente");
    let mut digits = [0u8; 24];
    let mut nd = 0usize;
    for &c in mantissa {
        if c != b'.' {
            digits[nd] = c;
            nd += 1;
        }
    }
    let digits = &digits[..nd];
    let nd = nd as i32;
    let point = exponent + 1; // posição do ponto decimal nos dígitos
    if d < 0.0 {
        by(b"-", &mut n);
    }
    if (-6..21).contains(&exponent) {
        if point <= 0 {
            by(b"0.", &mut n);
            for _ in 0..(-point) {
                by(b"0", &mut n);
            }
            by(digits, &mut n);
        } else if point >= nd {
            by(digits, &mut n);
            for _ in 0..(point - nd) {
                by(b"0", &mut n);
            }
            by(b".0", &mut n);
        } else {
            by(&digits[..point as usize], &mut n);
            by(b".", &mut n);
            by(&digits[point as usize..], &mut n);
        }
    } else {
        by(&digits[..1], &mut n);
        if nd > 1 {
            by(b".", &mut n);
            by(&digits[1..], &mut n);
        }
        by(b"e", &mut n);
        by(if exponent < 0 { b"-" } else { b"+" }, &mut n);
        let mut ex = [0u8; 8];
        let mut i = ex.len();
        let mut v = exponent.unsigned_abs();
        loop {
            i -= 1;
            ex[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        by(&ex[i..], &mut n);
    }
    n
}

/// A forma antiga de [`vm_double_string`], para os testes conferirem
/// a nova contra ela.
#[cfg(test)]
fn vm_double_string_old(d: f64) -> String {
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
    let (mantissa, exponent) = e.split_once('e').expect("formato {:e}");
    let exponent: i32 = exponent.parse().expect("expoente");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let n = digits.len() as i32;
    let point = exponent + 1; // posição do ponto decimal nos dígitos
    let mut out = String::new();
    if d < 0.0 {
        out.push('-');
    }
    if (-6..21).contains(&exponent) {
        if point <= 0 {
            out.push_str("0.");
            for _ in 0..(-point) {
                out.push('0');
            }
            out.push_str(&digits);
        } else if point >= n {
            out.push_str(&digits);
            for _ in 0..(point - n) {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    } else {
        out.push_str(&digits[..1]);
        if n > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.abs().to_string());
    }
    out
}

/// `Double_toString`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toString(this: f64) -> i64 {
    let mut buf = [0u8; VM_DOUBLE_SIZE];
    let n = write_vm_double(this, &mut buf);
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::textos::TextoRef::Um(&buf[..n])))
}

/// `_Double._bitsDe(d)` da sobreposição (`double.dart`): os bits do
/// `double`, a chave do cache do `toString` (o `identical` de dois `double`
/// da VM compara os bits).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_bits(d: f64) -> i64 {
    d.to_bits() as i64
}

/// `_Smi._deInteiro(v)` da sobreposição (`integers.dart`): os dígitos
/// decimais de `v`, o texto do `_Smi.toString` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_int_toString(v: i64) -> i64 {
    dartforge_to_string_i64(v)
}

/// `has63BitSmis()` (`DartForge_verdadeiro`): o `Smi` daqui tem 63 bits (R10).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_verdadeiro() -> u8 {
    1
}

/// `_Smi.hashCode`/`_Mint.hashCode`: o `HashIntegerOp` da VM — o produto de 96
/// bits de `this` (sem sinal) por `0x2d51`, as três palavras de 32 bits
/// combinadas por xor, cortado a 30 bits. O lowering o faz em linha
/// (`lower/boxes.rs`); fica para a chamada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_int_hashCode(this: i64) -> i64 {
    let p = u128::from(this as u64) * 0x2d51;
    let x = (p as u32) ^ ((p >> 32) as u32) ^ ((p >> 64) as u32);
    i64::from(x & 0x3fff_ffff)
}

/// `_Double.toInt()`: truncado, saturado fora da faixa de 64 bits (como a
/// VM); NaN e infinito lançam `UnsupportedError("Infinity or NaN toInt")`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_toInt(this: f64) -> i64 {
    if !this.is_finite() {
        let m = HEAP.with(|heap| heap.borrow_mut().allocate_str("Infinity or NaN toInt"));
        let e = with_roots(&[m], || dartforge_unsupported_error_new(m));
        with_roots(&[e], || dartforge_exception_throw(e, 3));
        return 0;
    }
    this as i64
}

/// `_Double.floorToDouble()` e afins.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_floor(this: f64) -> f64 {
    this.floor()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_ceil(this: f64) -> f64 {
    this.ceil()
}

/// `roundToDouble`: metade para longe de zero (`round` da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_round(this: f64) -> f64 {
    this.round()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_truncate(this: f64) -> f64 {
    this.trunc()
}

/// `_Double._modulo(other)`: `%` euclidiano de `double` (resultado nunca
/// negativo), como `DoubleModulo` da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_modulo(this: f64, other_one: f64) -> f64 {
    let r = this % other_one;
    if r < 0.0 {
        r + other_one.abs()
    } else if r == 0.0 {
        // A VM normaliza -0.0 para +0.0 no módulo; remainder preserva o sinal.
        0.0
    } else {
        r
    }
}

/// `_Double._remainder(other)`: o resto com o sinal do dividendo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_remainder(this: f64, other_one: f64) -> f64 {
    this % other_one
}

/// `_Double.hashCode` (o `HashDoubleOp` da VM): o do `int` quando o valor é
/// inteiro e cabe em 64 bits (`1.0.hashCode == 1.hashCode`, exigido pela
/// igualdade de `num`); senão os bits dobrados, `(b ^ b >>> 32)` em 62 bits.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_hashCode(this: f64) -> i64 {
    if this.is_finite() && this == this.trunc() && (-9.223_372_036_854_775_808e18..9.223_372_036_854_775_808e18).contains(&this) {
        return dartforge_nativo_DartForge_int_hashCode(this as i64);
    }
    let b = this.to_bits();
    ((b ^ (b >> 32)) & 0x3fff_ffff_ffff_ffff) as i64
}

/// `double._nativeParse(str, start, end)` (`Double_parse`): o número do
/// texto `[start, end)` ou null — decimal com expoente opcional, `NaN`,
/// `Infinity`, com sinal (o `CStringToDouble` da VM; `inf`/`nan` em
/// minúsculas não são aceitos, como na VM). Devolve a caixa (`RETORNO_REF`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_parse(text: i64, start: i64, end: i64) -> i64 {
    // Só a fatia: o `_JsonStringParser` passa o documento inteiro (`chunk`)
    // a cada número, e copiar o texto todo tornava o `jsonDecode` quadrático.
    // O número curto em ASCII (o de todo JSON) vai por um buffer da pilha,
    // sem as duas alocações da fatia e da `String`.
    let mut stack = [0u8; 64];
    let (i, f) = (start.max(0) as usize, end.max(0) as usize);
    let read_so_far = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(t) = heap.text(text) else { return Err(None) };
        if f < i || f > t.len() {
            return Err(None);
        }
        if f - i <= stack.len() && (i..f).all(|k| t.unit(k) < 0x80) {
            for k in i..f {
                stack[k - i] = t.unit(k) as u8;
            }
            return Ok(f - i);
        }
        Err(Some(t.piece(i, f).para_string()))
    });
    let long;
    let t: &str = match read_so_far {
        // Só ASCII foi copiado.
        Ok(n) => std::str::from_utf8(&stack[..n]).expect("ASCII"),
        Err(Some(s)) => {
            long = s;
            &long
        }
        Err(None) => return 0,
    };
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    let valid = body == "NaN"
        || body == "Infinity"
        || (!body.is_empty()
            && body.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
            && body.chars().any(|c| c.is_ascii_digit()));
    if !valid {
        return 0;
    }
    let v = if body == "NaN" {
        f64::NAN
    } else if body == "Infinity" {
        if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY }
    } else {
        match t.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return 0,
        }
    };
    HEAP.with(|heap| heap.borrow_mut().box_double(v))
}

/// Intrínsecos de `dart:math` (`_sqrt`, `_sin`…): a função da libm do Rust.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_sqrt(x: f64) -> f64 {
    x.sqrt()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_sin(x: f64) -> f64 {
    x.sin()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_cos(x: f64) -> f64 {
    x.cos()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_tan(x: f64) -> f64 {
    x.tan()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_acos(x: f64) -> f64 {
    x.acos()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_asin(x: f64) -> f64 {
    x.asin()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_atan(x: f64) -> f64 {
    x.atan()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_atan2(a: f64, b: f64) -> f64 {
    a.atan2(b)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_exp(x: f64) -> f64 {
    x.exp()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_log(x: f64) -> f64 {
    x.ln()
}
/// `_doublePow(base, exponent)`: `pow` da C (a VM chama `pow`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_math_pow(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

#[cfg(test)]
mod testes_modulo_double {
    use super::dartforge_nativo_DartForge_double_modulo as modulo;

    #[test]
    fn follows_vm_signs_and_zero() {
        assert_eq!(modulo(-7.5, 2.0), 0.5);
        assert_eq!(modulo(-7.5, -2.0), 0.5);
        assert_eq!(modulo(7.5, -2.0), 1.5);
        assert_eq!(modulo(-7.5, f64::INFINITY), f64::INFINITY);
        assert_eq!(modulo(-0.0, 2.0).to_bits(), 0.0f64.to_bits());
        assert!(modulo(7.5, 0.0).is_nan());
    }
}

#[cfg(test)]
mod tests_native_numbers {
    //! Os valores esperados são os da VM 3.6.2 (`print`, medidos).
    use super::*;

    /// A escrita sem alocação dá o mesmo texto que a forma antiga, em
    /// valores de todas as faixas de expoente (os bits de um gerador).
    #[test]
    fn double_to_string_without_allocating_matches_previous() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..200_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let d = f64::from_bits(x);
            assert_eq!(vm_double_string(d), vm_double_string_old(d), "{x:#x}");
            let short = (x % 2_000_000) as f64 / 1000.0 - 1000.0;
            assert_eq!(vm_double_string(short), vm_double_string_old(short));
        }
    }

    #[test]
    fn double_to_string_e_o_da_vm() {
        let cases: [(f64, &str); 27] = [
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
        for (d, expected) in cases {
            assert_eq!(vm_double_string(d), expected, "{d:e}");
        }
        assert_eq!(vm_double_string(f64::INFINITY), "Infinity");
        assert_eq!(vm_double_string(f64::NEG_INFINITY), "-Infinity");
    }

    #[test]
    fn integers_with_vm_rule() {
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
fn exact_decimal(d: f64) -> (Vec<u8>, i32) {
    let bits = d.abs().to_bits();
    let exp_bits = ((bits >> 52) & 0x7FF) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e) = if exp_bits == 0 { (frac, -1074) } else { (frac | (1u64 << 52), exp_bits - 1075) };
    let mut limbs: Vec<u32> = vec![(m % 1_000_000_000) as u32, ((m / 1_000_000_000) % 1_000_000_000) as u32, (m / 1_000_000_000_000_000_000) as u32];
    let mut mul = |f: u64, times: i32, limbs: &mut Vec<u32>| {
        for _ in 0..times {
            let mut goes = 0u64;
            for l in limbs.iter_mut() {
                let v = u64::from(*l) * f + goes;
                *l = (v % 1_000_000_000) as u32;
                goes = v / 1_000_000_000;
            }
            if goes > 0 {
                limbs.push(goes as u32);
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
    let mut text = limbs.last().unwrap().to_string();
    for l in limbs.iter().rev().skip(1) {
        text.push_str(&format!("{l:09}"));
    }
    let digits: Vec<u8> = text.bytes().map(|b| b - b'0').collect();
    let point = digits.len() as i32 - k;
    (digits, point)
}

/// Mantém `n` dígitos arredondando **meio para cima** sobre a expansão
/// exata (o que o double-conversion faz em `ToFixed`/`ToExponential`/
/// `ToPrecision`: `0.125.toStringAsFixed(2)` é `0.13`, `2.5` → `3`).
/// Devolve os dígitos (exatamente `n`, ou `n + 1` com vai-um) e o ponto.
fn round_to(digits: &[u8], point: i32, n: i32) -> (Vec<u8>, i32) {
    if n < 0 {
        return (Vec::new(), point);
    }
    let n = n as usize;
    let mut v: Vec<u8> = digits.iter().copied().take(n).collect();
    v.resize(n, 0);
    if digits.get(n).is_some_and(|&x| x >= 5) {
        let mut i = n;
        loop {
            if i == 0 {
                v.insert(0, 1);
                return (v, point + 1);
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
    (v, point)
}

fn digits_string(v: &[u8]) -> String {
    v.iter().map(|d| char::from(b'0' + d)).collect()
}

/// `toStringAsFixed` (native `Double_toStringAsFixed`, o `ToFixed` da VM):
/// o Dart já conferiu `0 <= f <= 20`, NaN e `|d| >= 1e21`. O sinal vem do
/// bit de sinal (`(-0.0).toStringAsFixed(2)` é `-0.00`).
pub fn double_with_fixed(d: f64, f: i64) -> String {
    let f = f.clamp(0, 20) as i32;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    let (digits, point) = if d == 0.0 { (vec![0], 1) } else { exact_decimal(d) };
    let (v, point) = round_to(&digits, point, point + f);
    // `v` tem os dígitos até a casa `f` depois do ponto; completa à esquerda.
    let integers = point.max(0) as usize;
    let mut all_items: Vec<u8> = Vec::new();
    if point < 0 {
        all_items.extend(std::iter::repeat(0).take((-point) as usize));
    }
    all_items.extend(&v);
    all_items.resize(integers + f as usize, 0);
    let (int, fr) = all_items.split_at(integers);
    if int.is_empty() {
        s.push('0');
    } else {
        s.push_str(&digits_string(int));
    }
    if f > 0 {
        s.push('.');
        s.push_str(&digits_string(fr));
    }
    s
}

/// `toStringAsExponential(r)` (`ToExponential`): `r + 1` dígitos
/// significativos, expoente com sinal (`1.250e-1`, `0.000e+0`).
pub fn double_with_exponent(d: f64, r: i64) -> String {
    // A VM passa -1 quando o argumento opcional é omitido. Nesse caso ela
    // pede a menor representação decimal que volta ao mesmo double, e não
    // zero casas fracionárias.
    if r == -1 {
        return double_with_short_exponent(d);
    }
    let r = r.clamp(0, 20) as i32;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    let (v, exponent) = if d == 0.0 {
        (vec![0; (r + 1) as usize], 0)
    } else {
        let (digits, point) = exact_decimal(d);
        let (mut v, point) = round_to(&digits, point, r + 1);
        v.truncate((r + 1) as usize);
        (v, point - 1)
    };
    s.push(char::from(b'0' + v[0]));
    if r > 0 {
        s.push('.');
        s.push_str(&digits_string(&v[1..]));
    }
    s.push('e');
    s.push(if exponent < 0 { '-' } else { '+' });
    s.push_str(&exponent.abs().to_string());
    s
}

fn double_with_short_exponent(d: f64) -> String {
    let mut out = String::new();
    if d.is_sign_negative() {
        out.push('-');
    }
    if d == 0.0 {
        out.push_str("0e+0");
        return out;
    }

    // `Display` usa a representação decimal curta com round-trip. Aqui só
    // deslocamos o ponto para a forma exponencial pedida por Dart.
    let decimal = d.abs().to_string();
    let (mantissa, original_exponent) = match decimal.find(['e', 'E']) {
        Some(i) => (&decimal[..i], decimal[i + 1..].parse::<i32>().unwrap_or(0)),
        None => (decimal.as_str(), 0),
    };
    let dot_pos = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let first = digits.iter().position(|&b| b != b'0').expect("double não zero");
    let last_one = digits.iter().rposition(|&b| b != b'0').unwrap();
    let exponent = dot_pos - first as i32 - 1 + original_exponent;
    out.push(digits[first] as char);
    if first < last_one {
        out.push('.');
        for &b in &digits[first + 1..=last_one] {
            out.push(b as char);
        }
    }
    out.push('e');
    out.push(if exponent < 0 { '-' } else { '+' });
    out.push_str(&exponent.abs().to_string());
    out
}

/// `toStringAsPrecision(p)` (`ToPrecision`, com até 6 zeros à esquerda e
/// nenhum à direita): notação exponencial quando o expoente é `< -6` ou
/// `>= p`.
pub fn double_with_precision(d: f64, p: i64) -> String {
    let p = p.clamp(1, 21) as i32;
    if d == 0.0 {
        let mut s = String::from(if d.is_sign_negative() { "-0" } else { "0" });
        if p > 1 {
            s.push('.');
            s.push_str(&"0".repeat((p - 1) as usize));
        }
        return s;
    }
    let (digits, point) = exact_decimal(d);
    let (mut v, point) = round_to(&digits, point, p);
    v.truncate(p as usize);
    let exponent = point - 1;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    if exponent < -6 || exponent >= p {
        s.push(char::from(b'0' + v[0]));
        if p > 1 {
            s.push('.');
            s.push_str(&digits_string(&v[1..]));
        }
        s.push('e');
        s.push(if exponent < 0 { '-' } else { '+' });
        s.push_str(&exponent.abs().to_string());
    } else if point <= 0 {
        s.push_str("0.");
        s.push_str(&"0".repeat((-point) as usize));
        s.push_str(&digits_string(&v));
    } else if point >= p {
        s.push_str(&digits_string(&v));
    } else {
        s.push_str(&digits_string(&v[..point as usize]));
        s.push('.');
        s.push_str(&digits_string(&v[point as usize..]));
    }
    s
}

/// `Double_toStringAsFixed`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toStringAsFixed(this: f64, digits: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_str(&double_with_fixed(this, digits)))
}

/// `Double_toStringAsExponential`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toStringAsExponential(this: f64, digits: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_str(&double_with_exponent(this, digits)))
}

/// `Double_toStringAsPrecision`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_toStringAsPrecision(this: f64, precision: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_str(&double_with_precision(this, precision)))
}

#[cfg(test)]
mod testes_to_string_as {
    //! Os esperados são os da VM 3.6.2 (medidos): `toStringAsFixed(0)`,
    //! `(2)`, `(5)`, `toStringAsExponential(3)`, `toStringAsPrecision(4)`.
    use super::*;

    #[test]
    fn exponential_without_precision_uses_vm_short_decimal() {
        let cases = [
            (123456.789, "1.23456789e+5"),
            (1.0, "1e+0"),
            (-0.0, "-0e+0"),
            (1e-7, "1e-7"),
            (1e20, "1e+20"),
            (0.1 / 3.0, "3.333333333333333e-2"),
        ];
        for (value, expected) in cases {
            assert_eq!(double_with_exponent(value, -1), expected, "{value}");
        }
    }

    #[test]
    fn the_three_forms_are_the_vms() {
        let cases: [(f64, [&str; 5]); 18] = [
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
        for (d, e) in cases {
            assert_eq!(double_with_fixed(d, 0), e[0], "{d}.toStringAsFixed(0)");
            assert_eq!(double_with_fixed(d, 2), e[1], "{d}.toStringAsFixed(2)");
            assert_eq!(double_with_fixed(d, 5), e[2], "{d}.toStringAsFixed(5)");
            assert_eq!(double_with_exponent(d, 3), e[3], "{d}.toStringAsExponential(3)");
            assert_eq!(double_with_precision(d, 4), e[4], "{d}.toStringAsPrecision(4)");
        }
    }
}
