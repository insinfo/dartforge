; W2 out-of-memory proof: a string doubled until the linear memory (linked
; with a 64 MiB maximum) cannot grow. The runtime must end with status 255 and
; an `Out of memory` line, never a bare trap or a Dart exception.
target triple = "wasm32-unknown-unknown"

declare ptr @dartforge_context()
declare i64 @dartforge_string_new(ptr, i64)
declare i64 @dartforge_string_concat(i64, i64)

@seed = private constant [8 x i8] c"dartforg"

%frame = type { ptr, i64, [1 x i64] }

define void @dartforge_entry() {
entry:
  %ctx = call ptr @dartforge_context()
  %topp = getelementptr inbounds i8, ptr %ctx, i64 8
  %f = alloca %frame, align 8
  %prev = load ptr, ptr %topp, align 8
  store ptr %prev, ptr %f, align 8
  %np = getelementptr inbounds %frame, ptr %f, i32 0, i32 1
  store i64 1, ptr %np, align 8
  %s0 = getelementptr inbounds %frame, ptr %f, i32 0, i32 2, i32 0
  store i64 0, ptr %s0, align 8
  store ptr %f, ptr %topp, align 8
  %s = call i64 @dartforge_string_new(ptr @seed, i64 8)
  store i64 %s, ptr %s0, align 8
  br label %grow
grow:
  %a = load i64, ptr %s0, align 8
  %b = call i64 @dartforge_string_concat(i64 %a, i64 %a)
  store i64 %b, ptr %s0, align 8
  br label %grow
}
