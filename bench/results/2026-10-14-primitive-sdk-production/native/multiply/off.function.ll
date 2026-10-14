define i64 @df.primitive_multiply$2edart..multiply(i64 %v0, i64 %v3) noinline nounwind {
b0:
  %sargs16 = alloca [2 x i64]
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
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
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  store i64 %v3, ptr %gcs1
  %v8 = and i64 %v0, 1
  %v9 = and i64 %v3, 1
  %v10 = and i64 %v8, %v9
  %v11 = icmp ne i64 %v10, 0
  br i1 %v11, label %b1, label %b2
b1:
  %v12 = ashr i64 %v0, 1
  store i64 0, ptr %gcs0
  %v13 = ashr i64 %v3, 1
  %v14 = mul i64 %v12, %v13
  store i64 0, ptr %gcs1
  %v15 = call i64 @df.box_int(i64 %v14)
  br label %b3
b2:
  %sa16_0 = getelementptr [2 x i64], ptr %sargs16, i64 0, i64 0
  store i64 %v3, ptr %sa16_0
  %sat16 = getelementptr [2 x i64], ptr %sargs16, i64 0, i64 1
  store i64 0, ptr %sat16
  %sic16 = getelementptr i64, ptr %area, i64 6
  %sf16 = call ptr @df.selector(ptr %sic16, i64 %v0, i64 -677990647923348682, ptr @df.seln.0, i64 3)
  %v16 = call i64 %sf16(i64 %v0, ptr %sargs16, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v17 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c0, 0
  br i1 %v18, label %b5, label %b4
b3:
  %v19 = phi i64 [ %v15, %b1 ], [ %v16, %b4 ]
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v19
b4:
  br label %b3
b5:
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 0
}
