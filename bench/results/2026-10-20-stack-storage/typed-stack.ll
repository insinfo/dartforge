define i64 @case_0(i64 %value, ptr %unknown) {
%p = alloca i64
 store i64 %value, ptr %p
 ret i64 0
}
define i64 @case_1(i64 %value, ptr %unknown) {
%p = alloca i64, i64 3
 %q = getelementptr i64, ptr %p, i64 2
 store i64 0, ptr %q
 ret i64 0
}
define i64 @case_2(i64 %value, ptr %unknown) {
%p = alloca [3 x i64]
 %q = getelementptr [3 x i64], ptr %p, i64 0, i64 2
 store i64 0, ptr %q
 ret i64 0
}
define i64 @case_3(i64 %value, ptr %unknown) {
%p = alloca [3 x i64]
 %q = getelementptr i64, ptr %p, i64 1
 %r = getelementptr i64, ptr %q, i64 1
 store i64 0, ptr %r
 ret i64 0
}
define i64 @case_4(i64 %value, ptr %unknown) {
%p = alloca { ptr, i64, [3 x i64] }, align 8
 %q = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %p, i64 0, i32 2, i64 2
 store i64 %value, ptr %q, align 8
 ret i64 0
}
define i64 @case_5(i64 %value, ptr %unknown) {
%"p" = alloca ptr
 store ptr %unknown, ptr %p
 ret i64 0
}
