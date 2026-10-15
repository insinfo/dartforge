define i64 @df.primitive_clamp$2edart..typedUnitClamp(i64 %v0) noinline nounwind {
b0:
  %sargs19 = alloca [3 x i64]
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
  %v6 = call i64 @df.class(i64 %v0)
  switch i64 %v6, label %b3 [ i64 4, label %b1 i64 2, label %b2 i64 3, label %b2 ]
b1:
  %v7 = call i64 @df.box_int(i64 %v4)
  store i64 %v7, ptr %gcs1
  %v8 = call i64 @df.box_int(i64 %v5)
  store i64 %v8, ptr %gcs2
  %v9 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v7, i64 %v8)
  %v10 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v10 to i64
  %v11 = icmp ne i64 %c0, 0
  br i1 %v11, label %b6, label %b5
b2:
  %v12 = call i64 @df.box_int(i64 %v4)
  store i64 %v12, ptr %gcs1
  %v13 = call i64 @df.box_int(i64 %v5)
  store i64 %v13, ptr %gcs2
  %v14 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v12, i64 %v13)
  %v15 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c1, 0
  br i1 %v16, label %b8, label %b7
b3:
  %v17 = call i64 @df.box_int(i64 %v4)
  store i64 %v17, ptr %gcs1
  %v18 = call i64 @df.box_int(i64 %v5)
  store i64 %v18, ptr %gcs2
  %sa19_0 = getelementptr [3 x i64], ptr %sargs19, i64 0, i64 0
  store i64 %v17, ptr %sa19_0
  %sa19_1 = getelementptr [3 x i64], ptr %sargs19, i64 0, i64 1
  store i64 %v18, ptr %sa19_1
  %sat19 = getelementptr [3 x i64], ptr %sargs19, i64 0, i64 2
  store i64 0, ptr %sat19
  %sic19 = getelementptr i64, ptr %area, i64 12
  %sf19 = call ptr @df.selector(ptr %sic19, i64 %v0, i64 -1664458868869587675, ptr @df.seln.1, i64 8)
  %v19 = call i64 %sf19(i64 %v0, ptr %sargs19, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v20 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v20 to i64
  %v21 = icmp ne i64 %c2, 0
  br i1 %v21, label %b10, label %b9
b4:
  %v22 = phi i64 [ %v9, %b5 ], [ %v14, %b7 ], [ %v19, %b9 ]
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 %v22
b5:
  br label %b4
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  br label %b4
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
b9:
  br label %b4
b10:
  %gcvolta10 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta10, ptr %ctxtopo, align 8
  ret i64 0
}
