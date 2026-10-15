define i64 @df.primitive_clamp$2edart..unitClamp(i64 %v0) noinline nounwind {
b0:
  %sargs31 = alloca [3 x i64]
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
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
  store { ptr, i64, [3 x i64] } { ptr null, i64 3, [3 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [3 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %v4 = add i64 0, 0
  %v5 = add i64 0, 1
  %v6 = call i64 @df.box_int(i64 %v4)
  store i64 %v6, ptr %gcs1
  %v7 = call i64 @df.box_int(i64 %v5)
  store i64 %v7, ptr %gcs2
  %v11 = call i64 @df.class(i64 %v0)
  %v12 = icmp eq i64 %v11, 2
  %v13 = icmp eq i64 %v11, 3
  %v14 = icmp eq i64 %v11, 4
  %c0 = zext i1 %v12 to i64
  %c1 = zext i1 %v13 to i64
  %v15 = or i64 %c0, %c1
  %c2 = zext i1 %v14 to i64
  %v16 = or i64 %v15, %c2
  %c3 = trunc i64 %v16 to i1
  br i1 %c3, label %b4, label %b3
b1:
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v8
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b4:
  br i1 %v14, label %b6, label %b5
b5:
  %v17 = call i64 @df.unbox_int(i64 %v0)
  %v18 = icmp slt i64 %v17, 0
  br i1 %v18, label %b9, label %b7
b6:
  %v20 = call double @df.unbox_double(i64 %v0)
  %v21 = bitcast double %v20 to i64
  %v22 = icmp eq i64 %v21, -9223372036854775808
  %v23 = fcmp olt double %v20, 0x0000000000000000
  %c4 = zext i1 %v23 to i64
  %c5 = zext i1 %v22 to i64
  %v24 = or i64 %c4, %c5
  %c6 = trunc i64 %v24 to i1
  br i1 %c6, label %b9, label %b8
b7:
  %v19 = icmp sgt i64 %v17, 1
  br i1 %v19, label %b10, label %b11
b8:
  %v25 = and i64 %v21, 9223372036854775807
  %v26 = icmp sgt i64 %v25, 9218868437227405312
  %v27 = fcmp ogt double %v20, 0x3FF0000000000000
  %c7 = zext i1 %v27 to i64
  %c8 = zext i1 %v26 to i64
  %v28 = or i64 %c7, %c8
  %c9 = trunc i64 %v28 to i1
  br i1 %c9, label %b10, label %b11
b9:
  br label %b12
b10:
  br label %b12
b11:
  br label %b12
b3:
  %v29 = phi i64 [ %v6, %pilha.ok ]
  %v30 = phi i64 [ %v7, %pilha.ok ]
  store i64 %v29, ptr %gcs1
  store i64 %v30, ptr %gcs2
  %sa31_0 = getelementptr [3 x i64], ptr %sargs31, i64 0, i64 0
  store i64 %v29, ptr %sa31_0
  %sa31_1 = getelementptr [3 x i64], ptr %sargs31, i64 0, i64 1
  store i64 %v30, ptr %sa31_1
  %sat31 = getelementptr [3 x i64], ptr %sargs31, i64 0, i64 2
  store i64 0, ptr %sat31
  %sic31 = getelementptr i64, ptr %area, i64 10
  %sf31 = call ptr @df.selector(ptr %sic31, i64 %v0, i64 9063860479497168791, ptr @df.seln.0, i64 7)
  %v31 = call i64 %sf31(i64 %v0, ptr %sargs31, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  br label %b12
b12:
  %v8 = phi i64 [ %v31, %b3 ], [ %v6, %b9 ], [ %v7, %b10 ], [ %v0, %b11 ]
  %v9 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v9 to i64
  %v10 = icmp ne i64 %c10, 0
  br i1 %v10, label %b2, label %b1
}
