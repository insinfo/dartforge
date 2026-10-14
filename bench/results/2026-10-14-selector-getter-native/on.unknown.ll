define i64 @df.getter_return_provenance$2edart..convertUnknown(i64 %v0, i64 %v3) noinline nounwind {
b0:
  %sargs7 = alloca [1 x i64]
  %sargs11 = alloca [2 x i64]
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
  %sat7 = getelementptr [1 x i64], ptr %sargs7, i64 0, i64 0
  store i64 0, ptr %sat7
  %sic7 = getelementptr i64, ptr %area, i64 26
  %sf7 = call ptr @df.selector(ptr %sic7, i64 %v0, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v7 = call i64 %sf7(i64 %v0, ptr %sargs7, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v7, ptr %gcs0
  %v8 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v8 to i64
  %v9 = icmp ne i64 %c0, 0
  br i1 %v9, label %b2, label %b1
b1:
  %sa11_0 = getelementptr [2 x i64], ptr %sargs11, i64 0, i64 0
  store i64 %v3, ptr %sa11_0
  %sat11 = getelementptr [2 x i64], ptr %sargs11, i64 0, i64 1
  store i64 0, ptr %sat11
  %sic11 = getelementptr i64, ptr %area, i64 28
  %sf11 = call ptr @df.selector(ptr %sic11, i64 %v7, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v11 = call i64 %sf11(i64 %v7, ptr %sargs11, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v12 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c1, 0
  br i1 %v13, label %b4, label %b3
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
b3:
  %gcvolta3 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta3, ptr %ctxtopo, align 8
  ret i64 %v11
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 0
}
