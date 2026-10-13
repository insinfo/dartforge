// Native runtime: `int`, `double` and `dart:math` natives of the SDK from source
// (P5b; unified space, P2: `DartForge_double_*`, `Double_parse`,
// `DartForge_int_hashCode` and `DartForge_math_*` came from `natives_lists.rs`).
//
// Each function is the VM native of the same name (`runtime/lib/integers.cc`,
// `double.cc`), with the symbol `dartforge_nativo_<Name>` and the signature in the
// HIR representation (R1): the receiver of `_IntegerImplementation`/`_Smi`/`_Mint`
// is the `i64`, that of `_Double` the `f64`; `bool` comes back as `u8`. The table
// that lowering consults is `crates/emit_native/src/nativos.rs`.
//
// The `*FromInteger` natives are called with SWAPPED operands, as in the VM:
// `int operator -(num other) => other._subFromInteger(this)` — the native's
// receiver is the RIGHT operand and the argument is the left one. Division by
// zero is checked in Dart before the call (`integers.dart`), as in the VM.

/// `Integer_addFromInteger`: `other + this` with wrapping overflow.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_addFromInteger(this: i64, other_one: i64) -> i64 {
    other_one.wrapping_add(this)
}

/// `Integer_subFromInteger`: `other - this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_subFromInteger(this: i64, other_one: i64) -> i64 {
    other_one.wrapping_sub(this)
}

/// `Integer_mulFromInteger`: `other * this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_mulFromInteger(this: i64, other_one: i64) -> i64 {
    other_one.wrapping_mul(this)
}

/// `Integer_truncDivFromInteger`: `other ~/ this`. `MIN ~/ -1` is `MIN`
/// (`Integer::ArithmeticOp`, kTRUNCDIV). `this == 0` does not get here.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_truncDivFromInteger(this: i64, other_one: i64) -> i64 {
    if this == 0 {
        throw_division_by_zero();
        return 0;
    }
    other_one.wrapping_div(this)
}

/// `Integer_moduloFromInteger`: `other % this`, always non-negative
/// (`Integer::ArithmeticOp`, kMOD: a negative remainder adds `|this|`).
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

/// Division by zero does not reach the native: `integers.dart` throws
/// `IntegerDivisionByZeroException` first (as in the VM, where the native only has
/// an `ASSERT`). Getting here is a compiler bug (N4), not a Dart exception.
fn throw_division_by_zero() {
    panic!("compiler bug: integer division by zero reached the native (Dart checks it first)");
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

/// Shift with the VM's rule (`ShiftOperationHelper` and `Integer::ShiftOp`): a
/// negative amount throws `ArgumentError`; an amount ≥ 64 gives 0 (`<<`, `>>>`) or
/// the sign (`>>`).
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

/// `Integer_shlFromInteger`: `other << this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_shlFromInteger(this: i64, other_one: i64) -> i64 {
    shift(other_one, this, 0)
}

/// `Integer_shrFromInteger`: `other >> this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_shrFromInteger(this: i64, other_one: i64) -> i64 {
    shift(other_one, this, 1)
}

/// `Integer_ushrFromInteger`: `other >>> this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_ushrFromInteger(this: i64, other_one: i64) -> i64 {
    shift(other_one, this, 2)
}

/// `Integer_greaterThanFromInteger`: `other > this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_greaterThanFromInteger(this: i64, other_one: i64) -> u8 {
    u8::from(other_one > this)
}

/// `Integer_equalToInteger`: `this == other`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Integer_equalToInteger(this: i64, other_one: i64) -> u8 {
    u8::from(this == other_one)
}

/// `Smi_bitNegate` and `Mint_bitNegate`: `~this`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Smi_bitNegate(this: i64) -> i64 {
    !this
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Mint_bitNegate(this: i64) -> i64 {
    !this
}

/// `Smi_bitLength` and `Mint_bitLength` (`Utils::BitLength`): the significant bits,
/// counting those of `~v` for negatives.
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

/// `Double_add` and family: IEEE arithmetic with the receiver on the left
/// (`_Double.+` calls `_add(other.toDouble())`).
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

/// `Double_equal`: IEEE `==` (NaN differs from everything).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_equal(this: f64, other_one: f64) -> u8 {
    u8::from(this == other_one)
}

/// `Double_equalToInteger`: `this == other.toDouble()`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_equalToInteger(this: f64, other_one: i64) -> u8 {
    u8::from(this == other_one as f64)
}

/// `Double_greaterThan`: `this > other`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_greaterThan(this: f64, other_one: f64) -> u8 {
    u8::from(this > other_one)
}

/// `Double_greaterThanFromInteger`: `other > this` (swapped operands, like `int`'s
/// `*FromInteger`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_greaterThanFromInteger(this: f64, other_one: i64) -> u8 {
    u8::from((other_one as f64) > this)
}

/// `Double_flipSignBit`: `-this` (including for 0.0 and NaN).
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

/// `Double_getIsNegative`: the sign bit, except NaN (`-0.0` is negative).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_getIsNegative(this: f64) -> u8 {
    u8::from(!this.is_nan() && this.is_sign_negative())
}

/// `double.toString` like the VM (`DoubleToCString`,
/// `runtime/vm/double_conversion.cc`): double-conversion's `ToShortest` with
/// `decimal_in_shortest_low = -6`, `high = 21`, `EMIT_POSITIVE_EXPONENT_SIGN`,
/// `EMIT_TRAILING_DECIMAL_POINT` and `EMIT_TRAILING_ZERO_AFTER_POINT`. The shortest
/// digits that round-trip to the same `double` come from Rust's `{:e}` formatting
/// (the same criterion as `ToShortest`).
pub fn vm_double_string(d: f64) -> String {
    let mut buf = [0u8; VM_DOUBLE_SIZE];
    let n = write_vm_double(d, &mut buf);
    // ASCII only.
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// The longest text of [`vm_double_string`]: `-` + 17 digits + `.` + `e-308` (or
/// `0.000000` + 17 digits) fits with room to spare.
const VM_DOUBLE_SIZE: usize = 48;

/// Stack buffer for `{:e}` (without the `String` of `format!`).
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

/// [`vm_double_string`] written into `out` (ASCII), without allocating: the
/// `Double_toString` of `jsonEncode` on a list of `double`s allocated four
/// `String`s per number. Returns the length.
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
    // The shortest digits: `{:e}` of the absolute value, `d[.ddd]e<exp>`.
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
    let point = exponent + 1; // position of the decimal point among the digits
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

/// The old form of [`vm_double_string`], for the tests to check the new one
/// against it.
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
    let point = exponent + 1; // position of the decimal point among the digits
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

/// The overlay's `_Double._bitsDe(d)` (`double.dart`): the bits of the `double`,
/// the key of the `toString` cache (the VM's `identical` of two `double`s compares
/// the bits).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_bits(d: f64) -> i64 {
    d.to_bits() as i64
}

/// The overlay's `_Smi._deInteiro(v)` (`integers.dart`): the decimal digits of
/// `v`, the text of the VM's `_Smi.toString`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_int_toString(v: i64) -> i64 {
    dartforge_to_string_i64(v)
}

/// `has63BitSmis()` (`DartForge_verdadeiro`): the `Smi` here has 63 bits (R10).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_verdadeiro() -> u8 {
    1
}

/// `_Smi.hashCode`/`_Mint.hashCode`: the VM's `HashIntegerOp` — the 96-bit product
/// of `this` (unsigned) by `0x2d51`, the three 32-bit words combined by xor, cut to
/// 30 bits. Lowering does it inline (`lower/boxes.rs`); this remains for the call.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_int_hashCode(this: i64) -> i64 {
    let p = u128::from(this as u64) * 0x2d51;
    let x = (p as u32) ^ ((p >> 32) as u32) ^ ((p >> 64) as u32);
    i64::from(x & 0x3fff_ffff)
}

/// `_Double.toInt()`: truncated, saturated outside the 64-bit range (like the VM);
/// NaN and infinity throw `UnsupportedError("Infinity or NaN toInt")`.
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

/// `_Double.floorToDouble()` and the like.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_floor(this: f64) -> f64 {
    this.floor()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_ceil(this: f64) -> f64 {
    this.ceil()
}

/// `roundToDouble`: half away from zero (the VM's `round`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_round(this: f64) -> f64 {
    this.round()
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_truncate(this: f64) -> f64 {
    this.trunc()
}

/// `_Double._modulo(other)`: the Euclidean `%` of `double` (the result is never
/// negative), like the VM's `DoubleModulo`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_modulo(this: f64, other_one: f64) -> f64 {
    let r = this % other_one;
    if r < 0.0 {
        r + other_one.abs()
    } else if r == 0.0 {
        // The VM normalizes -0.0 to +0.0 in modulo; remainder keeps the sign.
        0.0
    } else {
        r
    }
}

/// `_Double._remainder(other)`: the remainder with the sign of the dividend.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_remainder(this: f64, other_one: f64) -> f64 {
    this % other_one
}

/// `_Double.hashCode` (the VM's `HashDoubleOp`): that of the `int` when the value is
/// integral and fits in 64 bits (`1.0.hashCode == 1.hashCode`, required by `num`
/// equality); otherwise the folded bits, `(b ^ b >>> 32)` in 62 bits.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_double_hashCode(this: f64) -> i64 {
    if this.is_finite() && this == this.trunc() && (-9.223_372_036_854_775_808e18..9.223_372_036_854_775_808e18).contains(&this) {
        return dartforge_nativo_DartForge_int_hashCode(this as i64);
    }
    let b = this.to_bits();
    ((b ^ (b >> 32)) & 0x3fff_ffff_ffff_ffff) as i64
}

/// `double._nativeParse(str, start, end)` (`Double_parse`): the number in the text
/// `[start, end)` or null — decimal with an optional exponent, `NaN`, `Infinity`,
/// with a sign (the VM's `CStringToDouble`; lowercase `inf`/`nan` are not
/// accepted, as in the VM). Returns the box (`RETORNO_REF`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Double_parse(text: i64, start: i64, end: i64) -> i64 {
    // Only the slice: `_JsonStringParser` passes the whole document (`chunk`) for
    // every number, and copying the whole text made `jsonDecode` quadratic. A
    // short ASCII number (that of all JSON) goes through a stack buffer, without
    // the two allocations of the slice and the `String`.
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
        // Only ASCII was copied.
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

/// `dart:math` intrinsics (`_sqrt`, `_sin`…): the function from Rust's libm.
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
/// `_doublePow(base, exponent)`: C's `pow` (the VM calls `pow`).
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
    //! The expected values are those of the VM 3.6.2 (`print`, measured).
    use super::*;

    /// The allocation-free writing gives the same text as the old form, on values
    /// from every exponent range (the bits from a generator).
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
        // Swapped operands: `a op b` is `b._opFromInteger(a)`.
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

/// The EXACT decimal expansion of `|d|` (finite, non-zero): the digits (without
/// leading zeros) and the position of the point (how many digits come before it;
/// ≤ 0 is `0.000ddd`). `|d| = m·2^e`; for `e < 0` it is `m·5^k / 10^k`. Large
/// integers in base 10^9, with only multiplication by 2 and by 5.
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

/// Keeps `n` digits rounding **half up** on the exact expansion (what
/// double-conversion does in `ToFixed`/`ToExponential`/`ToPrecision`:
/// `0.125.toStringAsFixed(2)` is `0.13`, `2.5` → `3`). Returns the digits (exactly
/// `n`, or `n + 1` with a carry) and the point.
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

/// `toStringAsFixed` (the `Double_toStringAsFixed` native, the VM's `ToFixed`):
/// Dart already checked `0 <= f <= 20`, NaN and `|d| >= 1e21`. The sign comes from
/// the sign bit (`(-0.0).toStringAsFixed(2)` is `-0.00`).
pub fn double_with_fixed(d: f64, f: i64) -> String {
    let f = f.clamp(0, 20) as i32;
    let mut s = String::new();
    if d.is_sign_negative() {
        s.push('-');
    }
    let (digits, point) = if d == 0.0 { (vec![0], 1) } else { exact_decimal(d) };
    let (v, point) = round_to(&digits, point, point + f);
    // `v` has the digits up to place `f` after the point; pad on the left.
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

/// `toStringAsExponential(r)` (`ToExponential`): `r + 1` significant digits, a
/// signed exponent (`1.250e-1`, `0.000e+0`).
pub fn double_with_exponent(d: f64, r: i64) -> String {
    // The VM passes -1 when the optional argument is omitted. In that case it
    // asks for the shortest decimal representation that round-trips to the same
    // double, and not zero fractional digits.
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

    // `Display` uses the short round-trip decimal representation. Here we only
    // move the point to the exponential form Dart asks for.
    let decimal = d.abs().to_string();
    let (mantissa, original_exponent) = match decimal.find(['e', 'E']) {
        Some(i) => (&decimal[..i], decimal[i + 1..].parse::<i32>().unwrap_or(0)),
        None => (decimal.as_str(), 0),
    };
    let dot_pos = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let first = digits.iter().position(|&b| b != b'0').expect("non-zero double");
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

/// `toStringAsPrecision(p)` (`ToPrecision`, with up to 6 leading zeros and no
/// trailing ones): exponential notation when the exponent is `< -6` or `>= p`.
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
    //! The expected values are those of the VM 3.6.2 (measured): `toStringAsFixed(0)`,
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
