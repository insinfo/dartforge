// Native runtime: the SIMD types of `dart:typed_data` — `Float32x4`, `Int32x4`
// and `Float64x2` (the VM's `runtime/lib/simd128.cc`) — and access to them in
// typed lists (`_getFloat32x4`…).
//
// A SIMD value is a `RAW` block of two words with the lanes (`typed_lists.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §2.5), with the fixed cid `_Float32x4`,
// `_Int32x4` or `_Float64x2` (19–21): immutable, compared by identity as in the
// VM, and with the lanes in host endianness, as in the `Float32x4List` lists.
// Generated code boxes and unboxes inline (`@df.simd_box`,
// `llvm/tipados_ir.rs`). The operations follow the VM lane by lane: `double`s
// become `float` by rounding, comparisons give `-1`/`0`, `clamp` is
// `max(min(v, upper), lower)` (with the architecture's `min`/`max`,
// `clamp_hint`) and a `shuffle` mask outside `0..255` is a `RangeError`.

const CID_FLOAT32X4: i32 = crate::layout::cid::FLOAT32X4;
const CID_INT32X4: i32 = crate::layout::cid::INT32X4;
const CID_FLOAT64X2: i32 = crate::layout::cid::FLOAT64X2;

/// A new SIMD value of class `cid` with the lanes (`_type` is that of the element
/// of the corresponding list, only documentation at the call site).
fn simd_new(cid: i32, _type: u8, bytes: [u8; 16]) -> i64 {
    HEAP.with(|h| h.borrow_mut().new_simd(cid, bytes))
}

fn simd_bytes(h: i64) -> [u8; 16] {
    match HEAP.with(|heap| heap.borrow().simd(h)) {
        Some(b) => b,
        None => panic!("compiler bug: SIMD value expected"),
    }
}

fn f32x4_of(h: i64) -> [f32; 4] {
    let b = simd_bytes(h);
    std::array::from_fn(|i| f32::from_ne_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]))
}

fn i32x4_of(h: i64) -> [i32; 4] {
    let b = simd_bytes(h);
    std::array::from_fn(|i| i32::from_ne_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]))
}

fn f64x2_of(h: i64) -> [f64; 2] {
    let b = simd_bytes(h);
    std::array::from_fn(|i| {
        let mut x = [0u8; 8];
        x.copy_from_slice(&b[8 * i..8 * i + 8]);
        f64::from_ne_bytes(x)
    })
}

fn new_f32x4(v: [f32; 4]) -> i64 {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[4 * i..4 * i + 4].copy_from_slice(&x.to_ne_bytes());
    }
    simd_new(CID_FLOAT32X4, TYPE_FLOAT32X4, b)
}

fn new_i32x4(v: [i32; 4]) -> i64 {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[4 * i..4 * i + 4].copy_from_slice(&x.to_ne_bytes());
    }
    simd_new(CID_INT32X4, TYPE_INT32X4, b)
}

fn new_f64x2(v: [f64; 2]) -> i64 {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[8 * i..8 * i + 8].copy_from_slice(&x.to_ne_bytes());
    }
    simd_new(CID_FLOAT64X2, TYPE_FLOAT64X2, b)
}

/// The VM's `Utils::Minimum`/`Maximum` (with NaN, the second operand).
fn simd_min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { a } else { b }
}
fn simd_max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

/// The `clamp` of one lane, `max(min(v, upper), lower)`, like the VM's
/// `Float32x4_clamp`/`Float64x2_clamp` (`runtime/lib/simd128.cc`), which changes
/// with the architecture: on x64 they are the ternaries of `simd_min`/`simd_max`;
/// on arm64 they are its `vminf`/`vmaxf` (with NaN the first operand stays, and
/// between ±0 `min` gives `-0` and `max` gives `+0`).
#[cfg(target_arch = "aarch64")]
fn clamp_hint<T: PartialOrd + Copy>(v: T, lo: T, hi: T, negative: impl Fn(T) -> bool) -> T {
    let m = if v == hi {
        if negative(v) { v } else { hi }
    } else if v > hi {
        hi
    } else {
        v
    };
    if m == lo {
        if negative(m) { lo } else { m }
    } else if m < lo {
        lo
    } else {
        m
    }
}
#[cfg(not(any(target_arch = "aarch64", target_arch = "wasm32")))]
fn clamp_hint<T: PartialOrd + Copy>(v: T, lo: T, hi: T, _negative: impl Fn(T) -> bool) -> T {
    simd_max(simd_min(v, hi), lo)
}
/// dart2wasm's `clamp` (`_internal/wasm/lib/simd.dart`): `m = v > hi ? hi : v`,
/// then `m < lo ? lo : m`; a NaN lane stays NaN.
#[cfg(target_arch = "wasm32")]
fn clamp_hint<T: PartialOrd + Copy>(v: T, lo: T, hi: T, _negative: impl Fn(T) -> bool) -> T {
    let m = if v > hi { hi } else { v };
    if m < lo { lo } else { m }
}

/// The `shuffle`/`shuffleMix` mask: `0..255`, otherwise `RangeError`.
fn valid_mask(m: i64) -> Option<usize> {
    if (0..=255).contains(&m) {
        Some(m as usize)
    } else {
        throw_range_err(m, 0, 255, "mask");
        None
    }
}

fn shuffle<T: Copy>(a: [T; 4], b: [T; 4], m: usize) -> [T; 4] {
    [a[m & 3], a[(m >> 2) & 3], b[(m >> 4) & 3], b[(m >> 6) & 3]]
}

// --- Float32x4 ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_fromDoubles(x: f64, y: f64, z: f64, w: f64) -> i64 {
    new_f32x4([x as f32, y as f32, z as f32, w as f32])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_splat(v: f64) -> i64 {
    new_f32x4([v as f32; 4])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_zero() -> i64 {
    new_f32x4([0.0; 4])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_fromInt32x4Bits(v: i64) -> i64 {
    let b = simd_bytes(v);
    simd_new(CID_FLOAT32X4, TYPE_FLOAT32X4, b)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_fromFloat64x2(v: i64) -> i64 {
    let [x, y] = f64x2_of(v);
    new_f32x4([x as f32, y as f32, 0.0, 0.0])
}

fn f32x4_binary(a: i64, b: i64, f: impl Fn(f32, f32) -> f32) -> i64 {
    let (a, b) = (f32x4_of(a), f32x4_of(b));
    new_f32x4(std::array::from_fn(|i| f(a[i], b[i])))
}
fn f32x4_unary(a: i64, f: impl Fn(f32) -> f32) -> i64 {
    let a = f32x4_of(a);
    new_f32x4(std::array::from_fn(|i| f(a[i])))
}
fn f32x4_compare(a: i64, b: i64, f: impl Fn(f32, f32) -> bool) -> i64 {
    let (a, b) = (f32x4_of(a), f32x4_of(b));
    new_i32x4(std::array::from_fn(|i| if f(a[i], b[i]) { -1 } else { 0 }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_add(a: i64, b: i64) -> i64 {
    f32x4_binary(a, b, |x, y| x + y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_sub(a: i64, b: i64) -> i64 {
    f32x4_binary(a, b, |x, y| x - y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_mul(a: i64, b: i64) -> i64 {
    f32x4_binary(a, b, |x, y| x * y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_div(a: i64, b: i64) -> i64 {
    f32x4_binary(a, b, |x, y| x / y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_min(a: i64, b: i64) -> i64 {
    f32x4_binary(a, b, simd_min)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_max(a: i64, b: i64) -> i64 {
    f32x4_binary(a, b, simd_max)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_negate(a: i64) -> i64 {
    f32x4_unary(a, |x| -x)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_abs(a: i64) -> i64 {
    f32x4_unary(a, f32::abs)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_sqrt(a: i64) -> i64 {
    f32x4_unary(a, f32::sqrt)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_reciprocal(a: i64) -> i64 {
    f32x4_unary(a, |x| 1.0 / x)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_reciprocalSqrt(a: i64) -> i64 {
    f32x4_unary(a, |x| (1.0 / x).sqrt())
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_scale(a: i64, s: f64) -> i64 {
    let s = s as f32;
    f32x4_unary(a, |x| x * s)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_clamp(a: i64, lo: i64, hi: i64) -> i64 {
    let (a, lo, hi) = (f32x4_of(a), f32x4_of(lo), f32x4_of(hi));
    new_f32x4(std::array::from_fn(|i| clamp_hint(a[i], lo[i], hi[i], f32::is_sign_negative)))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_cmpequal(a: i64, b: i64) -> i64 {
    f32x4_compare(a, b, |x, y| x == y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_cmpnequal(a: i64, b: i64) -> i64 {
    f32x4_compare(a, b, |x, y| x != y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_cmplt(a: i64, b: i64) -> i64 {
    f32x4_compare(a, b, |x, y| x < y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_cmplte(a: i64, b: i64) -> i64 {
    f32x4_compare(a, b, |x, y| x <= y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_cmpgt(a: i64, b: i64) -> i64 {
    f32x4_compare(a, b, |x, y| x > y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_cmpgte(a: i64, b: i64) -> i64 {
    f32x4_compare(a, b, |x, y| x >= y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_getX(a: i64) -> f64 {
    f64::from(f32x4_of(a)[0])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_getY(a: i64) -> f64 {
    f64::from(f32x4_of(a)[1])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_getZ(a: i64) -> f64 {
    f64::from(f32x4_of(a)[2])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_getW(a: i64) -> f64 {
    f64::from(f32x4_of(a)[3])
}
fn f32x4_with(a: i64, i: usize, v: f64) -> i64 {
    let mut a = f32x4_of(a);
    a[i] = v as f32;
    new_f32x4(a)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_setX(a: i64, v: f64) -> i64 {
    f32x4_with(a, 0, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_setY(a: i64, v: f64) -> i64 {
    f32x4_with(a, 1, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_setZ(a: i64, v: f64) -> i64 {
    f32x4_with(a, 2, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_setW(a: i64, v: f64) -> i64 {
    f32x4_with(a, 3, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_getSignMask(a: i64) -> i64 {
    f32x4_of(a).iter().enumerate().map(|(i, x)| i64::from(x.to_bits() >> 31) << i).sum()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_shuffle(a: i64, m: i64) -> i64 {
    let Some(m) = valid_mask(m) else { return 0 };
    let a = f32x4_of(a);
    new_f32x4(shuffle(a, a, m))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float32x4_shuffleMix(a: i64, b: i64, m: i64) -> i64 {
    let Some(m) = valid_mask(m) else { return 0 };
    new_f32x4(shuffle(f32x4_of(a), f32x4_of(b), m))
}

// --- Int32x4 -----------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_fromInts(x: i64, y: i64, z: i64, w: i64) -> i64 {
    new_i32x4([x as i32, y as i32, z as i32, w as i32])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_fromBools(x: u8, y: u8, z: u8, w: u8) -> i64 {
    let f = |b: u8| if b != 0 { -1 } else { 0 };
    new_i32x4([f(x), f(y), f(z), f(w)])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_fromFloat32x4Bits(v: i64) -> i64 {
    let b = simd_bytes(v);
    simd_new(CID_INT32X4, TYPE_INT32X4, b)
}
fn i32x4_binary(a: i64, b: i64, f: impl Fn(i32, i32) -> i32) -> i64 {
    let (a, b) = (i32x4_of(a), i32x4_of(b));
    new_i32x4(std::array::from_fn(|i| f(a[i], b[i])))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_or(a: i64, b: i64) -> i64 {
    i32x4_binary(a, b, |x, y| x | y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_and(a: i64, b: i64) -> i64 {
    i32x4_binary(a, b, |x, y| x & y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_xor(a: i64, b: i64) -> i64 {
    i32x4_binary(a, b, |x, y| x ^ y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_add(a: i64, b: i64) -> i64 {
    i32x4_binary(a, b, i32::wrapping_add)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_sub(a: i64, b: i64) -> i64 {
    i32x4_binary(a, b, i32::wrapping_sub)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getX(a: i64) -> i64 {
    i64::from(i32x4_of(a)[0])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getY(a: i64) -> i64 {
    i64::from(i32x4_of(a)[1])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getZ(a: i64) -> i64 {
    i64::from(i32x4_of(a)[2])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getW(a: i64) -> i64 {
    i64::from(i32x4_of(a)[3])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getFlagX(a: i64) -> u8 {
    u8::from(i32x4_of(a)[0] != 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getFlagY(a: i64) -> u8 {
    u8::from(i32x4_of(a)[1] != 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getFlagZ(a: i64) -> u8 {
    u8::from(i32x4_of(a)[2] != 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getFlagW(a: i64) -> u8 {
    u8::from(i32x4_of(a)[3] != 0)
}
fn i32x4_with(a: i64, i: usize, v: i32) -> i64 {
    let mut a = i32x4_of(a);
    a[i] = v;
    new_i32x4(a)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setX(a: i64, v: i64) -> i64 {
    i32x4_with(a, 0, v as i32)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setY(a: i64, v: i64) -> i64 {
    i32x4_with(a, 1, v as i32)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setZ(a: i64, v: i64) -> i64 {
    i32x4_with(a, 2, v as i32)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setW(a: i64, v: i64) -> i64 {
    i32x4_with(a, 3, v as i32)
}
fn flag(b: u8) -> i32 {
    if b != 0 { -1 } else { 0 }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setFlagX(a: i64, b: u8) -> i64 {
    i32x4_with(a, 0, flag(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setFlagY(a: i64, b: u8) -> i64 {
    i32x4_with(a, 1, flag(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setFlagZ(a: i64, b: u8) -> i64 {
    i32x4_with(a, 2, flag(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_setFlagW(a: i64, b: u8) -> i64 {
    i32x4_with(a, 3, flag(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_getSignMask(a: i64) -> i64 {
    i32x4_of(a).iter().enumerate().map(|(i, x)| i64::from((*x as u32) >> 31) << i).sum()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_shuffle(a: i64, m: i64) -> i64 {
    let Some(m) = valid_mask(m) else { return 0 };
    let a = i32x4_of(a);
    new_i32x4(shuffle(a, a, m))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_shuffleMix(a: i64, b: i64, m: i64) -> i64 {
    let Some(m) = valid_mask(m) else { return 0 };
    new_i32x4(shuffle(i32x4_of(a), i32x4_of(b), m))
}
/// `select(t, f)`: bit by bit, those of `t` where the mask has 1, those of `f`
/// where it has 0.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Int32x4_select(m: i64, t: i64, f: i64) -> i64 {
    let (m, t, f) = (i32x4_of(m), f32x4_of(t), f32x4_of(f));
    new_f32x4(std::array::from_fn(|i| {
        let bits = (m[i] as u32 & t[i].to_bits()) | (!(m[i] as u32) & f[i].to_bits());
        f32::from_bits(bits)
    }))
}

// --- Float64x2 ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_fromDoubles(x: f64, y: f64) -> i64 {
    new_f64x2([x, y])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_splat(v: f64) -> i64 {
    new_f64x2([v, v])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_zero() -> i64 {
    new_f64x2([0.0, 0.0])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_fromFloat32x4(v: i64) -> i64 {
    let a = f32x4_of(v);
    new_f64x2([f64::from(a[0]), f64::from(a[1])])
}
fn f64x2_binary(a: i64, b: i64, f: impl Fn(f64, f64) -> f64) -> i64 {
    let (a, b) = (f64x2_of(a), f64x2_of(b));
    new_f64x2([f(a[0], b[0]), f(a[1], b[1])])
}
fn f64x2_unary(a: i64, f: impl Fn(f64) -> f64) -> i64 {
    let a = f64x2_of(a);
    new_f64x2([f(a[0]), f(a[1])])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_add(a: i64, b: i64) -> i64 {
    f64x2_binary(a, b, |x, y| x + y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_sub(a: i64, b: i64) -> i64 {
    f64x2_binary(a, b, |x, y| x - y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_mul(a: i64, b: i64) -> i64 {
    f64x2_binary(a, b, |x, y| x * y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_div(a: i64, b: i64) -> i64 {
    f64x2_binary(a, b, |x, y| x / y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_min(a: i64, b: i64) -> i64 {
    f64x2_binary(a, b, simd_min)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_max(a: i64, b: i64) -> i64 {
    f64x2_binary(a, b, simd_max)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_negate(a: i64) -> i64 {
    f64x2_unary(a, |x| -x)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_abs(a: i64) -> i64 {
    f64x2_unary(a, f64::abs)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_sqrt(a: i64) -> i64 {
    f64x2_unary(a, f64::sqrt)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_scale(a: i64, s: f64) -> i64 {
    f64x2_unary(a, |x| x * s)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_clamp(a: i64, lo: i64, hi: i64) -> i64 {
    let (a, lo, hi) = (f64x2_of(a), f64x2_of(lo), f64x2_of(hi));
    new_f64x2(std::array::from_fn(|i| clamp_hint(a[i], lo[i], hi[i], f64::is_sign_negative)))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_getX(a: i64) -> f64 {
    f64x2_of(a)[0]
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_getY(a: i64) -> f64 {
    f64x2_of(a)[1]
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_setX(a: i64, v: f64) -> i64 {
    new_f64x2([v, f64x2_of(a)[1]])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_setY(a: i64, v: f64) -> i64 {
    new_f64x2([f64x2_of(a)[0], v])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_Float64x2_getSignMask(a: i64) -> i64 {
    let a = f64x2_of(a);
    i64::from((a[0].to_bits() >> 63) as u8) | (i64::from((a[1].to_bits() >> 63) as u8) << 1)
}

// --- In typed lists -----------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_GetFloat32x4(this: i64, off: i64) -> i64 {
    match do_read::<16>(this, off) {
        Some(b) => simd_new(CID_FLOAT32X4, TYPE_FLOAT32X4, b),
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_SetFloat32x4(this: i64, off: i64, v: i64) {
    write_value::<16>(this, off, simd_bytes(v));
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_GetInt32x4(this: i64, off: i64) -> i64 {
    match do_read::<16>(this, off) {
        Some(b) => simd_new(CID_INT32X4, TYPE_INT32X4, b),
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_SetInt32x4(this: i64, off: i64, v: i64) {
    write_value::<16>(this, off, simd_bytes(v));
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_GetFloat64x2(this: i64, off: i64) -> i64 {
    match do_read::<16>(this, off) {
        Some(b) => simd_new(CID_FLOAT64X2, TYPE_FLOAT64X2, b),
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_TypedData_SetFloat64x2(this: i64, off: i64, v: i64) {
    write_value::<16>(this, off, simd_bytes(v));
}

/// Element `i` of a `Float32x4List`, `Int32x4List` or `Float64x2List` (the VM's
/// recognized `[]`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_native_DartForge_typed_index_simd(this: i64, i: i64) -> i64 {
    let Some((t, p)) = element(this, i) else { return 0 };
    let mut b = [0u8; 16];
    // SAFETY: `p` is element `i < len` (16 bytes) of the live list.
    unsafe { std::ptr::copy_nonoverlapping(p, b.as_mut_ptr(), 16) };
    match t.ty {
        TYPE_INT32X4 => simd_new(CID_INT32X4, TYPE_INT32X4, b),
        TYPE_FLOAT64X2 => simd_new(CID_FLOAT64X2, TYPE_FLOAT64X2, b),
        _ => simd_new(CID_FLOAT32X4, TYPE_FLOAT32X4, b),
    }
}

/// A `double` like C's `%f` (the VM's `ToCString` of the SIMD types).
fn percent_f(x: f64) -> String {
    if x.is_nan() {
        "nan".to_string()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".to_string() } else { "-inf".to_string() }
    } else {
        format!("{x:.6}")
    }
}

/// The text of a SIMD value (the VM's `Object.toString`): `[x, y, z, w]` with
/// `%f` for `Float32x4`/`Float64x2` and `%08x` for `Int32x4`; `None` for other
/// values.
fn simd_string(h: i64) -> Option<String> {
    if !crate::layout::is_object(h) {
        return None;
    }
    let cid = HEAP.with(|x| {
        let x = x.borrow();
        x.is_live_object(h).then(|| x.header(h).class_id)
    })?;
    let e = |c: i32| c == cid;
    if e(CID_FLOAT32X4) {
        let v: Vec<String> = f32x4_of(h).iter().map(|x| percent_f(f64::from(*x))).collect();
        Some(format!("[{}]", v.join(", ")))
    } else if e(CID_INT32X4) {
        let v: Vec<String> = i32x4_of(h).iter().map(|x| format!("{:08x}", *x as u32)).collect();
        Some(format!("[{}]", v.join(", ")))
    } else if e(CID_FLOAT64X2) {
        let v: Vec<String> = f64x2_of(h).iter().map(|x| percent_f(*x)).collect();
        Some(format!("[{}]", v.join(", ")))
    } else {
        None
    }
}
