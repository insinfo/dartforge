define i64 @df.color_numeric_performance$2edart..convertPixels(i64 %v0, i64 %v3) noinline nounwind {
b0:
  %sargs43 = alloca [2 x i64]
  %sargs38 = alloca [2 x i64]
  %sargs66 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [4 x i64] }, align 8
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
  store { ptr, i64, [4 x i64] } { ptr null, i64 4, [4 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  %v6 = add i64 0, 0
  %v9 = add i64 0, 0
  br label %b1
b1:
  %v80 = phi i64 [ %v6, %pilha.ok ], [ %v73, %b30 ]
  %v81 = phi i64 [ %v9, %pilha.ok ], [ %v76, %b30 ]
  %v14 = icmp slt i64 %v81, %v3
  br i1 %v14, label %b2, label %b4
b2:
  %v18 = add i64 0, 255
  %v19 = and i64 %v81, %v18
  %v20 = call i64 @df.core_len(i64 %v0)
  %v21 = icmp ult i64 %v19, %v20
  br i1 %v21, label %b5, label %b6
b4:
  %xcp79 = load i8, ptr %ctx, align 8
  %xcn79 = icmp ne i8 %xcp79, 0
  %xce79 = call i1 @llvm.expect.i1(i1 %xcn79, i1 false)
  br i1 %xce79, label %xc79.limpar, label %xc79.fim
xc79.limpar:
  call void @dartforge_exception_clear()
  br label %xc79.fim
xc79.fim:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 %v80
b5:
  %v22 = call i64 @df.core_storage(i64 %v0)
  %v23 = call i64 @df.core_shape(i64 %v22)
  %v24 = add i64 %v22, 22
  %v25 = icmp eq i64 %v23, 4
  br i1 %v25, label %b9, label %b10
b6:
  store i64 0, ptr %gcs1
  %v42 = call i64 @df.box_int(i64 %v19)
  store i64 %v42, ptr %gcs1
  %sa43_0 = getelementptr [2 x i64], ptr %sargs43, i64 0, i64 0
  store i64 %v42, ptr %sa43_0
  %sat43 = getelementptr [2 x i64], ptr %sargs43, i64 0, i64 1
  store i64 0, ptr %sat43
  %sic43 = getelementptr i64, ptr %area, i64 72
  %sf43 = call ptr @df.selector(ptr %sic43, i64 %v0, i64 2870274114568068918, ptr @df.seln.0, i64 5)
  %v43 = call i64 %sf43(i64 %v0, ptr %sargs43, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v44 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v44 to i64
  %v45 = icmp ne i64 %c0, 0
  br i1 %v45, label %b20, label %b19
b7:
  %v46 = phi i64 [ %v41, %b8 ], [ %v43, %b19 ]
  store i64 %v46, ptr %gcs1
  %v47 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v47, ptr %gcs2
  %v48 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v48 to i64
  %v49 = icmp ne i64 %c1, 0
  br i1 %v49, label %b22, label %b21
b8:
  %v41 = phi i64 [ %v26, %b9 ], [ %v29, %b11 ], [ %v32, %b13 ], [ %v36, %b15 ], [ %v38, %b17 ]
  br label %b7
b9:
  %v26 = call i64 @df.word_ref(i64 %v24, i64 %v19)
  br label %b8
b10:
  %v27 = icmp eq i64 %v23, 34
  br i1 %v27, label %b11, label %b12
b11:
  %cp28 = inttoptr i64 %v24 to ptr
  %cg28 = getelementptr i64, ptr %cp28, i64 %v19
  %v28 = load i64, ptr %cg28, align 1
  store i64 0, ptr %gcs1
  %v29 = call i64 @df.box_int(i64 %v28)
  br label %b8
b12:
  %v30 = icmp eq i64 %v23, 66
  br i1 %v30, label %b13, label %b14
b13:
  %cp31 = inttoptr i64 %v24 to ptr
  %cg31 = getelementptr double, ptr %cp31, i64 %v19
  %v31 = load double, ptr %cg31, align 1
  store i64 0, ptr %gcs1
  %v32 = call i64 @df.box_double(double %v31)
  br label %b8
b14:
  %v33 = icmp eq i64 %v23, 98
  br i1 %v33, label %b15, label %b16
b15:
  %cp34 = inttoptr i64 %v24 to ptr
  %cg34 = getelementptr i64, ptr %cp34, i64 %v19
  %v34 = load i64, ptr %cg34, align 1
  %v35 = icmp ne i64 %v34, 0
  store i64 0, ptr %gcs1
  %v36 = call i64 @df.box_bool(i1 %v35)
  br label %b8
b16:
  store i64 0, ptr %gcs1
  %v37 = call i64 @df.box_int(i64 %v19)
  store i64 %v37, ptr %gcs1
  %sa38_0 = getelementptr [2 x i64], ptr %sargs38, i64 0, i64 0
  store i64 %v37, ptr %sa38_0
  %sat38 = getelementptr [2 x i64], ptr %sargs38, i64 0, i64 1
  store i64 0, ptr %sat38
  %sic38 = getelementptr i64, ptr %area, i64 74
  %sf38 = call ptr @df.selector(ptr %sic38, i64 %v0, i64 2870274114568068918, ptr @df.seln.0, i64 5)
  %v38 = call i64 %sf38(i64 %v0, ptr %sargs38, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v39 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v39 to i64
  %v40 = icmp ne i64 %c2, 0
  br i1 %v40, label %b18, label %b17
b17:
  br label %b8
b18:
  %gcvolta18 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta18, ptr %ctxtopo, align 8
  ret i64 0
b19:
  br label %b7
b20:
  %gcvolta20 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta20, ptr %ctxtopo, align 8
  ret i64 0
b21:
  %v50 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v50, ptr %gcs3
  %v51 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v51 to i64
  %v52 = icmp ne i64 %c3, 0
  br i1 %v52, label %b24, label %b23
b22:
  %gcvolta22 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta22, ptr %ctxtopo, align 8
  ret i64 0
b23:
  %v53 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..convertFormatValue(i64 %v46, i64 %v47, i64 %v50)
  store i64 %v53, ptr %gcs1
  %v54 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v54 to i64
  %v55 = icmp ne i64 %c4, 0
  br i1 %v55, label %b26, label %b25
b24:
  %gcvolta24 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta24, ptr %ctxtopo, align 8
  ret i64 0
b25:
  %v56 = call i64 @df.class(i64 %v53)
  switch i64 %v56, label %b29 [ i64 4, label %b27 i64 2, label %b28 i64 3, label %b28 ]
b26:
  %gcvolta26 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta26, ptr %ctxtopo, align 8
  ret i64 0
b27:
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  %v57 = call double @df.unbox_double(i64 %v53)
  %v58 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v58 to i64
  %v59 = icmp ne i64 %c5, 0
  br i1 %v59, label %b32, label %b31
b28:
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  %v63 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v53)
  %v64 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v64 to i64
  %v65 = icmp ne i64 %c6, 0
  br i1 %v65, label %b36, label %b35
b29:
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  %sat66 = getelementptr [1 x i64], ptr %sargs66, i64 0, i64 0
  store i64 0, ptr %sat66
  %sic66 = getelementptr i64, ptr %area, i64 76
  %sf66 = call ptr @df.selector(ptr %sic66, i64 %v53, i64 4857594363590788828, ptr @df.seln.1, i64 8)
  %v66 = call i64 %sf66(i64 %v53, ptr %sargs66, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v66, ptr %gcs1
  %v67 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v67 to i64
  %v68 = icmp ne i64 %c7, 0
  br i1 %v68, label %b38, label %b37
b30:
  %v72 = phi i64 [ %v60, %b33 ], [ %v63, %b35 ], [ %v69, %b39 ]
  %v73 = add i64 %v80, %v72
  %v76 = add i64 %v81, 1
  br label %b1
b31:
  store i64 0, ptr %gcs1
  %v60 = call i64 @dartforge_native_DartForge_double_toInt(double %v57)
  %v61 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v61 to i64
  %v62 = icmp ne i64 %c8, 0
  br i1 %v62, label %b34, label %b33
b32:
  %gcvolta32 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta32, ptr %ctxtopo, align 8
  ret i64 0
b33:
  br label %b30
b34:
  %gcvolta34 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta34, ptr %ctxtopo, align 8
  ret i64 0
b35:
  br label %b30
b36:
  %gcvolta36 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta36, ptr %ctxtopo, align 8
  ret i64 0
b37:
  %v69 = call i64 @df.unbox_int(i64 %v66)
  %v70 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v70 to i64
  %v71 = icmp ne i64 %c9, 0
  br i1 %v71, label %b40, label %b39
b38:
  %gcvolta38 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta38, ptr %ctxtopo, align 8
  ret i64 0
b39:
  br label %b30
b40:
  %gcvolta40 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta40, ptr %ctxtopo, align 8
  ret i64 0
}
