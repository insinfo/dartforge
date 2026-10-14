define i64 @df.primitive_multiply$2edart..multiply(i64 %v0, i64 %v3) noinline nounwind {
b0:
  %sargs47 = alloca [2 x i64]
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
  %v19 = icmp eq i64 %v16, 4
  %c0 = zext i1 %v17 to i64
  %c1 = zext i1 %v18 to i64
  %v20 = or i64 %c0, %c1
  %c2 = zext i1 %v19 to i64
  %v21 = or i64 %v20, %c2
  %v22 = call i64 @df.class(i64 %v3)
  %v23 = icmp eq i64 %v22, 2
  %v24 = icmp eq i64 %v22, 3
  %v25 = icmp eq i64 %v22, 4
  %c3 = zext i1 %v23 to i64
  %c4 = zext i1 %v24 to i64
  %v26 = or i64 %c3, %c4
  %c5 = zext i1 %v25 to i64
  %v27 = or i64 %v26, %c5
  %v28 = and i64 %v21, %v27
  %c6 = trunc i64 %v28 to i1
  br i1 %c6, label %b4, label %b5
b3:
  %v51 = phi i64 [ %v15, %b1 ], [ %v50, %b6 ]
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v51
b4:
  %c7 = zext i1 %v19 to i64
  %c8 = zext i1 %v25 to i64
  %v29 = or i64 %c7, %c8
  %c9 = trunc i64 %v29 to i1
  br i1 %c9, label %b8, label %b7
b5:
  %sa47_0 = getelementptr [2 x i64], ptr %sargs47, i64 0, i64 0
  store i64 %v3, ptr %sa47_0
  %sat47 = getelementptr [2 x i64], ptr %sargs47, i64 0, i64 1
  store i64 0, ptr %sat47
  %sic47 = getelementptr i64, ptr %area, i64 6
  %sf47 = call ptr @df.selector(ptr %sic47, i64 %v0, i64 -677990647923348682, ptr @df.seln.0, i64 3)
  %v47 = call i64 %sf47(i64 %v0, ptr %sargs47, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v48 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v48 to i64
  %v49 = icmp ne i64 %c10, 0
  br i1 %v49, label %b19, label %b18
b6:
  %v50 = phi i64 [ %v46, %b9 ], [ %v47, %b18 ]
  br label %b3
b7:
  %v30 = call i64 @df.unbox_int(i64 %v0)
  %v31 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v31 to i64
  %v32 = icmp ne i64 %c11, 0
  br i1 %v32, label %b11, label %b10
b8:
  %v38 = call double @df.num_to_double(i64 %v0)
  %v39 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v39 to i64
  %v40 = icmp ne i64 %c12, 0
  br i1 %v40, label %b15, label %b14
b9:
  %v46 = phi i64 [ %v37, %b12 ], [ %v45, %b16 ]
  br label %b6
b10:
  store i64 0, ptr %gcs0
  %v33 = call i64 @df.unbox_int(i64 %v3)
  %v34 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v34 to i64
  %v35 = icmp ne i64 %c13, 0
  br i1 %v35, label %b13, label %b12
b11:
  %gcvolta11 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta11, ptr %ctxtopo, align 8
  ret i64 0
b12:
  %v36 = mul i64 %v30, %v33
  store i64 0, ptr %gcs1
  %v37 = call i64 @df.box_int(i64 %v36)
  br label %b9
b13:
  %gcvolta13 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta13, ptr %ctxtopo, align 8
  ret i64 0
b14:
  store i64 0, ptr %gcs0
  %v41 = call double @df.num_to_double(i64 %v3)
  %v42 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v42 to i64
  %v43 = icmp ne i64 %c14, 0
  br i1 %v43, label %b17, label %b16
b15:
  %gcvolta15 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta15, ptr %ctxtopo, align 8
  ret i64 0
b16:
  %v44 = fmul double %v38, %v41
  store i64 0, ptr %gcs1
  %v45 = call i64 @df.box_double(double %v44)
  br label %b9
b17:
  %gcvolta17 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta17, ptr %ctxtopo, align 8
  ret i64 0
b18:
  br label %b6
b19:
  %gcvolta19 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta19, ptr %ctxtopo, align 8
  ret i64 0
}
