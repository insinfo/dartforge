//! W0 Rust support fragment for `wasm32-mvp` (docs/PLANO-WASM-MVP.md, W0 item 5):
//! a `no_std` staticlib built for `wasm32v1-none`, linked with the generated
//! code into one module. It checks that `core`, the compiler builtins (128-bit
//! multiplication) and a controlled panic stay inside the MVP profile and import
//! nothing but the host ABI. `alloc` (growing `Vec`, `format!`) runs on a bump
//! allocator over a static arena, which checks the prebuilt `alloc` library.
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};

/// A bump allocator over a static arena; W0 only needs `alloc` to link and run
/// inside the profile, not to free.
struct Bump;

const ARENA_SIZE: usize = 1 << 16;
static mut ARENA: [u8; ARENA_SIZE] = [0; ARENA_SIZE];
static mut NEXT: usize = 0;

unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: wasm32-mvp has a single thread; the arena is never borrowed.
        unsafe {
            let base = core::ptr::addr_of_mut!(ARENA) as usize;
            let start = (base + NEXT + layout.align() - 1) & !(layout.align() - 1);
            let end = start + layout.size();
            if end > base + ARENA_SIZE {
                return core::ptr::null_mut();
            }
            NEXT = end - base;
            start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static ALLOCATOR: Bump = Bump;

#[link(wasm_import_module = "dartforge_host_v1")]
unsafe extern "C" {
    #[link_name = "write"]
    fn host_write(channel: i32, offset: i32, length: i32) -> i32;
}

/// Writes the decimal form of `value` at `out` (at most 20 bytes) and returns
/// its length.
///
/// # Safety
/// `out` points to 20 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dfrt_format_i64(value: i64, out: *mut u8) -> i32 {
    let mut digits = [0u8; 20];
    let mut n = 0;
    let negative = value < 0;
    // The magnitude as u64: `i64::MIN` has no positive i64.
    let mut v = value.unsigned_abs();
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let mut len = 0;
    // SAFETY: the caller's contract: 20 writable bytes (19 digits and a sign).
    unsafe {
        if negative {
            *out = b'-';
            len = 1;
        }
        for i in (0..n).rev() {
            *out.add(len) = digits[i];
            len += 1;
        }
    }
    len as i32
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
    let mut s: i64 = 0;
    for i in 0..n.max(0) as usize {
        // SAFETY: the caller's contract.
        s = s.wrapping_add(unsafe { *words.add(i) });
    }
    s
}

/// `alloc` inside the profile: the sum of `i * i` for `i < n` in a growing
/// `Vec`, times 100, plus the length of `format!("{n}-{sum}")`.
#[unsafe(no_mangle)]
pub extern "C" fn dfrt_alloc_probe(n: i32) -> i64 {
    let mut squares = Vec::new();
    for i in 0..i64::from(n) {
        squares.push(i * i);
    }
    let sum: i64 = squares.iter().sum();
    let text = alloc::format!("{n}-{sum}");
    sum * 100 + text.len() as i64
}

/// A runtime panic is not a Dart exception: it reports on stderr and traps.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    const MSG: &[u8] = b"dartforge runtime panic\n";
    // SAFETY: a host import; the message is a static in linear memory.
    unsafe {
        host_write(2, MSG.as_ptr() as i32, MSG.len() as i32);
    }
    core::arch::wasm32::unreachable()
}
