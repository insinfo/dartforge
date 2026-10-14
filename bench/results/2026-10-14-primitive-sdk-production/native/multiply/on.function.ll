define i64 @df.primitive_multiply$2edart..multiply(i64 %v0, i64 %v3) noinline nounwind {
b0:
  %sargs32 = alloca [2 x i64]
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
  %v16 = call i64 @df.class(i64 %v0)
  %v17 = icmp eq i64 %v16, 2
  %v18 = icmp eq i64 %v16, 3
  %c0 = zext i1 %v17 to i64
  %c1 = zext i1 %v18 to i64
  %v19 = or i64 %c0, %c1
  %v20 = icmp eq i64 %v16, 4
  %c2 = zext i1 %v20 to i64
  %v21 = or i64 %v19, %c2
  %v22 = call i64 @df.class(i64 %v3)
  %v23 = icmp eq i64 %v22, 2
  %v24 = icmp eq i64 %v22, 3
  %c3 = zext i1 %v23 to i64
  %c4 = zext i1 %v24 to i64
  %v25 = or i64 %c3, %c4
  %v26 = icmp eq i64 %v22, 4
  %c5 = zext i1 %v26 to i64
  %v27 = or i64 %v25, %c5
  %v28 = and i64 %v21, %v27
  %c6 = trunc i64 %v28 to i1
  br i1 %c6, label %b4, label %b5
b3:
  %v36 = phi i64 [ %v15, %b1 ], [ %v35, %b6 ]
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v36
b4:
  %v29 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v3)
  %v30 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v30 to i64
  %v31 = icmp ne i64 %c7, 0
  br i1 %v31, label %b8, label %b7
b5:
  %sa32_0 = getelementptr [2 x i64], ptr %sargs32, i64 0, i64 0
  store i64 %v3, ptr %sa32_0
  %sat32 = getelementptr [2 x i64], ptr %sargs32, i64 0, i64 1
  store i64 0, ptr %sat32
  %sic32 = getelementptr i64, ptr %area, i64 6
  %sf32 = call ptr @df.selector(ptr %sic32, i64 %v0, i64 -677990647923348682, ptr @df.seln.0, i64 3)
  %v32 = call i64 %sf32(i64 %v0, ptr %sargs32, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v33 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v33 to i64
  %v34 = icmp ne i64 %c8, 0
  br i1 %v34, label %b10, label %b9
b6:
  %v35 = phi i64 [ %v29, %b7 ], [ %v32, %b9 ]
  br label %b3
b7:
  br label %b6
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
b9:
  br label %b6
b10:
  %gcvolta10 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta10, ptr %ctxtopo, align 8
  ret i64 0
}
