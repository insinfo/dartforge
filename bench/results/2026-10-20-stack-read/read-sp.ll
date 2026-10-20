declare i64 @llvm.read_register.i64(metadata)
define i64 @read_stack() {
 %sp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
 ret i64 %sp
}
