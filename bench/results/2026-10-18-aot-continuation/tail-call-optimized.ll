; ModuleID = '/tmp/dartforge-tail-check.ll'
source_filename = "/tmp/dartforge-tail-check.ll"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx28.0.0"

@limit = external local_unnamed_addr global i64

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(read)
declare i64 @llvm.read_register.i64(metadata) #0

declare void @overflow() local_unnamed_addr

define void @plain() local_unnamed_addr {
entry:
  %sp = tail call i64 @llvm.read_register.i64(metadata !0)
  %l = load i64, ptr @limit, align 8
  %bad = icmp ult i64 %sp, %l
  br i1 %bad, label %fail, label %tailrecurse

tailrecurse:                                      ; preds = %entry, %tailrecurse
  br label %tailrecurse

fail:                                             ; preds = %entry
  tail call void @overflow()
  ret void
}

define void @protected() local_unnamed_addr #1 {
entry:
  %sp = call i64 @llvm.read_register.i64(metadata !0)
  %l = load i64, ptr @limit, align 8
  %bad = icmp ult i64 %sp, %l
  br i1 %bad, label %fail, label %next

common.ret:                                       ; preds = %next, %fail
  ret void

fail:                                             ; preds = %entry
  call void @overflow()
  br label %common.ret

next:                                             ; preds = %entry
  call void @protected()
  br label %common.ret
}

attributes #0 = { mustprogress nocallback nofree nosync nounwind willreturn memory(read) }
attributes #1 = { "disable-tail-calls"="true" }

!0 = !{!"sp"}
