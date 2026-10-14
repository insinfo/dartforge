define void @dart_main() nounwind {
b0:
  %sargs7 = alloca [2 x i64]
  %sargs33 = alloca [1 x i64]
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
  ret void
pilha.ok:
  store { ptr, i64, [2 x i64] } { ptr null, i64 2, [2 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [2 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %v0 = call i64 @df.dart$3aconvert..dartforgeFreshList()
  store i64 %v0, ptr %gcs0
  %v1 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v1 to i64
  %v2 = icmp ne i64 %c0, 0
  br i1 %v2, label %b2, label %b1
b1:
  %v6 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.9c589500711b89f8a7abea1d72d8180a", i64 2) to i64
  store i64 %v6, ptr %gcs1
  %sa7_0 = getelementptr [2 x i64], ptr %sargs7, i64 0, i64 0
  store i64 %v6, ptr %sa7_0
  %sat7 = getelementptr [2 x i64], ptr %sargs7, i64 0, i64 1
  store i64 0, ptr %sat7
  %sic7 = getelementptr i64, ptr %area, i64 6
  %sf7 = call ptr @df.selector(ptr %sic7, i64 %v0, i64 4270166662586539371, ptr @df.seln.1, i64 5)
  %v7 = call i64 %sf7(i64 %v0, ptr %sargs7, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v8 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v8 to i64
  %v9 = icmp ne i64 %c1, 0
  br i1 %v9, label %b5, label %b6
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret void
b4:
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v0)
  %v30 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v30 to i64
  %v31 = icmp ne i64 %c2, 0
  br i1 %v31, label %b15, label %b14
b5:
  store i64 0, ptr %gcs1
  %v13 = call i64 @dartforge_exception_peek_ref()
  store i64 %v13, ptr %gcs1
  %v14 = call i8 @dartforge_exception_catchable()
  %c3 = zext i8 %v14 to i64
  %v15 = icmp ne i64 %c3, 0
  br i1 %v15, label %b8, label %b9
b6:
  %v11 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v11 to i64
  %v12 = icmp ne i64 %c4, 0
  br i1 %v12, label %b5, label %b7
b7:
  br label %b4
b8:
  %v16 = icmp eq i64 %v13, 0
  %v17 = call i64 @df.class(i64 %v13)
  %v18 = call i8 @df.subclass(i64 %v17, i64 624)
  %c5 = zext i8 %v18 to i64
  %v19 = icmp ne i64 %c5, 0
  %v20 = xor i1 %v16, true
  %c6 = zext i1 %v19 to i64
  %c7 = zext i1 %v20 to i64
  %v21 = and i64 %c6, %c7
  %v22 = icmp ne i64 %v21, 0
  br i1 %v22, label %b10, label %b11
b9:
  br label %b11
b10:
  %xcp23 = load i8, ptr %ctx, align 8
  %xcn23 = icmp ne i8 %xcp23, 0
  %xce23 = call i1 @llvm.expect.i1(i1 %xcn23, i1 false)
  br i1 %xce23, label %xc23.limpar, label %xc23.fim
xc23.limpar:
  call void @dartforge_exception_clear()
  br label %xc23.fim
xc23.fim:
  %v24 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.cc9814f51071414adab80bfa8916eab3", i64 2) to i64
  store i64 %v24, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v24)
  %v26 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v26 to i64
  %v27 = icmp ne i64 %c8, 0
  br i1 %v27, label %b13, label %b12
b11:
  %gcvolta11 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta11, ptr %ctxtopo, align 8
  ret void
b12:
  br label %b4
b13:
  %gcvolta13 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta13, ptr %ctxtopo, align 8
  ret void
b14:
  %sat33 = getelementptr [1 x i64], ptr %sargs33, i64 0, i64 0
  store i64 0, ptr %sat33
  %sic33 = getelementptr i64, ptr %area, i64 8
  %sf33 = call ptr @df.selector(ptr %sic33, i64 %v0, i64 -6565620648322651272, ptr @df.seln.0, i64 8)
  %v33 = call i64 %sf33(i64 %v0, ptr %sargs33, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v33, ptr %gcs0
  %v34 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v34 to i64
  %v35 = icmp ne i64 %c9, 0
  br i1 %v35, label %b17, label %b16
b15:
  %gcvolta15 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta15, ptr %ctxtopo, align 8
  ret void
b16:
  call void @df.dart$3acore..print(i64 %v33)
  %v37 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v37 to i64
  %v38 = icmp ne i64 %c10, 0
  br i1 %v38, label %b19, label %b18
b17:
  %gcvolta17 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta17, ptr %ctxtopo, align 8
  ret void
b18:
  store i64 0, ptr %gcs0
  %ta39 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb39 = load ptr, ptr %ta39, align 8
  %tfa39 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf39 = load ptr, ptr %tfa39, align 8
  %tnx39 = getelementptr i8, ptr %tb39, i64 24
  %tz39 = icmp ugt ptr %tnx39, %tf39
  %tnp39 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn39 = load i64, ptr %tnp39, align 8
  %tk39 = icmp ule i64 %tn39, 1024
  %tq39 = or i1 %tz39, %tk39
  %tx39 = call i1 @llvm.expect.i1(i1 %tq39, i1 false)
  br i1 %tx39, label %ao39.lento, label %ao39.reg
ao39.reg:
  %trp39 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr39 = load ptr, ptr %trp39, align 8
  %trb39 = getelementptr inbounds i8, ptr %tr39, i64 1024
  %trv39 = load i8, ptr %trb39, align 1
  %trz39 = icmp eq i8 %trv39, 0
  %try39 = call i1 @llvm.expect.i1(i1 %trz39, i1 false)
  br i1 %try39, label %ao39.lento, label %ao39.rapido
ao39.rapido:
  store ptr %tnx39, ptr %ta39, align 8
  %tpp39 = getelementptr inbounds i8, ptr %tb39, i64 16
  store i64 0, ptr %tpp39, align 8
  store i64 4398046576641, ptr %tb39, align 8
  %thb39 = ptrtoint ptr %tb39 to i64
  %th39 = add i64 %thb39, 2
  br label %ao39.fim
ao39.lento:
  %tl39 = call i64 @dartforge_object_new_t(i64 1024, i64 1, ptr @df.mt.main$2edart.OtherList)
  br label %ao39.fim
ao39.fim:
  %v39 = phi i64 [ %th39, %ao39.rapido ], [ %tl39, %ao39.lento ]
  store i64 %v39, ptr %gcs0
  %v59 = add i64 0, 0
  %fxa60 = add i64 %v39, -2
  %fcb60 = inttoptr i64 %fxa60 to ptr
  %fp60 = getelementptr inbounds i8, ptr %fcb60, i64 16
  %fg60 = getelementptr inbounds i64, ptr %fp60, i64 0
  store i64 %v59, ptr %fg60, align 8
  %fmp60 = getelementptr inbounds i8, ptr %fcb60, i64 8
  %fm60 = load i32, ptr %fmp60, align 4
  %fmc60 = and i32 %fm60, -2
  store i32 %fmc60, ptr %fmp60, align 4
  %v41 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v41 to i64
  %v42 = icmp ne i64 %c11, 0
  br i1 %v42, label %b21, label %b20
b19:
  %gcvolta19 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta19, ptr %ctxtopo, align 8
  ret void
b20:
  %v46 = call i64 @df.main$2edart..unknownLength(i64 %v39)
  %v47 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v47 to i64
  %v48 = icmp ne i64 %c12, 0
  br i1 %v48, label %b23, label %b22
b21:
  %gcvolta21 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta21, ptr %ctxtopo, align 8
  ret void
b22:
  %v49 = call i64 @df.box_int(i64 %v46)
  store i64 %v49, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v49)
  %v51 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v51 to i64
  %v52 = icmp ne i64 %c13, 0
  br i1 %v52, label %b25, label %b24
b23:
  %gcvolta23 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta23, ptr %ctxtopo, align 8
  ret void
b24:
  %fxk54 = and i64 %v39, -9223372036854775801
  %fxo54 = icmp eq i64 %fxk54, 2
  %fxa54 = add i64 %v39, -2
  %fxq54 = inttoptr i64 %fxa54 to ptr
  %fxvp54 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv54 = load ptr, ptr %fxvp54, align 8
  %fxh54 = select i1 %fxo54, ptr %fxq54, ptr %fxv54
  %fxfp54 = getelementptr inbounds i8, ptr %fxh54, i64 1
  %fxf54 = load i8, ptr %fxfp54, align 1
  %fxfb54 = and i8 %fxf54, 1
  %fxfo54 = icmp ne i8 %fxfb54, 0
  %fxcp54 = getelementptr inbounds i8, ptr %fxh54, i64 16
  %fxc54 = load ptr, ptr %fxcp54, align 8
  %fcb54 = select i1 %fxfo54, ptr %fxc54, ptr %fxh54
  %fp54 = getelementptr inbounds i8, ptr %fcb54, i64 16
  %fg54 = getelementptr inbounds i64, ptr %fp54, i64 0
  %v54 = load i64, ptr %fg54, align 8
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v55 = call i64 @df.box_int(i64 %v54)
  store i64 %v55, ptr %gcs0
  call void @df.dart$3acore..print(i64 %v55)
  %v57 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v57 to i64
  %v58 = icmp ne i64 %c14, 0
  br i1 %v58, label %b27, label %b26
b25:
  %gcvolta25 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta25, ptr %ctxtopo, align 8
  ret void
b26:
  %gcvolta26 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta26, ptr %ctxtopo, align 8
  ret void
b27:
  %gcvolta27 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta27, ptr %ctxtopo, align 8
  ret void
}
