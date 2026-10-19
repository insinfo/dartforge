target triple = "arm64-apple-macosx11.0.0"
@limit = external global i64
declare i64 @llvm.read_register.i64(metadata)
declare void @overflow()
define void @plain() {
entry:
 %sp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
 %l = load i64, ptr @limit
 %bad = icmp ult i64 %sp, %l
 br i1 %bad, label %fail, label %next
fail:
 call void @overflow()
 ret void
next:
 call void @plain()
 ret void
}
define void @protected() "disable-tail-calls"="true" {
entry:
 %sp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
 %l = load i64, ptr @limit
 %bad = icmp ult i64 %sp, %l
 br i1 %bad, label %fail, label %next
fail:
 call void @overflow()
 ret void
next:
 call void @protected()
 ret void
}
