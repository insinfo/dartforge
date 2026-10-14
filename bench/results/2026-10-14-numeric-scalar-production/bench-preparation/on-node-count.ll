define i64 @df.bench$2edart.Node.count(i64 %v0) nounwind {
b0:
  %gcq = alloca { ptr, i64, [2 x i64] }, align 8
  %pilhaq = alloca i8, align 1
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
  %v1 = add i64 0, 1
  %fxa2 = add i64 %v0, -2
  %fcb2 = inttoptr i64 %fxa2 to ptr
  %fp2 = getelementptr inbounds i8, ptr %fcb2, i64 16
  %fg2 = getelementptr inbounds i64, ptr %fp2, i64 0
  %v2 = load i64, ptr %fg2, align 8
  store i64 %v2, ptr %gcs1
  %v3 = icmp eq i64 %v2, 0
  br i1 %v3, label %b1, label %b2
b1:
  %v8 = phi i64 [ 0, %pilha.ok ], [ %v4, %b3 ]
  %v30 = phi i1 [ true, %pilha.ok ], [ false, %b3 ]
  %c0 = zext i1 %v30 to i64
  %v9 = icmp eq i64 %c0, 1
  br i1 %v9, label %b5, label %b6
b2:
  %v4 = call i64 @df.bench$2edart.Node.count(i64 %v2)
  %v5 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v5 to i64
  %v6 = icmp ne i64 %c1, 0
  br i1 %v6, label %b4, label %b3
b3:
  br label %b1
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
b5:
  %v13 = add i64 0, 0
  br label %b7
b6:
  %v10 = add i64 %v8, 0
  %v11 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c2, 0
  br i1 %v12, label %b9, label %b8
b7:
  %v14 = phi i64 [ %v10, %b8 ], [ %v13, %b5 ]
  %v15 = add i64 %v1, %v14
  %fxa16 = add i64 %v0, -2
  %fcb16 = inttoptr i64 %fxa16 to ptr
  %fp16 = getelementptr inbounds i8, ptr %fcb16, i64 16
  %fg16 = getelementptr inbounds i64, ptr %fp16, i64 1
  %v16 = load i64, ptr %fg16, align 8
  store i64 %v16, ptr %gcs0
  store i64 0, ptr %gcs1
  %v17 = icmp eq i64 %v16, 0
  br i1 %v17, label %b10, label %b11
b8:
  br label %b7
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret i64 0
b10:
  %v22 = phi i64 [ 0, %b7 ], [ %v18, %b12 ]
  %v31 = phi i1 [ true, %b7 ], [ false, %b12 ]
  %c3 = zext i1 %v31 to i64
  %v23 = icmp eq i64 %c3, 1
  br i1 %v23, label %b14, label %b15
b11:
  %v18 = call i64 @df.bench$2edart.Node.count(i64 %v16)
  %v19 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v19 to i64
  %v20 = icmp ne i64 %c4, 0
  br i1 %v20, label %b13, label %b12
b12:
  br label %b10
b13:
  %gcvolta13 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta13, ptr %ctxtopo, align 8
  ret i64 0
b14:
  %v27 = add i64 0, 0
  br label %b16
b15:
  %v24 = add i64 %v22, 0
  %v25 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v25 to i64
  %v26 = icmp ne i64 %c5, 0
  br i1 %v26, label %b18, label %b17
b16:
  %v28 = phi i64 [ %v24, %b17 ], [ %v27, %b14 ]
  %v29 = add i64 %v15, %v28
  %gcvolta16 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta16, ptr %ctxtopo, align 8
  ret i64 %v29
b17:
  br label %b16
b18:
  %gcvolta18 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta18, ptr %ctxtopo, align 8
  ret i64 0
}
