define i64 @df.main$2edart..roundTrip(i64 %v0) noinline nounwind {
b0:
  %gcq = alloca { ptr, i64, [1 x i64] }, align 8
  %pilhaq = alloca i8, align 1
  %area = call ptr @df.get_area()
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load ptr, ptr %pilhalp, align 8
  %pilhab = icmp ult ptr %pilhaq, %pilhal
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
  %v4 = icmp eq i64 %v0, 0
  %v5 = call i64 @df.class(i64 %v0)
  %v6 = call i8 @df.subclass(i64 %v5, i64 673)
  %c0 = zext i8 %v6 to i64
  %v7 = icmp ne i64 %c0, 0
  %v8 = xor i1 %v4, true
  %c1 = zext i1 %v7 to i64
  %c2 = zext i1 %v8 to i64
  %v9 = and i64 %c1, %c2
  %v10 = icmp ne i64 %v9, 0
  br i1 %v10, label %b2, label %b1
b1:
  %ga11 = getelementptr i64, ptr %area, i64 1
  %v11 = load i8, ptr %ga11
  %c3 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c3, 0
  br i1 %v12, label %b3, label %b4
b2:
  %v20 = call double @df.unbox_double(i64 %v0)
  %v21 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v21 to i64
  %v22 = icmp ne i64 %c4, 0
  br i1 %v22, label %b9, label %b8
b3:
  %ga13 = getelementptr i64, ptr %area, i64 0
  %v13 = load i64, ptr %ga13
  br label %b5
b4:
  %v14 = call i64 @df.rti.877bf59dd991ad96.main$2edart()
  br label %b5
b5:
  %v15 = phi i64 [ %v13, %b3 ], [ %v14, %b4 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v15, i64 1, i64 0)
  %v17 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c5, 0
  br i1 %v18, label %b7, label %b6
b6:
  br label %b2
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 0
b8:
  store i64 0, ptr %gcs0
  %v25 = call i64 @df.box_double(double %v20)
  %xcp26 = load i8, ptr %ctx, align 8
  %xcn26 = icmp ne i8 %xcp26, 0
  %xce26 = call i1 @llvm.expect.i1(i1 %xcn26, i1 false)
  br i1 %xce26, label %xc26.limpar, label %xc26.fim
xc26.limpar:
  call void @dartforge_exception_clear()
  br label %xc26.fim
xc26.fim:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 %v25
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret i64 0
}
