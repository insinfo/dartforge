; W2 proof (docs/PLANO-WASM-MVP.md, phase W2): the real DartForge runtime,
; compiled for wasm32, driven by hand-written code that follows the generated
; code's contracts — the shadow-stack frame linked at `Context + 8`, live values
; kept in its slots and reloaded after every allocation (the young collection
; moves objects). Not Dart -> Wasm yet: the SDK and the emitter come in W3.
target triple = "wasm32-unknown-unknown"

declare ptr @dartforge_context()
declare i64 @dartforge_string_new(ptr, i64)
declare i64 @dartforge_string_concat(i64, i64)
declare void @dartforge_gc_collect()
declare void @dartforge_print_string(i64)
declare void @dartforge_print_i64(i64)
declare void @dartforge_print_bool(i8)
declare i64 @dartforge_native_DateTime_currentTimeMicros()
declare i64 @dartforge_native_Stopwatch_now()
declare i64 @dartforge_native_Random_initialSeed()
declare i64 @dartforge_native_DateTime_timeZoneOffsetInSeconds(i64)
declare void @dartforge_gc_stats(ptr)
declare ptr @llvm.stacksave.p0()

@hello = private constant [5 x i8] c"hello"
@wasm = private constant [5 x i8] c" wasm"

; The roots frame: previous frame, slot count, two slots.
%frame = type { ptr, i64, [2 x i64] }

; The module entry, as the emitter writes it: the program's entry and its
; `toString` (here the identity: nothing calls it) handed to the runtime.
declare i32 @dartforge_init(ptr, ptr)
define i64 @to_string(i64 %v) {
  ret i64 %v
}
define i32 @dartforge_run() {
  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @to_string)
  ret i32 %r
}

define void @dartforge_entry() {
entry:
  %ctx = call ptr @dartforge_context()
  %topp = getelementptr inbounds i8, ptr %ctx, i64 8
  %f = alloca %frame, align 8
  %prev = load ptr, ptr %topp, align 8
  store ptr %prev, ptr %f, align 8
  %np = getelementptr inbounds %frame, ptr %f, i32 0, i32 1
  store i64 2, ptr %np, align 8
  %s0 = getelementptr inbounds %frame, ptr %f, i32 0, i32 2, i32 0
  %s1 = getelementptr inbounds %frame, ptr %f, i32 0, i32 2, i32 1
  store i64 0, ptr %s0, align 8
  store i64 0, ptr %s1, align 8
  store ptr %f, ptr %topp, align 8

  %h = call i64 @dartforge_string_new(ptr @hello, i64 5)
  store i64 %h, ptr %s0, align 8
  %w = call i64 @dartforge_string_new(ptr @wasm, i64 5)
  store i64 %w, ptr %s1, align 8
  br label %loop

; 200 000 concatenations of garbage, a full collection every 50 000: the
; young space fills and moves the two live strings many times.
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %latch ]
  %a = load i64, ptr %s0, align 8
  %b = load i64, ptr %s1, align 8
  %garbage = call i64 @dartforge_string_concat(i64 %a, i64 %b)
  %r = urem i64 %i, 50000
  %full = icmp eq i64 %r, 49999
  br i1 %full, label %collect, label %latch
collect:
  call void @dartforge_gc_collect()
  br label %latch
latch:
  %next = add i64 %i, 1
  %more = icmp ult i64 %next, 200000
  br i1 %more, label %loop, label %done

done:
  %a2 = load i64, ptr %s0, align 8
  %b2 = load i64, ptr %s1, align 8
  %c = call i64 @dartforge_string_concat(i64 %a2, i64 %b2)
  call void @dartforge_print_string(i64 %c)
  ; 64-bit integers cross the runtime boundary intact.
  call void @dartforge_print_i64(i64 -9223372036854775807)
  ; The wall clock is after 2020 (host `clock_realtime_us`).
  %now = call i64 @dartforge_native_DateTime_currentTimeMicros()
  %recent = icmp sgt i64 %now, 1577836800000000
  %recent8 = zext i1 %recent to i8
  call void @dartforge_print_bool(i8 %recent8)
  ; The monotonic clock does not go back (host `clock_monotonic_us`).
  %t0 = call i64 @dartforge_native_Stopwatch_now()
  %t1 = call i64 @dartforge_native_Stopwatch_now()
  %mono = icmp sge i64 %t1, %t0
  %mono8 = zext i1 %mono to i8
  call void @dartforge_print_bool(i8 %mono8)
  ; Two seeds from the host's generator differ (host `random_fill`).
  %r0 = call i64 @dartforge_native_Random_initialSeed()
  %r1 = call i64 @dartforge_native_Random_initialSeed()
  %differ = icmp ne i64 %r0, %r1
  %differ8 = zext i1 %differ to i8
  call void @dartforge_print_bool(i8 %differ8)
  ; The zone offset is within ±18 h (host `time_zone`).
  %z = call i64 @dartforge_native_DateTime_timeZoneOffsetInSeconds(i64 1700000000)
  %z1 = add i64 %z, 64800
  %zok = icmp ule i64 %z1, 129600
  %zok8 = zext i1 %zok to i8
  call void @dartforge_print_bool(i8 %zok8)

  ; The stack limit the generated prologue compares with `__stack_pointer`
  ; (`Context + 376`, an 8-byte cell even on wasm32) lies inside the linear
  ; stack: above 1 (no limit) and below the current stack pointer.
  %limitp = getelementptr inbounds i8, ptr %ctx, i64 376
  %limit = load i64, ptr %limitp, align 8
  %sp = call ptr @llvm.stacksave.p0()
  %spi = ptrtoint ptr %sp to i64
  %above = icmp ugt i64 %limit, 1
  %below = icmp ult i64 %limit, %spi
  %inside = and i1 %above, %below
  %inside8 = zext i1 %inside to i8
  call void @dartforge_print_bool(i8 %inside8)

  ; The collector did run: at least the four full collections of the loop.
  %stats = alloca [4 x i64], align 8
  call void @dartforge_gc_stats(ptr %stats)
  %collections = load i64, ptr %stats, align 8
  %ran = icmp uge i64 %collections, 4
  %ran8 = zext i1 %ran to i8
  call void @dartforge_print_bool(i8 %ran8)

  store ptr %prev, ptr %topp, align 8
  ret void
}
