// Runtime nativo: os tipos SIMD do `dart:typed_data` — `Float32x4`,
// `Int32x4` e `Float64x2` (`runtime/lib/simd128.cc` da VM) — e o acesso a
// eles nas listas tipadas (`_getFloat32x4`…).
//
// Um valor SIMD é um bloco `BRUTO` de duas palavras com as pistas
// (`tipadas.rs`, docs/NATIVO-ESPACO-UNIFICADO.md §2.5), de cid fixo
// `_Float32x4`, `_Int32x4` ou `_Float64x2` (19–21): imutável, comparado por
// identidade como na VM, e com as pistas no endian do hospedeiro, como nas
// listas `Float32x4List`. O código gerado encaixota e desencaixota em linha
// (`@df.simd_caixa`, `llvm/tipados_ir.rs`). As operações seguem a VM pista a
// pista: os `double` viram `float` por arredondamento, as comparações dão
// `-1`/`0`, o `clamp` é `max(min(v, superior), inferior)` (com o `min`/`max`
// da arquitetura, `clamp_pista`) e a máscara de `shuffle` fora de `0..255` é
// `RangeError`.

const CID_FLOAT32X4: i32 = crate::layout::cid::FLOAT32X4;
const CID_INT32X4: i32 = crate::layout::cid::INT32X4;
const CID_FLOAT64X2: i32 = crate::layout::cid::FLOAT64X2;

/// Um valor SIMD novo da classe `cid` com as pistas (`_tipo` é o do elemento
/// da lista correspondente, só documentação no ponto de chamada).
fn simd_novo(cid: i32, _tipo: u8, bytes: [u8; 16]) -> i64 {
    HEAP.with(|h| h.borrow_mut().novo_simd(cid, bytes))
}

fn simd_bytes(h: i64) -> [u8; 16] {
    match HEAP.with(|heap| heap.borrow().simd(h)) {
        Some(b) => b,
        None => panic!("bug do compilador: valor SIMD esperado"),
    }
}

fn f32x4_de(h: i64) -> [f32; 4] {
    let b = simd_bytes(h);
    std::array::from_fn(|i| f32::from_ne_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]))
}

fn i32x4_de(h: i64) -> [i32; 4] {
    let b = simd_bytes(h);
    std::array::from_fn(|i| i32::from_ne_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]))
}

fn f64x2_de(h: i64) -> [f64; 2] {
    let b = simd_bytes(h);
    std::array::from_fn(|i| {
        let mut x = [0u8; 8];
        x.copy_from_slice(&b[8 * i..8 * i + 8]);
        f64::from_ne_bytes(x)
    })
}

fn novo_f32x4(v: [f32; 4]) -> i64 {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[4 * i..4 * i + 4].copy_from_slice(&x.to_ne_bytes());
    }
    simd_novo(CID_FLOAT32X4, TIPO_FLOAT32X4, b)
}

fn novo_i32x4(v: [i32; 4]) -> i64 {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[4 * i..4 * i + 4].copy_from_slice(&x.to_ne_bytes());
    }
    simd_novo(CID_INT32X4, TIPO_INT32X4, b)
}

fn novo_f64x2(v: [f64; 2]) -> i64 {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[8 * i..8 * i + 8].copy_from_slice(&x.to_ne_bytes());
    }
    simd_novo(CID_FLOAT64X2, TIPO_FLOAT64X2, b)
}

/// `Utils::Minimum`/`Maximum` da VM (com NaN, o segundo operando).
fn simd_min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { a } else { b }
}
fn simd_max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

/// O `clamp` de uma pista, `max(min(v, superior), inferior)`, como o
/// `Float32x4_clamp`/`Float64x2_clamp` da VM (`runtime/lib/simd128.cc`), que
/// muda com a arquitetura: no x64 são os ternários de `simd_min`/`simd_max`;
/// no arm64 são o `vminf`/`vmaxf` dela (com NaN fica o primeiro operando, e
/// entre ±0 o `min` dá `-0` e o `max` dá `+0`).
#[cfg(target_arch = "aarch64")]
fn clamp_pista<T: PartialOrd + Copy>(v: T, lo: T, hi: T, negativo: impl Fn(T) -> bool) -> T {
    let m = if v == hi {
        if negativo(v) { v } else { hi }
    } else if v > hi {
        hi
    } else {
        v
    };
    if m == lo {
        if negativo(m) { lo } else { m }
    } else if m < lo {
        lo
    } else {
        m
    }
}
#[cfg(not(target_arch = "aarch64"))]
fn clamp_pista<T: PartialOrd + Copy>(v: T, lo: T, hi: T, _negativo: impl Fn(T) -> bool) -> T {
    simd_max(simd_min(v, hi), lo)
}

/// A máscara de `shuffle`/`shuffleMix`: `0..255`, senão `RangeError`.
fn mascara_valida(m: i64) -> Option<usize> {
    if (0..=255).contains(&m) {
        Some(m as usize)
    } else {
        lancar_range(m, 0, 255, "mask");
        None
    }
}

fn embaralhar<T: Copy>(a: [T; 4], b: [T; 4], m: usize) -> [T; 4] {
    [a[m & 3], a[(m >> 2) & 3], b[(m >> 4) & 3], b[(m >> 6) & 3]]
}

// --- Float32x4 ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_fromDoubles(x: f64, y: f64, z: f64, w: f64) -> i64 {
    novo_f32x4([x as f32, y as f32, z as f32, w as f32])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_splat(v: f64) -> i64 {
    novo_f32x4([v as f32; 4])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_zero() -> i64 {
    novo_f32x4([0.0; 4])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_fromInt32x4Bits(v: i64) -> i64 {
    let b = simd_bytes(v);
    simd_novo(CID_FLOAT32X4, TIPO_FLOAT32X4, b)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_fromFloat64x2(v: i64) -> i64 {
    let [x, y] = f64x2_de(v);
    novo_f32x4([x as f32, y as f32, 0.0, 0.0])
}

fn f32x4_binaria(a: i64, b: i64, f: impl Fn(f32, f32) -> f32) -> i64 {
    let (a, b) = (f32x4_de(a), f32x4_de(b));
    novo_f32x4(std::array::from_fn(|i| f(a[i], b[i])))
}
fn f32x4_unaria(a: i64, f: impl Fn(f32) -> f32) -> i64 {
    let a = f32x4_de(a);
    novo_f32x4(std::array::from_fn(|i| f(a[i])))
}
fn f32x4_compara(a: i64, b: i64, f: impl Fn(f32, f32) -> bool) -> i64 {
    let (a, b) = (f32x4_de(a), f32x4_de(b));
    novo_i32x4(std::array::from_fn(|i| if f(a[i], b[i]) { -1 } else { 0 }))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_add(a: i64, b: i64) -> i64 {
    f32x4_binaria(a, b, |x, y| x + y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_sub(a: i64, b: i64) -> i64 {
    f32x4_binaria(a, b, |x, y| x - y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_mul(a: i64, b: i64) -> i64 {
    f32x4_binaria(a, b, |x, y| x * y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_div(a: i64, b: i64) -> i64 {
    f32x4_binaria(a, b, |x, y| x / y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_min(a: i64, b: i64) -> i64 {
    f32x4_binaria(a, b, simd_min)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_max(a: i64, b: i64) -> i64 {
    f32x4_binaria(a, b, simd_max)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_negate(a: i64) -> i64 {
    f32x4_unaria(a, |x| -x)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_abs(a: i64) -> i64 {
    f32x4_unaria(a, f32::abs)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_sqrt(a: i64) -> i64 {
    f32x4_unaria(a, f32::sqrt)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_reciprocal(a: i64) -> i64 {
    f32x4_unaria(a, |x| 1.0 / x)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_reciprocalSqrt(a: i64) -> i64 {
    f32x4_unaria(a, |x| (1.0 / x).sqrt())
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_scale(a: i64, s: f64) -> i64 {
    let s = s as f32;
    f32x4_unaria(a, |x| x * s)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_clamp(a: i64, lo: i64, hi: i64) -> i64 {
    let (a, lo, hi) = (f32x4_de(a), f32x4_de(lo), f32x4_de(hi));
    novo_f32x4(std::array::from_fn(|i| clamp_pista(a[i], lo[i], hi[i], f32::is_sign_negative)))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_cmpequal(a: i64, b: i64) -> i64 {
    f32x4_compara(a, b, |x, y| x == y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_cmpnequal(a: i64, b: i64) -> i64 {
    f32x4_compara(a, b, |x, y| x != y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_cmplt(a: i64, b: i64) -> i64 {
    f32x4_compara(a, b, |x, y| x < y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_cmplte(a: i64, b: i64) -> i64 {
    f32x4_compara(a, b, |x, y| x <= y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_cmpgt(a: i64, b: i64) -> i64 {
    f32x4_compara(a, b, |x, y| x > y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_cmpgte(a: i64, b: i64) -> i64 {
    f32x4_compara(a, b, |x, y| x >= y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_getX(a: i64) -> f64 {
    f64::from(f32x4_de(a)[0])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_getY(a: i64) -> f64 {
    f64::from(f32x4_de(a)[1])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_getZ(a: i64) -> f64 {
    f64::from(f32x4_de(a)[2])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_getW(a: i64) -> f64 {
    f64::from(f32x4_de(a)[3])
}
fn f32x4_com(a: i64, i: usize, v: f64) -> i64 {
    let mut a = f32x4_de(a);
    a[i] = v as f32;
    novo_f32x4(a)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_setX(a: i64, v: f64) -> i64 {
    f32x4_com(a, 0, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_setY(a: i64, v: f64) -> i64 {
    f32x4_com(a, 1, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_setZ(a: i64, v: f64) -> i64 {
    f32x4_com(a, 2, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_setW(a: i64, v: f64) -> i64 {
    f32x4_com(a, 3, v)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_getSignMask(a: i64) -> i64 {
    f32x4_de(a).iter().enumerate().map(|(i, x)| i64::from(x.to_bits() >> 31) << i).sum()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_shuffle(a: i64, m: i64) -> i64 {
    let Some(m) = mascara_valida(m) else { return 0 };
    let a = f32x4_de(a);
    novo_f32x4(embaralhar(a, a, m))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float32x4_shuffleMix(a: i64, b: i64, m: i64) -> i64 {
    let Some(m) = mascara_valida(m) else { return 0 };
    novo_f32x4(embaralhar(f32x4_de(a), f32x4_de(b), m))
}

// --- Int32x4 -----------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_fromInts(x: i64, y: i64, z: i64, w: i64) -> i64 {
    novo_i32x4([x as i32, y as i32, z as i32, w as i32])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_fromBools(x: u8, y: u8, z: u8, w: u8) -> i64 {
    let f = |b: u8| if b != 0 { -1 } else { 0 };
    novo_i32x4([f(x), f(y), f(z), f(w)])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_fromFloat32x4Bits(v: i64) -> i64 {
    let b = simd_bytes(v);
    simd_novo(CID_INT32X4, TIPO_INT32X4, b)
}
fn i32x4_binaria(a: i64, b: i64, f: impl Fn(i32, i32) -> i32) -> i64 {
    let (a, b) = (i32x4_de(a), i32x4_de(b));
    novo_i32x4(std::array::from_fn(|i| f(a[i], b[i])))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_or(a: i64, b: i64) -> i64 {
    i32x4_binaria(a, b, |x, y| x | y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_and(a: i64, b: i64) -> i64 {
    i32x4_binaria(a, b, |x, y| x & y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_xor(a: i64, b: i64) -> i64 {
    i32x4_binaria(a, b, |x, y| x ^ y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_add(a: i64, b: i64) -> i64 {
    i32x4_binaria(a, b, i32::wrapping_add)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_sub(a: i64, b: i64) -> i64 {
    i32x4_binaria(a, b, i32::wrapping_sub)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getX(a: i64) -> i64 {
    i64::from(i32x4_de(a)[0])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getY(a: i64) -> i64 {
    i64::from(i32x4_de(a)[1])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getZ(a: i64) -> i64 {
    i64::from(i32x4_de(a)[2])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getW(a: i64) -> i64 {
    i64::from(i32x4_de(a)[3])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getFlagX(a: i64) -> u8 {
    u8::from(i32x4_de(a)[0] != 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getFlagY(a: i64) -> u8 {
    u8::from(i32x4_de(a)[1] != 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getFlagZ(a: i64) -> u8 {
    u8::from(i32x4_de(a)[2] != 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getFlagW(a: i64) -> u8 {
    u8::from(i32x4_de(a)[3] != 0)
}
fn i32x4_com(a: i64, i: usize, v: i32) -> i64 {
    let mut a = i32x4_de(a);
    a[i] = v;
    novo_i32x4(a)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setX(a: i64, v: i64) -> i64 {
    i32x4_com(a, 0, v as i32)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setY(a: i64, v: i64) -> i64 {
    i32x4_com(a, 1, v as i32)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setZ(a: i64, v: i64) -> i64 {
    i32x4_com(a, 2, v as i32)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setW(a: i64, v: i64) -> i64 {
    i32x4_com(a, 3, v as i32)
}
fn bandeira(b: u8) -> i32 {
    if b != 0 { -1 } else { 0 }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setFlagX(a: i64, b: u8) -> i64 {
    i32x4_com(a, 0, bandeira(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setFlagY(a: i64, b: u8) -> i64 {
    i32x4_com(a, 1, bandeira(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setFlagZ(a: i64, b: u8) -> i64 {
    i32x4_com(a, 2, bandeira(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_setFlagW(a: i64, b: u8) -> i64 {
    i32x4_com(a, 3, bandeira(b))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_getSignMask(a: i64) -> i64 {
    i32x4_de(a).iter().enumerate().map(|(i, x)| i64::from((*x as u32) >> 31) << i).sum()
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_shuffle(a: i64, m: i64) -> i64 {
    let Some(m) = mascara_valida(m) else { return 0 };
    let a = i32x4_de(a);
    novo_i32x4(embaralhar(a, a, m))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_shuffleMix(a: i64, b: i64, m: i64) -> i64 {
    let Some(m) = mascara_valida(m) else { return 0 };
    novo_i32x4(embaralhar(i32x4_de(a), i32x4_de(b), m))
}
/// `select(t, f)`: bit a bit, os de `t` onde a máscara tem 1, os de `f`
/// onde tem 0.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Int32x4_select(m: i64, t: i64, f: i64) -> i64 {
    let (m, t, f) = (i32x4_de(m), f32x4_de(t), f32x4_de(f));
    novo_f32x4(std::array::from_fn(|i| {
        let bits = (m[i] as u32 & t[i].to_bits()) | (!(m[i] as u32) & f[i].to_bits());
        f32::from_bits(bits)
    }))
}

// --- Float64x2 ---------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_fromDoubles(x: f64, y: f64) -> i64 {
    novo_f64x2([x, y])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_splat(v: f64) -> i64 {
    novo_f64x2([v, v])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_zero() -> i64 {
    novo_f64x2([0.0, 0.0])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_fromFloat32x4(v: i64) -> i64 {
    let a = f32x4_de(v);
    novo_f64x2([f64::from(a[0]), f64::from(a[1])])
}
fn f64x2_binaria(a: i64, b: i64, f: impl Fn(f64, f64) -> f64) -> i64 {
    let (a, b) = (f64x2_de(a), f64x2_de(b));
    novo_f64x2([f(a[0], b[0]), f(a[1], b[1])])
}
fn f64x2_unaria(a: i64, f: impl Fn(f64) -> f64) -> i64 {
    let a = f64x2_de(a);
    novo_f64x2([f(a[0]), f(a[1])])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_add(a: i64, b: i64) -> i64 {
    f64x2_binaria(a, b, |x, y| x + y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_sub(a: i64, b: i64) -> i64 {
    f64x2_binaria(a, b, |x, y| x - y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_mul(a: i64, b: i64) -> i64 {
    f64x2_binaria(a, b, |x, y| x * y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_div(a: i64, b: i64) -> i64 {
    f64x2_binaria(a, b, |x, y| x / y)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_min(a: i64, b: i64) -> i64 {
    f64x2_binaria(a, b, simd_min)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_max(a: i64, b: i64) -> i64 {
    f64x2_binaria(a, b, simd_max)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_negate(a: i64) -> i64 {
    f64x2_unaria(a, |x| -x)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_abs(a: i64) -> i64 {
    f64x2_unaria(a, f64::abs)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_sqrt(a: i64) -> i64 {
    f64x2_unaria(a, f64::sqrt)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_scale(a: i64, s: f64) -> i64 {
    f64x2_unaria(a, |x| x * s)
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_clamp(a: i64, lo: i64, hi: i64) -> i64 {
    let (a, lo, hi) = (f64x2_de(a), f64x2_de(lo), f64x2_de(hi));
    novo_f64x2(std::array::from_fn(|i| clamp_pista(a[i], lo[i], hi[i], f64::is_sign_negative)))
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_getX(a: i64) -> f64 {
    f64x2_de(a)[0]
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_getY(a: i64) -> f64 {
    f64x2_de(a)[1]
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_setX(a: i64, v: f64) -> i64 {
    novo_f64x2([v, f64x2_de(a)[1]])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_setY(a: i64, v: f64) -> i64 {
    novo_f64x2([f64x2_de(a)[0], v])
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Float64x2_getSignMask(a: i64) -> i64 {
    let a = f64x2_de(a);
    i64::from((a[0].to_bits() >> 63) as u8) | (i64::from((a[1].to_bits() >> 63) as u8) << 1)
}

// --- Nas listas tipadas -------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_GetFloat32x4(this: i64, off: i64) -> i64 {
    match ler::<16>(this, off) {
        Some(b) => simd_novo(CID_FLOAT32X4, TIPO_FLOAT32X4, b),
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_SetFloat32x4(this: i64, off: i64, v: i64) {
    gravar::<16>(this, off, simd_bytes(v));
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_GetInt32x4(this: i64, off: i64) -> i64 {
    match ler::<16>(this, off) {
        Some(b) => simd_novo(CID_INT32X4, TIPO_INT32X4, b),
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_SetInt32x4(this: i64, off: i64, v: i64) {
    gravar::<16>(this, off, simd_bytes(v));
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_GetFloat64x2(this: i64, off: i64) -> i64 {
    match ler::<16>(this, off) {
        Some(b) => simd_novo(CID_FLOAT64X2, TIPO_FLOAT64X2, b),
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_SetFloat64x2(this: i64, off: i64, v: i64) {
    gravar::<16>(this, off, simd_bytes(v));
}

/// O elemento `i` de uma `Float32x4List`, `Int32x4List` ou `Float64x2List`
/// (o `[]` reconhecido da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_indexar_simd(this: i64, i: i64) -> i64 {
    let Some((t, p)) = elemento(this, i) else { return 0 };
    let mut b = [0u8; 16];
    // SAFETY: `p` é o elemento `i < len` (16 bytes) da lista viva.
    unsafe { std::ptr::copy_nonoverlapping(p, b.as_mut_ptr(), 16) };
    match t.tipo {
        TIPO_INT32X4 => simd_novo(CID_INT32X4, TIPO_INT32X4, b),
        TIPO_FLOAT64X2 => simd_novo(CID_FLOAT64X2, TIPO_FLOAT64X2, b),
        _ => simd_novo(CID_FLOAT32X4, TIPO_FLOAT32X4, b),
    }
}

/// Um `double` como o `%f` do C (o `ToCString` dos SIMD na VM).
fn percent_f(x: f64) -> String {
    if x.is_nan() {
        "nan".to_string()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".to_string() } else { "-inf".to_string() }
    } else {
        format!("{x:.6}")
    }
}

/// O texto de um valor SIMD (o `Object.toString` da VM): `[x, y, z, w]` com
/// `%f` nos `Float32x4`/`Float64x2` e `%08x` nos `Int32x4`; `None` para os
/// outros valores.
fn texto_simd(h: i64) -> Option<String> {
    if !crate::layout::e_objeto(h) {
        return None;
    }
    let cid = HEAP.with(|x| {
        let x = x.borrow();
        x.e_objeto_vivo(h).then(|| x.cabecalho(h).class_id)
    })?;
    let e = |c: i32| c == cid;
    if e(CID_FLOAT32X4) {
        let v: Vec<String> = f32x4_de(h).iter().map(|x| percent_f(f64::from(*x))).collect();
        Some(format!("[{}]", v.join(", ")))
    } else if e(CID_INT32X4) {
        let v: Vec<String> = i32x4_de(h).iter().map(|x| format!("{:08x}", *x as u32)).collect();
        Some(format!("[{}]", v.join(", ")))
    } else if e(CID_FLOAT64X2) {
        let v: Vec<String> = f64x2_de(h).iter().map(|x| percent_f(*x)).collect();
        Some(format!("[{}]", v.join(", ")))
    } else {
        None
    }
}
