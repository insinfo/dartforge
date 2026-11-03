// The shortest decimal digits of a `double` that read back as the same
// `double`: the digits of the VM's `double.toString` (double-conversion's
// `ToShortest`: the shortest correctly rounded digits, the closest to the exact
// value, a tie rounding to the even digit).
//
// The algorithm is Ryū (Ulf Adams, "Ryū: fast float-to-string conversion",
// PLDI 2018; https://github.com/ulfjack/ryu, Apache-2.0 or BSL-1.0, see
// THIRD_PARTY_NOTICES.md), with the small tables: the 128-bit multipliers
// `5^i` and `2^k / 5^i` are computed from one in every 26 and a 2-bit
// correction per index. This replaces Rust's `{:e}` (Grisu with the Dragon
// fallback), which also linked the exact-precision paths of `core::fmt`
// (25 KB of a WebAssembly module).

/// `m · 10^e`, the shortest decimal form of a positive finite `double`.
struct ShortestDecimal {
    digits: u64,
    exponent: i32,
}

/// `5^i` for `i < 26`.
const DOUBLE_POW5: [u64; 26] = {
    let mut t = [1u64; 26];
    let mut i = 1;
    while i < 26 {
        t[i] = t[i - 1] * 5;
        i += 1;
    }
    t
};

/// `2^k / 5^(26·n)` (rounded up), as `(low, high)` 64-bit halves.
const DOUBLE_POW5_INV_SPLIT2: [(u64, u64); 15] = [
    (1, 2305843009213693952),
    (5955668970331000884, 1784059615882449851),
    (8982663654677661702, 1380349269358112757),
    (7286864317269821294, 2135987035920910082),
    (7005857020398200553, 1652639921975621497),
    (17965325103354776697, 1278668206209430417),
    (8928596168509315048, 1978643211784836272),
    (10075671573058298858, 1530901034580419511),
    (597001226353042382, 1184477304306571148),
    (1527430471115325346, 1832889850782397517),
    (12533209867169019542, 1418129833677084982),
    (5577825024675947042, 2194449627517475473),
    (11006974540203867551, 1697873161311732311),
    (10313493231639821582, 1313665730009899186),
    (12701016819766672773, 2032799256770390445),
];

/// The 2-bit corrections of the inverse multipliers, 16 per word.
const POW5_INV_OFFSETS: [u32; 19] = [
    0x54544554, 0x04055545, 0x10041000, 0x00400414, 0x40010000, 0x41155555, 0x00000454,
    0x00010044, 0x40000000, 0x44000041, 0x50454450, 0x55550054, 0x51655554, 0x40004000,
    0x01000001, 0x00010500, 0x51515411, 0x05555554, 0x00000000,
];

/// `5^(26·n)`, the top 125 bits, as `(low, high)` 64-bit halves.
const DOUBLE_POW5_SPLIT2: [(u64, u64); 13] = [
    (0, 1152921504606846976),
    (0, 1490116119384765625),
    (1032610780636961552, 1925929944387235853),
    (7910200175544436838, 1244603055572228341),
    (16941905809032713930, 1608611746708759036),
    (13024893955298202172, 2079081953128979843),
    (6607496772837067824, 1343575221513417750),
    (17332926989895652603, 1736530273035216783),
    (13037379183483547984, 2244412773384604712),
    (1605989338741628675, 1450417759929778918),
    (9630225068416591280, 1874621017369538693),
    (665883850346957067, 1211445438634777304),
    (14931890668723713708, 1565756531257009982),
];

/// The 2-bit corrections of the multipliers, 16 per word.
const POW5_OFFSETS: [u32; 21] = [
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x40000000, 0x59695995, 0x55545555,
    0x56555515, 0x41150504, 0x40555410, 0x44555145, 0x44504540, 0x45555550, 0x40004000,
    0x96440440, 0x55565565, 0x54454045, 0x40154151, 0x55559155, 0x51405555, 0x00000105,
];

const POW5_INV_BITCOUNT: i32 = 125;
const POW5_BITCOUNT: i32 = 125;

/// `ceil(log2(5^e))` (1 for `e == 0`), `0 <= e <= 3528`.
fn pow5bits(e: i32) -> i32 {
    (((e as u32) * 1217359) >> 19) as i32 + 1
}

/// `floor(log10(2^e))`, `0 <= e <= 1650`.
fn log10_pow2(e: i32) -> u32 {
    ((e as u32) * 78913) >> 18
}

/// `floor(log10(5^e))`, `0 <= e <= 2620`.
fn log10_pow5(e: i32) -> u32 {
    ((e as u32) * 732923) >> 20
}

/// The multiplier of `5^i` (`i <= 325`).
fn pow5_multiplier(i: u32) -> (u64, u64) {
    let base = i / 26;
    let base2 = base * 26;
    let offset = i - base2;
    let mul = DOUBLE_POW5_SPLIT2[base as usize];
    if offset == 0 {
        return mul;
    }
    let m = u128::from(DOUBLE_POW5[offset as usize]);
    let b0 = m * u128::from(mul.0);
    let b2 = m * u128::from(mul.1);
    let delta = (pow5bits(i as i32) - pow5bits(base2 as i32)) as u32;
    let correction = (POW5_OFFSETS[(i / 16) as usize] >> ((i % 16) << 1)) & 3;
    let sum = (b0 >> delta) + (b2 << (64 - delta)) + u128::from(correction);
    (sum as u64, (sum >> 64) as u64)
}

/// The multiplier of `5^-i` (`i <= 341`).
fn inv_pow5_multiplier(i: u32) -> (u64, u64) {
    let base = i.div_ceil(26);
    let base2 = base * 26;
    let offset = base2 - i;
    let mul = DOUBLE_POW5_INV_SPLIT2[base as usize];
    if offset == 0 {
        return mul;
    }
    let m = u128::from(DOUBLE_POW5[offset as usize]);
    let b0 = m * u128::from(mul.0 - 1);
    let b2 = m * u128::from(mul.1);
    let delta = (pow5bits(base2 as i32) - pow5bits(i as i32)) as u32;
    let correction = (POW5_INV_OFFSETS[(i / 16) as usize] >> ((i % 16) << 1)) & 3;
    let sum = (b0 >> delta) + (b2 << (64 - delta)) + 1 + u128::from(correction);
    (sum as u64, (sum >> 64) as u64)
}

/// `(m · mul) >> j`, `j >= 64`.
fn mul_shift(m: u64, mul: (u64, u64), j: u32) -> u64 {
    let b0 = u128::from(m) * u128::from(mul.0);
    let b2 = u128::from(m) * u128::from(mul.1);
    (((b0 >> 64) + b2) >> (j - 64)) as u64
}

/// The number of factors 5 of `v > 0`.
fn pow5_factor(mut v: u64) -> u32 {
    // 5 · M_INV_5 = 1 (mod 2^64); a multiple of 5 times it stays below 2^64 / 5.
    const M_INV_5: u64 = 14757395258967641293;
    const N_DIV_5: u64 = 3689348814741910323;
    let mut count = 0;
    loop {
        v = v.wrapping_mul(M_INV_5);
        if v > N_DIV_5 {
            return count;
        }
        count += 1;
    }
}

fn multiple_of_power_of_5(v: u64, p: u32) -> bool {
    pow5_factor(v) >= p
}

fn multiple_of_power_of_2(v: u64, p: u32) -> bool {
    v & ((1u64 << p) - 1) == 0
}

/// The shortest decimal form of the positive finite `double` with the IEEE
/// fields `mantissa` (52 bits) and `exponent` (11 bits, not all ones, not both
/// zero).
fn shortest_decimal(mantissa: u64, exponent: u32) -> ShortestDecimal {
    // Two more bits for the bounds of the interval.
    let (e2, m2) = if exponent == 0 {
        (1 - 1023 - 52 - 2, mantissa)
    } else {
        (exponent as i32 - 1023 - 52 - 2, (1u64 << 52) | mantissa)
    };
    // An even mantissa rounds to itself from the bounds, so they are in the
    // interval (round half to even when the text is read back).
    let accept_bounds = m2 & 1 == 0;
    let mv = 4 * m2;
    // The lower bound is closer when the mantissa is a power of two.
    let mm_shift = u64::from(mantissa != 0 || exponent <= 1);
    let (mut vr, mut vp, mut vm, e10);
    let mut vm_trailing_zeros = false;
    let mut vr_trailing_zeros = false;
    if e2 >= 0 {
        let q = log10_pow2(e2) - u32::from(e2 > 3);
        e10 = q as i32;
        let k = POW5_INV_BITCOUNT + pow5bits(q as i32) - 1;
        let i = (-e2 + q as i32 + k) as u32;
        let mul = inv_pow5_multiplier(q);
        vr = mul_shift(mv, mul, i);
        vp = mul_shift(mv + 2, mul, i);
        vm = mul_shift(mv - 1 - mm_shift, mul, i);
        if q <= 21 {
            // Only one of mp, mv and mm can be a multiple of 5.
            if mv % 5 == 0 {
                vr_trailing_zeros = multiple_of_power_of_5(mv, q);
            } else if accept_bounds {
                vm_trailing_zeros = multiple_of_power_of_5(mv - 1 - mm_shift, q);
            } else {
                vp -= u64::from(multiple_of_power_of_5(mv + 2, q));
            }
        }
    } else {
        let q = log10_pow5(-e2) - u32::from(-e2 > 1);
        e10 = q as i32 + e2;
        let i = -e2 - q as i32;
        let k = pow5bits(i) - POW5_BITCOUNT;
        let j = (q as i32 - k) as u32;
        let mul = pow5_multiplier(i as u32);
        vr = mul_shift(mv, mul, j);
        vp = mul_shift(mv + 2, mul, j);
        vm = mul_shift(mv - 1 - mm_shift, mul, j);
        if q <= 1 {
            // mv = 4 · m2 has at least two trailing zero bits.
            vr_trailing_zeros = true;
            if accept_bounds {
                // mm = mv - 1 - mm_shift has one iff mm_shift == 1.
                vm_trailing_zeros = mm_shift == 1;
            } else {
                // mp = mv + 2 always has one.
                vp -= 1;
            }
        } else if q < 63 {
            // The full product has at least q trailing zeros iff mv has q
            // trailing zero bits (its factors of 5 outnumber q, -e2 >= q).
            vr_trailing_zeros = multiple_of_power_of_2(mv, q);
        }
    }
    // Remove the digits that keep the result inside the interval.
    let mut removed = 0i32;
    let digits = if vm_trailing_zeros || vr_trailing_zeros {
        // The general case (rare: ~0.7%).
        let mut last_removed = 0u64;
        while vp / 10 > vm / 10 {
            vm_trailing_zeros &= vm % 10 == 0;
            vr_trailing_zeros &= last_removed == 0;
            last_removed = vr % 10;
            vr /= 10;
            vp /= 10;
            vm /= 10;
            removed += 1;
        }
        if vm_trailing_zeros {
            while vm % 10 == 0 {
                vr_trailing_zeros &= last_removed == 0;
                last_removed = vr % 10;
                vr /= 10;
                vp /= 10;
                vm /= 10;
                removed += 1;
            }
        }
        if vr_trailing_zeros && last_removed == 5 && vr % 2 == 0 {
            // Exactly halfway (…50…0): round to even.
            last_removed = 4;
        }
        vr + u64::from((vr == vm && (!accept_bounds || !vm_trailing_zeros)) || last_removed >= 5)
    } else {
        // The common case: no exact zeros below the digits.
        let mut round_up = false;
        if vp / 100 > vm / 100 {
            round_up = vr % 100 >= 50;
            vr /= 100;
            vp /= 100;
            vm /= 100;
            removed += 2;
        }
        while vp / 10 > vm / 10 {
            round_up = vr % 10 >= 5;
            vr /= 10;
            vp /= 10;
            vm /= 10;
            removed += 1;
        }
        vr + u64::from(vr == vm || round_up)
    };
    ShortestDecimal {
        digits,
        exponent: e10 + removed,
    }
}

/// The shortest digits of the positive finite nonzero `d` in ASCII into `out`, and the
/// decimal exponent of the first one (`d ≈ 0.d₁d₂… · 10^(exponent + 1)`, that
/// is, `d₁.d₂… · 10^exponent`). Returns the number of digits (1 to 17).
fn shortest_digits(d: f64, out: &mut [u8; 17]) -> (usize, i32) {
    let bits = d.to_bits();
    let mantissa = bits & ((1u64 << 52) - 1);
    let exponent = ((bits >> 52) & 0x7ff) as u32;
    let v = shortest_decimal(mantissa, exponent);
    let mut n = 1usize;
    let mut x = v.digits / 10;
    while x != 0 {
        n += 1;
        x /= 10;
    }
    let mut x = v.digits;
    for c in out[..n].iter_mut().rev() {
        *c = b'0' + (x % 10) as u8;
        x /= 10;
    }
    (n, v.exponent + n as i32 - 1)
}
