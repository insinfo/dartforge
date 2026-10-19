declare i64 @dartforge_object_new(i64, i64)
define i64 @allocate(i64 %"\22{ ret i64 0 }\22") {
 %o = call i64 @dartforge_object_new(i64 399, i64 0)
 ret i64 7
}
