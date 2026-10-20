define internal i64 @df.string_len(i64 %s) alwaysinline nounwind {
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  %n = load i64, ptr %q, align 8
  ret i64 %n
}

define i64 @df.lengths$2edart..stable(i64 %v0) noinline "disable-tail-calls"="true" nounwind {
b0:
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  %v3 = add i64 0, 0
  %v6 = add i64 0, 0
  %v11 = call i64 @df.string_len(i64 %v0)
  br label %b1
b1:
  %v33 = phi i64 [ %v3, %pilha.ok ], [ %v26, %b6 ]
  %v34 = phi i64 [ %v6, %pilha.ok ], [ %v29, %b6 ]
  %v12 = icmp slt i64 %v34, %v11
  br i1 %v12, label %b2, label %b4
b2:
  %ip13 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv13 = load atomic i8, ptr %ip13 monotonic, align 8
  %v13 = call i8 @llvm.expect.i8(i8 %iv13, i8 0)
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b5, label %b6
b4:
  %xcp32 = load i8, ptr %ctx, align 8
  %xcn32 = icmp ne i8 %xcp32, 0
  %xce32 = call i1 @llvm.expect.i1(i1 %xcn32, i1 false)
  br i1 %xce32, label %xc32.limpar, label %xc32.fim
xc32.limpar:
  call void @dartforge_exception_clear()
  br label %xc32.fim
xc32.fim:
  ret i64 %v33
b5:
  call void @dartforge_safepoint()
  %v16 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v16 to i64
  %v17 = icmp ne i64 %c1, 0
  br i1 %v17, label %b8, label %b7
b6:
  %v20 = call i64 @df.lengths$2edart..observe(i64 %v34)
  %v25 = add i64 %v20, %v11
  %v26 = add i64 %v33, %v25
  %v29 = add i64 %v34, 1
  br label %b1
b7:
  br label %b6
b8:
  ret i64 0
}

define i64 @df.lengths$2edart..changing(i64 %v0, i64 %v3) noinline "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  store i64 %v3, ptr %gcs1
  %v9 = add i64 0, 0
  %v12 = add i64 0, 0
  br label %b1
b1:
  %v46 = phi i64 [ %v0, %pilha.ok ], [ %v39, %b12 ]
  %v47 = phi i64 [ %v9, %pilha.ok ], [ %v42, %b12 ]
  %v48 = phi i64 [ %v12, %pilha.ok ], [ %v32, %b12 ]
  %v17 = call i64 @df.string_len(i64 %v46)
  %v18 = icmp slt i64 %v47, %v17
  br i1 %v18, label %b2, label %b3
b2:
  %ip19 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv19 = load atomic i8, ptr %ip19 monotonic, align 8
  %v19 = call i8 @llvm.expect.i8(i8 %iv19, i8 0)
  %c0 = zext i8 %v19 to i64
  %v20 = icmp ne i64 %c0, 0
  br i1 %v20, label %b4, label %b5
b3:
  %xcp45 = load i8, ptr %ctx, align 8
  %xcn45 = icmp ne i8 %xcp45, 0
  %xce45 = call i1 @llvm.expect.i1(i1 %xcn45, i1 false)
  br i1 %xce45, label %xc45.limpar, label %xc45.fim
xc45.limpar:
  call void @dartforge_exception_clear()
  br label %xc45.fim
xc45.fim:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v48
b4:
  call void @dartforge_safepoint()
  %v22 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v22 to i64
  %v23 = icmp ne i64 %c1, 0
  br i1 %v23, label %b7, label %b6
b5:
  %v26 = call i64 @df.lengths$2edart..observe(i64 %v47)
  %v31 = add i64 %v26, %v17
  %v32 = add i64 %v48, %v31
  %v35 = and i64 %v47, 1
  %v36 = icmp eq i64 %v35, 0
  br i1 %v36, label %b10, label %b11
b6:
  br label %b5
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 0
b10:
  br label %b12
b11:
  br label %b12
b12:
  %v39 = phi i64 [ %v3, %b10 ], [ %v0, %b11 ]
  %v42 = add i64 %v47, 1
  br label %b1
}

define i64 @df.lengths$2edart..nullable(i64 %v0, i1 %v3) noinline "disable-tail-calls"="true" nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)
  br i1 %pilhax, label %pilha.estouro, label %pilha.ok
pilha.estouro:
  call void @dartforge_stack_overflow()
  ret i64 0
pilha.ok:
  store { ptr, i64, [1 x i64] } { ptr null, i64 1, [1 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [1 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %v7 = xor i1 %v3, true
  br i1 %v7, label %b1, label %b3
b1:
  %v8 = add i64 0, 123
  %xcp9 = load i8, ptr %ctx, align 8
  %xcn9 = icmp ne i8 %xcp9, 0
  %xce9 = call i1 @llvm.expect.i1(i1 %xcn9, i1 false)
  br i1 %xce9, label %xc9.limpar, label %xc9.fim
xc9.limpar:
  call void @dartforge_exception_clear()
  br label %xc9.fim
xc9.fim:
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v8
b3:
  %v11 = icmp eq i64 %v0, 0
  br i1 %v11, label %b5, label %b6
b5:
  store i64 0, ptr %gcs0
  %v12 = call i64 @dartforge_null_check_error_new()
  store i64 %v12, ptr %gcs0
  call void @dartforge_exception_throw(i64 %v12, i8 3)
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 0
b6:
  %v14 = call i64 @df.lengths$2edart..stable(i64 %v0)
  %v15 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c0, 0
  br i1 %v16, label %b9, label %b8
b8:
  %xcp17 = load i8, ptr %ctx, align 8
  %xcn17 = icmp ne i8 %xcp17, 0
  %xce17 = call i1 @llvm.expect.i1(i1 %xcn17, i1 false)
  br i1 %xce17, label %xc17.limpar, label %xc17.fim
xc17.limpar:
  call void @dartforge_exception_clear()
  br label %xc17.fim
xc17.fim:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 %v14
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret i64 0
}
