//! W0 Rust support fragment for `wasm32-linear` (docs/PLANO-WASM-MVP.md, W0 item
//! 5): a staticlib with `std`, built for Rust's default `wasm32-unknown-unknown`
//! and linked with the generated code into one module. It checks that `std`
//! (its dlmalloc allocator over `memory.grow`, `Vec`, `format!`,
//! `thread_local!`), the compiler builtins (128-bit multiplication) and a
//! controlled panic stay inside the profile and import nothing but the host ABI.

use std::cell::Cell;

#[link(wasm_import_module = "dartforge_host_v1")]
unsafe extern "C" {
    #[link_name = "write"]
    fn host_write(channel: i32, offset: i32, length: i32) -> i32;
}

thread_local! {
    /// Single-threaded on this target: a plain per-instance cell.
    static PROBES: Cell<i64> = const { Cell::new(0) };
}

/// Writes the decimal form of `value` at `out` (at most 20 bytes) and returns
/// its length.
///
/// # Safety
/// `out` points to 20 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dfrt_format_i64(value: i64, out: *mut u8) -> i32 {
    let text = value.to_string();
    // SAFETY: the caller's contract: 20 writable bytes (19 digits and a sign).
    unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), out, text.len()) };
    text.len() as i32
}

/// The high 64 bits of the signed 128-bit product.
#[unsafe(no_mangle)]
pub extern "C" fn dfrt_mul_hi(a: i64, b: i64) -> i64 {
    ((i128::from(a) * i128::from(b)) >> 64) as i64
}

/// The wrapping sum of `n` i64 words at `words`.
///
/// # Safety
/// `words` points to `n` readable, aligned i64 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dfrt_checked_sum(words: *const i64, n: i32) -> i64 {
    // SAFETY: the caller's contract.
    let words = unsafe { std::slice::from_raw_parts(words, n.max(0) as usize) };
    words.iter().fold(0i64, |s, w| s.wrapping_add(*w))
}

/// `std` inside the profile: the sum of `i * i` for `i < n` in a `Vec` that
/// grows past the initial memory (1 MiB of `i64`s forces `memory.grow`), times
/// 100, plus the length of `format!("{n}-{sum}")`, plus the probe count kept in
/// a `thread_local!` (1 on the first call).
#[unsafe(no_mangle)]
pub extern "C" fn dfrt_alloc_probe(n: i32) -> i64 {
    let mut squares = Vec::new();
    for i in 0..i64::from(n) {
        squares.push(i * i);
    }
    let mut ballast: Vec<i64> = Vec::with_capacity(1 << 17);
    ballast.resize(1 << 17, 1);
    let sum: i64 = squares.iter().sum::<i64>() + ballast.iter().sum::<i64>() - (1 << 17);
    let text = format!("{n}-{sum}");
    let count = PROBES.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    sum * 100 + text.len() as i64 + count
}

/// Installs the panic hook: a runtime panic is not a Dart exception, it
/// reports on stderr through the host ABI and then traps (`panic=abort`).
#[unsafe(no_mangle)]
pub extern "C" fn dfrt_init() {
    std::panic::set_hook(Box::new(|info| {
        let text = format!("dartforge runtime panic: {info}\n");
        // SAFETY: a host import; the text lives in linear memory for the call.
        unsafe { host_write(2, text.as_ptr() as i32, text.len() as i32) };
    }));
}
