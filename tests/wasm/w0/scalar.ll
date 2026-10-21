; W0 scalar proof (docs/PLANO-WASM-MVP.md, phase W0): the kinds of operations
; DartForge's emitter produces, without the runtime, GC or Dart libraries.
; Not Dart -> Wasm: the LLVM side of the ABI between generated code, the Rust
; support fragment (rt.rs) and the host (dartforge_host_v1).
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

; The host ABI v1: write(channel, offset, length) -> status.
declare i32 @dartforge_host_write(i32, i32, i32) #0

; The Rust support fragment.
declare i32 @dfrt_format_i64(i64, ptr)
declare i64 @dfrt_mul_hi(i64, i64)
declare i64 @dfrt_checked_sum(ptr, i32)
declare i64 @dfrt_alloc_probe(i32)

@line = internal global [64 x i8] zeroinitializer, align 8
@words = internal global [4 x i64] zeroinitializer, align 8

define i64 @fib(i64 %n) {
entry:
  %small = icmp slt i64 %n, 2
  br i1 %small, label %base, label %rec
base:
  ret i64 %n
rec:
  %a = sub i64 %n, 1
  %b = sub i64 %n, 2
  %fa = call i64 @fib(i64 %a)
  %fb = call i64 @fib(i64 %b)
  %r = add i64 %fa, %fb
  ret i64 %r
}

define internal i64 @twice(i64 %x) {
  %r = shl i64 %x, 1
  ret i64 %r
}

define internal i64 @negate(i64 %x) {
  %r = sub i64 0, %x
  ret i64 %r
}

; Prints `label=value\n` through the host; the number is formatted by Rust.
define internal void @emit(i8 %label, i64 %value) {
  %p = getelementptr inbounds [64 x i8], ptr @line, i32 0, i32 0
  store i8 %label, ptr %p, align 1
  %q = getelementptr inbounds i8, ptr %p, i32 1
  store i8 61, ptr %q, align 1
  %digits = getelementptr inbounds i8, ptr %p, i32 2
  %n = call i32 @dfrt_format_i64(i64 %value, ptr %digits)
  %end = add i32 %n, 2
  %nl = getelementptr inbounds i8, ptr %p, i32 %end
  store i8 10, ptr %nl, align 1
  %len = add i32 %end, 1
  %off = ptrtoint ptr %p to i32
  %s = call i32 @dartforge_host_write(i32 1, i32 %off, i32 %len)
  ret void
}

define i32 @dartforge_abi_version() {
  ret i32 1
}

define i32 @dartforge_run() {
entry:
  ; Recursive calls and i64 adds.
  %f = call i64 @fib(i64 30)
  call void @emit(i8 102, i64 %f)
  ; Sign extension of a byte: shifts under the MVP CPU, `i64.extend8_s`
  ; (post-MVP) under LLVM's generic CPU -- the script's negative control.
  %fb = add i64 %f, 160
  %byte = trunc i64 %fb to i8
  %sx = sext i8 %byte to i64
  call void @emit(i8 101, i64 %sx)
  ; Modular i64 multiplication (wraps like Dart's int).
  %big = mul i64 9223372036854775807, 3
  call void @emit(i8 109, i64 %big)
  ; Truncating division and remainder with negative operands.
  %d = sdiv i64 -7, 2
  call void @emit(i8 100, i64 %d)
  %r = srem i64 -7, 2
  call void @emit(i8 114, i64 %r)
  ; Arithmetic and logical shifts of a negative value.
  %sa = ashr i64 -1024, 3
  call void @emit(i8 97, i64 %sa)
  %sl = lshr i64 -1, 60
  call void @emit(i8 108, i64 %sl)
  ; double -> int of an in-range value (MVP `i64.trunc_f64_s`, no saturating form).
  %x = fdiv double 1.0e10, 3.0
  %xi = fptosi double %x to i64
  call void @emit(i8 120, i64 %xi)
  ; Indirect calls through the function table.
  %pick = icmp sgt i64 %f, 0
  %fp = select i1 %pick, ptr @twice, ptr @negate
  %t = call i64 %fp(i64 21)
  call void @emit(i8 116, i64 %t)
  %gp = select i1 %pick, ptr @negate, ptr @twice
  %u = call i64 %gp(i64 21)
  call void @emit(i8 117, i64 %u)
  ; The high half of a 128-bit product, through Rust (`__multi3` builtin).
  %h = call i64 @dfrt_mul_hi(i64 9223372036854775807, i64 9223372036854775807)
  call void @emit(i8 104, i64 %h)
  ; i64 values passed through a buffer in linear memory.
  %w0 = getelementptr inbounds [4 x i64], ptr @words, i32 0, i32 0
  store i64 1099511627776, ptr %w0, align 8
  %w1 = getelementptr inbounds [4 x i64], ptr @words, i32 0, i32 1
  store i64 -5, ptr %w1, align 8
  %w2 = getelementptr inbounds [4 x i64], ptr @words, i32 0, i32 2
  store i64 9223372036854775807, ptr %w2, align 8
  %sum = call i64 @dfrt_checked_sum(ptr %w0, i32 3)
  call void @emit(i8 115, i64 %sum)
  %v = call i64 @dfrt_alloc_probe(i32 1000)
  call void @emit(i8 118, i64 %v)
  ret i32 0
}

attributes #0 = { "wasm-import-module"="dartforge_host_v1" "wasm-import-name"="write" }
