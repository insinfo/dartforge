declare i64 @dartforge_object_new(i64, i64)
define i64 @identity(i64 %"}") { ret i64 %"}" }
define i64 @allocate() {
 %o = call i64 @dartforge_object_new(i64 399, i64 0)
 ret i64 7
} ; a closing comment with an unmatched quote "
define i64 @following() { ret i64 8 }
