define i64 @df.primitive_clamp$2edart..genericClamp(i64 %v0, i64 %v3, i64 %v6) noinline nounwind {
b0:
  %sargs12 = alloca [3 x i64]
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
  store i64 %v3, ptr %gcs1
  store i64 %v6, ptr %gcs2
  %sa12_0 = getelementptr [3 x i64], ptr %sargs12, i64 0, i64 0
  store i64 %v3, ptr %sa12_0
  %sa12_1 = getelementptr [3 x i64], ptr %sargs12, i64 0, i64 1
  store i64 %v6, ptr %sa12_1
  %sat12 = getelementptr [3 x i64], ptr %sargs12, i64 0, i64 2
  store i64 0, ptr %sat12
  %sic12 = getelementptr i64, ptr %area, i64 14
  %sf12 = call ptr @df.selector(ptr %sic12, i64 %v0, i64 9063860479497168791, ptr @df.seln.0, i64 7)
  %v12 = call i64 %sf12(i64 %v0, ptr %sargs12, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v13 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v13 to i64
  %v14 = icmp ne i64 %c0, 0
  br i1 %v14, label %b2, label %b1
b1:
  %gcvolta1 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1, ptr %ctxtopo, align 8
  ret i64 %v12
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret i64 0
}
