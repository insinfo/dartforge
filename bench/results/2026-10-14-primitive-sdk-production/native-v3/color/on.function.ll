define i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..convertFormatValue(i64 %v0, i64 %v3, i64 %v6) nounwind {
b0:
  %sargs16 = alloca [2 x i64]
  %sargs93 = alloca [2 x i64]
  %sargs106 = alloca [2 x i64]
  %sargs435 = alloca [2 x i64]
  %sargs186 = alloca [2 x i64]
  %sargs228 = alloca [2 x i64]
  %sargs261 = alloca [2 x i64]
  %sargs294 = alloca [2 x i64]
  %sargs327 = alloca [2 x i64]
  %sargs360 = alloca [2 x i64]
  %sargs393 = alloca [2 x i64]
  %sargs426 = alloca [2 x i64]
  %sargs750 = alloca [2 x i64]
  %sargs511 = alloca [2 x i64]
  %sargs576 = alloca [2 x i64]
  %sargs609 = alloca [2 x i64]
  %sargs642 = alloca [2 x i64]
  %sargs675 = alloca [2 x i64]
  %sargs708 = alloca [2 x i64]
  %sargs741 = alloca [2 x i64]
  %sargs1045 = alloca [2 x i64]
  %sargs826 = alloca [2 x i64]
  %sargs914 = alloca [2 x i64]
  %sargs947 = alloca [2 x i64]
  %sargs1003 = alloca [2 x i64]
  %sargs1036 = alloca [2 x i64]
  %sargs1310 = alloca [2 x i64]
  %sargs1121 = alloca [2 x i64]
  %sargs1301 = alloca [2 x i64]
  %sargs1565 = alloca [2 x i64]
  %sargs1386 = alloca [2 x i64]
  %sargs1920 = alloca [2 x i64]
  %sargs1641 = alloca [2 x i64]
  %sargs1661 = alloca [2 x i64]
  %sargs1700 = alloca [2 x i64]
  %sargs1739 = alloca [2 x i64]
  %sargs1778 = alloca [2 x i64]
  %sargs1817 = alloca [2 x i64]
  %sargs1878 = alloca [2 x i64]
  %sargs1911 = alloca [2 x i64]
  %sargs2255 = alloca [2 x i64]
  %sargs1996 = alloca [2 x i64]
  %sargs2016 = alloca [2 x i64]
  %sargs2055 = alloca [2 x i64]
  %sargs2094 = alloca [2 x i64]
  %sargs2133 = alloca [2 x i64]
  %sargs2172 = alloca [2 x i64]
  %sargs2590 = alloca [2 x i64]
  %sargs2331 = alloca [2 x i64]
  %sargs2351 = alloca [2 x i64]
  %sargs2390 = alloca [2 x i64]
  %sargs2429 = alloca [2 x i64]
  %sargs2468 = alloca [2 x i64]
  %sargs2507 = alloca [2 x i64]
  %sargs2666 = alloca [2 x i64]
  %sargs2695 = alloca [3 x i64]
  %sargs2725 = alloca [2 x i64]
  %sargs2740 = alloca [1 x i64]
  %sargs2765 = alloca [3 x i64]
  %sargs2795 = alloca [2 x i64]
  %sargs2810 = alloca [1 x i64]
  %sargs2835 = alloca [3 x i64]
  %sargs2865 = alloca [2 x i64]
  %sargs2880 = alloca [1 x i64]
  %sargs2905 = alloca [3 x i64]
  %sargs2935 = alloca [2 x i64]
  %sargs2950 = alloca [1 x i64]
  %sargs2975 = alloca [3 x i64]
  %sargs3005 = alloca [2 x i64]
  %sargs3020 = alloca [1 x i64]
  %sargs3036 = alloca [2 x i64]
  %sargs3060 = alloca [3 x i64]
  %sargs3090 = alloca [2 x i64]
  %sargs3112 = alloca [3 x i64]
  %sargs3142 = alloca [2 x i64]
  %sargs3158 = alloca [1 x i64]
  %sargs3174 = alloca [2 x i64]
  %sargs3198 = alloca [3 x i64]
  %sargs3228 = alloca [2 x i64]
  %sargs3250 = alloca [3 x i64]
  %sargs3280 = alloca [2 x i64]
  %sargs3296 = alloca [1 x i64]
  %sargs3312 = alloca [2 x i64]
  %sargs3336 = alloca [3 x i64]
  %sargs3366 = alloca [2 x i64]
  %sargs3388 = alloca [3 x i64]
  %sargs3418 = alloca [2 x i64]
  %sargs3434 = alloca [1 x i64]
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
  store i64 %v3, ptr %gcs1
  store i64 %v6, ptr %gcs2
  %v11 = icmp eq i64 %v3, 0
  %v12 = icmp eq i64 %v6, 0
  %c0 = zext i1 %v11 to i64
  %c1 = zext i1 %v12 to i64
  %v13 = or i64 %c0, %c1
  %v14 = icmp ne i64 %v13, 0
  br i1 %v14, label %b1, label %b2
b1:
  %v15 = icmp eq i64 %v3, %v6
  br label %b3
b2:
  %sa16_0 = getelementptr [2 x i64], ptr %sargs16, i64 0, i64 0
  store i64 %v6, ptr %sa16_0
  %sat16 = getelementptr [2 x i64], ptr %sargs16, i64 0, i64 1
  store i64 0, ptr %sat16
  %sic16 = getelementptr i64, ptr %area, i64 162
  %sf16 = call ptr @df.selector(ptr %sic16, i64 %v3, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v16 = call i64 %sf16(i64 %v3, ptr %sargs16, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v16, ptr %gcs3
  %v17 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c2, 0
  br i1 %v18, label %b5, label %b4
b3:
  %v22 = phi i1 [ %v15, %b1 ], [ %v19, %b6 ]
  br i1 %v22, label %b8, label %b10
b4:
  %v19 = call i1 @df.unbox_bool(i64 %v16)
  %v20 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v20 to i64
  %v21 = icmp ne i64 %c3, 0
  br i1 %v21, label %b7, label %b6
b5:
  %gcvolta5 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta5, ptr %ctxtopo, align 8
  ret i64 0
b6:
  br label %b3
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret i64 0
b8:
  %xcp24 = load i8, ptr %ctx, align 8
  %xcn24 = icmp ne i8 %xcp24, 0
  %xce24 = call i1 @llvm.expect.i1(i1 %xcn24, i1 false)
  br i1 %xce24, label %xc24.limpar, label %xc24.fim
xc24.limpar:
  call void @dartforge_exception_clear()
  br label %xc24.fim
xc24.fim:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 %v0
b10:
  store i64 0, ptr %gcs3
  %v26 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v26, ptr %gcs3
  %v27 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v27 to i64
  %v28 = icmp ne i64 %c4, 0
  br i1 %v28, label %b27, label %b26
b12:
  %gcvolta12 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta12, ptr %ctxtopo, align 8
  ret i64 0
b24:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v2599 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v2599, ptr %gcs1
  %v2600 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v2600 to i64
  %v2601 = icmp ne i64 %c5, 0
  br i1 %v2601, label %b1679, label %b1678
b25:
  store i64 0, ptr %gcs3
  %v30 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v30, ptr %gcs3
  %v31 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v31 to i64
  %v32 = icmp ne i64 %c6, 0
  br i1 %v32, label %b31, label %b30
b26:
  %v29 = icmp eq i64 %v26, %v3
  br i1 %v29, label %b28, label %b25
b27:
  %gcvolta27 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta27, ptr %ctxtopo, align 8
  ret i64 0
b28:
  %v75 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v76 = call i64 @df.box_int(i64 %v75)
  store i64 %v76, ptr %gcs1
  %v77 = icmp eq i64 %v0, 0
  %v78 = icmp eq i64 %v76, 0
  %c7 = zext i1 %v77 to i64
  %c8 = zext i1 %v78 to i64
  %v79 = or i64 %c7, %c8
  %v80 = icmp ne i64 %v79, 0
  br i1 %v80, label %b75, label %b76
b29:
  store i64 0, ptr %gcs3
  %v34 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v34, ptr %gcs3
  %v35 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v35 to i64
  %v36 = icmp ne i64 %c9, 0
  br i1 %v36, label %b35, label %b34
b30:
  %v33 = icmp eq i64 %v30, %v3
  br i1 %v33, label %b32, label %b29
b31:
  %gcvolta31 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta31, ptr %ctxtopo, align 8
  ret i64 0
b32:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v119 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v119, ptr %gcs1
  %v120 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v120 to i64
  %v121 = icmp ne i64 %c10, 0
  br i1 %v121, label %b121, label %b120
b33:
  store i64 0, ptr %gcs3
  %v38 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v38, ptr %gcs3
  %v39 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v39 to i64
  %v40 = icmp ne i64 %c11, 0
  br i1 %v40, label %b39, label %b38
b34:
  %v37 = icmp eq i64 %v34, %v3
  br i1 %v37, label %b36, label %b33
b35:
  %gcvolta35 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta35, ptr %ctxtopo, align 8
  ret i64 0
b36:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v444 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v444, ptr %gcs1
  %v445 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v445 to i64
  %v446 = icmp ne i64 %c12, 0
  br i1 %v446, label %b324, label %b323
b37:
  store i64 0, ptr %gcs3
  %v42 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v42, ptr %gcs3
  %v43 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v43 to i64
  %v44 = icmp ne i64 %c13, 0
  br i1 %v44, label %b43, label %b42
b38:
  %v41 = icmp eq i64 %v38, %v3
  br i1 %v41, label %b40, label %b37
b39:
  %gcvolta39 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta39, ptr %ctxtopo, align 8
  ret i64 0
b40:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v759 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v759, ptr %gcs1
  %v760 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v760 to i64
  %v761 = icmp ne i64 %c14, 0
  br i1 %v761, label %b521, label %b520
b41:
  store i64 0, ptr %gcs3
  %v46 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v46, ptr %gcs3
  %v47 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v47 to i64
  %v48 = icmp ne i64 %c15, 0
  br i1 %v48, label %b47, label %b46
b42:
  %v45 = icmp eq i64 %v42, %v3
  br i1 %v45, label %b44, label %b41
b43:
  %gcvolta43 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta43, ptr %ctxtopo, align 8
  ret i64 0
b44:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v1054 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1054, ptr %gcs1
  %v1055 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v1055 to i64
  %v1056 = icmp ne i64 %c16, 0
  br i1 %v1056, label %b706, label %b705
b45:
  store i64 0, ptr %gcs3
  %v50 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v50, ptr %gcs3
  %v51 = load i8, ptr %ctx, align 8
  %c17 = zext i8 %v51 to i64
  %v52 = icmp ne i64 %c17, 0
  br i1 %v52, label %b51, label %b50
b46:
  %v49 = icmp eq i64 %v46, %v3
  br i1 %v49, label %b48, label %b45
b47:
  %gcvolta47 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta47, ptr %ctxtopo, align 8
  ret i64 0
b48:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v1319 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1319, ptr %gcs1
  %v1320 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v1320 to i64
  %v1321 = icmp ne i64 %c18, 0
  br i1 %v1321, label %b873, label %b872
b49:
  store i64 0, ptr %gcs3
  %v54 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v54, ptr %gcs3
  %v55 = load i8, ptr %ctx, align 8
  %c19 = zext i8 %v55 to i64
  %v56 = icmp ne i64 %c19, 0
  br i1 %v56, label %b55, label %b54
b50:
  %v53 = icmp eq i64 %v50, %v3
  br i1 %v53, label %b52, label %b49
b51:
  %gcvolta51 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta51, ptr %ctxtopo, align 8
  ret i64 0
b52:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v1574 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1574, ptr %gcs1
  %v1575 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v1575 to i64
  %v1576 = icmp ne i64 %c20, 0
  br i1 %v1576, label %b1034, label %b1033
b53:
  store i64 0, ptr %gcs3
  %v58 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v58, ptr %gcs3
  %v59 = load i8, ptr %ctx, align 8
  %c21 = zext i8 %v59 to i64
  %v60 = icmp ne i64 %c21, 0
  br i1 %v60, label %b59, label %b58
b54:
  %v57 = icmp eq i64 %v54, %v3
  br i1 %v57, label %b56, label %b53
b55:
  %gcvolta55 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta55, ptr %ctxtopo, align 8
  ret i64 0
b56:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v1929 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1929, ptr %gcs1
  %v1930 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v1930 to i64
  %v1931 = icmp ne i64 %c22, 0
  br i1 %v1931, label %b1257, label %b1256
b57:
  store i64 0, ptr %gcs3
  %v62 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v62, ptr %gcs3
  %v63 = load i8, ptr %ctx, align 8
  %c23 = zext i8 %v63 to i64
  %v64 = icmp ne i64 %c23, 0
  br i1 %v64, label %b63, label %b62
b58:
  %v61 = icmp eq i64 %v58, %v3
  br i1 %v61, label %b60, label %b57
b59:
  %gcvolta59 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta59, ptr %ctxtopo, align 8
  ret i64 0
b60:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs3
  %v2264 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v2264, ptr %gcs1
  %v2265 = load i8, ptr %ctx, align 8
  %c24 = zext i8 %v2265 to i64
  %v2266 = icmp ne i64 %c24, 0
  br i1 %v2266, label %b1468, label %b1467
b61:
  store i64 0, ptr %gcs3
  %v66 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v66, ptr %gcs3
  %v67 = load i8, ptr %ctx, align 8
  %c25 = zext i8 %v67 to i64
  %v68 = icmp ne i64 %c25, 0
  br i1 %v68, label %b68, label %b67
b62:
  %v65 = icmp eq i64 %v62, %v3
  br i1 %v65, label %b64, label %b61
b63:
  %gcvolta63 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta63, ptr %ctxtopo, align 8
  ret i64 0
b64:
  br label %b24
b66:
  store i64 0, ptr %gcs3
  %v70 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v70, ptr %gcs3
  %v71 = load i8, ptr %ctx, align 8
  %c26 = zext i8 %v71 to i64
  %v72 = icmp ne i64 %c26, 0
  br i1 %v72, label %b73, label %b72
b67:
  %v69 = icmp eq i64 %v66, %v3
  br i1 %v69, label %b69, label %b66
b68:
  %gcvolta68 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta68, ptr %ctxtopo, align 8
  ret i64 0
b69:
  br label %b24
b71:
  br label %b12
b72:
  %v73 = icmp eq i64 %v70, %v3
  br i1 %v73, label %b74, label %b71
b73:
  %gcvolta73 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta73, ptr %ctxtopo, align 8
  ret i64 0
b74:
  br label %b24
b75:
  %v81 = icmp eq i64 %v0, %v76
  br label %b77
b76:
  %v82 = call i64 @df.class(i64 %v0)
  switch i64 %v82, label %b81 [ i64 675, label %b78 i64 4, label %b79 i64 2, label %b80 i64 3, label %b80 ]
b77:
  %v100 = phi i1 [ %v81, %b75 ], [ %v99, %b82 ]
  br i1 %v100, label %b93, label %b94
b78:
  %v83 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v76)
  %v84 = load i8, ptr %ctx, align 8
  %c27 = zext i8 %v84 to i64
  %v85 = icmp ne i64 %c27, 0
  br i1 %v85, label %b84, label %b83
b79:
  %v87 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v76)
  %v88 = load i8, ptr %ctx, align 8
  %c28 = zext i8 %v88 to i64
  %v89 = icmp ne i64 %c28, 0
  br i1 %v89, label %b86, label %b85
b80:
  %v90 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v76)
  %v91 = load i8, ptr %ctx, align 8
  %c29 = zext i8 %v91 to i64
  %v92 = icmp ne i64 %c29, 0
  br i1 %v92, label %b88, label %b87
b81:
  %sa93_0 = getelementptr [2 x i64], ptr %sargs93, i64 0, i64 0
  store i64 %v76, ptr %sa93_0
  %sat93 = getelementptr [2 x i64], ptr %sargs93, i64 0, i64 1
  store i64 0, ptr %sat93
  %sic93 = getelementptr i64, ptr %area, i64 164
  %sf93 = call ptr @df.selector(ptr %sic93, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v93 = call i64 %sf93(i64 %v0, ptr %sargs93, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v93, ptr %gcs0
  %v94 = load i8, ptr %ctx, align 8
  %c30 = zext i8 %v94 to i64
  %v95 = icmp ne i64 %c30, 0
  br i1 %v95, label %b90, label %b89
b82:
  %v99 = phi i1 [ %v86, %b83 ], [ %v87, %b85 ], [ %v90, %b87 ], [ %v96, %b91 ]
  br label %b77
b83:
  %c31 = zext i8 %v83 to i64
  %v86 = icmp ne i64 %c31, 0
  br label %b82
b84:
  %gcvolta84 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta84, ptr %ctxtopo, align 8
  ret i64 0
b85:
  br label %b82
b86:
  %gcvolta86 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta86, ptr %ctxtopo, align 8
  ret i64 0
b87:
  br label %b82
b88:
  %gcvolta88 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta88, ptr %ctxtopo, align 8
  ret i64 0
b89:
  store i64 0, ptr %gcs1
  %v96 = call i1 @df.unbox_bool(i64 %v93)
  %v97 = load i8, ptr %ctx, align 8
  %c32 = zext i8 %v97 to i64
  %v98 = icmp ne i64 %c32, 0
  br i1 %v98, label %b92, label %b91
b90:
  %gcvolta90 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta90, ptr %ctxtopo, align 8
  ret i64 0
b91:
  br label %b82
b92:
  %gcvolta92 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta92, ptr %ctxtopo, align 8
  ret i64 0
b93:
  %v101 = add i64 0, 0
  br label %b95
b94:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v102 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..formatMaxValue()
  store i64 %v102, ptr %gcs0
  %v103 = load i8, ptr %ctx, align 8
  %c33 = zext i8 %v103 to i64
  %v104 = icmp ne i64 %c33, 0
  br i1 %v104, label %b97, label %b96
b95:
  %v115 = phi i64 [ %v101, %b93 ], [ %v112, %b103 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v116 = call i64 @df.box_int(i64 %v115)
  %xcp117 = load i8, ptr %ctx, align 8
  %xcn117 = icmp ne i8 %xcp117, 0
  %xce117 = call i1 @llvm.expect.i1(i1 %xcn117, i1 false)
  br i1 %xce117, label %xc117.limpar, label %xc117.fim
xc117.limpar:
  call void @dartforge_exception_clear()
  br label %xc117.fim
xc117.fim:
  %gcvolta95 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta95, ptr %ctxtopo, align 8
  ret i64 %v116
b96:
  %sa106_0 = getelementptr [2 x i64], ptr %sargs106, i64 0, i64 0
  store i64 %v6, ptr %sa106_0
  %sat106 = getelementptr [2 x i64], ptr %sargs106, i64 0, i64 1
  store i64 0, ptr %sat106
  %sic106 = getelementptr i64, ptr %area, i64 166
  %sf106 = call ptr @df.selector(ptr %sic106, i64 %v102, i64 2870274114568068918, ptr @df.seln.5, i64 5)
  %v106 = call i64 %sf106(i64 %v102, ptr %sargs106, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v106, ptr %gcs0
  %v107 = load i8, ptr %ctx, align 8
  %c34 = zext i8 %v107 to i64
  %v108 = icmp ne i64 %c34, 0
  br i1 %v108, label %b99, label %b98
b97:
  %gcvolta97 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta97, ptr %ctxtopo, align 8
  ret i64 0
b98:
  store i64 0, ptr %gcs2
  %v109 = icmp eq i64 %v106, 0
  br i1 %v109, label %b100, label %b101
b99:
  %gcvolta99 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta99, ptr %ctxtopo, align 8
  ret i64 0
b100:
  store i64 0, ptr %gcs0
  %v110 = call i64 @dartforge_null_check_error_new()
  store i64 %v110, ptr %gcs0
  call void @dartforge_exception_throw(i64 %v110, i8 3)
  %gcvolta100 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta100, ptr %ctxtopo, align 8
  ret i64 0
b101:
  %v112 = call i64 @df.unbox_int(i64 %v106)
  %v113 = load i8, ptr %ctx, align 8
  %c35 = zext i8 %v113 to i64
  %v114 = icmp ne i64 %c35, 0
  br i1 %v114, label %b104, label %b103
b103:
  br label %b95
b104:
  %gcvolta104 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta104, ptr %ctxtopo, align 8
  ret i64 0
b118:
  %v433 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v434 = call i64 @df.box_int(i64 %v433)
  store i64 %v434, ptr %gcs1
  %sa435_0 = getelementptr [2 x i64], ptr %sargs435, i64 0, i64 0
  store i64 %v434, ptr %sa435_0
  %sat435 = getelementptr [2 x i64], ptr %sargs435, i64 0, i64 1
  store i64 0, ptr %sat435
  %sic435 = getelementptr i64, ptr %area, i64 168
  %sf435 = call ptr @df.selector(ptr %sic435, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v435 = call i64 %sf435(i64 %v0, ptr %sargs435, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v435, ptr %gcs0
  %v436 = load i8, ptr %ctx, align 8
  %c36 = zext i8 %v436 to i64
  %v437 = icmp ne i64 %c36, 0
  br i1 %v437, label %b305, label %b304
b119:
  store i64 0, ptr %gcs1
  %v123 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v123, ptr %gcs1
  %v124 = load i8, ptr %ctx, align 8
  %c37 = zext i8 %v124 to i64
  %v125 = icmp ne i64 %c37, 0
  br i1 %v125, label %b125, label %b124
b120:
  %v122 = icmp eq i64 %v119, %v6
  br i1 %v122, label %b122, label %b119
b121:
  %gcvolta121 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta121, ptr %ctxtopo, align 8
  ret i64 0
b122:
  %v168 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v169 = call i64 @df.box_int(i64 %v168)
  store i64 %v169, ptr %gcs1
  %v170 = icmp eq i64 %v0, 0
  %v171 = icmp eq i64 %v169, 0
  %c38 = zext i1 %v170 to i64
  %c39 = zext i1 %v171 to i64
  %v172 = or i64 %c38, %c39
  %v173 = icmp ne i64 %v172, 0
  br i1 %v173, label %b169, label %b170
b123:
  store i64 0, ptr %gcs1
  %v127 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v127, ptr %gcs1
  %v128 = load i8, ptr %ctx, align 8
  %c40 = zext i8 %v128 to i64
  %v129 = icmp ne i64 %c40, 0
  br i1 %v129, label %b129, label %b128
b124:
  %v126 = icmp eq i64 %v123, %v6
  br i1 %v126, label %b126, label %b123
b125:
  %gcvolta125 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta125, ptr %ctxtopo, align 8
  ret i64 0
b126:
  %xcp200 = load i8, ptr %ctx, align 8
  %xcn200 = icmp ne i8 %xcp200, 0
  %xce200 = call i1 @llvm.expect.i1(i1 %xcn200, i1 false)
  br i1 %xce200, label %xc200.limpar, label %xc200.fim
xc200.limpar:
  call void @dartforge_exception_clear()
  br label %xc200.fim
xc200.fim:
  %gcvolta126 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta126, ptr %ctxtopo, align 8
  ret i64 %v0
b127:
  store i64 0, ptr %gcs1
  %v131 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v131, ptr %gcs1
  %v132 = load i8, ptr %ctx, align 8
  %c41 = zext i8 %v132 to i64
  %v133 = icmp ne i64 %c41, 0
  br i1 %v133, label %b133, label %b132
b128:
  %v130 = icmp eq i64 %v127, %v6
  br i1 %v130, label %b130, label %b127
b129:
  %gcvolta129 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta129, ptr %ctxtopo, align 8
  ret i64 0
b130:
  %v202 = add i64 0, 5
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v203 = and i64 %v0, 1
  %v204 = icmp ne i64 %v203, 0
  br i1 %v204, label %b192, label %b193
b131:
  store i64 0, ptr %gcs1
  %v135 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v135, ptr %gcs1
  %v136 = load i8, ptr %ctx, align 8
  %c42 = zext i8 %v136 to i64
  %v137 = icmp ne i64 %c42, 0
  br i1 %v137, label %b137, label %b136
b132:
  %v134 = icmp eq i64 %v131, %v6
  br i1 %v134, label %b134, label %b131
b133:
  %gcvolta133 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta133, ptr %ctxtopo, align 8
  ret i64 0
b134:
  %v235 = add i64 0, 75
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v236 = and i64 %v0, 1
  %v237 = icmp ne i64 %v236, 0
  br i1 %v237, label %b208, label %b209
b135:
  store i64 0, ptr %gcs1
  %v139 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v139, ptr %gcs1
  %v140 = load i8, ptr %ctx, align 8
  %c43 = zext i8 %v140 to i64
  %v141 = icmp ne i64 %c43, 0
  br i1 %v141, label %b141, label %b140
b136:
  %v138 = icmp eq i64 %v135, %v6
  br i1 %v138, label %b138, label %b135
b137:
  %gcvolta137 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta137, ptr %ctxtopo, align 8
  ret i64 0
b138:
  %v268 = add i64 0, 21845
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v269 = and i64 %v0, 1
  %v270 = icmp ne i64 %v269, 0
  br i1 %v270, label %b224, label %b225
b139:
  store i64 0, ptr %gcs1
  %v143 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v143, ptr %gcs1
  %v144 = load i8, ptr %ctx, align 8
  %c44 = zext i8 %v144 to i64
  %v145 = icmp ne i64 %c44, 0
  br i1 %v145, label %b145, label %b144
b140:
  %v142 = icmp eq i64 %v139, %v6
  br i1 %v142, label %b142, label %b139
b141:
  %gcvolta141 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta141, ptr %ctxtopo, align 8
  ret i64 0
b142:
  %v301 = add i64 0, 1431655765
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v302 = and i64 %v0, 1
  %v303 = icmp ne i64 %v302, 0
  br i1 %v303, label %b240, label %b241
b143:
  store i64 0, ptr %gcs1
  %v147 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v147, ptr %gcs1
  %v148 = load i8, ptr %ctx, align 8
  %c45 = zext i8 %v148 to i64
  %v149 = icmp ne i64 %c45, 0
  br i1 %v149, label %b149, label %b148
b144:
  %v146 = icmp eq i64 %v143, %v6
  br i1 %v146, label %b146, label %b143
b145:
  %gcvolta145 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta145, ptr %ctxtopo, align 8
  ret i64 0
b146:
  %v334 = add i64 0, 42
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v335 = and i64 %v0, 1
  %v336 = icmp ne i64 %v335, 0
  br i1 %v336, label %b256, label %b257
b147:
  store i64 0, ptr %gcs1
  %v151 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v151, ptr %gcs1
  %v152 = load i8, ptr %ctx, align 8
  %c46 = zext i8 %v152 to i64
  %v153 = icmp ne i64 %c46, 0
  br i1 %v153, label %b153, label %b152
b148:
  %v150 = icmp eq i64 %v147, %v6
  br i1 %v150, label %b150, label %b147
b149:
  %gcvolta149 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta149, ptr %ctxtopo, align 8
  ret i64 0
b150:
  %v367 = add i64 0, 10922
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v368 = and i64 %v0, 1
  %v369 = icmp ne i64 %v368, 0
  br i1 %v369, label %b272, label %b273
b151:
  store i64 0, ptr %gcs1
  %v155 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v155, ptr %gcs1
  %v156 = load i8, ptr %ctx, align 8
  %c47 = zext i8 %v156 to i64
  %v157 = icmp ne i64 %c47, 0
  br i1 %v157, label %b157, label %b156
b152:
  %v154 = icmp eq i64 %v151, %v6
  br i1 %v154, label %b154, label %b151
b153:
  %gcvolta153 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta153, ptr %ctxtopo, align 8
  ret i64 0
b154:
  %v400 = add i64 0, 715827882
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v401 = and i64 %v0, 1
  %v402 = icmp ne i64 %v401, 0
  br i1 %v402, label %b288, label %b289
b155:
  store i64 0, ptr %gcs1
  %v159 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v159, ptr %gcs1
  %v160 = load i8, ptr %ctx, align 8
  %c48 = zext i8 %v160 to i64
  %v161 = icmp ne i64 %c48, 0
  br i1 %v161, label %b162, label %b161
b156:
  %v158 = icmp eq i64 %v155, %v6
  br i1 %v158, label %b158, label %b155
b157:
  %gcvolta157 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta157, ptr %ctxtopo, align 8
  ret i64 0
b158:
  br label %b118
b160:
  store i64 0, ptr %gcs1
  %v163 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v163, ptr %gcs1
  %v164 = load i8, ptr %ctx, align 8
  %c49 = zext i8 %v164 to i64
  %v165 = icmp ne i64 %c49, 0
  br i1 %v165, label %b167, label %b166
b161:
  %v162 = icmp eq i64 %v159, %v6
  br i1 %v162, label %b163, label %b160
b162:
  %gcvolta162 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta162, ptr %ctxtopo, align 8
  ret i64 0
b163:
  br label %b118
b165:
  br label %b12
b166:
  %v166 = icmp eq i64 %v163, %v6
  br i1 %v166, label %b168, label %b165
b167:
  %gcvolta167 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta167, ptr %ctxtopo, align 8
  ret i64 0
b168:
  br label %b118
b169:
  %v174 = icmp eq i64 %v0, %v169
  br label %b171
b170:
  %v175 = call i64 @df.class(i64 %v0)
  switch i64 %v175, label %b175 [ i64 675, label %b172 i64 4, label %b173 i64 2, label %b174 i64 3, label %b174 ]
b171:
  %v193 = phi i1 [ %v174, %b169 ], [ %v192, %b176 ]
  br i1 %v193, label %b187, label %b188
b172:
  %v176 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v169)
  %v177 = load i8, ptr %ctx, align 8
  %c50 = zext i8 %v177 to i64
  %v178 = icmp ne i64 %c50, 0
  br i1 %v178, label %b178, label %b177
b173:
  %v180 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v169)
  %v181 = load i8, ptr %ctx, align 8
  %c51 = zext i8 %v181 to i64
  %v182 = icmp ne i64 %c51, 0
  br i1 %v182, label %b180, label %b179
b174:
  %v183 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v169)
  %v184 = load i8, ptr %ctx, align 8
  %c52 = zext i8 %v184 to i64
  %v185 = icmp ne i64 %c52, 0
  br i1 %v185, label %b182, label %b181
b175:
  %sa186_0 = getelementptr [2 x i64], ptr %sargs186, i64 0, i64 0
  store i64 %v169, ptr %sa186_0
  %sat186 = getelementptr [2 x i64], ptr %sargs186, i64 0, i64 1
  store i64 0, ptr %sat186
  %sic186 = getelementptr i64, ptr %area, i64 170
  %sf186 = call ptr @df.selector(ptr %sic186, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v186 = call i64 %sf186(i64 %v0, ptr %sargs186, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v186, ptr %gcs0
  %v187 = load i8, ptr %ctx, align 8
  %c53 = zext i8 %v187 to i64
  %v188 = icmp ne i64 %c53, 0
  br i1 %v188, label %b184, label %b183
b176:
  %v192 = phi i1 [ %v179, %b177 ], [ %v180, %b179 ], [ %v183, %b181 ], [ %v189, %b185 ]
  br label %b171
b177:
  %c54 = zext i8 %v176 to i64
  %v179 = icmp ne i64 %c54, 0
  br label %b176
b178:
  %gcvolta178 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta178, ptr %ctxtopo, align 8
  ret i64 0
b179:
  br label %b176
b180:
  %gcvolta180 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta180, ptr %ctxtopo, align 8
  ret i64 0
b181:
  br label %b176
b182:
  %gcvolta182 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta182, ptr %ctxtopo, align 8
  ret i64 0
b183:
  store i64 0, ptr %gcs1
  %v189 = call i1 @df.unbox_bool(i64 %v186)
  %v190 = load i8, ptr %ctx, align 8
  %c55 = zext i8 %v190 to i64
  %v191 = icmp ne i64 %c55, 0
  br i1 %v191, label %b186, label %b185
b184:
  %gcvolta184 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta184, ptr %ctxtopo, align 8
  ret i64 0
b185:
  br label %b176
b186:
  %gcvolta186 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta186, ptr %ctxtopo, align 8
  ret i64 0
b187:
  %v194 = add i64 0, 0
  br label %b189
b188:
  %v195 = add i64 0, 1
  br label %b189
b189:
  %v196 = phi i64 [ %v194, %b187 ], [ %v195, %b188 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v197 = call i64 @df.box_int(i64 %v196)
  %xcp198 = load i8, ptr %ctx, align 8
  %xcn198 = icmp ne i8 %xcp198, 0
  %xce198 = call i1 @llvm.expect.i1(i1 %xcn198, i1 false)
  br i1 %xce198, label %xc198.limpar, label %xc198.fim
xc198.limpar:
  call void @dartforge_exception_clear()
  br label %xc198.fim
xc198.fim:
  %gcvolta189 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta189, ptr %ctxtopo, align 8
  ret i64 %v197
b192:
  %v205 = ashr i64 %v0, 1
  %v206 = mul i64 %v205, %v202
  store i64 0, ptr %gcs0
  %v207 = call i64 @df.box_int(i64 %v206)
  br label %b194
b193:
  %v208 = call i64 @df.class(i64 %v0)
  %v209 = icmp eq i64 %v208, 2
  %v210 = icmp eq i64 %v208, 3
  %v211 = icmp eq i64 %v208, 4
  %c56 = zext i1 %v209 to i64
  %c57 = zext i1 %v210 to i64
  %v212 = or i64 %c56, %c57
  %c58 = zext i1 %v211 to i64
  %v213 = or i64 %v212, %c58
  %c59 = trunc i64 %v213 to i1
  br i1 %c59, label %b195, label %b196
b194:
  %v232 = phi i64 [ %v207, %b192 ], [ %v231, %b197 ]
  %xcp233 = load i8, ptr %ctx, align 8
  %xcn233 = icmp ne i8 %xcp233, 0
  %xce233 = call i1 @llvm.expect.i1(i1 %xcn233, i1 false)
  br i1 %xce233, label %xc233.limpar, label %xc233.fim
xc233.limpar:
  call void @dartforge_exception_clear()
  br label %xc233.fim
xc233.fim:
  %gcvolta194 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta194, ptr %ctxtopo, align 8
  ret i64 %v232
b195:
  br i1 %v211, label %b199, label %b198
b196:
  %v227 = call i64 @df.box_int(i64 %v202)
  store i64 %v227, ptr %gcs1
  %sa228_0 = getelementptr [2 x i64], ptr %sargs228, i64 0, i64 0
  store i64 %v227, ptr %sa228_0
  %sat228 = getelementptr [2 x i64], ptr %sargs228, i64 0, i64 1
  store i64 0, ptr %sat228
  %sic228 = getelementptr i64, ptr %area, i64 172
  %sf228 = call ptr @df.selector(ptr %sic228, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v228 = call i64 %sf228(i64 %v0, ptr %sargs228, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v229 = load i8, ptr %ctx, align 8
  %c60 = zext i8 %v229 to i64
  %v230 = icmp ne i64 %c60, 0
  br i1 %v230, label %b206, label %b205
b197:
  %v231 = phi i64 [ %v226, %b200 ], [ %v228, %b205 ]
  br label %b194
b198:
  %v215 = call i64 @df.unbox_int(i64 %v0)
  %v216 = load i8, ptr %ctx, align 8
  %c61 = zext i8 %v216 to i64
  %v217 = icmp ne i64 %c61, 0
  br i1 %v217, label %b202, label %b201
b199:
  %v220 = call double @df.num_to_double(i64 %v0)
  %v221 = load i8, ptr %ctx, align 8
  %c62 = zext i8 %v221 to i64
  %v222 = icmp ne i64 %c62, 0
  br i1 %v222, label %b204, label %b203
b200:
  %v226 = phi i64 [ %v219, %b201 ], [ %v225, %b203 ]
  br label %b197
b201:
  %v218 = mul i64 %v215, %v202
  store i64 0, ptr %gcs0
  %v219 = call i64 @df.box_int(i64 %v218)
  br label %b200
b202:
  %gcvolta202 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta202, ptr %ctxtopo, align 8
  ret i64 0
b203:
  %v223 = sitofp i64 %v202 to double
  %v224 = fmul double %v220, %v223
  store i64 0, ptr %gcs0
  %v225 = call i64 @df.box_double(double %v224)
  br label %b200
b204:
  %gcvolta204 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta204, ptr %ctxtopo, align 8
  ret i64 0
b205:
  br label %b197
b206:
  %gcvolta206 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta206, ptr %ctxtopo, align 8
  ret i64 0
b208:
  %v238 = ashr i64 %v0, 1
  %v239 = mul i64 %v238, %v235
  store i64 0, ptr %gcs0
  %v240 = call i64 @df.box_int(i64 %v239)
  br label %b210
b209:
  %v241 = call i64 @df.class(i64 %v0)
  %v242 = icmp eq i64 %v241, 2
  %v243 = icmp eq i64 %v241, 3
  %v244 = icmp eq i64 %v241, 4
  %c63 = zext i1 %v242 to i64
  %c64 = zext i1 %v243 to i64
  %v245 = or i64 %c63, %c64
  %c65 = zext i1 %v244 to i64
  %v246 = or i64 %v245, %c65
  %c66 = trunc i64 %v246 to i1
  br i1 %c66, label %b211, label %b212
b210:
  %v265 = phi i64 [ %v240, %b208 ], [ %v264, %b213 ]
  %xcp266 = load i8, ptr %ctx, align 8
  %xcn266 = icmp ne i8 %xcp266, 0
  %xce266 = call i1 @llvm.expect.i1(i1 %xcn266, i1 false)
  br i1 %xce266, label %xc266.limpar, label %xc266.fim
xc266.limpar:
  call void @dartforge_exception_clear()
  br label %xc266.fim
xc266.fim:
  %gcvolta210 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta210, ptr %ctxtopo, align 8
  ret i64 %v265
b211:
  br i1 %v244, label %b215, label %b214
b212:
  %v260 = call i64 @df.box_int(i64 %v235)
  store i64 %v260, ptr %gcs1
  %sa261_0 = getelementptr [2 x i64], ptr %sargs261, i64 0, i64 0
  store i64 %v260, ptr %sa261_0
  %sat261 = getelementptr [2 x i64], ptr %sargs261, i64 0, i64 1
  store i64 0, ptr %sat261
  %sic261 = getelementptr i64, ptr %area, i64 174
  %sf261 = call ptr @df.selector(ptr %sic261, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v261 = call i64 %sf261(i64 %v0, ptr %sargs261, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v262 = load i8, ptr %ctx, align 8
  %c67 = zext i8 %v262 to i64
  %v263 = icmp ne i64 %c67, 0
  br i1 %v263, label %b222, label %b221
b213:
  %v264 = phi i64 [ %v259, %b216 ], [ %v261, %b221 ]
  br label %b210
b214:
  %v248 = call i64 @df.unbox_int(i64 %v0)
  %v249 = load i8, ptr %ctx, align 8
  %c68 = zext i8 %v249 to i64
  %v250 = icmp ne i64 %c68, 0
  br i1 %v250, label %b218, label %b217
b215:
  %v253 = call double @df.num_to_double(i64 %v0)
  %v254 = load i8, ptr %ctx, align 8
  %c69 = zext i8 %v254 to i64
  %v255 = icmp ne i64 %c69, 0
  br i1 %v255, label %b220, label %b219
b216:
  %v259 = phi i64 [ %v252, %b217 ], [ %v258, %b219 ]
  br label %b213
b217:
  %v251 = mul i64 %v248, %v235
  store i64 0, ptr %gcs0
  %v252 = call i64 @df.box_int(i64 %v251)
  br label %b216
b218:
  %gcvolta218 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta218, ptr %ctxtopo, align 8
  ret i64 0
b219:
  %v256 = sitofp i64 %v235 to double
  %v257 = fmul double %v253, %v256
  store i64 0, ptr %gcs0
  %v258 = call i64 @df.box_double(double %v257)
  br label %b216
b220:
  %gcvolta220 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta220, ptr %ctxtopo, align 8
  ret i64 0
b221:
  br label %b213
b222:
  %gcvolta222 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta222, ptr %ctxtopo, align 8
  ret i64 0
b224:
  %v271 = ashr i64 %v0, 1
  %v272 = mul i64 %v271, %v268
  store i64 0, ptr %gcs0
  %v273 = call i64 @df.box_int(i64 %v272)
  br label %b226
b225:
  %v274 = call i64 @df.class(i64 %v0)
  %v275 = icmp eq i64 %v274, 2
  %v276 = icmp eq i64 %v274, 3
  %v277 = icmp eq i64 %v274, 4
  %c70 = zext i1 %v275 to i64
  %c71 = zext i1 %v276 to i64
  %v278 = or i64 %c70, %c71
  %c72 = zext i1 %v277 to i64
  %v279 = or i64 %v278, %c72
  %c73 = trunc i64 %v279 to i1
  br i1 %c73, label %b227, label %b228
b226:
  %v298 = phi i64 [ %v273, %b224 ], [ %v297, %b229 ]
  %xcp299 = load i8, ptr %ctx, align 8
  %xcn299 = icmp ne i8 %xcp299, 0
  %xce299 = call i1 @llvm.expect.i1(i1 %xcn299, i1 false)
  br i1 %xce299, label %xc299.limpar, label %xc299.fim
xc299.limpar:
  call void @dartforge_exception_clear()
  br label %xc299.fim
xc299.fim:
  %gcvolta226 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta226, ptr %ctxtopo, align 8
  ret i64 %v298
b227:
  br i1 %v277, label %b231, label %b230
b228:
  %v293 = call i64 @df.box_int(i64 %v268)
  store i64 %v293, ptr %gcs1
  %sa294_0 = getelementptr [2 x i64], ptr %sargs294, i64 0, i64 0
  store i64 %v293, ptr %sa294_0
  %sat294 = getelementptr [2 x i64], ptr %sargs294, i64 0, i64 1
  store i64 0, ptr %sat294
  %sic294 = getelementptr i64, ptr %area, i64 176
  %sf294 = call ptr @df.selector(ptr %sic294, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v294 = call i64 %sf294(i64 %v0, ptr %sargs294, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v295 = load i8, ptr %ctx, align 8
  %c74 = zext i8 %v295 to i64
  %v296 = icmp ne i64 %c74, 0
  br i1 %v296, label %b238, label %b237
b229:
  %v297 = phi i64 [ %v292, %b232 ], [ %v294, %b237 ]
  br label %b226
b230:
  %v281 = call i64 @df.unbox_int(i64 %v0)
  %v282 = load i8, ptr %ctx, align 8
  %c75 = zext i8 %v282 to i64
  %v283 = icmp ne i64 %c75, 0
  br i1 %v283, label %b234, label %b233
b231:
  %v286 = call double @df.num_to_double(i64 %v0)
  %v287 = load i8, ptr %ctx, align 8
  %c76 = zext i8 %v287 to i64
  %v288 = icmp ne i64 %c76, 0
  br i1 %v288, label %b236, label %b235
b232:
  %v292 = phi i64 [ %v285, %b233 ], [ %v291, %b235 ]
  br label %b229
b233:
  %v284 = mul i64 %v281, %v268
  store i64 0, ptr %gcs0
  %v285 = call i64 @df.box_int(i64 %v284)
  br label %b232
b234:
  %gcvolta234 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta234, ptr %ctxtopo, align 8
  ret i64 0
b235:
  %v289 = sitofp i64 %v268 to double
  %v290 = fmul double %v286, %v289
  store i64 0, ptr %gcs0
  %v291 = call i64 @df.box_double(double %v290)
  br label %b232
b236:
  %gcvolta236 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta236, ptr %ctxtopo, align 8
  ret i64 0
b237:
  br label %b229
b238:
  %gcvolta238 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta238, ptr %ctxtopo, align 8
  ret i64 0
b240:
  %v304 = ashr i64 %v0, 1
  %v305 = mul i64 %v304, %v301
  store i64 0, ptr %gcs0
  %v306 = call i64 @df.box_int(i64 %v305)
  br label %b242
b241:
  %v307 = call i64 @df.class(i64 %v0)
  %v308 = icmp eq i64 %v307, 2
  %v309 = icmp eq i64 %v307, 3
  %v310 = icmp eq i64 %v307, 4
  %c77 = zext i1 %v308 to i64
  %c78 = zext i1 %v309 to i64
  %v311 = or i64 %c77, %c78
  %c79 = zext i1 %v310 to i64
  %v312 = or i64 %v311, %c79
  %c80 = trunc i64 %v312 to i1
  br i1 %c80, label %b243, label %b244
b242:
  %v331 = phi i64 [ %v306, %b240 ], [ %v330, %b245 ]
  %xcp332 = load i8, ptr %ctx, align 8
  %xcn332 = icmp ne i8 %xcp332, 0
  %xce332 = call i1 @llvm.expect.i1(i1 %xcn332, i1 false)
  br i1 %xce332, label %xc332.limpar, label %xc332.fim
xc332.limpar:
  call void @dartforge_exception_clear()
  br label %xc332.fim
xc332.fim:
  %gcvolta242 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta242, ptr %ctxtopo, align 8
  ret i64 %v331
b243:
  br i1 %v310, label %b247, label %b246
b244:
  %v326 = call i64 @df.box_int(i64 %v301)
  store i64 %v326, ptr %gcs1
  %sa327_0 = getelementptr [2 x i64], ptr %sargs327, i64 0, i64 0
  store i64 %v326, ptr %sa327_0
  %sat327 = getelementptr [2 x i64], ptr %sargs327, i64 0, i64 1
  store i64 0, ptr %sat327
  %sic327 = getelementptr i64, ptr %area, i64 178
  %sf327 = call ptr @df.selector(ptr %sic327, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v327 = call i64 %sf327(i64 %v0, ptr %sargs327, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v328 = load i8, ptr %ctx, align 8
  %c81 = zext i8 %v328 to i64
  %v329 = icmp ne i64 %c81, 0
  br i1 %v329, label %b254, label %b253
b245:
  %v330 = phi i64 [ %v325, %b248 ], [ %v327, %b253 ]
  br label %b242
b246:
  %v314 = call i64 @df.unbox_int(i64 %v0)
  %v315 = load i8, ptr %ctx, align 8
  %c82 = zext i8 %v315 to i64
  %v316 = icmp ne i64 %c82, 0
  br i1 %v316, label %b250, label %b249
b247:
  %v319 = call double @df.num_to_double(i64 %v0)
  %v320 = load i8, ptr %ctx, align 8
  %c83 = zext i8 %v320 to i64
  %v321 = icmp ne i64 %c83, 0
  br i1 %v321, label %b252, label %b251
b248:
  %v325 = phi i64 [ %v318, %b249 ], [ %v324, %b251 ]
  br label %b245
b249:
  %v317 = mul i64 %v314, %v301
  store i64 0, ptr %gcs0
  %v318 = call i64 @df.box_int(i64 %v317)
  br label %b248
b250:
  %gcvolta250 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta250, ptr %ctxtopo, align 8
  ret i64 0
b251:
  %v322 = sitofp i64 %v301 to double
  %v323 = fmul double %v319, %v322
  store i64 0, ptr %gcs0
  %v324 = call i64 @df.box_double(double %v323)
  br label %b248
b252:
  %gcvolta252 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta252, ptr %ctxtopo, align 8
  ret i64 0
b253:
  br label %b245
b254:
  %gcvolta254 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta254, ptr %ctxtopo, align 8
  ret i64 0
b256:
  %v337 = ashr i64 %v0, 1
  %v338 = mul i64 %v337, %v334
  store i64 0, ptr %gcs0
  %v339 = call i64 @df.box_int(i64 %v338)
  br label %b258
b257:
  %v340 = call i64 @df.class(i64 %v0)
  %v341 = icmp eq i64 %v340, 2
  %v342 = icmp eq i64 %v340, 3
  %v343 = icmp eq i64 %v340, 4
  %c84 = zext i1 %v341 to i64
  %c85 = zext i1 %v342 to i64
  %v344 = or i64 %c84, %c85
  %c86 = zext i1 %v343 to i64
  %v345 = or i64 %v344, %c86
  %c87 = trunc i64 %v345 to i1
  br i1 %c87, label %b259, label %b260
b258:
  %v364 = phi i64 [ %v339, %b256 ], [ %v363, %b261 ]
  %xcp365 = load i8, ptr %ctx, align 8
  %xcn365 = icmp ne i8 %xcp365, 0
  %xce365 = call i1 @llvm.expect.i1(i1 %xcn365, i1 false)
  br i1 %xce365, label %xc365.limpar, label %xc365.fim
xc365.limpar:
  call void @dartforge_exception_clear()
  br label %xc365.fim
xc365.fim:
  %gcvolta258 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta258, ptr %ctxtopo, align 8
  ret i64 %v364
b259:
  br i1 %v343, label %b263, label %b262
b260:
  %v359 = call i64 @df.box_int(i64 %v334)
  store i64 %v359, ptr %gcs1
  %sa360_0 = getelementptr [2 x i64], ptr %sargs360, i64 0, i64 0
  store i64 %v359, ptr %sa360_0
  %sat360 = getelementptr [2 x i64], ptr %sargs360, i64 0, i64 1
  store i64 0, ptr %sat360
  %sic360 = getelementptr i64, ptr %area, i64 180
  %sf360 = call ptr @df.selector(ptr %sic360, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v360 = call i64 %sf360(i64 %v0, ptr %sargs360, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v361 = load i8, ptr %ctx, align 8
  %c88 = zext i8 %v361 to i64
  %v362 = icmp ne i64 %c88, 0
  br i1 %v362, label %b270, label %b269
b261:
  %v363 = phi i64 [ %v358, %b264 ], [ %v360, %b269 ]
  br label %b258
b262:
  %v347 = call i64 @df.unbox_int(i64 %v0)
  %v348 = load i8, ptr %ctx, align 8
  %c89 = zext i8 %v348 to i64
  %v349 = icmp ne i64 %c89, 0
  br i1 %v349, label %b266, label %b265
b263:
  %v352 = call double @df.num_to_double(i64 %v0)
  %v353 = load i8, ptr %ctx, align 8
  %c90 = zext i8 %v353 to i64
  %v354 = icmp ne i64 %c90, 0
  br i1 %v354, label %b268, label %b267
b264:
  %v358 = phi i64 [ %v351, %b265 ], [ %v357, %b267 ]
  br label %b261
b265:
  %v350 = mul i64 %v347, %v334
  store i64 0, ptr %gcs0
  %v351 = call i64 @df.box_int(i64 %v350)
  br label %b264
b266:
  %gcvolta266 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta266, ptr %ctxtopo, align 8
  ret i64 0
b267:
  %v355 = sitofp i64 %v334 to double
  %v356 = fmul double %v352, %v355
  store i64 0, ptr %gcs0
  %v357 = call i64 @df.box_double(double %v356)
  br label %b264
b268:
  %gcvolta268 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta268, ptr %ctxtopo, align 8
  ret i64 0
b269:
  br label %b261
b270:
  %gcvolta270 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta270, ptr %ctxtopo, align 8
  ret i64 0
b272:
  %v370 = ashr i64 %v0, 1
  %v371 = mul i64 %v370, %v367
  store i64 0, ptr %gcs0
  %v372 = call i64 @df.box_int(i64 %v371)
  br label %b274
b273:
  %v373 = call i64 @df.class(i64 %v0)
  %v374 = icmp eq i64 %v373, 2
  %v375 = icmp eq i64 %v373, 3
  %v376 = icmp eq i64 %v373, 4
  %c91 = zext i1 %v374 to i64
  %c92 = zext i1 %v375 to i64
  %v377 = or i64 %c91, %c92
  %c93 = zext i1 %v376 to i64
  %v378 = or i64 %v377, %c93
  %c94 = trunc i64 %v378 to i1
  br i1 %c94, label %b275, label %b276
b274:
  %v397 = phi i64 [ %v372, %b272 ], [ %v396, %b277 ]
  %xcp398 = load i8, ptr %ctx, align 8
  %xcn398 = icmp ne i8 %xcp398, 0
  %xce398 = call i1 @llvm.expect.i1(i1 %xcn398, i1 false)
  br i1 %xce398, label %xc398.limpar, label %xc398.fim
xc398.limpar:
  call void @dartforge_exception_clear()
  br label %xc398.fim
xc398.fim:
  %gcvolta274 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta274, ptr %ctxtopo, align 8
  ret i64 %v397
b275:
  br i1 %v376, label %b279, label %b278
b276:
  %v392 = call i64 @df.box_int(i64 %v367)
  store i64 %v392, ptr %gcs1
  %sa393_0 = getelementptr [2 x i64], ptr %sargs393, i64 0, i64 0
  store i64 %v392, ptr %sa393_0
  %sat393 = getelementptr [2 x i64], ptr %sargs393, i64 0, i64 1
  store i64 0, ptr %sat393
  %sic393 = getelementptr i64, ptr %area, i64 182
  %sf393 = call ptr @df.selector(ptr %sic393, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v393 = call i64 %sf393(i64 %v0, ptr %sargs393, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v394 = load i8, ptr %ctx, align 8
  %c95 = zext i8 %v394 to i64
  %v395 = icmp ne i64 %c95, 0
  br i1 %v395, label %b286, label %b285
b277:
  %v396 = phi i64 [ %v391, %b280 ], [ %v393, %b285 ]
  br label %b274
b278:
  %v380 = call i64 @df.unbox_int(i64 %v0)
  %v381 = load i8, ptr %ctx, align 8
  %c96 = zext i8 %v381 to i64
  %v382 = icmp ne i64 %c96, 0
  br i1 %v382, label %b282, label %b281
b279:
  %v385 = call double @df.num_to_double(i64 %v0)
  %v386 = load i8, ptr %ctx, align 8
  %c97 = zext i8 %v386 to i64
  %v387 = icmp ne i64 %c97, 0
  br i1 %v387, label %b284, label %b283
b280:
  %v391 = phi i64 [ %v384, %b281 ], [ %v390, %b283 ]
  br label %b277
b281:
  %v383 = mul i64 %v380, %v367
  store i64 0, ptr %gcs0
  %v384 = call i64 @df.box_int(i64 %v383)
  br label %b280
b282:
  %gcvolta282 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta282, ptr %ctxtopo, align 8
  ret i64 0
b283:
  %v388 = sitofp i64 %v367 to double
  %v389 = fmul double %v385, %v388
  store i64 0, ptr %gcs0
  %v390 = call i64 @df.box_double(double %v389)
  br label %b280
b284:
  %gcvolta284 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta284, ptr %ctxtopo, align 8
  ret i64 0
b285:
  br label %b277
b286:
  %gcvolta286 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta286, ptr %ctxtopo, align 8
  ret i64 0
b288:
  %v403 = ashr i64 %v0, 1
  %v404 = mul i64 %v403, %v400
  store i64 0, ptr %gcs0
  %v405 = call i64 @df.box_int(i64 %v404)
  br label %b290
b289:
  %v406 = call i64 @df.class(i64 %v0)
  %v407 = icmp eq i64 %v406, 2
  %v408 = icmp eq i64 %v406, 3
  %v409 = icmp eq i64 %v406, 4
  %c98 = zext i1 %v407 to i64
  %c99 = zext i1 %v408 to i64
  %v410 = or i64 %c98, %c99
  %c100 = zext i1 %v409 to i64
  %v411 = or i64 %v410, %c100
  %c101 = trunc i64 %v411 to i1
  br i1 %c101, label %b291, label %b292
b290:
  %v430 = phi i64 [ %v405, %b288 ], [ %v429, %b293 ]
  %xcp431 = load i8, ptr %ctx, align 8
  %xcn431 = icmp ne i8 %xcp431, 0
  %xce431 = call i1 @llvm.expect.i1(i1 %xcn431, i1 false)
  br i1 %xce431, label %xc431.limpar, label %xc431.fim
xc431.limpar:
  call void @dartforge_exception_clear()
  br label %xc431.fim
xc431.fim:
  %gcvolta290 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta290, ptr %ctxtopo, align 8
  ret i64 %v430
b291:
  br i1 %v409, label %b295, label %b294
b292:
  %v425 = call i64 @df.box_int(i64 %v400)
  store i64 %v425, ptr %gcs1
  %sa426_0 = getelementptr [2 x i64], ptr %sargs426, i64 0, i64 0
  store i64 %v425, ptr %sa426_0
  %sat426 = getelementptr [2 x i64], ptr %sargs426, i64 0, i64 1
  store i64 0, ptr %sat426
  %sic426 = getelementptr i64, ptr %area, i64 184
  %sf426 = call ptr @df.selector(ptr %sic426, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v426 = call i64 %sf426(i64 %v0, ptr %sargs426, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v427 = load i8, ptr %ctx, align 8
  %c102 = zext i8 %v427 to i64
  %v428 = icmp ne i64 %c102, 0
  br i1 %v428, label %b302, label %b301
b293:
  %v429 = phi i64 [ %v424, %b296 ], [ %v426, %b301 ]
  br label %b290
b294:
  %v413 = call i64 @df.unbox_int(i64 %v0)
  %v414 = load i8, ptr %ctx, align 8
  %c103 = zext i8 %v414 to i64
  %v415 = icmp ne i64 %c103, 0
  br i1 %v415, label %b298, label %b297
b295:
  %v418 = call double @df.num_to_double(i64 %v0)
  %v419 = load i8, ptr %ctx, align 8
  %c104 = zext i8 %v419 to i64
  %v420 = icmp ne i64 %c104, 0
  br i1 %v420, label %b300, label %b299
b296:
  %v424 = phi i64 [ %v417, %b297 ], [ %v423, %b299 ]
  br label %b293
b297:
  %v416 = mul i64 %v413, %v400
  store i64 0, ptr %gcs0
  %v417 = call i64 @df.box_int(i64 %v416)
  br label %b296
b298:
  %gcvolta298 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta298, ptr %ctxtopo, align 8
  ret i64 0
b299:
  %v421 = sitofp i64 %v400 to double
  %v422 = fmul double %v418, %v421
  store i64 0, ptr %gcs0
  %v423 = call i64 @df.box_double(double %v422)
  br label %b296
b300:
  %gcvolta300 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta300, ptr %ctxtopo, align 8
  ret i64 0
b301:
  br label %b293
b302:
  %gcvolta302 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta302, ptr %ctxtopo, align 8
  ret i64 0
b304:
  store i64 0, ptr %gcs1
  %v438 = call double @df.unbox_double(i64 %v435)
  %v439 = load i8, ptr %ctx, align 8
  %c105 = zext i8 %v439 to i64
  %v440 = icmp ne i64 %c105, 0
  br i1 %v440, label %b307, label %b306
b305:
  %gcvolta305 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta305, ptr %ctxtopo, align 8
  ret i64 0
b306:
  %xcp442 = load i8, ptr %ctx, align 8
  %xcn442 = icmp ne i8 %xcp442, 0
  %xce442 = call i1 @llvm.expect.i1(i1 %xcn442, i1 false)
  br i1 %xce442, label %xc442.limpar, label %xc442.fim
xc442.limpar:
  call void @dartforge_exception_clear()
  br label %xc442.fim
xc442.fim:
  %gcvolta306 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta306, ptr %ctxtopo, align 8
  ret i64 %v435
b307:
  %gcvolta307 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta307, ptr %ctxtopo, align 8
  ret i64 0
b321:
  %v748 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v749 = call i64 @df.box_int(i64 %v748)
  store i64 %v749, ptr %gcs1
  %sa750_0 = getelementptr [2 x i64], ptr %sargs750, i64 0, i64 0
  store i64 %v749, ptr %sa750_0
  %sat750 = getelementptr [2 x i64], ptr %sargs750, i64 0, i64 1
  store i64 0, ptr %sat750
  %sic750 = getelementptr i64, ptr %area, i64 186
  %sf750 = call ptr @df.selector(ptr %sic750, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v750 = call i64 %sf750(i64 %v0, ptr %sargs750, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v750, ptr %gcs0
  %v751 = load i8, ptr %ctx, align 8
  %c106 = zext i8 %v751 to i64
  %v752 = icmp ne i64 %c106, 0
  br i1 %v752, label %b502, label %b501
b322:
  store i64 0, ptr %gcs1
  %v448 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v448, ptr %gcs1
  %v449 = load i8, ptr %ctx, align 8
  %c107 = zext i8 %v449 to i64
  %v450 = icmp ne i64 %c107, 0
  br i1 %v450, label %b328, label %b327
b323:
  %v447 = icmp eq i64 %v444, %v6
  br i1 %v447, label %b325, label %b322
b324:
  %gcvolta324 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta324, ptr %ctxtopo, align 8
  ret i64 0
b325:
  %v493 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v494 = call i64 @df.box_int(i64 %v493)
  store i64 %v494, ptr %gcs1
  %v495 = icmp eq i64 %v0, 0
  %v496 = icmp eq i64 %v494, 0
  %c108 = zext i1 %v495 to i64
  %c109 = zext i1 %v496 to i64
  %v497 = or i64 %c108, %c109
  %v498 = icmp ne i64 %v497, 0
  br i1 %v498, label %b372, label %b373
b326:
  store i64 0, ptr %gcs1
  %v452 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v452, ptr %gcs1
  %v453 = load i8, ptr %ctx, align 8
  %c110 = zext i8 %v453 to i64
  %v454 = icmp ne i64 %c110, 0
  br i1 %v454, label %b332, label %b331
b327:
  %v451 = icmp eq i64 %v448, %v6
  br i1 %v451, label %b329, label %b326
b328:
  %gcvolta328 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta328, ptr %ctxtopo, align 8
  ret i64 0
b329:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v525 = icmp eq i64 %v0, 0
  %v526 = call i64 @df.class(i64 %v0)
  %v527 = call i8 @df.subclass(i64 %v526, i64 674)
  %c111 = zext i8 %v527 to i64
  %v528 = icmp ne i64 %c111, 0
  %v529 = xor i1 %v525, true
  %c112 = zext i1 %v528 to i64
  %c113 = zext i1 %v529 to i64
  %v530 = and i64 %c112, %c113
  %v531 = icmp ne i64 %v530, 0
  br i1 %v531, label %b395, label %b394
b330:
  store i64 0, ptr %gcs1
  %v456 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v456, ptr %gcs1
  %v457 = load i8, ptr %ctx, align 8
  %c114 = zext i8 %v457 to i64
  %v458 = icmp ne i64 %c114, 0
  br i1 %v458, label %b336, label %b335
b331:
  %v455 = icmp eq i64 %v452, %v6
  br i1 %v455, label %b333, label %b330
b332:
  %gcvolta332 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta332, ptr %ctxtopo, align 8
  ret i64 0
b333:
  %xcp548 = load i8, ptr %ctx, align 8
  %xcn548 = icmp ne i8 %xcp548, 0
  %xce548 = call i1 @llvm.expect.i1(i1 %xcn548, i1 false)
  br i1 %xce548, label %xc548.limpar, label %xc548.fim
xc548.limpar:
  call void @dartforge_exception_clear()
  br label %xc548.fim
xc548.fim:
  %gcvolta333 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta333, ptr %ctxtopo, align 8
  ret i64 %v0
b334:
  store i64 0, ptr %gcs1
  %v460 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v460, ptr %gcs1
  %v461 = load i8, ptr %ctx, align 8
  %c115 = zext i8 %v461 to i64
  %v462 = icmp ne i64 %c115, 0
  br i1 %v462, label %b340, label %b339
b335:
  %v459 = icmp eq i64 %v456, %v6
  br i1 %v459, label %b337, label %b334
b336:
  %gcvolta336 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta336, ptr %ctxtopo, align 8
  ret i64 0
b337:
  %v550 = add i64 0, 17
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v551 = and i64 %v0, 1
  %v552 = icmp ne i64 %v551, 0
  br i1 %v552, label %b405, label %b406
b338:
  store i64 0, ptr %gcs1
  %v464 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v464, ptr %gcs1
  %v465 = load i8, ptr %ctx, align 8
  %c116 = zext i8 %v465 to i64
  %v466 = icmp ne i64 %c116, 0
  br i1 %v466, label %b344, label %b343
b339:
  %v463 = icmp eq i64 %v460, %v6
  br i1 %v463, label %b341, label %b338
b340:
  %gcvolta340 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta340, ptr %ctxtopo, align 8
  ret i64 0
b341:
  %v583 = add i64 0, 4369
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v584 = and i64 %v0, 1
  %v585 = icmp ne i64 %v584, 0
  br i1 %v585, label %b421, label %b422
b342:
  store i64 0, ptr %gcs1
  %v468 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v468, ptr %gcs1
  %v469 = load i8, ptr %ctx, align 8
  %c117 = zext i8 %v469 to i64
  %v470 = icmp ne i64 %c117, 0
  br i1 %v470, label %b348, label %b347
b343:
  %v467 = icmp eq i64 %v464, %v6
  br i1 %v467, label %b345, label %b342
b344:
  %gcvolta344 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta344, ptr %ctxtopo, align 8
  ret i64 0
b345:
  %v616 = add i64 0, 286331153
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v617 = and i64 %v0, 1
  %v618 = icmp ne i64 %v617, 0
  br i1 %v618, label %b437, label %b438
b346:
  store i64 0, ptr %gcs1
  %v472 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v472, ptr %gcs1
  %v473 = load i8, ptr %ctx, align 8
  %c118 = zext i8 %v473 to i64
  %v474 = icmp ne i64 %c118, 0
  br i1 %v474, label %b352, label %b351
b347:
  %v471 = icmp eq i64 %v468, %v6
  br i1 %v471, label %b349, label %b346
b348:
  %gcvolta348 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta348, ptr %ctxtopo, align 8
  ret i64 0
b349:
  %v649 = add i64 0, 8
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v650 = and i64 %v0, 1
  %v651 = icmp ne i64 %v650, 0
  br i1 %v651, label %b453, label %b454
b350:
  store i64 0, ptr %gcs1
  %v476 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v476, ptr %gcs1
  %v477 = load i8, ptr %ctx, align 8
  %c119 = zext i8 %v477 to i64
  %v478 = icmp ne i64 %c119, 0
  br i1 %v478, label %b356, label %b355
b351:
  %v475 = icmp eq i64 %v472, %v6
  br i1 %v475, label %b353, label %b350
b352:
  %gcvolta352 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta352, ptr %ctxtopo, align 8
  ret i64 0
b353:
  %v682 = add i64 0, 2184
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v683 = and i64 %v0, 1
  %v684 = icmp ne i64 %v683, 0
  br i1 %v684, label %b469, label %b470
b354:
  store i64 0, ptr %gcs1
  %v480 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v480, ptr %gcs1
  %v481 = load i8, ptr %ctx, align 8
  %c120 = zext i8 %v481 to i64
  %v482 = icmp ne i64 %c120, 0
  br i1 %v482, label %b360, label %b359
b355:
  %v479 = icmp eq i64 %v476, %v6
  br i1 %v479, label %b357, label %b354
b356:
  %gcvolta356 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta356, ptr %ctxtopo, align 8
  ret i64 0
b357:
  %v715 = add i64 0, 143165576
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v716 = and i64 %v0, 1
  %v717 = icmp ne i64 %v716, 0
  br i1 %v717, label %b485, label %b486
b358:
  store i64 0, ptr %gcs1
  %v484 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v484, ptr %gcs1
  %v485 = load i8, ptr %ctx, align 8
  %c121 = zext i8 %v485 to i64
  %v486 = icmp ne i64 %c121, 0
  br i1 %v486, label %b365, label %b364
b359:
  %v483 = icmp eq i64 %v480, %v6
  br i1 %v483, label %b361, label %b358
b360:
  %gcvolta360 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta360, ptr %ctxtopo, align 8
  ret i64 0
b361:
  br label %b321
b363:
  store i64 0, ptr %gcs1
  %v488 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v488, ptr %gcs1
  %v489 = load i8, ptr %ctx, align 8
  %c122 = zext i8 %v489 to i64
  %v490 = icmp ne i64 %c122, 0
  br i1 %v490, label %b370, label %b369
b364:
  %v487 = icmp eq i64 %v484, %v6
  br i1 %v487, label %b366, label %b363
b365:
  %gcvolta365 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta365, ptr %ctxtopo, align 8
  ret i64 0
b366:
  br label %b321
b368:
  br label %b12
b369:
  %v491 = icmp eq i64 %v488, %v6
  br i1 %v491, label %b371, label %b368
b370:
  %gcvolta370 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta370, ptr %ctxtopo, align 8
  ret i64 0
b371:
  br label %b321
b372:
  %v499 = icmp eq i64 %v0, %v494
  br label %b374
b373:
  %v500 = call i64 @df.class(i64 %v0)
  switch i64 %v500, label %b378 [ i64 675, label %b375 i64 4, label %b376 i64 2, label %b377 i64 3, label %b377 ]
b374:
  %v518 = phi i1 [ %v499, %b372 ], [ %v517, %b379 ]
  br i1 %v518, label %b390, label %b391
b375:
  %v501 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v494)
  %v502 = load i8, ptr %ctx, align 8
  %c123 = zext i8 %v502 to i64
  %v503 = icmp ne i64 %c123, 0
  br i1 %v503, label %b381, label %b380
b376:
  %v505 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v494)
  %v506 = load i8, ptr %ctx, align 8
  %c124 = zext i8 %v506 to i64
  %v507 = icmp ne i64 %c124, 0
  br i1 %v507, label %b383, label %b382
b377:
  %v508 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v494)
  %v509 = load i8, ptr %ctx, align 8
  %c125 = zext i8 %v509 to i64
  %v510 = icmp ne i64 %c125, 0
  br i1 %v510, label %b385, label %b384
b378:
  %sa511_0 = getelementptr [2 x i64], ptr %sargs511, i64 0, i64 0
  store i64 %v494, ptr %sa511_0
  %sat511 = getelementptr [2 x i64], ptr %sargs511, i64 0, i64 1
  store i64 0, ptr %sat511
  %sic511 = getelementptr i64, ptr %area, i64 188
  %sf511 = call ptr @df.selector(ptr %sic511, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v511 = call i64 %sf511(i64 %v0, ptr %sargs511, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v511, ptr %gcs0
  %v512 = load i8, ptr %ctx, align 8
  %c126 = zext i8 %v512 to i64
  %v513 = icmp ne i64 %c126, 0
  br i1 %v513, label %b387, label %b386
b379:
  %v517 = phi i1 [ %v504, %b380 ], [ %v505, %b382 ], [ %v508, %b384 ], [ %v514, %b388 ]
  br label %b374
b380:
  %c127 = zext i8 %v501 to i64
  %v504 = icmp ne i64 %c127, 0
  br label %b379
b381:
  %gcvolta381 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta381, ptr %ctxtopo, align 8
  ret i64 0
b382:
  br label %b379
b383:
  %gcvolta383 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta383, ptr %ctxtopo, align 8
  ret i64 0
b384:
  br label %b379
b385:
  %gcvolta385 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta385, ptr %ctxtopo, align 8
  ret i64 0
b386:
  store i64 0, ptr %gcs1
  %v514 = call i1 @df.unbox_bool(i64 %v511)
  %v515 = load i8, ptr %ctx, align 8
  %c128 = zext i8 %v515 to i64
  %v516 = icmp ne i64 %c128, 0
  br i1 %v516, label %b389, label %b388
b387:
  %gcvolta387 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta387, ptr %ctxtopo, align 8
  ret i64 0
b388:
  br label %b379
b389:
  %gcvolta389 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta389, ptr %ctxtopo, align 8
  ret i64 0
b390:
  %v519 = add i64 0, 0
  br label %b392
b391:
  %v520 = add i64 0, 1
  br label %b392
b392:
  %v521 = phi i64 [ %v519, %b390 ], [ %v520, %b391 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v522 = call i64 @df.box_int(i64 %v521)
  %xcp523 = load i8, ptr %ctx, align 8
  %xcn523 = icmp ne i8 %xcp523, 0
  %xce523 = call i1 @llvm.expect.i1(i1 %xcn523, i1 false)
  br i1 %xce523, label %xc523.limpar, label %xc523.fim
xc523.limpar:
  call void @dartforge_exception_clear()
  br label %xc523.fim
xc523.fim:
  %gcvolta392 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta392, ptr %ctxtopo, align 8
  ret i64 %v522
b394:
  %ga532 = getelementptr i64, ptr %area, i64 7
  %v532 = load i8, ptr %ga532
  %c129 = zext i8 %v532 to i64
  %v533 = icmp ne i64 %c129, 0
  br i1 %v533, label %b396, label %b397
b395:
  %v540 = call i64 @df.unbox_int(i64 %v0)
  %v541 = load i8, ptr %ctx, align 8
  %c130 = zext i8 %v541 to i64
  %v542 = icmp ne i64 %c130, 0
  br i1 %v542, label %b402, label %b401
b396:
  %ga534 = getelementptr i64, ptr %area, i64 6
  %v534 = load i64, ptr %ga534
  br label %b398
b397:
  %v535 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b398
b398:
  %v536 = phi i64 [ %v534, %b396 ], [ %v535, %b397 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v536, i64 0, i64 0)
  %v538 = load i8, ptr %ctx, align 8
  %c131 = zext i8 %v538 to i64
  %v539 = icmp ne i64 %c131, 0
  br i1 %v539, label %b400, label %b399
b399:
  br label %b395
b400:
  %gcvolta400 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta400, ptr %ctxtopo, align 8
  ret i64 0
b401:
  %v543 = add i64 0, 1
  %v544 = ashr i64 %v540, %v543
  store i64 0, ptr %gcs0
  %v545 = call i64 @df.box_int(i64 %v544)
  %xcp546 = load i8, ptr %ctx, align 8
  %xcn546 = icmp ne i8 %xcp546, 0
  %xce546 = call i1 @llvm.expect.i1(i1 %xcn546, i1 false)
  br i1 %xce546, label %xc546.limpar, label %xc546.fim
xc546.limpar:
  call void @dartforge_exception_clear()
  br label %xc546.fim
xc546.fim:
  %gcvolta401 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta401, ptr %ctxtopo, align 8
  ret i64 %v545
b402:
  %gcvolta402 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta402, ptr %ctxtopo, align 8
  ret i64 0
b405:
  %v553 = ashr i64 %v0, 1
  %v554 = mul i64 %v553, %v550
  store i64 0, ptr %gcs0
  %v555 = call i64 @df.box_int(i64 %v554)
  br label %b407
b406:
  %v556 = call i64 @df.class(i64 %v0)
  %v557 = icmp eq i64 %v556, 2
  %v558 = icmp eq i64 %v556, 3
  %v559 = icmp eq i64 %v556, 4
  %c132 = zext i1 %v557 to i64
  %c133 = zext i1 %v558 to i64
  %v560 = or i64 %c132, %c133
  %c134 = zext i1 %v559 to i64
  %v561 = or i64 %v560, %c134
  %c135 = trunc i64 %v561 to i1
  br i1 %c135, label %b408, label %b409
b407:
  %v580 = phi i64 [ %v555, %b405 ], [ %v579, %b410 ]
  %xcp581 = load i8, ptr %ctx, align 8
  %xcn581 = icmp ne i8 %xcp581, 0
  %xce581 = call i1 @llvm.expect.i1(i1 %xcn581, i1 false)
  br i1 %xce581, label %xc581.limpar, label %xc581.fim
xc581.limpar:
  call void @dartforge_exception_clear()
  br label %xc581.fim
xc581.fim:
  %gcvolta407 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta407, ptr %ctxtopo, align 8
  ret i64 %v580
b408:
  br i1 %v559, label %b412, label %b411
b409:
  %v575 = call i64 @df.box_int(i64 %v550)
  store i64 %v575, ptr %gcs1
  %sa576_0 = getelementptr [2 x i64], ptr %sargs576, i64 0, i64 0
  store i64 %v575, ptr %sa576_0
  %sat576 = getelementptr [2 x i64], ptr %sargs576, i64 0, i64 1
  store i64 0, ptr %sat576
  %sic576 = getelementptr i64, ptr %area, i64 190
  %sf576 = call ptr @df.selector(ptr %sic576, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v576 = call i64 %sf576(i64 %v0, ptr %sargs576, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v577 = load i8, ptr %ctx, align 8
  %c136 = zext i8 %v577 to i64
  %v578 = icmp ne i64 %c136, 0
  br i1 %v578, label %b419, label %b418
b410:
  %v579 = phi i64 [ %v574, %b413 ], [ %v576, %b418 ]
  br label %b407
b411:
  %v563 = call i64 @df.unbox_int(i64 %v0)
  %v564 = load i8, ptr %ctx, align 8
  %c137 = zext i8 %v564 to i64
  %v565 = icmp ne i64 %c137, 0
  br i1 %v565, label %b415, label %b414
b412:
  %v568 = call double @df.num_to_double(i64 %v0)
  %v569 = load i8, ptr %ctx, align 8
  %c138 = zext i8 %v569 to i64
  %v570 = icmp ne i64 %c138, 0
  br i1 %v570, label %b417, label %b416
b413:
  %v574 = phi i64 [ %v567, %b414 ], [ %v573, %b416 ]
  br label %b410
b414:
  %v566 = mul i64 %v563, %v550
  store i64 0, ptr %gcs0
  %v567 = call i64 @df.box_int(i64 %v566)
  br label %b413
b415:
  %gcvolta415 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta415, ptr %ctxtopo, align 8
  ret i64 0
b416:
  %v571 = sitofp i64 %v550 to double
  %v572 = fmul double %v568, %v571
  store i64 0, ptr %gcs0
  %v573 = call i64 @df.box_double(double %v572)
  br label %b413
b417:
  %gcvolta417 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta417, ptr %ctxtopo, align 8
  ret i64 0
b418:
  br label %b410
b419:
  %gcvolta419 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta419, ptr %ctxtopo, align 8
  ret i64 0
b421:
  %v586 = ashr i64 %v0, 1
  %v587 = mul i64 %v586, %v583
  store i64 0, ptr %gcs0
  %v588 = call i64 @df.box_int(i64 %v587)
  br label %b423
b422:
  %v589 = call i64 @df.class(i64 %v0)
  %v590 = icmp eq i64 %v589, 2
  %v591 = icmp eq i64 %v589, 3
  %v592 = icmp eq i64 %v589, 4
  %c139 = zext i1 %v590 to i64
  %c140 = zext i1 %v591 to i64
  %v593 = or i64 %c139, %c140
  %c141 = zext i1 %v592 to i64
  %v594 = or i64 %v593, %c141
  %c142 = trunc i64 %v594 to i1
  br i1 %c142, label %b424, label %b425
b423:
  %v613 = phi i64 [ %v588, %b421 ], [ %v612, %b426 ]
  %xcp614 = load i8, ptr %ctx, align 8
  %xcn614 = icmp ne i8 %xcp614, 0
  %xce614 = call i1 @llvm.expect.i1(i1 %xcn614, i1 false)
  br i1 %xce614, label %xc614.limpar, label %xc614.fim
xc614.limpar:
  call void @dartforge_exception_clear()
  br label %xc614.fim
xc614.fim:
  %gcvolta423 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta423, ptr %ctxtopo, align 8
  ret i64 %v613
b424:
  br i1 %v592, label %b428, label %b427
b425:
  %v608 = call i64 @df.box_int(i64 %v583)
  store i64 %v608, ptr %gcs1
  %sa609_0 = getelementptr [2 x i64], ptr %sargs609, i64 0, i64 0
  store i64 %v608, ptr %sa609_0
  %sat609 = getelementptr [2 x i64], ptr %sargs609, i64 0, i64 1
  store i64 0, ptr %sat609
  %sic609 = getelementptr i64, ptr %area, i64 192
  %sf609 = call ptr @df.selector(ptr %sic609, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v609 = call i64 %sf609(i64 %v0, ptr %sargs609, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v610 = load i8, ptr %ctx, align 8
  %c143 = zext i8 %v610 to i64
  %v611 = icmp ne i64 %c143, 0
  br i1 %v611, label %b435, label %b434
b426:
  %v612 = phi i64 [ %v607, %b429 ], [ %v609, %b434 ]
  br label %b423
b427:
  %v596 = call i64 @df.unbox_int(i64 %v0)
  %v597 = load i8, ptr %ctx, align 8
  %c144 = zext i8 %v597 to i64
  %v598 = icmp ne i64 %c144, 0
  br i1 %v598, label %b431, label %b430
b428:
  %v601 = call double @df.num_to_double(i64 %v0)
  %v602 = load i8, ptr %ctx, align 8
  %c145 = zext i8 %v602 to i64
  %v603 = icmp ne i64 %c145, 0
  br i1 %v603, label %b433, label %b432
b429:
  %v607 = phi i64 [ %v600, %b430 ], [ %v606, %b432 ]
  br label %b426
b430:
  %v599 = mul i64 %v596, %v583
  store i64 0, ptr %gcs0
  %v600 = call i64 @df.box_int(i64 %v599)
  br label %b429
b431:
  %gcvolta431 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta431, ptr %ctxtopo, align 8
  ret i64 0
b432:
  %v604 = sitofp i64 %v583 to double
  %v605 = fmul double %v601, %v604
  store i64 0, ptr %gcs0
  %v606 = call i64 @df.box_double(double %v605)
  br label %b429
b433:
  %gcvolta433 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta433, ptr %ctxtopo, align 8
  ret i64 0
b434:
  br label %b426
b435:
  %gcvolta435 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta435, ptr %ctxtopo, align 8
  ret i64 0
b437:
  %v619 = ashr i64 %v0, 1
  %v620 = mul i64 %v619, %v616
  store i64 0, ptr %gcs0
  %v621 = call i64 @df.box_int(i64 %v620)
  br label %b439
b438:
  %v622 = call i64 @df.class(i64 %v0)
  %v623 = icmp eq i64 %v622, 2
  %v624 = icmp eq i64 %v622, 3
  %v625 = icmp eq i64 %v622, 4
  %c146 = zext i1 %v623 to i64
  %c147 = zext i1 %v624 to i64
  %v626 = or i64 %c146, %c147
  %c148 = zext i1 %v625 to i64
  %v627 = or i64 %v626, %c148
  %c149 = trunc i64 %v627 to i1
  br i1 %c149, label %b440, label %b441
b439:
  %v646 = phi i64 [ %v621, %b437 ], [ %v645, %b442 ]
  %xcp647 = load i8, ptr %ctx, align 8
  %xcn647 = icmp ne i8 %xcp647, 0
  %xce647 = call i1 @llvm.expect.i1(i1 %xcn647, i1 false)
  br i1 %xce647, label %xc647.limpar, label %xc647.fim
xc647.limpar:
  call void @dartforge_exception_clear()
  br label %xc647.fim
xc647.fim:
  %gcvolta439 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta439, ptr %ctxtopo, align 8
  ret i64 %v646
b440:
  br i1 %v625, label %b444, label %b443
b441:
  %v641 = call i64 @df.box_int(i64 %v616)
  store i64 %v641, ptr %gcs1
  %sa642_0 = getelementptr [2 x i64], ptr %sargs642, i64 0, i64 0
  store i64 %v641, ptr %sa642_0
  %sat642 = getelementptr [2 x i64], ptr %sargs642, i64 0, i64 1
  store i64 0, ptr %sat642
  %sic642 = getelementptr i64, ptr %area, i64 194
  %sf642 = call ptr @df.selector(ptr %sic642, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v642 = call i64 %sf642(i64 %v0, ptr %sargs642, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v643 = load i8, ptr %ctx, align 8
  %c150 = zext i8 %v643 to i64
  %v644 = icmp ne i64 %c150, 0
  br i1 %v644, label %b451, label %b450
b442:
  %v645 = phi i64 [ %v640, %b445 ], [ %v642, %b450 ]
  br label %b439
b443:
  %v629 = call i64 @df.unbox_int(i64 %v0)
  %v630 = load i8, ptr %ctx, align 8
  %c151 = zext i8 %v630 to i64
  %v631 = icmp ne i64 %c151, 0
  br i1 %v631, label %b447, label %b446
b444:
  %v634 = call double @df.num_to_double(i64 %v0)
  %v635 = load i8, ptr %ctx, align 8
  %c152 = zext i8 %v635 to i64
  %v636 = icmp ne i64 %c152, 0
  br i1 %v636, label %b449, label %b448
b445:
  %v640 = phi i64 [ %v633, %b446 ], [ %v639, %b448 ]
  br label %b442
b446:
  %v632 = mul i64 %v629, %v616
  store i64 0, ptr %gcs0
  %v633 = call i64 @df.box_int(i64 %v632)
  br label %b445
b447:
  %gcvolta447 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta447, ptr %ctxtopo, align 8
  ret i64 0
b448:
  %v637 = sitofp i64 %v616 to double
  %v638 = fmul double %v634, %v637
  store i64 0, ptr %gcs0
  %v639 = call i64 @df.box_double(double %v638)
  br label %b445
b449:
  %gcvolta449 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta449, ptr %ctxtopo, align 8
  ret i64 0
b450:
  br label %b442
b451:
  %gcvolta451 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta451, ptr %ctxtopo, align 8
  ret i64 0
b453:
  %v652 = ashr i64 %v0, 1
  %v653 = mul i64 %v652, %v649
  store i64 0, ptr %gcs0
  %v654 = call i64 @df.box_int(i64 %v653)
  br label %b455
b454:
  %v655 = call i64 @df.class(i64 %v0)
  %v656 = icmp eq i64 %v655, 2
  %v657 = icmp eq i64 %v655, 3
  %v658 = icmp eq i64 %v655, 4
  %c153 = zext i1 %v656 to i64
  %c154 = zext i1 %v657 to i64
  %v659 = or i64 %c153, %c154
  %c155 = zext i1 %v658 to i64
  %v660 = or i64 %v659, %c155
  %c156 = trunc i64 %v660 to i1
  br i1 %c156, label %b456, label %b457
b455:
  %v679 = phi i64 [ %v654, %b453 ], [ %v678, %b458 ]
  %xcp680 = load i8, ptr %ctx, align 8
  %xcn680 = icmp ne i8 %xcp680, 0
  %xce680 = call i1 @llvm.expect.i1(i1 %xcn680, i1 false)
  br i1 %xce680, label %xc680.limpar, label %xc680.fim
xc680.limpar:
  call void @dartforge_exception_clear()
  br label %xc680.fim
xc680.fim:
  %gcvolta455 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta455, ptr %ctxtopo, align 8
  ret i64 %v679
b456:
  br i1 %v658, label %b460, label %b459
b457:
  %v674 = call i64 @df.box_int(i64 %v649)
  store i64 %v674, ptr %gcs1
  %sa675_0 = getelementptr [2 x i64], ptr %sargs675, i64 0, i64 0
  store i64 %v674, ptr %sa675_0
  %sat675 = getelementptr [2 x i64], ptr %sargs675, i64 0, i64 1
  store i64 0, ptr %sat675
  %sic675 = getelementptr i64, ptr %area, i64 196
  %sf675 = call ptr @df.selector(ptr %sic675, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v675 = call i64 %sf675(i64 %v0, ptr %sargs675, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v676 = load i8, ptr %ctx, align 8
  %c157 = zext i8 %v676 to i64
  %v677 = icmp ne i64 %c157, 0
  br i1 %v677, label %b467, label %b466
b458:
  %v678 = phi i64 [ %v673, %b461 ], [ %v675, %b466 ]
  br label %b455
b459:
  %v662 = call i64 @df.unbox_int(i64 %v0)
  %v663 = load i8, ptr %ctx, align 8
  %c158 = zext i8 %v663 to i64
  %v664 = icmp ne i64 %c158, 0
  br i1 %v664, label %b463, label %b462
b460:
  %v667 = call double @df.num_to_double(i64 %v0)
  %v668 = load i8, ptr %ctx, align 8
  %c159 = zext i8 %v668 to i64
  %v669 = icmp ne i64 %c159, 0
  br i1 %v669, label %b465, label %b464
b461:
  %v673 = phi i64 [ %v666, %b462 ], [ %v672, %b464 ]
  br label %b458
b462:
  %v665 = mul i64 %v662, %v649
  store i64 0, ptr %gcs0
  %v666 = call i64 @df.box_int(i64 %v665)
  br label %b461
b463:
  %gcvolta463 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta463, ptr %ctxtopo, align 8
  ret i64 0
b464:
  %v670 = sitofp i64 %v649 to double
  %v671 = fmul double %v667, %v670
  store i64 0, ptr %gcs0
  %v672 = call i64 @df.box_double(double %v671)
  br label %b461
b465:
  %gcvolta465 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta465, ptr %ctxtopo, align 8
  ret i64 0
b466:
  br label %b458
b467:
  %gcvolta467 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta467, ptr %ctxtopo, align 8
  ret i64 0
b469:
  %v685 = ashr i64 %v0, 1
  %v686 = mul i64 %v685, %v682
  store i64 0, ptr %gcs0
  %v687 = call i64 @df.box_int(i64 %v686)
  br label %b471
b470:
  %v688 = call i64 @df.class(i64 %v0)
  %v689 = icmp eq i64 %v688, 2
  %v690 = icmp eq i64 %v688, 3
  %v691 = icmp eq i64 %v688, 4
  %c160 = zext i1 %v689 to i64
  %c161 = zext i1 %v690 to i64
  %v692 = or i64 %c160, %c161
  %c162 = zext i1 %v691 to i64
  %v693 = or i64 %v692, %c162
  %c163 = trunc i64 %v693 to i1
  br i1 %c163, label %b472, label %b473
b471:
  %v712 = phi i64 [ %v687, %b469 ], [ %v711, %b474 ]
  %xcp713 = load i8, ptr %ctx, align 8
  %xcn713 = icmp ne i8 %xcp713, 0
  %xce713 = call i1 @llvm.expect.i1(i1 %xcn713, i1 false)
  br i1 %xce713, label %xc713.limpar, label %xc713.fim
xc713.limpar:
  call void @dartforge_exception_clear()
  br label %xc713.fim
xc713.fim:
  %gcvolta471 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta471, ptr %ctxtopo, align 8
  ret i64 %v712
b472:
  br i1 %v691, label %b476, label %b475
b473:
  %v707 = call i64 @df.box_int(i64 %v682)
  store i64 %v707, ptr %gcs1
  %sa708_0 = getelementptr [2 x i64], ptr %sargs708, i64 0, i64 0
  store i64 %v707, ptr %sa708_0
  %sat708 = getelementptr [2 x i64], ptr %sargs708, i64 0, i64 1
  store i64 0, ptr %sat708
  %sic708 = getelementptr i64, ptr %area, i64 198
  %sf708 = call ptr @df.selector(ptr %sic708, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v708 = call i64 %sf708(i64 %v0, ptr %sargs708, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v709 = load i8, ptr %ctx, align 8
  %c164 = zext i8 %v709 to i64
  %v710 = icmp ne i64 %c164, 0
  br i1 %v710, label %b483, label %b482
b474:
  %v711 = phi i64 [ %v706, %b477 ], [ %v708, %b482 ]
  br label %b471
b475:
  %v695 = call i64 @df.unbox_int(i64 %v0)
  %v696 = load i8, ptr %ctx, align 8
  %c165 = zext i8 %v696 to i64
  %v697 = icmp ne i64 %c165, 0
  br i1 %v697, label %b479, label %b478
b476:
  %v700 = call double @df.num_to_double(i64 %v0)
  %v701 = load i8, ptr %ctx, align 8
  %c166 = zext i8 %v701 to i64
  %v702 = icmp ne i64 %c166, 0
  br i1 %v702, label %b481, label %b480
b477:
  %v706 = phi i64 [ %v699, %b478 ], [ %v705, %b480 ]
  br label %b474
b478:
  %v698 = mul i64 %v695, %v682
  store i64 0, ptr %gcs0
  %v699 = call i64 @df.box_int(i64 %v698)
  br label %b477
b479:
  %gcvolta479 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta479, ptr %ctxtopo, align 8
  ret i64 0
b480:
  %v703 = sitofp i64 %v682 to double
  %v704 = fmul double %v700, %v703
  store i64 0, ptr %gcs0
  %v705 = call i64 @df.box_double(double %v704)
  br label %b477
b481:
  %gcvolta481 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta481, ptr %ctxtopo, align 8
  ret i64 0
b482:
  br label %b474
b483:
  %gcvolta483 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta483, ptr %ctxtopo, align 8
  ret i64 0
b485:
  %v718 = ashr i64 %v0, 1
  %v719 = mul i64 %v718, %v715
  store i64 0, ptr %gcs0
  %v720 = call i64 @df.box_int(i64 %v719)
  br label %b487
b486:
  %v721 = call i64 @df.class(i64 %v0)
  %v722 = icmp eq i64 %v721, 2
  %v723 = icmp eq i64 %v721, 3
  %v724 = icmp eq i64 %v721, 4
  %c167 = zext i1 %v722 to i64
  %c168 = zext i1 %v723 to i64
  %v725 = or i64 %c167, %c168
  %c169 = zext i1 %v724 to i64
  %v726 = or i64 %v725, %c169
  %c170 = trunc i64 %v726 to i1
  br i1 %c170, label %b488, label %b489
b487:
  %v745 = phi i64 [ %v720, %b485 ], [ %v744, %b490 ]
  %xcp746 = load i8, ptr %ctx, align 8
  %xcn746 = icmp ne i8 %xcp746, 0
  %xce746 = call i1 @llvm.expect.i1(i1 %xcn746, i1 false)
  br i1 %xce746, label %xc746.limpar, label %xc746.fim
xc746.limpar:
  call void @dartforge_exception_clear()
  br label %xc746.fim
xc746.fim:
  %gcvolta487 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta487, ptr %ctxtopo, align 8
  ret i64 %v745
b488:
  br i1 %v724, label %b492, label %b491
b489:
  %v740 = call i64 @df.box_int(i64 %v715)
  store i64 %v740, ptr %gcs1
  %sa741_0 = getelementptr [2 x i64], ptr %sargs741, i64 0, i64 0
  store i64 %v740, ptr %sa741_0
  %sat741 = getelementptr [2 x i64], ptr %sargs741, i64 0, i64 1
  store i64 0, ptr %sat741
  %sic741 = getelementptr i64, ptr %area, i64 200
  %sf741 = call ptr @df.selector(ptr %sic741, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v741 = call i64 %sf741(i64 %v0, ptr %sargs741, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v742 = load i8, ptr %ctx, align 8
  %c171 = zext i8 %v742 to i64
  %v743 = icmp ne i64 %c171, 0
  br i1 %v743, label %b499, label %b498
b490:
  %v744 = phi i64 [ %v739, %b493 ], [ %v741, %b498 ]
  br label %b487
b491:
  %v728 = call i64 @df.unbox_int(i64 %v0)
  %v729 = load i8, ptr %ctx, align 8
  %c172 = zext i8 %v729 to i64
  %v730 = icmp ne i64 %c172, 0
  br i1 %v730, label %b495, label %b494
b492:
  %v733 = call double @df.num_to_double(i64 %v0)
  %v734 = load i8, ptr %ctx, align 8
  %c173 = zext i8 %v734 to i64
  %v735 = icmp ne i64 %c173, 0
  br i1 %v735, label %b497, label %b496
b493:
  %v739 = phi i64 [ %v732, %b494 ], [ %v738, %b496 ]
  br label %b490
b494:
  %v731 = mul i64 %v728, %v715
  store i64 0, ptr %gcs0
  %v732 = call i64 @df.box_int(i64 %v731)
  br label %b493
b495:
  %gcvolta495 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta495, ptr %ctxtopo, align 8
  ret i64 0
b496:
  %v736 = sitofp i64 %v715 to double
  %v737 = fmul double %v733, %v736
  store i64 0, ptr %gcs0
  %v738 = call i64 @df.box_double(double %v737)
  br label %b493
b497:
  %gcvolta497 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta497, ptr %ctxtopo, align 8
  ret i64 0
b498:
  br label %b490
b499:
  %gcvolta499 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta499, ptr %ctxtopo, align 8
  ret i64 0
b501:
  store i64 0, ptr %gcs1
  %v753 = call double @df.unbox_double(i64 %v750)
  %v754 = load i8, ptr %ctx, align 8
  %c174 = zext i8 %v754 to i64
  %v755 = icmp ne i64 %c174, 0
  br i1 %v755, label %b504, label %b503
b502:
  %gcvolta502 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta502, ptr %ctxtopo, align 8
  ret i64 0
b503:
  %xcp757 = load i8, ptr %ctx, align 8
  %xcn757 = icmp ne i8 %xcp757, 0
  %xce757 = call i1 @llvm.expect.i1(i1 %xcn757, i1 false)
  br i1 %xce757, label %xc757.limpar, label %xc757.fim
xc757.limpar:
  call void @dartforge_exception_clear()
  br label %xc757.fim
xc757.fim:
  %gcvolta503 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta503, ptr %ctxtopo, align 8
  ret i64 %v750
b504:
  %gcvolta504 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta504, ptr %ctxtopo, align 8
  ret i64 0
b518:
  %v1043 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1044 = call i64 @df.box_int(i64 %v1043)
  store i64 %v1044, ptr %gcs1
  %sa1045_0 = getelementptr [2 x i64], ptr %sargs1045, i64 0, i64 0
  store i64 %v1044, ptr %sa1045_0
  %sat1045 = getelementptr [2 x i64], ptr %sargs1045, i64 0, i64 1
  store i64 0, ptr %sat1045
  %sic1045 = getelementptr i64, ptr %area, i64 202
  %sf1045 = call ptr @df.selector(ptr %sic1045, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1045 = call i64 %sf1045(i64 %v0, ptr %sargs1045, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1045, ptr %gcs0
  %v1046 = load i8, ptr %ctx, align 8
  %c175 = zext i8 %v1046 to i64
  %v1047 = icmp ne i64 %c175, 0
  br i1 %v1047, label %b687, label %b686
b519:
  store i64 0, ptr %gcs1
  %v763 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v763, ptr %gcs1
  %v764 = load i8, ptr %ctx, align 8
  %c176 = zext i8 %v764 to i64
  %v765 = icmp ne i64 %c176, 0
  br i1 %v765, label %b525, label %b524
b520:
  %v762 = icmp eq i64 %v759, %v6
  br i1 %v762, label %b522, label %b519
b521:
  %gcvolta521 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta521, ptr %ctxtopo, align 8
  ret i64 0
b522:
  %v808 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v809 = call i64 @df.box_int(i64 %v808)
  store i64 %v809, ptr %gcs1
  %v810 = icmp eq i64 %v0, 0
  %v811 = icmp eq i64 %v809, 0
  %c177 = zext i1 %v810 to i64
  %c178 = zext i1 %v811 to i64
  %v812 = or i64 %c177, %c178
  %v813 = icmp ne i64 %v812, 0
  br i1 %v813, label %b569, label %b570
b523:
  store i64 0, ptr %gcs1
  %v767 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v767, ptr %gcs1
  %v768 = load i8, ptr %ctx, align 8
  %c179 = zext i8 %v768 to i64
  %v769 = icmp ne i64 %c179, 0
  br i1 %v769, label %b529, label %b528
b524:
  %v766 = icmp eq i64 %v763, %v6
  br i1 %v766, label %b526, label %b523
b525:
  %gcvolta525 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta525, ptr %ctxtopo, align 8
  ret i64 0
b526:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v840 = icmp eq i64 %v0, 0
  %v841 = call i64 @df.class(i64 %v0)
  %v842 = call i8 @df.subclass(i64 %v841, i64 674)
  %c180 = zext i8 %v842 to i64
  %v843 = icmp ne i64 %c180, 0
  %v844 = xor i1 %v840, true
  %c181 = zext i1 %v843 to i64
  %c182 = zext i1 %v844 to i64
  %v845 = and i64 %c181, %c182
  %v846 = icmp ne i64 %v845, 0
  br i1 %v846, label %b592, label %b591
b527:
  store i64 0, ptr %gcs1
  %v771 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v771, ptr %gcs1
  %v772 = load i8, ptr %ctx, align 8
  %c183 = zext i8 %v772 to i64
  %v773 = icmp ne i64 %c183, 0
  br i1 %v773, label %b533, label %b532
b528:
  %v770 = icmp eq i64 %v767, %v6
  br i1 %v770, label %b530, label %b527
b529:
  %gcvolta529 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta529, ptr %ctxtopo, align 8
  ret i64 0
b530:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v863 = icmp eq i64 %v0, 0
  %v864 = call i64 @df.class(i64 %v0)
  %v865 = call i8 @df.subclass(i64 %v864, i64 674)
  %c184 = zext i8 %v865 to i64
  %v866 = icmp ne i64 %c184, 0
  %v867 = xor i1 %v863, true
  %c185 = zext i1 %v866 to i64
  %c186 = zext i1 %v867 to i64
  %v868 = and i64 %c185, %c186
  %v869 = icmp ne i64 %v868, 0
  br i1 %v869, label %b602, label %b601
b531:
  store i64 0, ptr %gcs1
  %v775 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v775, ptr %gcs1
  %v776 = load i8, ptr %ctx, align 8
  %c187 = zext i8 %v776 to i64
  %v777 = icmp ne i64 %c187, 0
  br i1 %v777, label %b537, label %b536
b532:
  %v774 = icmp eq i64 %v771, %v6
  br i1 %v774, label %b534, label %b531
b533:
  %gcvolta533 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta533, ptr %ctxtopo, align 8
  ret i64 0
b534:
  %xcp886 = load i8, ptr %ctx, align 8
  %xcn886 = icmp ne i8 %xcp886, 0
  %xce886 = call i1 @llvm.expect.i1(i1 %xcn886, i1 false)
  br i1 %xce886, label %xc886.limpar, label %xc886.fim
xc886.limpar:
  call void @dartforge_exception_clear()
  br label %xc886.fim
xc886.fim:
  %gcvolta534 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta534, ptr %ctxtopo, align 8
  ret i64 %v0
b535:
  store i64 0, ptr %gcs1
  %v779 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v779, ptr %gcs1
  %v780 = load i8, ptr %ctx, align 8
  %c188 = zext i8 %v780 to i64
  %v781 = icmp ne i64 %c188, 0
  br i1 %v781, label %b541, label %b540
b536:
  %v778 = icmp eq i64 %v775, %v6
  br i1 %v778, label %b538, label %b535
b537:
  %gcvolta537 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta537, ptr %ctxtopo, align 8
  ret i64 0
b538:
  %v888 = add i64 0, 257
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v889 = and i64 %v0, 1
  %v890 = icmp ne i64 %v889, 0
  br i1 %v890, label %b612, label %b613
b539:
  store i64 0, ptr %gcs1
  %v783 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v783, ptr %gcs1
  %v784 = load i8, ptr %ctx, align 8
  %c189 = zext i8 %v784 to i64
  %v785 = icmp ne i64 %c189, 0
  br i1 %v785, label %b545, label %b544
b540:
  %v782 = icmp eq i64 %v779, %v6
  br i1 %v782, label %b542, label %b539
b541:
  %gcvolta541 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta541, ptr %ctxtopo, align 8
  ret i64 0
b542:
  %v921 = add i64 0, 16843009
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v922 = and i64 %v0, 1
  %v923 = icmp ne i64 %v922, 0
  br i1 %v923, label %b628, label %b629
b543:
  store i64 0, ptr %gcs1
  %v787 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v787, ptr %gcs1
  %v788 = load i8, ptr %ctx, align 8
  %c190 = zext i8 %v788 to i64
  %v789 = icmp ne i64 %c190, 0
  br i1 %v789, label %b549, label %b548
b544:
  %v786 = icmp eq i64 %v783, %v6
  br i1 %v786, label %b546, label %b543
b545:
  %gcvolta545 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta545, ptr %ctxtopo, align 8
  ret i64 0
b546:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v954 = icmp eq i64 %v0, 0
  %v955 = call i64 @df.class(i64 %v0)
  %v956 = call i8 @df.subclass(i64 %v955, i64 674)
  %c191 = zext i8 %v956 to i64
  %v957 = icmp ne i64 %c191, 0
  %v958 = xor i1 %v954, true
  %c192 = zext i1 %v957 to i64
  %c193 = zext i1 %v958 to i64
  %v959 = and i64 %c192, %c193
  %v960 = icmp ne i64 %v959, 0
  br i1 %v960, label %b645, label %b644
b547:
  store i64 0, ptr %gcs1
  %v791 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v791, ptr %gcs1
  %v792 = load i8, ptr %ctx, align 8
  %c194 = zext i8 %v792 to i64
  %v793 = icmp ne i64 %c194, 0
  br i1 %v793, label %b553, label %b552
b548:
  %v790 = icmp eq i64 %v787, %v6
  br i1 %v790, label %b550, label %b547
b549:
  %gcvolta549 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta549, ptr %ctxtopo, align 8
  ret i64 0
b550:
  %v977 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v978 = and i64 %v0, 1
  %v979 = icmp ne i64 %v978, 0
  br i1 %v979, label %b654, label %b655
b551:
  store i64 0, ptr %gcs1
  %v795 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v795, ptr %gcs1
  %v796 = load i8, ptr %ctx, align 8
  %c195 = zext i8 %v796 to i64
  %v797 = icmp ne i64 %c195, 0
  br i1 %v797, label %b557, label %b556
b552:
  %v794 = icmp eq i64 %v791, %v6
  br i1 %v794, label %b554, label %b551
b553:
  %gcvolta553 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta553, ptr %ctxtopo, align 8
  ret i64 0
b554:
  %v1010 = add i64 0, 8421504
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1011 = and i64 %v0, 1
  %v1012 = icmp ne i64 %v1011, 0
  br i1 %v1012, label %b670, label %b671
b555:
  store i64 0, ptr %gcs1
  %v799 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v799, ptr %gcs1
  %v800 = load i8, ptr %ctx, align 8
  %c196 = zext i8 %v800 to i64
  %v801 = icmp ne i64 %c196, 0
  br i1 %v801, label %b562, label %b561
b556:
  %v798 = icmp eq i64 %v795, %v6
  br i1 %v798, label %b558, label %b555
b557:
  %gcvolta557 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta557, ptr %ctxtopo, align 8
  ret i64 0
b558:
  br label %b518
b560:
  store i64 0, ptr %gcs1
  %v803 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v803, ptr %gcs1
  %v804 = load i8, ptr %ctx, align 8
  %c197 = zext i8 %v804 to i64
  %v805 = icmp ne i64 %c197, 0
  br i1 %v805, label %b567, label %b566
b561:
  %v802 = icmp eq i64 %v799, %v6
  br i1 %v802, label %b563, label %b560
b562:
  %gcvolta562 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta562, ptr %ctxtopo, align 8
  ret i64 0
b563:
  br label %b518
b565:
  br label %b12
b566:
  %v806 = icmp eq i64 %v803, %v6
  br i1 %v806, label %b568, label %b565
b567:
  %gcvolta567 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta567, ptr %ctxtopo, align 8
  ret i64 0
b568:
  br label %b518
b569:
  %v814 = icmp eq i64 %v0, %v809
  br label %b571
b570:
  %v815 = call i64 @df.class(i64 %v0)
  switch i64 %v815, label %b575 [ i64 675, label %b572 i64 4, label %b573 i64 2, label %b574 i64 3, label %b574 ]
b571:
  %v833 = phi i1 [ %v814, %b569 ], [ %v832, %b576 ]
  br i1 %v833, label %b587, label %b588
b572:
  %v816 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v809)
  %v817 = load i8, ptr %ctx, align 8
  %c198 = zext i8 %v817 to i64
  %v818 = icmp ne i64 %c198, 0
  br i1 %v818, label %b578, label %b577
b573:
  %v820 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v809)
  %v821 = load i8, ptr %ctx, align 8
  %c199 = zext i8 %v821 to i64
  %v822 = icmp ne i64 %c199, 0
  br i1 %v822, label %b580, label %b579
b574:
  %v823 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v809)
  %v824 = load i8, ptr %ctx, align 8
  %c200 = zext i8 %v824 to i64
  %v825 = icmp ne i64 %c200, 0
  br i1 %v825, label %b582, label %b581
b575:
  %sa826_0 = getelementptr [2 x i64], ptr %sargs826, i64 0, i64 0
  store i64 %v809, ptr %sa826_0
  %sat826 = getelementptr [2 x i64], ptr %sargs826, i64 0, i64 1
  store i64 0, ptr %sat826
  %sic826 = getelementptr i64, ptr %area, i64 204
  %sf826 = call ptr @df.selector(ptr %sic826, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v826 = call i64 %sf826(i64 %v0, ptr %sargs826, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v826, ptr %gcs0
  %v827 = load i8, ptr %ctx, align 8
  %c201 = zext i8 %v827 to i64
  %v828 = icmp ne i64 %c201, 0
  br i1 %v828, label %b584, label %b583
b576:
  %v832 = phi i1 [ %v819, %b577 ], [ %v820, %b579 ], [ %v823, %b581 ], [ %v829, %b585 ]
  br label %b571
b577:
  %c202 = zext i8 %v816 to i64
  %v819 = icmp ne i64 %c202, 0
  br label %b576
b578:
  %gcvolta578 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta578, ptr %ctxtopo, align 8
  ret i64 0
b579:
  br label %b576
b580:
  %gcvolta580 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta580, ptr %ctxtopo, align 8
  ret i64 0
b581:
  br label %b576
b582:
  %gcvolta582 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta582, ptr %ctxtopo, align 8
  ret i64 0
b583:
  store i64 0, ptr %gcs1
  %v829 = call i1 @df.unbox_bool(i64 %v826)
  %v830 = load i8, ptr %ctx, align 8
  %c203 = zext i8 %v830 to i64
  %v831 = icmp ne i64 %c203, 0
  br i1 %v831, label %b586, label %b585
b584:
  %gcvolta584 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta584, ptr %ctxtopo, align 8
  ret i64 0
b585:
  br label %b576
b586:
  %gcvolta586 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta586, ptr %ctxtopo, align 8
  ret i64 0
b587:
  %v834 = add i64 0, 0
  br label %b589
b588:
  %v835 = add i64 0, 1
  br label %b589
b589:
  %v836 = phi i64 [ %v834, %b587 ], [ %v835, %b588 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v837 = call i64 @df.box_int(i64 %v836)
  %xcp838 = load i8, ptr %ctx, align 8
  %xcn838 = icmp ne i8 %xcp838, 0
  %xce838 = call i1 @llvm.expect.i1(i1 %xcn838, i1 false)
  br i1 %xce838, label %xc838.limpar, label %xc838.fim
xc838.limpar:
  call void @dartforge_exception_clear()
  br label %xc838.fim
xc838.fim:
  %gcvolta589 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta589, ptr %ctxtopo, align 8
  ret i64 %v837
b591:
  %ga847 = getelementptr i64, ptr %area, i64 7
  %v847 = load i8, ptr %ga847
  %c204 = zext i8 %v847 to i64
  %v848 = icmp ne i64 %c204, 0
  br i1 %v848, label %b593, label %b594
b592:
  %v855 = call i64 @df.unbox_int(i64 %v0)
  %v856 = load i8, ptr %ctx, align 8
  %c205 = zext i8 %v856 to i64
  %v857 = icmp ne i64 %c205, 0
  br i1 %v857, label %b599, label %b598
b593:
  %ga849 = getelementptr i64, ptr %area, i64 6
  %v849 = load i64, ptr %ga849
  br label %b595
b594:
  %v850 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b595
b595:
  %v851 = phi i64 [ %v849, %b593 ], [ %v850, %b594 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v851, i64 0, i64 0)
  %v853 = load i8, ptr %ctx, align 8
  %c206 = zext i8 %v853 to i64
  %v854 = icmp ne i64 %c206, 0
  br i1 %v854, label %b597, label %b596
b596:
  br label %b592
b597:
  %gcvolta597 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta597, ptr %ctxtopo, align 8
  ret i64 0
b598:
  %v858 = add i64 0, 6
  %v859 = ashr i64 %v855, %v858
  store i64 0, ptr %gcs0
  %v860 = call i64 @df.box_int(i64 %v859)
  %xcp861 = load i8, ptr %ctx, align 8
  %xcn861 = icmp ne i8 %xcp861, 0
  %xce861 = call i1 @llvm.expect.i1(i1 %xcn861, i1 false)
  br i1 %xce861, label %xc861.limpar, label %xc861.fim
xc861.limpar:
  call void @dartforge_exception_clear()
  br label %xc861.fim
xc861.fim:
  %gcvolta598 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta598, ptr %ctxtopo, align 8
  ret i64 %v860
b599:
  %gcvolta599 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta599, ptr %ctxtopo, align 8
  ret i64 0
b601:
  %ga870 = getelementptr i64, ptr %area, i64 7
  %v870 = load i8, ptr %ga870
  %c207 = zext i8 %v870 to i64
  %v871 = icmp ne i64 %c207, 0
  br i1 %v871, label %b603, label %b604
b602:
  %v878 = call i64 @df.unbox_int(i64 %v0)
  %v879 = load i8, ptr %ctx, align 8
  %c208 = zext i8 %v879 to i64
  %v880 = icmp ne i64 %c208, 0
  br i1 %v880, label %b609, label %b608
b603:
  %ga872 = getelementptr i64, ptr %area, i64 6
  %v872 = load i64, ptr %ga872
  br label %b605
b604:
  %v873 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b605
b605:
  %v874 = phi i64 [ %v872, %b603 ], [ %v873, %b604 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v874, i64 0, i64 0)
  %v876 = load i8, ptr %ctx, align 8
  %c209 = zext i8 %v876 to i64
  %v877 = icmp ne i64 %c209, 0
  br i1 %v877, label %b607, label %b606
b606:
  br label %b602
b607:
  %gcvolta607 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta607, ptr %ctxtopo, align 8
  ret i64 0
b608:
  %v881 = add i64 0, 4
  %v882 = ashr i64 %v878, %v881
  store i64 0, ptr %gcs0
  %v883 = call i64 @df.box_int(i64 %v882)
  %xcp884 = load i8, ptr %ctx, align 8
  %xcn884 = icmp ne i8 %xcp884, 0
  %xce884 = call i1 @llvm.expect.i1(i1 %xcn884, i1 false)
  br i1 %xce884, label %xc884.limpar, label %xc884.fim
xc884.limpar:
  call void @dartforge_exception_clear()
  br label %xc884.fim
xc884.fim:
  %gcvolta608 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta608, ptr %ctxtopo, align 8
  ret i64 %v883
b609:
  %gcvolta609 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta609, ptr %ctxtopo, align 8
  ret i64 0
b612:
  %v891 = ashr i64 %v0, 1
  %v892 = mul i64 %v891, %v888
  store i64 0, ptr %gcs0
  %v893 = call i64 @df.box_int(i64 %v892)
  br label %b614
b613:
  %v894 = call i64 @df.class(i64 %v0)
  %v895 = icmp eq i64 %v894, 2
  %v896 = icmp eq i64 %v894, 3
  %v897 = icmp eq i64 %v894, 4
  %c210 = zext i1 %v895 to i64
  %c211 = zext i1 %v896 to i64
  %v898 = or i64 %c210, %c211
  %c212 = zext i1 %v897 to i64
  %v899 = or i64 %v898, %c212
  %c213 = trunc i64 %v899 to i1
  br i1 %c213, label %b615, label %b616
b614:
  %v918 = phi i64 [ %v893, %b612 ], [ %v917, %b617 ]
  %xcp919 = load i8, ptr %ctx, align 8
  %xcn919 = icmp ne i8 %xcp919, 0
  %xce919 = call i1 @llvm.expect.i1(i1 %xcn919, i1 false)
  br i1 %xce919, label %xc919.limpar, label %xc919.fim
xc919.limpar:
  call void @dartforge_exception_clear()
  br label %xc919.fim
xc919.fim:
  %gcvolta614 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta614, ptr %ctxtopo, align 8
  ret i64 %v918
b615:
  br i1 %v897, label %b619, label %b618
b616:
  %v913 = call i64 @df.box_int(i64 %v888)
  store i64 %v913, ptr %gcs1
  %sa914_0 = getelementptr [2 x i64], ptr %sargs914, i64 0, i64 0
  store i64 %v913, ptr %sa914_0
  %sat914 = getelementptr [2 x i64], ptr %sargs914, i64 0, i64 1
  store i64 0, ptr %sat914
  %sic914 = getelementptr i64, ptr %area, i64 206
  %sf914 = call ptr @df.selector(ptr %sic914, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v914 = call i64 %sf914(i64 %v0, ptr %sargs914, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v915 = load i8, ptr %ctx, align 8
  %c214 = zext i8 %v915 to i64
  %v916 = icmp ne i64 %c214, 0
  br i1 %v916, label %b626, label %b625
b617:
  %v917 = phi i64 [ %v912, %b620 ], [ %v914, %b625 ]
  br label %b614
b618:
  %v901 = call i64 @df.unbox_int(i64 %v0)
  %v902 = load i8, ptr %ctx, align 8
  %c215 = zext i8 %v902 to i64
  %v903 = icmp ne i64 %c215, 0
  br i1 %v903, label %b622, label %b621
b619:
  %v906 = call double @df.num_to_double(i64 %v0)
  %v907 = load i8, ptr %ctx, align 8
  %c216 = zext i8 %v907 to i64
  %v908 = icmp ne i64 %c216, 0
  br i1 %v908, label %b624, label %b623
b620:
  %v912 = phi i64 [ %v905, %b621 ], [ %v911, %b623 ]
  br label %b617
b621:
  %v904 = mul i64 %v901, %v888
  store i64 0, ptr %gcs0
  %v905 = call i64 @df.box_int(i64 %v904)
  br label %b620
b622:
  %gcvolta622 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta622, ptr %ctxtopo, align 8
  ret i64 0
b623:
  %v909 = sitofp i64 %v888 to double
  %v910 = fmul double %v906, %v909
  store i64 0, ptr %gcs0
  %v911 = call i64 @df.box_double(double %v910)
  br label %b620
b624:
  %gcvolta624 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta624, ptr %ctxtopo, align 8
  ret i64 0
b625:
  br label %b617
b626:
  %gcvolta626 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta626, ptr %ctxtopo, align 8
  ret i64 0
b628:
  %v924 = ashr i64 %v0, 1
  %v925 = mul i64 %v924, %v921
  store i64 0, ptr %gcs0
  %v926 = call i64 @df.box_int(i64 %v925)
  br label %b630
b629:
  %v927 = call i64 @df.class(i64 %v0)
  %v928 = icmp eq i64 %v927, 2
  %v929 = icmp eq i64 %v927, 3
  %v930 = icmp eq i64 %v927, 4
  %c217 = zext i1 %v928 to i64
  %c218 = zext i1 %v929 to i64
  %v931 = or i64 %c217, %c218
  %c219 = zext i1 %v930 to i64
  %v932 = or i64 %v931, %c219
  %c220 = trunc i64 %v932 to i1
  br i1 %c220, label %b631, label %b632
b630:
  %v951 = phi i64 [ %v926, %b628 ], [ %v950, %b633 ]
  %xcp952 = load i8, ptr %ctx, align 8
  %xcn952 = icmp ne i8 %xcp952, 0
  %xce952 = call i1 @llvm.expect.i1(i1 %xcn952, i1 false)
  br i1 %xce952, label %xc952.limpar, label %xc952.fim
xc952.limpar:
  call void @dartforge_exception_clear()
  br label %xc952.fim
xc952.fim:
  %gcvolta630 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta630, ptr %ctxtopo, align 8
  ret i64 %v951
b631:
  br i1 %v930, label %b635, label %b634
b632:
  %v946 = call i64 @df.box_int(i64 %v921)
  store i64 %v946, ptr %gcs1
  %sa947_0 = getelementptr [2 x i64], ptr %sargs947, i64 0, i64 0
  store i64 %v946, ptr %sa947_0
  %sat947 = getelementptr [2 x i64], ptr %sargs947, i64 0, i64 1
  store i64 0, ptr %sat947
  %sic947 = getelementptr i64, ptr %area, i64 208
  %sf947 = call ptr @df.selector(ptr %sic947, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v947 = call i64 %sf947(i64 %v0, ptr %sargs947, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v948 = load i8, ptr %ctx, align 8
  %c221 = zext i8 %v948 to i64
  %v949 = icmp ne i64 %c221, 0
  br i1 %v949, label %b642, label %b641
b633:
  %v950 = phi i64 [ %v945, %b636 ], [ %v947, %b641 ]
  br label %b630
b634:
  %v934 = call i64 @df.unbox_int(i64 %v0)
  %v935 = load i8, ptr %ctx, align 8
  %c222 = zext i8 %v935 to i64
  %v936 = icmp ne i64 %c222, 0
  br i1 %v936, label %b638, label %b637
b635:
  %v939 = call double @df.num_to_double(i64 %v0)
  %v940 = load i8, ptr %ctx, align 8
  %c223 = zext i8 %v940 to i64
  %v941 = icmp ne i64 %c223, 0
  br i1 %v941, label %b640, label %b639
b636:
  %v945 = phi i64 [ %v938, %b637 ], [ %v944, %b639 ]
  br label %b633
b637:
  %v937 = mul i64 %v934, %v921
  store i64 0, ptr %gcs0
  %v938 = call i64 @df.box_int(i64 %v937)
  br label %b636
b638:
  %gcvolta638 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta638, ptr %ctxtopo, align 8
  ret i64 0
b639:
  %v942 = sitofp i64 %v921 to double
  %v943 = fmul double %v939, %v942
  store i64 0, ptr %gcs0
  %v944 = call i64 @df.box_double(double %v943)
  br label %b636
b640:
  %gcvolta640 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta640, ptr %ctxtopo, align 8
  ret i64 0
b641:
  br label %b633
b642:
  %gcvolta642 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta642, ptr %ctxtopo, align 8
  ret i64 0
b644:
  %ga961 = getelementptr i64, ptr %area, i64 7
  %v961 = load i8, ptr %ga961
  %c224 = zext i8 %v961 to i64
  %v962 = icmp ne i64 %c224, 0
  br i1 %v962, label %b646, label %b647
b645:
  %v969 = call i64 @df.unbox_int(i64 %v0)
  %v970 = load i8, ptr %ctx, align 8
  %c225 = zext i8 %v970 to i64
  %v971 = icmp ne i64 %c225, 0
  br i1 %v971, label %b652, label %b651
b646:
  %ga963 = getelementptr i64, ptr %area, i64 6
  %v963 = load i64, ptr %ga963
  br label %b648
b647:
  %v964 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b648
b648:
  %v965 = phi i64 [ %v963, %b646 ], [ %v964, %b647 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v965, i64 0, i64 0)
  %v967 = load i8, ptr %ctx, align 8
  %c226 = zext i8 %v967 to i64
  %v968 = icmp ne i64 %c226, 0
  br i1 %v968, label %b650, label %b649
b649:
  br label %b645
b650:
  %gcvolta650 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta650, ptr %ctxtopo, align 8
  ret i64 0
b651:
  %v972 = add i64 0, 1
  %v973 = ashr i64 %v969, %v972
  store i64 0, ptr %gcs0
  %v974 = call i64 @df.box_int(i64 %v973)
  %xcp975 = load i8, ptr %ctx, align 8
  %xcn975 = icmp ne i8 %xcp975, 0
  %xce975 = call i1 @llvm.expect.i1(i1 %xcn975, i1 false)
  br i1 %xce975, label %xc975.limpar, label %xc975.fim
xc975.limpar:
  call void @dartforge_exception_clear()
  br label %xc975.fim
xc975.fim:
  %gcvolta651 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta651, ptr %ctxtopo, align 8
  ret i64 %v974
b652:
  %gcvolta652 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta652, ptr %ctxtopo, align 8
  ret i64 0
b654:
  %v980 = ashr i64 %v0, 1
  %v981 = mul i64 %v980, %v977
  store i64 0, ptr %gcs0
  %v982 = call i64 @df.box_int(i64 %v981)
  br label %b656
b655:
  %v983 = call i64 @df.class(i64 %v0)
  %v984 = icmp eq i64 %v983, 2
  %v985 = icmp eq i64 %v983, 3
  %v986 = icmp eq i64 %v983, 4
  %c227 = zext i1 %v984 to i64
  %c228 = zext i1 %v985 to i64
  %v987 = or i64 %c227, %c228
  %c229 = zext i1 %v986 to i64
  %v988 = or i64 %v987, %c229
  %c230 = trunc i64 %v988 to i1
  br i1 %c230, label %b657, label %b658
b656:
  %v1007 = phi i64 [ %v982, %b654 ], [ %v1006, %b659 ]
  %xcp1008 = load i8, ptr %ctx, align 8
  %xcn1008 = icmp ne i8 %xcp1008, 0
  %xce1008 = call i1 @llvm.expect.i1(i1 %xcn1008, i1 false)
  br i1 %xce1008, label %xc1008.limpar, label %xc1008.fim
xc1008.limpar:
  call void @dartforge_exception_clear()
  br label %xc1008.fim
xc1008.fim:
  %gcvolta656 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta656, ptr %ctxtopo, align 8
  ret i64 %v1007
b657:
  br i1 %v986, label %b661, label %b660
b658:
  %v1002 = call i64 @df.box_int(i64 %v977)
  store i64 %v1002, ptr %gcs1
  %sa1003_0 = getelementptr [2 x i64], ptr %sargs1003, i64 0, i64 0
  store i64 %v1002, ptr %sa1003_0
  %sat1003 = getelementptr [2 x i64], ptr %sargs1003, i64 0, i64 1
  store i64 0, ptr %sat1003
  %sic1003 = getelementptr i64, ptr %area, i64 210
  %sf1003 = call ptr @df.selector(ptr %sic1003, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v1003 = call i64 %sf1003(i64 %v0, ptr %sargs1003, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1004 = load i8, ptr %ctx, align 8
  %c231 = zext i8 %v1004 to i64
  %v1005 = icmp ne i64 %c231, 0
  br i1 %v1005, label %b668, label %b667
b659:
  %v1006 = phi i64 [ %v1001, %b662 ], [ %v1003, %b667 ]
  br label %b656
b660:
  %v990 = call i64 @df.unbox_int(i64 %v0)
  %v991 = load i8, ptr %ctx, align 8
  %c232 = zext i8 %v991 to i64
  %v992 = icmp ne i64 %c232, 0
  br i1 %v992, label %b664, label %b663
b661:
  %v995 = call double @df.num_to_double(i64 %v0)
  %v996 = load i8, ptr %ctx, align 8
  %c233 = zext i8 %v996 to i64
  %v997 = icmp ne i64 %c233, 0
  br i1 %v997, label %b666, label %b665
b662:
  %v1001 = phi i64 [ %v994, %b663 ], [ %v1000, %b665 ]
  br label %b659
b663:
  %v993 = mul i64 %v990, %v977
  store i64 0, ptr %gcs0
  %v994 = call i64 @df.box_int(i64 %v993)
  br label %b662
b664:
  %gcvolta664 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta664, ptr %ctxtopo, align 8
  ret i64 0
b665:
  %v998 = sitofp i64 %v977 to double
  %v999 = fmul double %v995, %v998
  store i64 0, ptr %gcs0
  %v1000 = call i64 @df.box_double(double %v999)
  br label %b662
b666:
  %gcvolta666 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta666, ptr %ctxtopo, align 8
  ret i64 0
b667:
  br label %b659
b668:
  %gcvolta668 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta668, ptr %ctxtopo, align 8
  ret i64 0
b670:
  %v1013 = ashr i64 %v0, 1
  %v1014 = mul i64 %v1013, %v1010
  store i64 0, ptr %gcs0
  %v1015 = call i64 @df.box_int(i64 %v1014)
  br label %b672
b671:
  %v1016 = call i64 @df.class(i64 %v0)
  %v1017 = icmp eq i64 %v1016, 2
  %v1018 = icmp eq i64 %v1016, 3
  %v1019 = icmp eq i64 %v1016, 4
  %c234 = zext i1 %v1017 to i64
  %c235 = zext i1 %v1018 to i64
  %v1020 = or i64 %c234, %c235
  %c236 = zext i1 %v1019 to i64
  %v1021 = or i64 %v1020, %c236
  %c237 = trunc i64 %v1021 to i1
  br i1 %c237, label %b673, label %b674
b672:
  %v1040 = phi i64 [ %v1015, %b670 ], [ %v1039, %b675 ]
  %xcp1041 = load i8, ptr %ctx, align 8
  %xcn1041 = icmp ne i8 %xcp1041, 0
  %xce1041 = call i1 @llvm.expect.i1(i1 %xcn1041, i1 false)
  br i1 %xce1041, label %xc1041.limpar, label %xc1041.fim
xc1041.limpar:
  call void @dartforge_exception_clear()
  br label %xc1041.fim
xc1041.fim:
  %gcvolta672 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta672, ptr %ctxtopo, align 8
  ret i64 %v1040
b673:
  br i1 %v1019, label %b677, label %b676
b674:
  %v1035 = call i64 @df.box_int(i64 %v1010)
  store i64 %v1035, ptr %gcs1
  %sa1036_0 = getelementptr [2 x i64], ptr %sargs1036, i64 0, i64 0
  store i64 %v1035, ptr %sa1036_0
  %sat1036 = getelementptr [2 x i64], ptr %sargs1036, i64 0, i64 1
  store i64 0, ptr %sat1036
  %sic1036 = getelementptr i64, ptr %area, i64 212
  %sf1036 = call ptr @df.selector(ptr %sic1036, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v1036 = call i64 %sf1036(i64 %v0, ptr %sargs1036, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1037 = load i8, ptr %ctx, align 8
  %c238 = zext i8 %v1037 to i64
  %v1038 = icmp ne i64 %c238, 0
  br i1 %v1038, label %b684, label %b683
b675:
  %v1039 = phi i64 [ %v1034, %b678 ], [ %v1036, %b683 ]
  br label %b672
b676:
  %v1023 = call i64 @df.unbox_int(i64 %v0)
  %v1024 = load i8, ptr %ctx, align 8
  %c239 = zext i8 %v1024 to i64
  %v1025 = icmp ne i64 %c239, 0
  br i1 %v1025, label %b680, label %b679
b677:
  %v1028 = call double @df.num_to_double(i64 %v0)
  %v1029 = load i8, ptr %ctx, align 8
  %c240 = zext i8 %v1029 to i64
  %v1030 = icmp ne i64 %c240, 0
  br i1 %v1030, label %b682, label %b681
b678:
  %v1034 = phi i64 [ %v1027, %b679 ], [ %v1033, %b681 ]
  br label %b675
b679:
  %v1026 = mul i64 %v1023, %v1010
  store i64 0, ptr %gcs0
  %v1027 = call i64 @df.box_int(i64 %v1026)
  br label %b678
b680:
  %gcvolta680 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta680, ptr %ctxtopo, align 8
  ret i64 0
b681:
  %v1031 = sitofp i64 %v1010 to double
  %v1032 = fmul double %v1028, %v1031
  store i64 0, ptr %gcs0
  %v1033 = call i64 @df.box_double(double %v1032)
  br label %b678
b682:
  %gcvolta682 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta682, ptr %ctxtopo, align 8
  ret i64 0
b683:
  br label %b675
b684:
  %gcvolta684 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta684, ptr %ctxtopo, align 8
  ret i64 0
b686:
  store i64 0, ptr %gcs1
  %v1048 = call double @df.unbox_double(i64 %v1045)
  %v1049 = load i8, ptr %ctx, align 8
  %c241 = zext i8 %v1049 to i64
  %v1050 = icmp ne i64 %c241, 0
  br i1 %v1050, label %b689, label %b688
b687:
  %gcvolta687 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta687, ptr %ctxtopo, align 8
  ret i64 0
b688:
  %xcp1052 = load i8, ptr %ctx, align 8
  %xcn1052 = icmp ne i8 %xcp1052, 0
  %xce1052 = call i1 @llvm.expect.i1(i1 %xcn1052, i1 false)
  br i1 %xce1052, label %xc1052.limpar, label %xc1052.fim
xc1052.limpar:
  call void @dartforge_exception_clear()
  br label %xc1052.fim
xc1052.fim:
  %gcvolta688 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta688, ptr %ctxtopo, align 8
  ret i64 %v1045
b689:
  %gcvolta689 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta689, ptr %ctxtopo, align 8
  ret i64 0
b703:
  %v1308 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1309 = call i64 @df.box_int(i64 %v1308)
  store i64 %v1309, ptr %gcs1
  %sa1310_0 = getelementptr [2 x i64], ptr %sargs1310, i64 0, i64 0
  store i64 %v1309, ptr %sa1310_0
  %sat1310 = getelementptr [2 x i64], ptr %sargs1310, i64 0, i64 1
  store i64 0, ptr %sat1310
  %sic1310 = getelementptr i64, ptr %area, i64 214
  %sf1310 = call ptr @df.selector(ptr %sic1310, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1310 = call i64 %sf1310(i64 %v0, ptr %sargs1310, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1310, ptr %gcs0
  %v1311 = load i8, ptr %ctx, align 8
  %c242 = zext i8 %v1311 to i64
  %v1312 = icmp ne i64 %c242, 0
  br i1 %v1312, label %b854, label %b853
b704:
  store i64 0, ptr %gcs1
  %v1058 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1058, ptr %gcs1
  %v1059 = load i8, ptr %ctx, align 8
  %c243 = zext i8 %v1059 to i64
  %v1060 = icmp ne i64 %c243, 0
  br i1 %v1060, label %b710, label %b709
b705:
  %v1057 = icmp eq i64 %v1054, %v6
  br i1 %v1057, label %b707, label %b704
b706:
  %gcvolta706 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta706, ptr %ctxtopo, align 8
  ret i64 0
b707:
  %v1103 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1104 = call i64 @df.box_int(i64 %v1103)
  store i64 %v1104, ptr %gcs1
  %v1105 = icmp eq i64 %v0, 0
  %v1106 = icmp eq i64 %v1104, 0
  %c244 = zext i1 %v1105 to i64
  %c245 = zext i1 %v1106 to i64
  %v1107 = or i64 %c244, %c245
  %v1108 = icmp ne i64 %v1107, 0
  br i1 %v1108, label %b754, label %b755
b708:
  store i64 0, ptr %gcs1
  %v1062 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1062, ptr %gcs1
  %v1063 = load i8, ptr %ctx, align 8
  %c246 = zext i8 %v1063 to i64
  %v1064 = icmp ne i64 %c246, 0
  br i1 %v1064, label %b714, label %b713
b709:
  %v1061 = icmp eq i64 %v1058, %v6
  br i1 %v1061, label %b711, label %b708
b710:
  %gcvolta710 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta710, ptr %ctxtopo, align 8
  ret i64 0
b711:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1135 = icmp eq i64 %v0, 0
  %v1136 = call i64 @df.class(i64 %v0)
  %v1137 = call i8 @df.subclass(i64 %v1136, i64 674)
  %c247 = zext i8 %v1137 to i64
  %v1138 = icmp ne i64 %c247, 0
  %v1139 = xor i1 %v1135, true
  %c248 = zext i1 %v1138 to i64
  %c249 = zext i1 %v1139 to i64
  %v1140 = and i64 %c248, %c249
  %v1141 = icmp ne i64 %v1140, 0
  br i1 %v1141, label %b777, label %b776
b712:
  store i64 0, ptr %gcs1
  %v1066 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1066, ptr %gcs1
  %v1067 = load i8, ptr %ctx, align 8
  %c250 = zext i8 %v1067 to i64
  %v1068 = icmp ne i64 %c250, 0
  br i1 %v1068, label %b718, label %b717
b713:
  %v1065 = icmp eq i64 %v1062, %v6
  br i1 %v1065, label %b715, label %b712
b714:
  %gcvolta714 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta714, ptr %ctxtopo, align 8
  ret i64 0
b715:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1158 = icmp eq i64 %v0, 0
  %v1159 = call i64 @df.class(i64 %v0)
  %v1160 = call i8 @df.subclass(i64 %v1159, i64 674)
  %c251 = zext i8 %v1160 to i64
  %v1161 = icmp ne i64 %c251, 0
  %v1162 = xor i1 %v1158, true
  %c252 = zext i1 %v1161 to i64
  %c253 = zext i1 %v1162 to i64
  %v1163 = and i64 %c252, %c253
  %v1164 = icmp ne i64 %v1163, 0
  br i1 %v1164, label %b787, label %b786
b716:
  store i64 0, ptr %gcs1
  %v1070 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1070, ptr %gcs1
  %v1071 = load i8, ptr %ctx, align 8
  %c254 = zext i8 %v1071 to i64
  %v1072 = icmp ne i64 %c254, 0
  br i1 %v1072, label %b722, label %b721
b717:
  %v1069 = icmp eq i64 %v1066, %v6
  br i1 %v1069, label %b719, label %b716
b718:
  %gcvolta718 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta718, ptr %ctxtopo, align 8
  ret i64 0
b719:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1181 = icmp eq i64 %v0, 0
  %v1182 = call i64 @df.class(i64 %v0)
  %v1183 = call i8 @df.subclass(i64 %v1182, i64 674)
  %c255 = zext i8 %v1183 to i64
  %v1184 = icmp ne i64 %c255, 0
  %v1185 = xor i1 %v1181, true
  %c256 = zext i1 %v1184 to i64
  %c257 = zext i1 %v1185 to i64
  %v1186 = and i64 %c256, %c257
  %v1187 = icmp ne i64 %v1186, 0
  br i1 %v1187, label %b797, label %b796
b720:
  store i64 0, ptr %gcs1
  %v1074 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1074, ptr %gcs1
  %v1075 = load i8, ptr %ctx, align 8
  %c258 = zext i8 %v1075 to i64
  %v1076 = icmp ne i64 %c258, 0
  br i1 %v1076, label %b726, label %b725
b721:
  %v1073 = icmp eq i64 %v1070, %v6
  br i1 %v1073, label %b723, label %b720
b722:
  %gcvolta722 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta722, ptr %ctxtopo, align 8
  ret i64 0
b723:
  %xcp1204 = load i8, ptr %ctx, align 8
  %xcn1204 = icmp ne i8 %xcp1204, 0
  %xce1204 = call i1 @llvm.expect.i1(i1 %xcn1204, i1 false)
  br i1 %xce1204, label %xc1204.limpar, label %xc1204.fim
xc1204.limpar:
  call void @dartforge_exception_clear()
  br label %xc1204.fim
xc1204.fim:
  %gcvolta723 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta723, ptr %ctxtopo, align 8
  ret i64 %v0
b724:
  store i64 0, ptr %gcs1
  %v1078 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1078, ptr %gcs1
  %v1079 = load i8, ptr %ctx, align 8
  %c259 = zext i8 %v1079 to i64
  %v1080 = icmp ne i64 %c259, 0
  br i1 %v1080, label %b730, label %b729
b725:
  %v1077 = icmp eq i64 %v1074, %v6
  br i1 %v1077, label %b727, label %b724
b726:
  %gcvolta726 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta726, ptr %ctxtopo, align 8
  ret i64 0
b727:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1206 = icmp eq i64 %v0, 0
  %v1207 = call i64 @df.class(i64 %v0)
  %v1208 = call i8 @df.subclass(i64 %v1207, i64 674)
  %c260 = zext i8 %v1208 to i64
  %v1209 = icmp ne i64 %c260, 0
  %v1210 = xor i1 %v1206, true
  %c261 = zext i1 %v1209 to i64
  %c262 = zext i1 %v1210 to i64
  %v1211 = and i64 %c261, %c262
  %v1212 = icmp ne i64 %v1211, 0
  br i1 %v1212, label %b808, label %b807
b728:
  store i64 0, ptr %gcs1
  %v1082 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1082, ptr %gcs1
  %v1083 = load i8, ptr %ctx, align 8
  %c263 = zext i8 %v1083 to i64
  %v1084 = icmp ne i64 %c263, 0
  br i1 %v1084, label %b734, label %b733
b729:
  %v1081 = icmp eq i64 %v1078, %v6
  br i1 %v1081, label %b731, label %b728
b730:
  %gcvolta730 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta730, ptr %ctxtopo, align 8
  ret i64 0
b731:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1229 = icmp eq i64 %v0, 0
  %v1230 = call i64 @df.class(i64 %v0)
  %v1231 = call i8 @df.subclass(i64 %v1230, i64 674)
  %c264 = zext i8 %v1231 to i64
  %v1232 = icmp ne i64 %c264, 0
  %v1233 = xor i1 %v1229, true
  %c265 = zext i1 %v1232 to i64
  %c266 = zext i1 %v1233 to i64
  %v1234 = and i64 %c265, %c266
  %v1235 = icmp ne i64 %v1234, 0
  br i1 %v1235, label %b818, label %b817
b732:
  store i64 0, ptr %gcs1
  %v1086 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1086, ptr %gcs1
  %v1087 = load i8, ptr %ctx, align 8
  %c267 = zext i8 %v1087 to i64
  %v1088 = icmp ne i64 %c267, 0
  br i1 %v1088, label %b738, label %b737
b733:
  %v1085 = icmp eq i64 %v1082, %v6
  br i1 %v1085, label %b735, label %b732
b734:
  %gcvolta734 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta734, ptr %ctxtopo, align 8
  ret i64 0
b735:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1252 = icmp eq i64 %v0, 0
  %v1253 = call i64 @df.class(i64 %v0)
  %v1254 = call i8 @df.subclass(i64 %v1253, i64 674)
  %c268 = zext i8 %v1254 to i64
  %v1255 = icmp ne i64 %c268, 0
  %v1256 = xor i1 %v1252, true
  %c269 = zext i1 %v1255 to i64
  %c270 = zext i1 %v1256 to i64
  %v1257 = and i64 %c269, %c270
  %v1258 = icmp ne i64 %v1257, 0
  br i1 %v1258, label %b828, label %b827
b736:
  store i64 0, ptr %gcs1
  %v1090 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1090, ptr %gcs1
  %v1091 = load i8, ptr %ctx, align 8
  %c271 = zext i8 %v1091 to i64
  %v1092 = icmp ne i64 %c271, 0
  br i1 %v1092, label %b742, label %b741
b737:
  %v1089 = icmp eq i64 %v1086, %v6
  br i1 %v1089, label %b739, label %b736
b738:
  %gcvolta738 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta738, ptr %ctxtopo, align 8
  ret i64 0
b739:
  %v1275 = add i64 0, 524296
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1276 = and i64 %v0, 1
  %v1277 = icmp ne i64 %v1276, 0
  br i1 %v1277, label %b837, label %b838
b740:
  store i64 0, ptr %gcs1
  %v1094 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1094, ptr %gcs1
  %v1095 = load i8, ptr %ctx, align 8
  %c272 = zext i8 %v1095 to i64
  %v1096 = icmp ne i64 %c272, 0
  br i1 %v1096, label %b747, label %b746
b741:
  %v1093 = icmp eq i64 %v1090, %v6
  br i1 %v1093, label %b743, label %b740
b742:
  %gcvolta742 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta742, ptr %ctxtopo, align 8
  ret i64 0
b743:
  br label %b703
b745:
  store i64 0, ptr %gcs1
  %v1098 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1098, ptr %gcs1
  %v1099 = load i8, ptr %ctx, align 8
  %c273 = zext i8 %v1099 to i64
  %v1100 = icmp ne i64 %c273, 0
  br i1 %v1100, label %b752, label %b751
b746:
  %v1097 = icmp eq i64 %v1094, %v6
  br i1 %v1097, label %b748, label %b745
b747:
  %gcvolta747 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta747, ptr %ctxtopo, align 8
  ret i64 0
b748:
  br label %b703
b750:
  br label %b12
b751:
  %v1101 = icmp eq i64 %v1098, %v6
  br i1 %v1101, label %b753, label %b750
b752:
  %gcvolta752 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta752, ptr %ctxtopo, align 8
  ret i64 0
b753:
  br label %b703
b754:
  %v1109 = icmp eq i64 %v0, %v1104
  br label %b756
b755:
  %v1110 = call i64 @df.class(i64 %v0)
  switch i64 %v1110, label %b760 [ i64 675, label %b757 i64 4, label %b758 i64 2, label %b759 i64 3, label %b759 ]
b756:
  %v1128 = phi i1 [ %v1109, %b754 ], [ %v1127, %b761 ]
  br i1 %v1128, label %b772, label %b773
b757:
  %v1111 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1104)
  %v1112 = load i8, ptr %ctx, align 8
  %c274 = zext i8 %v1112 to i64
  %v1113 = icmp ne i64 %c274, 0
  br i1 %v1113, label %b763, label %b762
b758:
  %v1115 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1104)
  %v1116 = load i8, ptr %ctx, align 8
  %c275 = zext i8 %v1116 to i64
  %v1117 = icmp ne i64 %c275, 0
  br i1 %v1117, label %b765, label %b764
b759:
  %v1118 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1104)
  %v1119 = load i8, ptr %ctx, align 8
  %c276 = zext i8 %v1119 to i64
  %v1120 = icmp ne i64 %c276, 0
  br i1 %v1120, label %b767, label %b766
b760:
  %sa1121_0 = getelementptr [2 x i64], ptr %sargs1121, i64 0, i64 0
  store i64 %v1104, ptr %sa1121_0
  %sat1121 = getelementptr [2 x i64], ptr %sargs1121, i64 0, i64 1
  store i64 0, ptr %sat1121
  %sic1121 = getelementptr i64, ptr %area, i64 216
  %sf1121 = call ptr @df.selector(ptr %sic1121, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1121 = call i64 %sf1121(i64 %v0, ptr %sargs1121, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1121, ptr %gcs0
  %v1122 = load i8, ptr %ctx, align 8
  %c277 = zext i8 %v1122 to i64
  %v1123 = icmp ne i64 %c277, 0
  br i1 %v1123, label %b769, label %b768
b761:
  %v1127 = phi i1 [ %v1114, %b762 ], [ %v1115, %b764 ], [ %v1118, %b766 ], [ %v1124, %b770 ]
  br label %b756
b762:
  %c278 = zext i8 %v1111 to i64
  %v1114 = icmp ne i64 %c278, 0
  br label %b761
b763:
  %gcvolta763 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta763, ptr %ctxtopo, align 8
  ret i64 0
b764:
  br label %b761
b765:
  %gcvolta765 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta765, ptr %ctxtopo, align 8
  ret i64 0
b766:
  br label %b761
b767:
  %gcvolta767 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta767, ptr %ctxtopo, align 8
  ret i64 0
b768:
  store i64 0, ptr %gcs1
  %v1124 = call i1 @df.unbox_bool(i64 %v1121)
  %v1125 = load i8, ptr %ctx, align 8
  %c279 = zext i8 %v1125 to i64
  %v1126 = icmp ne i64 %c279, 0
  br i1 %v1126, label %b771, label %b770
b769:
  %gcvolta769 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta769, ptr %ctxtopo, align 8
  ret i64 0
b770:
  br label %b761
b771:
  %gcvolta771 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta771, ptr %ctxtopo, align 8
  ret i64 0
b772:
  %v1129 = add i64 0, 0
  br label %b774
b773:
  %v1130 = add i64 0, 1
  br label %b774
b774:
  %v1131 = phi i64 [ %v1129, %b772 ], [ %v1130, %b773 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1132 = call i64 @df.box_int(i64 %v1131)
  %xcp1133 = load i8, ptr %ctx, align 8
  %xcn1133 = icmp ne i8 %xcp1133, 0
  %xce1133 = call i1 @llvm.expect.i1(i1 %xcn1133, i1 false)
  br i1 %xce1133, label %xc1133.limpar, label %xc1133.fim
xc1133.limpar:
  call void @dartforge_exception_clear()
  br label %xc1133.fim
xc1133.fim:
  %gcvolta774 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta774, ptr %ctxtopo, align 8
  ret i64 %v1132
b776:
  %ga1142 = getelementptr i64, ptr %area, i64 7
  %v1142 = load i8, ptr %ga1142
  %c280 = zext i8 %v1142 to i64
  %v1143 = icmp ne i64 %c280, 0
  br i1 %v1143, label %b778, label %b779
b777:
  %v1150 = call i64 @df.unbox_int(i64 %v0)
  %v1151 = load i8, ptr %ctx, align 8
  %c281 = zext i8 %v1151 to i64
  %v1152 = icmp ne i64 %c281, 0
  br i1 %v1152, label %b784, label %b783
b778:
  %ga1144 = getelementptr i64, ptr %area, i64 6
  %v1144 = load i64, ptr %ga1144
  br label %b780
b779:
  %v1145 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b780
b780:
  %v1146 = phi i64 [ %v1144, %b778 ], [ %v1145, %b779 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1146, i64 0, i64 0)
  %v1148 = load i8, ptr %ctx, align 8
  %c282 = zext i8 %v1148 to i64
  %v1149 = icmp ne i64 %c282, 0
  br i1 %v1149, label %b782, label %b781
b781:
  br label %b777
b782:
  %gcvolta782 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta782, ptr %ctxtopo, align 8
  ret i64 0
b783:
  %v1153 = add i64 0, 14
  %v1154 = ashr i64 %v1150, %v1153
  store i64 0, ptr %gcs0
  %v1155 = call i64 @df.box_int(i64 %v1154)
  %xcp1156 = load i8, ptr %ctx, align 8
  %xcn1156 = icmp ne i8 %xcp1156, 0
  %xce1156 = call i1 @llvm.expect.i1(i1 %xcn1156, i1 false)
  br i1 %xce1156, label %xc1156.limpar, label %xc1156.fim
xc1156.limpar:
  call void @dartforge_exception_clear()
  br label %xc1156.fim
xc1156.fim:
  %gcvolta783 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta783, ptr %ctxtopo, align 8
  ret i64 %v1155
b784:
  %gcvolta784 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta784, ptr %ctxtopo, align 8
  ret i64 0
b786:
  %ga1165 = getelementptr i64, ptr %area, i64 7
  %v1165 = load i8, ptr %ga1165
  %c283 = zext i8 %v1165 to i64
  %v1166 = icmp ne i64 %c283, 0
  br i1 %v1166, label %b788, label %b789
b787:
  %v1173 = call i64 @df.unbox_int(i64 %v0)
  %v1174 = load i8, ptr %ctx, align 8
  %c284 = zext i8 %v1174 to i64
  %v1175 = icmp ne i64 %c284, 0
  br i1 %v1175, label %b794, label %b793
b788:
  %ga1167 = getelementptr i64, ptr %area, i64 6
  %v1167 = load i64, ptr %ga1167
  br label %b790
b789:
  %v1168 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b790
b790:
  %v1169 = phi i64 [ %v1167, %b788 ], [ %v1168, %b789 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1169, i64 0, i64 0)
  %v1171 = load i8, ptr %ctx, align 8
  %c285 = zext i8 %v1171 to i64
  %v1172 = icmp ne i64 %c285, 0
  br i1 %v1172, label %b792, label %b791
b791:
  br label %b787
b792:
  %gcvolta792 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta792, ptr %ctxtopo, align 8
  ret i64 0
b793:
  %v1176 = add i64 0, 12
  %v1177 = ashr i64 %v1173, %v1176
  store i64 0, ptr %gcs0
  %v1178 = call i64 @df.box_int(i64 %v1177)
  %xcp1179 = load i8, ptr %ctx, align 8
  %xcn1179 = icmp ne i8 %xcp1179, 0
  %xce1179 = call i1 @llvm.expect.i1(i1 %xcn1179, i1 false)
  br i1 %xce1179, label %xc1179.limpar, label %xc1179.fim
xc1179.limpar:
  call void @dartforge_exception_clear()
  br label %xc1179.fim
xc1179.fim:
  %gcvolta793 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta793, ptr %ctxtopo, align 8
  ret i64 %v1178
b794:
  %gcvolta794 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta794, ptr %ctxtopo, align 8
  ret i64 0
b796:
  %ga1188 = getelementptr i64, ptr %area, i64 7
  %v1188 = load i8, ptr %ga1188
  %c286 = zext i8 %v1188 to i64
  %v1189 = icmp ne i64 %c286, 0
  br i1 %v1189, label %b798, label %b799
b797:
  %v1196 = call i64 @df.unbox_int(i64 %v0)
  %v1197 = load i8, ptr %ctx, align 8
  %c287 = zext i8 %v1197 to i64
  %v1198 = icmp ne i64 %c287, 0
  br i1 %v1198, label %b804, label %b803
b798:
  %ga1190 = getelementptr i64, ptr %area, i64 6
  %v1190 = load i64, ptr %ga1190
  br label %b800
b799:
  %v1191 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b800
b800:
  %v1192 = phi i64 [ %v1190, %b798 ], [ %v1191, %b799 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1192, i64 0, i64 0)
  %v1194 = load i8, ptr %ctx, align 8
  %c288 = zext i8 %v1194 to i64
  %v1195 = icmp ne i64 %c288, 0
  br i1 %v1195, label %b802, label %b801
b801:
  br label %b797
b802:
  %gcvolta802 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta802, ptr %ctxtopo, align 8
  ret i64 0
b803:
  %v1199 = add i64 0, 8
  %v1200 = ashr i64 %v1196, %v1199
  store i64 0, ptr %gcs0
  %v1201 = call i64 @df.box_int(i64 %v1200)
  %xcp1202 = load i8, ptr %ctx, align 8
  %xcn1202 = icmp ne i8 %xcp1202, 0
  %xce1202 = call i1 @llvm.expect.i1(i1 %xcn1202, i1 false)
  br i1 %xce1202, label %xc1202.limpar, label %xc1202.fim
xc1202.limpar:
  call void @dartforge_exception_clear()
  br label %xc1202.fim
xc1202.fim:
  %gcvolta803 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta803, ptr %ctxtopo, align 8
  ret i64 %v1201
b804:
  %gcvolta804 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta804, ptr %ctxtopo, align 8
  ret i64 0
b807:
  %ga1213 = getelementptr i64, ptr %area, i64 7
  %v1213 = load i8, ptr %ga1213
  %c289 = zext i8 %v1213 to i64
  %v1214 = icmp ne i64 %c289, 0
  br i1 %v1214, label %b809, label %b810
b808:
  %v1221 = call i64 @df.unbox_int(i64 %v0)
  %v1222 = load i8, ptr %ctx, align 8
  %c290 = zext i8 %v1222 to i64
  %v1223 = icmp ne i64 %c290, 0
  br i1 %v1223, label %b815, label %b814
b809:
  %ga1215 = getelementptr i64, ptr %area, i64 6
  %v1215 = load i64, ptr %ga1215
  br label %b811
b810:
  %v1216 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b811
b811:
  %v1217 = phi i64 [ %v1215, %b809 ], [ %v1216, %b810 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1217, i64 0, i64 0)
  %v1219 = load i8, ptr %ctx, align 8
  %c291 = zext i8 %v1219 to i64
  %v1220 = icmp ne i64 %c291, 0
  br i1 %v1220, label %b813, label %b812
b812:
  br label %b808
b813:
  %gcvolta813 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta813, ptr %ctxtopo, align 8
  ret i64 0
b814:
  %v1224 = add i64 0, 8
  %v1225 = shl i64 %v1221, %v1224
  store i64 0, ptr %gcs0
  %v1226 = call i64 @df.box_int(i64 %v1225)
  %xcp1227 = load i8, ptr %ctx, align 8
  %xcn1227 = icmp ne i8 %xcp1227, 0
  %xce1227 = call i1 @llvm.expect.i1(i1 %xcn1227, i1 false)
  br i1 %xce1227, label %xc1227.limpar, label %xc1227.fim
xc1227.limpar:
  call void @dartforge_exception_clear()
  br label %xc1227.fim
xc1227.fim:
  %gcvolta814 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta814, ptr %ctxtopo, align 8
  ret i64 %v1226
b815:
  %gcvolta815 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta815, ptr %ctxtopo, align 8
  ret i64 0
b817:
  %ga1236 = getelementptr i64, ptr %area, i64 7
  %v1236 = load i8, ptr %ga1236
  %c292 = zext i8 %v1236 to i64
  %v1237 = icmp ne i64 %c292, 0
  br i1 %v1237, label %b819, label %b820
b818:
  %v1244 = call i64 @df.unbox_int(i64 %v0)
  %v1245 = load i8, ptr %ctx, align 8
  %c293 = zext i8 %v1245 to i64
  %v1246 = icmp ne i64 %c293, 0
  br i1 %v1246, label %b825, label %b824
b819:
  %ga1238 = getelementptr i64, ptr %area, i64 6
  %v1238 = load i64, ptr %ga1238
  br label %b821
b820:
  %v1239 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b821
b821:
  %v1240 = phi i64 [ %v1238, %b819 ], [ %v1239, %b820 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1240, i64 0, i64 0)
  %v1242 = load i8, ptr %ctx, align 8
  %c294 = zext i8 %v1242 to i64
  %v1243 = icmp ne i64 %c294, 0
  br i1 %v1243, label %b823, label %b822
b822:
  br label %b818
b823:
  %gcvolta823 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta823, ptr %ctxtopo, align 8
  ret i64 0
b824:
  %v1247 = add i64 0, 9
  %v1248 = ashr i64 %v1244, %v1247
  store i64 0, ptr %gcs0
  %v1249 = call i64 @df.box_int(i64 %v1248)
  %xcp1250 = load i8, ptr %ctx, align 8
  %xcn1250 = icmp ne i8 %xcp1250, 0
  %xce1250 = call i1 @llvm.expect.i1(i1 %xcn1250, i1 false)
  br i1 %xce1250, label %xc1250.limpar, label %xc1250.fim
xc1250.limpar:
  call void @dartforge_exception_clear()
  br label %xc1250.fim
xc1250.fim:
  %gcvolta824 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta824, ptr %ctxtopo, align 8
  ret i64 %v1249
b825:
  %gcvolta825 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta825, ptr %ctxtopo, align 8
  ret i64 0
b827:
  %ga1259 = getelementptr i64, ptr %area, i64 7
  %v1259 = load i8, ptr %ga1259
  %c295 = zext i8 %v1259 to i64
  %v1260 = icmp ne i64 %c295, 0
  br i1 %v1260, label %b829, label %b830
b828:
  %v1267 = call i64 @df.unbox_int(i64 %v0)
  %v1268 = load i8, ptr %ctx, align 8
  %c296 = zext i8 %v1268 to i64
  %v1269 = icmp ne i64 %c296, 0
  br i1 %v1269, label %b835, label %b834
b829:
  %ga1261 = getelementptr i64, ptr %area, i64 6
  %v1261 = load i64, ptr %ga1261
  br label %b831
b830:
  %v1262 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b831
b831:
  %v1263 = phi i64 [ %v1261, %b829 ], [ %v1262, %b830 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1263, i64 0, i64 0)
  %v1265 = load i8, ptr %ctx, align 8
  %c297 = zext i8 %v1265 to i64
  %v1266 = icmp ne i64 %c297, 0
  br i1 %v1266, label %b833, label %b832
b832:
  br label %b828
b833:
  %gcvolta833 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta833, ptr %ctxtopo, align 8
  ret i64 0
b834:
  %v1270 = add i64 0, 1
  %v1271 = ashr i64 %v1267, %v1270
  store i64 0, ptr %gcs0
  %v1272 = call i64 @df.box_int(i64 %v1271)
  %xcp1273 = load i8, ptr %ctx, align 8
  %xcn1273 = icmp ne i8 %xcp1273, 0
  %xce1273 = call i1 @llvm.expect.i1(i1 %xcn1273, i1 false)
  br i1 %xce1273, label %xc1273.limpar, label %xc1273.fim
xc1273.limpar:
  call void @dartforge_exception_clear()
  br label %xc1273.fim
xc1273.fim:
  %gcvolta834 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta834, ptr %ctxtopo, align 8
  ret i64 %v1272
b835:
  %gcvolta835 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta835, ptr %ctxtopo, align 8
  ret i64 0
b837:
  %v1278 = ashr i64 %v0, 1
  %v1279 = mul i64 %v1278, %v1275
  store i64 0, ptr %gcs0
  %v1280 = call i64 @df.box_int(i64 %v1279)
  br label %b839
b838:
  %v1281 = call i64 @df.class(i64 %v0)
  %v1282 = icmp eq i64 %v1281, 2
  %v1283 = icmp eq i64 %v1281, 3
  %v1284 = icmp eq i64 %v1281, 4
  %c298 = zext i1 %v1282 to i64
  %c299 = zext i1 %v1283 to i64
  %v1285 = or i64 %c298, %c299
  %c300 = zext i1 %v1284 to i64
  %v1286 = or i64 %v1285, %c300
  %c301 = trunc i64 %v1286 to i1
  br i1 %c301, label %b840, label %b841
b839:
  %v1305 = phi i64 [ %v1280, %b837 ], [ %v1304, %b842 ]
  %xcp1306 = load i8, ptr %ctx, align 8
  %xcn1306 = icmp ne i8 %xcp1306, 0
  %xce1306 = call i1 @llvm.expect.i1(i1 %xcn1306, i1 false)
  br i1 %xce1306, label %xc1306.limpar, label %xc1306.fim
xc1306.limpar:
  call void @dartforge_exception_clear()
  br label %xc1306.fim
xc1306.fim:
  %gcvolta839 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta839, ptr %ctxtopo, align 8
  ret i64 %v1305
b840:
  br i1 %v1284, label %b844, label %b843
b841:
  %v1300 = call i64 @df.box_int(i64 %v1275)
  store i64 %v1300, ptr %gcs1
  %sa1301_0 = getelementptr [2 x i64], ptr %sargs1301, i64 0, i64 0
  store i64 %v1300, ptr %sa1301_0
  %sat1301 = getelementptr [2 x i64], ptr %sargs1301, i64 0, i64 1
  store i64 0, ptr %sat1301
  %sic1301 = getelementptr i64, ptr %area, i64 218
  %sf1301 = call ptr @df.selector(ptr %sic1301, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v1301 = call i64 %sf1301(i64 %v0, ptr %sargs1301, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1302 = load i8, ptr %ctx, align 8
  %c302 = zext i8 %v1302 to i64
  %v1303 = icmp ne i64 %c302, 0
  br i1 %v1303, label %b851, label %b850
b842:
  %v1304 = phi i64 [ %v1299, %b845 ], [ %v1301, %b850 ]
  br label %b839
b843:
  %v1288 = call i64 @df.unbox_int(i64 %v0)
  %v1289 = load i8, ptr %ctx, align 8
  %c303 = zext i8 %v1289 to i64
  %v1290 = icmp ne i64 %c303, 0
  br i1 %v1290, label %b847, label %b846
b844:
  %v1293 = call double @df.num_to_double(i64 %v0)
  %v1294 = load i8, ptr %ctx, align 8
  %c304 = zext i8 %v1294 to i64
  %v1295 = icmp ne i64 %c304, 0
  br i1 %v1295, label %b849, label %b848
b845:
  %v1299 = phi i64 [ %v1292, %b846 ], [ %v1298, %b848 ]
  br label %b842
b846:
  %v1291 = mul i64 %v1288, %v1275
  store i64 0, ptr %gcs0
  %v1292 = call i64 @df.box_int(i64 %v1291)
  br label %b845
b847:
  %gcvolta847 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta847, ptr %ctxtopo, align 8
  ret i64 0
b848:
  %v1296 = sitofp i64 %v1275 to double
  %v1297 = fmul double %v1293, %v1296
  store i64 0, ptr %gcs0
  %v1298 = call i64 @df.box_double(double %v1297)
  br label %b845
b849:
  %gcvolta849 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta849, ptr %ctxtopo, align 8
  ret i64 0
b850:
  br label %b842
b851:
  %gcvolta851 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta851, ptr %ctxtopo, align 8
  ret i64 0
b853:
  store i64 0, ptr %gcs1
  %v1313 = call double @df.unbox_double(i64 %v1310)
  %v1314 = load i8, ptr %ctx, align 8
  %c305 = zext i8 %v1314 to i64
  %v1315 = icmp ne i64 %c305, 0
  br i1 %v1315, label %b856, label %b855
b854:
  %gcvolta854 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta854, ptr %ctxtopo, align 8
  ret i64 0
b855:
  %xcp1317 = load i8, ptr %ctx, align 8
  %xcn1317 = icmp ne i8 %xcp1317, 0
  %xce1317 = call i1 @llvm.expect.i1(i1 %xcn1317, i1 false)
  br i1 %xce1317, label %xc1317.limpar, label %xc1317.fim
xc1317.limpar:
  call void @dartforge_exception_clear()
  br label %xc1317.fim
xc1317.fim:
  %gcvolta855 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta855, ptr %ctxtopo, align 8
  ret i64 %v1310
b856:
  %gcvolta856 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta856, ptr %ctxtopo, align 8
  ret i64 0
b870:
  %v1563 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1564 = call i64 @df.box_int(i64 %v1563)
  store i64 %v1564, ptr %gcs1
  %sa1565_0 = getelementptr [2 x i64], ptr %sargs1565, i64 0, i64 0
  store i64 %v1564, ptr %sa1565_0
  %sat1565 = getelementptr [2 x i64], ptr %sargs1565, i64 0, i64 1
  store i64 0, ptr %sat1565
  %sic1565 = getelementptr i64, ptr %area, i64 220
  %sf1565 = call ptr @df.selector(ptr %sic1565, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1565 = call i64 %sf1565(i64 %v0, ptr %sargs1565, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1565, ptr %gcs0
  %v1566 = load i8, ptr %ctx, align 8
  %c306 = zext i8 %v1566 to i64
  %v1567 = icmp ne i64 %c306, 0
  br i1 %v1567, label %b1015, label %b1014
b871:
  store i64 0, ptr %gcs1
  %v1323 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1323, ptr %gcs1
  %v1324 = load i8, ptr %ctx, align 8
  %c307 = zext i8 %v1324 to i64
  %v1325 = icmp ne i64 %c307, 0
  br i1 %v1325, label %b877, label %b876
b872:
  %v1322 = icmp eq i64 %v1319, %v6
  br i1 %v1322, label %b874, label %b871
b873:
  %gcvolta873 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta873, ptr %ctxtopo, align 8
  ret i64 0
b874:
  %v1368 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1369 = call i64 @df.box_int(i64 %v1368)
  store i64 %v1369, ptr %gcs1
  %v1370 = icmp eq i64 %v0, 0
  %v1371 = icmp eq i64 %v1369, 0
  %c308 = zext i1 %v1370 to i64
  %c309 = zext i1 %v1371 to i64
  %v1372 = or i64 %c308, %c309
  %v1373 = icmp ne i64 %v1372, 0
  br i1 %v1373, label %b921, label %b922
b875:
  store i64 0, ptr %gcs1
  %v1327 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1327, ptr %gcs1
  %v1328 = load i8, ptr %ctx, align 8
  %c310 = zext i8 %v1328 to i64
  %v1329 = icmp ne i64 %c310, 0
  br i1 %v1329, label %b881, label %b880
b876:
  %v1326 = icmp eq i64 %v1323, %v6
  br i1 %v1326, label %b878, label %b875
b877:
  %gcvolta877 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta877, ptr %ctxtopo, align 8
  ret i64 0
b878:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1400 = icmp eq i64 %v0, 0
  %v1401 = call i64 @df.class(i64 %v0)
  %v1402 = call i8 @df.subclass(i64 %v1401, i64 674)
  %c311 = zext i8 %v1402 to i64
  %v1403 = icmp ne i64 %c311, 0
  %v1404 = xor i1 %v1400, true
  %c312 = zext i1 %v1403 to i64
  %c313 = zext i1 %v1404 to i64
  %v1405 = and i64 %c312, %c313
  %v1406 = icmp ne i64 %v1405, 0
  br i1 %v1406, label %b944, label %b943
b879:
  store i64 0, ptr %gcs1
  %v1331 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1331, ptr %gcs1
  %v1332 = load i8, ptr %ctx, align 8
  %c314 = zext i8 %v1332 to i64
  %v1333 = icmp ne i64 %c314, 0
  br i1 %v1333, label %b885, label %b884
b880:
  %v1330 = icmp eq i64 %v1327, %v6
  br i1 %v1330, label %b882, label %b879
b881:
  %gcvolta881 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta881, ptr %ctxtopo, align 8
  ret i64 0
b882:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1423 = icmp eq i64 %v0, 0
  %v1424 = call i64 @df.class(i64 %v0)
  %v1425 = call i8 @df.subclass(i64 %v1424, i64 674)
  %c315 = zext i8 %v1425 to i64
  %v1426 = icmp ne i64 %c315, 0
  %v1427 = xor i1 %v1423, true
  %c316 = zext i1 %v1426 to i64
  %c317 = zext i1 %v1427 to i64
  %v1428 = and i64 %c316, %c317
  %v1429 = icmp ne i64 %v1428, 0
  br i1 %v1429, label %b954, label %b953
b883:
  store i64 0, ptr %gcs1
  %v1335 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1335, ptr %gcs1
  %v1336 = load i8, ptr %ctx, align 8
  %c318 = zext i8 %v1336 to i64
  %v1337 = icmp ne i64 %c318, 0
  br i1 %v1337, label %b889, label %b888
b884:
  %v1334 = icmp eq i64 %v1331, %v6
  br i1 %v1334, label %b886, label %b883
b885:
  %gcvolta885 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta885, ptr %ctxtopo, align 8
  ret i64 0
b886:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1446 = icmp eq i64 %v0, 0
  %v1447 = call i64 @df.class(i64 %v0)
  %v1448 = call i8 @df.subclass(i64 %v1447, i64 674)
  %c319 = zext i8 %v1448 to i64
  %v1449 = icmp ne i64 %c319, 0
  %v1450 = xor i1 %v1446, true
  %c320 = zext i1 %v1449 to i64
  %c321 = zext i1 %v1450 to i64
  %v1451 = and i64 %c320, %c321
  %v1452 = icmp ne i64 %v1451, 0
  br i1 %v1452, label %b964, label %b963
b887:
  store i64 0, ptr %gcs1
  %v1339 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1339, ptr %gcs1
  %v1340 = load i8, ptr %ctx, align 8
  %c322 = zext i8 %v1340 to i64
  %v1341 = icmp ne i64 %c322, 0
  br i1 %v1341, label %b893, label %b892
b888:
  %v1338 = icmp eq i64 %v1335, %v6
  br i1 %v1338, label %b890, label %b887
b889:
  %gcvolta889 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta889, ptr %ctxtopo, align 8
  ret i64 0
b890:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1469 = icmp eq i64 %v0, 0
  %v1470 = call i64 @df.class(i64 %v0)
  %v1471 = call i8 @df.subclass(i64 %v1470, i64 674)
  %c323 = zext i8 %v1471 to i64
  %v1472 = icmp ne i64 %c323, 0
  %v1473 = xor i1 %v1469, true
  %c324 = zext i1 %v1472 to i64
  %c325 = zext i1 %v1473 to i64
  %v1474 = and i64 %c324, %c325
  %v1475 = icmp ne i64 %v1474, 0
  br i1 %v1475, label %b974, label %b973
b891:
  store i64 0, ptr %gcs1
  %v1343 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1343, ptr %gcs1
  %v1344 = load i8, ptr %ctx, align 8
  %c326 = zext i8 %v1344 to i64
  %v1345 = icmp ne i64 %c326, 0
  br i1 %v1345, label %b897, label %b896
b892:
  %v1342 = icmp eq i64 %v1339, %v6
  br i1 %v1342, label %b894, label %b891
b893:
  %gcvolta893 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta893, ptr %ctxtopo, align 8
  ret i64 0
b894:
  %xcp1492 = load i8, ptr %ctx, align 8
  %xcn1492 = icmp ne i8 %xcp1492, 0
  %xce1492 = call i1 @llvm.expect.i1(i1 %xcn1492, i1 false)
  br i1 %xce1492, label %xc1492.limpar, label %xc1492.fim
xc1492.limpar:
  call void @dartforge_exception_clear()
  br label %xc1492.fim
xc1492.fim:
  %gcvolta894 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta894, ptr %ctxtopo, align 8
  ret i64 %v0
b895:
  store i64 0, ptr %gcs1
  %v1347 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1347, ptr %gcs1
  %v1348 = load i8, ptr %ctx, align 8
  %c327 = zext i8 %v1348 to i64
  %v1349 = icmp ne i64 %c327, 0
  br i1 %v1349, label %b901, label %b900
b896:
  %v1346 = icmp eq i64 %v1343, %v6
  br i1 %v1346, label %b898, label %b895
b897:
  %gcvolta897 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta897, ptr %ctxtopo, align 8
  ret i64 0
b898:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1494 = icmp eq i64 %v0, 0
  %v1495 = call i64 @df.class(i64 %v0)
  %v1496 = call i8 @df.subclass(i64 %v1495, i64 674)
  %c328 = zext i8 %v1496 to i64
  %v1497 = icmp ne i64 %c328, 0
  %v1498 = xor i1 %v1494, true
  %c329 = zext i1 %v1497 to i64
  %c330 = zext i1 %v1498 to i64
  %v1499 = and i64 %c329, %c330
  %v1500 = icmp ne i64 %v1499, 0
  br i1 %v1500, label %b985, label %b984
b899:
  store i64 0, ptr %gcs1
  %v1351 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1351, ptr %gcs1
  %v1352 = load i8, ptr %ctx, align 8
  %c331 = zext i8 %v1352 to i64
  %v1353 = icmp ne i64 %c331, 0
  br i1 %v1353, label %b905, label %b904
b900:
  %v1350 = icmp eq i64 %v1347, %v6
  br i1 %v1350, label %b902, label %b899
b901:
  %gcvolta901 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta901, ptr %ctxtopo, align 8
  ret i64 0
b902:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1517 = icmp eq i64 %v0, 0
  %v1518 = call i64 @df.class(i64 %v0)
  %v1519 = call i8 @df.subclass(i64 %v1518, i64 674)
  %c332 = zext i8 %v1519 to i64
  %v1520 = icmp ne i64 %c332, 0
  %v1521 = xor i1 %v1517, true
  %c333 = zext i1 %v1520 to i64
  %c334 = zext i1 %v1521 to i64
  %v1522 = and i64 %c333, %c334
  %v1523 = icmp ne i64 %v1522, 0
  br i1 %v1523, label %b995, label %b994
b903:
  store i64 0, ptr %gcs1
  %v1355 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1355, ptr %gcs1
  %v1356 = load i8, ptr %ctx, align 8
  %c335 = zext i8 %v1356 to i64
  %v1357 = icmp ne i64 %c335, 0
  br i1 %v1357, label %b909, label %b908
b904:
  %v1354 = icmp eq i64 %v1351, %v6
  br i1 %v1354, label %b906, label %b903
b905:
  %gcvolta905 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta905, ptr %ctxtopo, align 8
  ret i64 0
b906:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1540 = icmp eq i64 %v0, 0
  %v1541 = call i64 @df.class(i64 %v0)
  %v1542 = call i8 @df.subclass(i64 %v1541, i64 674)
  %c336 = zext i8 %v1542 to i64
  %v1543 = icmp ne i64 %c336, 0
  %v1544 = xor i1 %v1540, true
  %c337 = zext i1 %v1543 to i64
  %c338 = zext i1 %v1544 to i64
  %v1545 = and i64 %c337, %c338
  %v1546 = icmp ne i64 %v1545, 0
  br i1 %v1546, label %b1005, label %b1004
b907:
  store i64 0, ptr %gcs1
  %v1359 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1359, ptr %gcs1
  %v1360 = load i8, ptr %ctx, align 8
  %c339 = zext i8 %v1360 to i64
  %v1361 = icmp ne i64 %c339, 0
  br i1 %v1361, label %b914, label %b913
b908:
  %v1358 = icmp eq i64 %v1355, %v6
  br i1 %v1358, label %b910, label %b907
b909:
  %gcvolta909 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta909, ptr %ctxtopo, align 8
  ret i64 0
b910:
  br label %b870
b912:
  store i64 0, ptr %gcs1
  %v1363 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1363, ptr %gcs1
  %v1364 = load i8, ptr %ctx, align 8
  %c340 = zext i8 %v1364 to i64
  %v1365 = icmp ne i64 %c340, 0
  br i1 %v1365, label %b919, label %b918
b913:
  %v1362 = icmp eq i64 %v1359, %v6
  br i1 %v1362, label %b915, label %b912
b914:
  %gcvolta914 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta914, ptr %ctxtopo, align 8
  ret i64 0
b915:
  br label %b870
b917:
  br label %b12
b918:
  %v1366 = icmp eq i64 %v1363, %v6
  br i1 %v1366, label %b920, label %b917
b919:
  %gcvolta919 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta919, ptr %ctxtopo, align 8
  ret i64 0
b920:
  br label %b870
b921:
  %v1374 = icmp eq i64 %v0, %v1369
  br label %b923
b922:
  %v1375 = call i64 @df.class(i64 %v0)
  switch i64 %v1375, label %b927 [ i64 675, label %b924 i64 4, label %b925 i64 2, label %b926 i64 3, label %b926 ]
b923:
  %v1393 = phi i1 [ %v1374, %b921 ], [ %v1392, %b928 ]
  br i1 %v1393, label %b939, label %b940
b924:
  %v1376 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1369)
  %v1377 = load i8, ptr %ctx, align 8
  %c341 = zext i8 %v1377 to i64
  %v1378 = icmp ne i64 %c341, 0
  br i1 %v1378, label %b930, label %b929
b925:
  %v1380 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1369)
  %v1381 = load i8, ptr %ctx, align 8
  %c342 = zext i8 %v1381 to i64
  %v1382 = icmp ne i64 %c342, 0
  br i1 %v1382, label %b932, label %b931
b926:
  %v1383 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1369)
  %v1384 = load i8, ptr %ctx, align 8
  %c343 = zext i8 %v1384 to i64
  %v1385 = icmp ne i64 %c343, 0
  br i1 %v1385, label %b934, label %b933
b927:
  %sa1386_0 = getelementptr [2 x i64], ptr %sargs1386, i64 0, i64 0
  store i64 %v1369, ptr %sa1386_0
  %sat1386 = getelementptr [2 x i64], ptr %sargs1386, i64 0, i64 1
  store i64 0, ptr %sat1386
  %sic1386 = getelementptr i64, ptr %area, i64 222
  %sf1386 = call ptr @df.selector(ptr %sic1386, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1386 = call i64 %sf1386(i64 %v0, ptr %sargs1386, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1386, ptr %gcs0
  %v1387 = load i8, ptr %ctx, align 8
  %c344 = zext i8 %v1387 to i64
  %v1388 = icmp ne i64 %c344, 0
  br i1 %v1388, label %b936, label %b935
b928:
  %v1392 = phi i1 [ %v1379, %b929 ], [ %v1380, %b931 ], [ %v1383, %b933 ], [ %v1389, %b937 ]
  br label %b923
b929:
  %c345 = zext i8 %v1376 to i64
  %v1379 = icmp ne i64 %c345, 0
  br label %b928
b930:
  %gcvolta930 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta930, ptr %ctxtopo, align 8
  ret i64 0
b931:
  br label %b928
b932:
  %gcvolta932 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta932, ptr %ctxtopo, align 8
  ret i64 0
b933:
  br label %b928
b934:
  %gcvolta934 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta934, ptr %ctxtopo, align 8
  ret i64 0
b935:
  store i64 0, ptr %gcs1
  %v1389 = call i1 @df.unbox_bool(i64 %v1386)
  %v1390 = load i8, ptr %ctx, align 8
  %c346 = zext i8 %v1390 to i64
  %v1391 = icmp ne i64 %c346, 0
  br i1 %v1391, label %b938, label %b937
b936:
  %gcvolta936 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta936, ptr %ctxtopo, align 8
  ret i64 0
b937:
  br label %b928
b938:
  %gcvolta938 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta938, ptr %ctxtopo, align 8
  ret i64 0
b939:
  %v1394 = add i64 0, 0
  br label %b941
b940:
  %v1395 = add i64 0, 1
  br label %b941
b941:
  %v1396 = phi i64 [ %v1394, %b939 ], [ %v1395, %b940 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1397 = call i64 @df.box_int(i64 %v1396)
  %xcp1398 = load i8, ptr %ctx, align 8
  %xcn1398 = icmp ne i8 %xcp1398, 0
  %xce1398 = call i1 @llvm.expect.i1(i1 %xcn1398, i1 false)
  br i1 %xce1398, label %xc1398.limpar, label %xc1398.fim
xc1398.limpar:
  call void @dartforge_exception_clear()
  br label %xc1398.fim
xc1398.fim:
  %gcvolta941 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta941, ptr %ctxtopo, align 8
  ret i64 %v1397
b943:
  %ga1407 = getelementptr i64, ptr %area, i64 7
  %v1407 = load i8, ptr %ga1407
  %c347 = zext i8 %v1407 to i64
  %v1408 = icmp ne i64 %c347, 0
  br i1 %v1408, label %b945, label %b946
b944:
  %v1415 = call i64 @df.unbox_int(i64 %v0)
  %v1416 = load i8, ptr %ctx, align 8
  %c348 = zext i8 %v1416 to i64
  %v1417 = icmp ne i64 %c348, 0
  br i1 %v1417, label %b951, label %b950
b945:
  %ga1409 = getelementptr i64, ptr %area, i64 6
  %v1409 = load i64, ptr %ga1409
  br label %b947
b946:
  %v1410 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b947
b947:
  %v1411 = phi i64 [ %v1409, %b945 ], [ %v1410, %b946 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1411, i64 0, i64 0)
  %v1413 = load i8, ptr %ctx, align 8
  %c349 = zext i8 %v1413 to i64
  %v1414 = icmp ne i64 %c349, 0
  br i1 %v1414, label %b949, label %b948
b948:
  br label %b944
b949:
  %gcvolta949 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta949, ptr %ctxtopo, align 8
  ret i64 0
b950:
  %v1418 = add i64 0, 30
  %v1419 = ashr i64 %v1415, %v1418
  store i64 0, ptr %gcs0
  %v1420 = call i64 @df.box_int(i64 %v1419)
  %xcp1421 = load i8, ptr %ctx, align 8
  %xcn1421 = icmp ne i8 %xcp1421, 0
  %xce1421 = call i1 @llvm.expect.i1(i1 %xcn1421, i1 false)
  br i1 %xce1421, label %xc1421.limpar, label %xc1421.fim
xc1421.limpar:
  call void @dartforge_exception_clear()
  br label %xc1421.fim
xc1421.fim:
  %gcvolta950 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta950, ptr %ctxtopo, align 8
  ret i64 %v1420
b951:
  %gcvolta951 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta951, ptr %ctxtopo, align 8
  ret i64 0
b953:
  %ga1430 = getelementptr i64, ptr %area, i64 7
  %v1430 = load i8, ptr %ga1430
  %c350 = zext i8 %v1430 to i64
  %v1431 = icmp ne i64 %c350, 0
  br i1 %v1431, label %b955, label %b956
b954:
  %v1438 = call i64 @df.unbox_int(i64 %v0)
  %v1439 = load i8, ptr %ctx, align 8
  %c351 = zext i8 %v1439 to i64
  %v1440 = icmp ne i64 %c351, 0
  br i1 %v1440, label %b961, label %b960
b955:
  %ga1432 = getelementptr i64, ptr %area, i64 6
  %v1432 = load i64, ptr %ga1432
  br label %b957
b956:
  %v1433 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b957
b957:
  %v1434 = phi i64 [ %v1432, %b955 ], [ %v1433, %b956 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1434, i64 0, i64 0)
  %v1436 = load i8, ptr %ctx, align 8
  %c352 = zext i8 %v1436 to i64
  %v1437 = icmp ne i64 %c352, 0
  br i1 %v1437, label %b959, label %b958
b958:
  br label %b954
b959:
  %gcvolta959 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta959, ptr %ctxtopo, align 8
  ret i64 0
b960:
  %v1441 = add i64 0, 28
  %v1442 = ashr i64 %v1438, %v1441
  store i64 0, ptr %gcs0
  %v1443 = call i64 @df.box_int(i64 %v1442)
  %xcp1444 = load i8, ptr %ctx, align 8
  %xcn1444 = icmp ne i8 %xcp1444, 0
  %xce1444 = call i1 @llvm.expect.i1(i1 %xcn1444, i1 false)
  br i1 %xce1444, label %xc1444.limpar, label %xc1444.fim
xc1444.limpar:
  call void @dartforge_exception_clear()
  br label %xc1444.fim
xc1444.fim:
  %gcvolta960 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta960, ptr %ctxtopo, align 8
  ret i64 %v1443
b961:
  %gcvolta961 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta961, ptr %ctxtopo, align 8
  ret i64 0
b963:
  %ga1453 = getelementptr i64, ptr %area, i64 7
  %v1453 = load i8, ptr %ga1453
  %c353 = zext i8 %v1453 to i64
  %v1454 = icmp ne i64 %c353, 0
  br i1 %v1454, label %b965, label %b966
b964:
  %v1461 = call i64 @df.unbox_int(i64 %v0)
  %v1462 = load i8, ptr %ctx, align 8
  %c354 = zext i8 %v1462 to i64
  %v1463 = icmp ne i64 %c354, 0
  br i1 %v1463, label %b971, label %b970
b965:
  %ga1455 = getelementptr i64, ptr %area, i64 6
  %v1455 = load i64, ptr %ga1455
  br label %b967
b966:
  %v1456 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b967
b967:
  %v1457 = phi i64 [ %v1455, %b965 ], [ %v1456, %b966 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1457, i64 0, i64 0)
  %v1459 = load i8, ptr %ctx, align 8
  %c355 = zext i8 %v1459 to i64
  %v1460 = icmp ne i64 %c355, 0
  br i1 %v1460, label %b969, label %b968
b968:
  br label %b964
b969:
  %gcvolta969 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta969, ptr %ctxtopo, align 8
  ret i64 0
b970:
  %v1464 = add i64 0, 24
  %v1465 = ashr i64 %v1461, %v1464
  store i64 0, ptr %gcs0
  %v1466 = call i64 @df.box_int(i64 %v1465)
  %xcp1467 = load i8, ptr %ctx, align 8
  %xcn1467 = icmp ne i8 %xcp1467, 0
  %xce1467 = call i1 @llvm.expect.i1(i1 %xcn1467, i1 false)
  br i1 %xce1467, label %xc1467.limpar, label %xc1467.fim
xc1467.limpar:
  call void @dartforge_exception_clear()
  br label %xc1467.fim
xc1467.fim:
  %gcvolta970 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta970, ptr %ctxtopo, align 8
  ret i64 %v1466
b971:
  %gcvolta971 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta971, ptr %ctxtopo, align 8
  ret i64 0
b973:
  %ga1476 = getelementptr i64, ptr %area, i64 7
  %v1476 = load i8, ptr %ga1476
  %c356 = zext i8 %v1476 to i64
  %v1477 = icmp ne i64 %c356, 0
  br i1 %v1477, label %b975, label %b976
b974:
  %v1484 = call i64 @df.unbox_int(i64 %v0)
  %v1485 = load i8, ptr %ctx, align 8
  %c357 = zext i8 %v1485 to i64
  %v1486 = icmp ne i64 %c357, 0
  br i1 %v1486, label %b981, label %b980
b975:
  %ga1478 = getelementptr i64, ptr %area, i64 6
  %v1478 = load i64, ptr %ga1478
  br label %b977
b976:
  %v1479 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b977
b977:
  %v1480 = phi i64 [ %v1478, %b975 ], [ %v1479, %b976 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1480, i64 0, i64 0)
  %v1482 = load i8, ptr %ctx, align 8
  %c358 = zext i8 %v1482 to i64
  %v1483 = icmp ne i64 %c358, 0
  br i1 %v1483, label %b979, label %b978
b978:
  br label %b974
b979:
  %gcvolta979 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta979, ptr %ctxtopo, align 8
  ret i64 0
b980:
  %v1487 = add i64 0, 16
  %v1488 = ashr i64 %v1484, %v1487
  store i64 0, ptr %gcs0
  %v1489 = call i64 @df.box_int(i64 %v1488)
  %xcp1490 = load i8, ptr %ctx, align 8
  %xcn1490 = icmp ne i8 %xcp1490, 0
  %xce1490 = call i1 @llvm.expect.i1(i1 %xcn1490, i1 false)
  br i1 %xce1490, label %xc1490.limpar, label %xc1490.fim
xc1490.limpar:
  call void @dartforge_exception_clear()
  br label %xc1490.fim
xc1490.fim:
  %gcvolta980 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta980, ptr %ctxtopo, align 8
  ret i64 %v1489
b981:
  %gcvolta981 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta981, ptr %ctxtopo, align 8
  ret i64 0
b984:
  %ga1501 = getelementptr i64, ptr %area, i64 7
  %v1501 = load i8, ptr %ga1501
  %c359 = zext i8 %v1501 to i64
  %v1502 = icmp ne i64 %c359, 0
  br i1 %v1502, label %b986, label %b987
b985:
  %v1509 = call i64 @df.unbox_int(i64 %v0)
  %v1510 = load i8, ptr %ctx, align 8
  %c360 = zext i8 %v1510 to i64
  %v1511 = icmp ne i64 %c360, 0
  br i1 %v1511, label %b992, label %b991
b986:
  %ga1503 = getelementptr i64, ptr %area, i64 6
  %v1503 = load i64, ptr %ga1503
  br label %b988
b987:
  %v1504 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b988
b988:
  %v1505 = phi i64 [ %v1503, %b986 ], [ %v1504, %b987 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1505, i64 0, i64 0)
  %v1507 = load i8, ptr %ctx, align 8
  %c361 = zext i8 %v1507 to i64
  %v1508 = icmp ne i64 %c361, 0
  br i1 %v1508, label %b990, label %b989
b989:
  br label %b985
b990:
  %gcvolta990 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta990, ptr %ctxtopo, align 8
  ret i64 0
b991:
  %v1512 = add i64 0, 25
  %v1513 = ashr i64 %v1509, %v1512
  store i64 0, ptr %gcs0
  %v1514 = call i64 @df.box_int(i64 %v1513)
  %xcp1515 = load i8, ptr %ctx, align 8
  %xcn1515 = icmp ne i8 %xcp1515, 0
  %xce1515 = call i1 @llvm.expect.i1(i1 %xcn1515, i1 false)
  br i1 %xce1515, label %xc1515.limpar, label %xc1515.fim
xc1515.limpar:
  call void @dartforge_exception_clear()
  br label %xc1515.fim
xc1515.fim:
  %gcvolta991 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta991, ptr %ctxtopo, align 8
  ret i64 %v1514
b992:
  %gcvolta992 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta992, ptr %ctxtopo, align 8
  ret i64 0
b994:
  %ga1524 = getelementptr i64, ptr %area, i64 7
  %v1524 = load i8, ptr %ga1524
  %c362 = zext i8 %v1524 to i64
  %v1525 = icmp ne i64 %c362, 0
  br i1 %v1525, label %b996, label %b997
b995:
  %v1532 = call i64 @df.unbox_int(i64 %v0)
  %v1533 = load i8, ptr %ctx, align 8
  %c363 = zext i8 %v1533 to i64
  %v1534 = icmp ne i64 %c363, 0
  br i1 %v1534, label %b1002, label %b1001
b996:
  %ga1526 = getelementptr i64, ptr %area, i64 6
  %v1526 = load i64, ptr %ga1526
  br label %b998
b997:
  %v1527 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b998
b998:
  %v1528 = phi i64 [ %v1526, %b996 ], [ %v1527, %b997 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1528, i64 0, i64 0)
  %v1530 = load i8, ptr %ctx, align 8
  %c364 = zext i8 %v1530 to i64
  %v1531 = icmp ne i64 %c364, 0
  br i1 %v1531, label %b1000, label %b999
b999:
  br label %b995
b1000:
  %gcvolta1000 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1000, ptr %ctxtopo, align 8
  ret i64 0
b1001:
  %v1535 = add i64 0, 17
  %v1536 = ashr i64 %v1532, %v1535
  store i64 0, ptr %gcs0
  %v1537 = call i64 @df.box_int(i64 %v1536)
  %xcp1538 = load i8, ptr %ctx, align 8
  %xcn1538 = icmp ne i8 %xcp1538, 0
  %xce1538 = call i1 @llvm.expect.i1(i1 %xcn1538, i1 false)
  br i1 %xce1538, label %xc1538.limpar, label %xc1538.fim
xc1538.limpar:
  call void @dartforge_exception_clear()
  br label %xc1538.fim
xc1538.fim:
  %gcvolta1001 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1001, ptr %ctxtopo, align 8
  ret i64 %v1537
b1002:
  %gcvolta1002 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1002, ptr %ctxtopo, align 8
  ret i64 0
b1004:
  %ga1547 = getelementptr i64, ptr %area, i64 7
  %v1547 = load i8, ptr %ga1547
  %c365 = zext i8 %v1547 to i64
  %v1548 = icmp ne i64 %c365, 0
  br i1 %v1548, label %b1006, label %b1007
b1005:
  %v1555 = call i64 @df.unbox_int(i64 %v0)
  %v1556 = load i8, ptr %ctx, align 8
  %c366 = zext i8 %v1556 to i64
  %v1557 = icmp ne i64 %c366, 0
  br i1 %v1557, label %b1012, label %b1011
b1006:
  %ga1549 = getelementptr i64, ptr %area, i64 6
  %v1549 = load i64, ptr %ga1549
  br label %b1008
b1007:
  %v1550 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1008
b1008:
  %v1551 = phi i64 [ %v1549, %b1006 ], [ %v1550, %b1007 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1551, i64 0, i64 0)
  %v1553 = load i8, ptr %ctx, align 8
  %c367 = zext i8 %v1553 to i64
  %v1554 = icmp ne i64 %c367, 0
  br i1 %v1554, label %b1010, label %b1009
b1009:
  br label %b1005
b1010:
  %gcvolta1010 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1010, ptr %ctxtopo, align 8
  ret i64 0
b1011:
  %v1558 = add i64 0, 1
  %v1559 = ashr i64 %v1555, %v1558
  store i64 0, ptr %gcs0
  %v1560 = call i64 @df.box_int(i64 %v1559)
  %xcp1561 = load i8, ptr %ctx, align 8
  %xcn1561 = icmp ne i8 %xcp1561, 0
  %xce1561 = call i1 @llvm.expect.i1(i1 %xcn1561, i1 false)
  br i1 %xce1561, label %xc1561.limpar, label %xc1561.fim
xc1561.limpar:
  call void @dartforge_exception_clear()
  br label %xc1561.fim
xc1561.fim:
  %gcvolta1011 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1011, ptr %ctxtopo, align 8
  ret i64 %v1560
b1012:
  %gcvolta1012 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1012, ptr %ctxtopo, align 8
  ret i64 0
b1014:
  store i64 0, ptr %gcs1
  %v1568 = call double @df.unbox_double(i64 %v1565)
  %v1569 = load i8, ptr %ctx, align 8
  %c368 = zext i8 %v1569 to i64
  %v1570 = icmp ne i64 %c368, 0
  br i1 %v1570, label %b1017, label %b1016
b1015:
  %gcvolta1015 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1015, ptr %ctxtopo, align 8
  ret i64 0
b1016:
  %xcp1572 = load i8, ptr %ctx, align 8
  %xcn1572 = icmp ne i8 %xcp1572, 0
  %xce1572 = call i1 @llvm.expect.i1(i1 %xcn1572, i1 false)
  br i1 %xce1572, label %xc1572.limpar, label %xc1572.fim
xc1572.limpar:
  call void @dartforge_exception_clear()
  br label %xc1572.fim
xc1572.fim:
  %gcvolta1016 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1016, ptr %ctxtopo, align 8
  ret i64 %v1565
b1017:
  %gcvolta1017 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1017, ptr %ctxtopo, align 8
  ret i64 0
b1031:
  %v1918 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1919 = call i64 @df.box_int(i64 %v1918)
  store i64 %v1919, ptr %gcs1
  %sa1920_0 = getelementptr [2 x i64], ptr %sargs1920, i64 0, i64 0
  store i64 %v1919, ptr %sa1920_0
  %sat1920 = getelementptr [2 x i64], ptr %sargs1920, i64 0, i64 1
  store i64 0, ptr %sat1920
  %sic1920 = getelementptr i64, ptr %area, i64 224
  %sf1920 = call ptr @df.selector(ptr %sic1920, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1920 = call i64 %sf1920(i64 %v0, ptr %sargs1920, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1920, ptr %gcs0
  %v1921 = load i8, ptr %ctx, align 8
  %c369 = zext i8 %v1921 to i64
  %v1922 = icmp ne i64 %c369, 0
  br i1 %v1922, label %b1238, label %b1237
b1032:
  store i64 0, ptr %gcs1
  %v1578 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1578, ptr %gcs1
  %v1579 = load i8, ptr %ctx, align 8
  %c370 = zext i8 %v1579 to i64
  %v1580 = icmp ne i64 %c370, 0
  br i1 %v1580, label %b1038, label %b1037
b1033:
  %v1577 = icmp eq i64 %v1574, %v6
  br i1 %v1577, label %b1035, label %b1032
b1034:
  %gcvolta1034 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1034, ptr %ctxtopo, align 8
  ret i64 0
b1035:
  %v1623 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1624 = call i64 @df.box_int(i64 %v1623)
  store i64 %v1624, ptr %gcs1
  %v1625 = icmp eq i64 %v0, 0
  %v1626 = icmp eq i64 %v1624, 0
  %c371 = zext i1 %v1625 to i64
  %c372 = zext i1 %v1626 to i64
  %v1627 = or i64 %c371, %c372
  %v1628 = icmp ne i64 %v1627, 0
  br i1 %v1628, label %b1082, label %b1083
b1036:
  store i64 0, ptr %gcs1
  %v1582 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1582, ptr %gcs1
  %v1583 = load i8, ptr %ctx, align 8
  %c373 = zext i8 %v1583 to i64
  %v1584 = icmp ne i64 %c373, 0
  br i1 %v1584, label %b1042, label %b1041
b1037:
  %v1581 = icmp eq i64 %v1578, %v6
  br i1 %v1581, label %b1039, label %b1036
b1038:
  %gcvolta1038 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1038, ptr %ctxtopo, align 8
  ret i64 0
b1039:
  %v1655 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1656 = and i64 %v0, 1
  %v1657 = icmp ne i64 %v1656, 0
  br i1 %v1657, label %b1104, label %b1105
b1040:
  store i64 0, ptr %gcs1
  %v1586 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1586, ptr %gcs1
  %v1587 = load i8, ptr %ctx, align 8
  %c374 = zext i8 %v1587 to i64
  %v1588 = icmp ne i64 %c374, 0
  br i1 %v1588, label %b1046, label %b1045
b1041:
  %v1585 = icmp eq i64 %v1582, %v6
  br i1 %v1585, label %b1043, label %b1040
b1042:
  %gcvolta1042 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1042, ptr %ctxtopo, align 8
  ret i64 0
b1043:
  %v1694 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1695 = and i64 %v0, 1
  %v1696 = icmp ne i64 %v1695, 0
  br i1 %v1696, label %b1124, label %b1125
b1044:
  store i64 0, ptr %gcs1
  %v1590 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1590, ptr %gcs1
  %v1591 = load i8, ptr %ctx, align 8
  %c375 = zext i8 %v1591 to i64
  %v1592 = icmp ne i64 %c375, 0
  br i1 %v1592, label %b1050, label %b1049
b1045:
  %v1589 = icmp eq i64 %v1586, %v6
  br i1 %v1589, label %b1047, label %b1044
b1046:
  %gcvolta1046 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1046, ptr %ctxtopo, align 8
  ret i64 0
b1047:
  %v1733 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1734 = and i64 %v0, 1
  %v1735 = icmp ne i64 %v1734, 0
  br i1 %v1735, label %b1144, label %b1145
b1048:
  store i64 0, ptr %gcs1
  %v1594 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1594, ptr %gcs1
  %v1595 = load i8, ptr %ctx, align 8
  %c376 = zext i8 %v1595 to i64
  %v1596 = icmp ne i64 %c376, 0
  br i1 %v1596, label %b1054, label %b1053
b1049:
  %v1593 = icmp eq i64 %v1590, %v6
  br i1 %v1593, label %b1051, label %b1048
b1050:
  %gcvolta1050 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1050, ptr %ctxtopo, align 8
  ret i64 0
b1051:
  %v1772 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1773 = and i64 %v0, 1
  %v1774 = icmp ne i64 %v1773, 0
  br i1 %v1774, label %b1164, label %b1165
b1052:
  store i64 0, ptr %gcs1
  %v1598 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1598, ptr %gcs1
  %v1599 = load i8, ptr %ctx, align 8
  %c377 = zext i8 %v1599 to i64
  %v1600 = icmp ne i64 %c377, 0
  br i1 %v1600, label %b1058, label %b1057
b1053:
  %v1597 = icmp eq i64 %v1594, %v6
  br i1 %v1597, label %b1055, label %b1052
b1054:
  %gcvolta1054 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1054, ptr %ctxtopo, align 8
  ret i64 0
b1055:
  %v1811 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1812 = and i64 %v0, 1
  %v1813 = icmp ne i64 %v1812, 0
  br i1 %v1813, label %b1184, label %b1185
b1056:
  store i64 0, ptr %gcs1
  %v1602 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1602, ptr %gcs1
  %v1603 = load i8, ptr %ctx, align 8
  %c378 = zext i8 %v1603 to i64
  %v1604 = icmp ne i64 %c378, 0
  br i1 %v1604, label %b1062, label %b1061
b1057:
  %v1601 = icmp eq i64 %v1598, %v6
  br i1 %v1601, label %b1059, label %b1056
b1058:
  %gcvolta1058 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1058, ptr %ctxtopo, align 8
  ret i64 0
b1059:
  %xcp1850 = load i8, ptr %ctx, align 8
  %xcn1850 = icmp ne i8 %xcp1850, 0
  %xce1850 = call i1 @llvm.expect.i1(i1 %xcn1850, i1 false)
  br i1 %xce1850, label %xc1850.limpar, label %xc1850.fim
xc1850.limpar:
  call void @dartforge_exception_clear()
  br label %xc1850.fim
xc1850.fim:
  %gcvolta1059 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1059, ptr %ctxtopo, align 8
  ret i64 %v0
b1060:
  store i64 0, ptr %gcs1
  %v1606 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1606, ptr %gcs1
  %v1607 = load i8, ptr %ctx, align 8
  %c379 = zext i8 %v1607 to i64
  %v1608 = icmp ne i64 %c379, 0
  br i1 %v1608, label %b1066, label %b1065
b1061:
  %v1605 = icmp eq i64 %v1602, %v6
  br i1 %v1605, label %b1063, label %b1060
b1062:
  %gcvolta1062 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1062, ptr %ctxtopo, align 8
  ret i64 0
b1063:
  %v1852 = add i64 0, 258
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1853 = and i64 %v0, 1
  %v1854 = icmp ne i64 %v1853, 0
  br i1 %v1854, label %b1205, label %b1206
b1064:
  store i64 0, ptr %gcs1
  %v1610 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1610, ptr %gcs1
  %v1611 = load i8, ptr %ctx, align 8
  %c380 = zext i8 %v1611 to i64
  %v1612 = icmp ne i64 %c380, 0
  br i1 %v1612, label %b1070, label %b1069
b1065:
  %v1609 = icmp eq i64 %v1606, %v6
  br i1 %v1609, label %b1067, label %b1064
b1066:
  %gcvolta1066 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1066, ptr %ctxtopo, align 8
  ret i64 0
b1067:
  %v1885 = add i64 0, 16909320
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1886 = and i64 %v0, 1
  %v1887 = icmp ne i64 %v1886, 0
  br i1 %v1887, label %b1221, label %b1222
b1068:
  store i64 0, ptr %gcs1
  %v1614 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1614, ptr %gcs1
  %v1615 = load i8, ptr %ctx, align 8
  %c381 = zext i8 %v1615 to i64
  %v1616 = icmp ne i64 %c381, 0
  br i1 %v1616, label %b1075, label %b1074
b1069:
  %v1613 = icmp eq i64 %v1610, %v6
  br i1 %v1613, label %b1071, label %b1068
b1070:
  %gcvolta1070 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1070, ptr %ctxtopo, align 8
  ret i64 0
b1071:
  br label %b1031
b1073:
  store i64 0, ptr %gcs1
  %v1618 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1618, ptr %gcs1
  %v1619 = load i8, ptr %ctx, align 8
  %c382 = zext i8 %v1619 to i64
  %v1620 = icmp ne i64 %c382, 0
  br i1 %v1620, label %b1080, label %b1079
b1074:
  %v1617 = icmp eq i64 %v1614, %v6
  br i1 %v1617, label %b1076, label %b1073
b1075:
  %gcvolta1075 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1075, ptr %ctxtopo, align 8
  ret i64 0
b1076:
  br label %b1031
b1078:
  br label %b12
b1079:
  %v1621 = icmp eq i64 %v1618, %v6
  br i1 %v1621, label %b1081, label %b1078
b1080:
  %gcvolta1080 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1080, ptr %ctxtopo, align 8
  ret i64 0
b1081:
  br label %b1031
b1082:
  %v1629 = icmp eq i64 %v0, %v1624
  br label %b1084
b1083:
  %v1630 = call i64 @df.class(i64 %v0)
  switch i64 %v1630, label %b1088 [ i64 675, label %b1085 i64 4, label %b1086 i64 2, label %b1087 i64 3, label %b1087 ]
b1084:
  %v1648 = phi i1 [ %v1629, %b1082 ], [ %v1647, %b1089 ]
  br i1 %v1648, label %b1100, label %b1101
b1085:
  %v1631 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1624)
  %v1632 = load i8, ptr %ctx, align 8
  %c383 = zext i8 %v1632 to i64
  %v1633 = icmp ne i64 %c383, 0
  br i1 %v1633, label %b1091, label %b1090
b1086:
  %v1635 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1624)
  %v1636 = load i8, ptr %ctx, align 8
  %c384 = zext i8 %v1636 to i64
  %v1637 = icmp ne i64 %c384, 0
  br i1 %v1637, label %b1093, label %b1092
b1087:
  %v1638 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1624)
  %v1639 = load i8, ptr %ctx, align 8
  %c385 = zext i8 %v1639 to i64
  %v1640 = icmp ne i64 %c385, 0
  br i1 %v1640, label %b1095, label %b1094
b1088:
  %sa1641_0 = getelementptr [2 x i64], ptr %sargs1641, i64 0, i64 0
  store i64 %v1624, ptr %sa1641_0
  %sat1641 = getelementptr [2 x i64], ptr %sargs1641, i64 0, i64 1
  store i64 0, ptr %sat1641
  %sic1641 = getelementptr i64, ptr %area, i64 226
  %sf1641 = call ptr @df.selector(ptr %sic1641, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1641 = call i64 %sf1641(i64 %v0, ptr %sargs1641, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1641, ptr %gcs0
  %v1642 = load i8, ptr %ctx, align 8
  %c386 = zext i8 %v1642 to i64
  %v1643 = icmp ne i64 %c386, 0
  br i1 %v1643, label %b1097, label %b1096
b1089:
  %v1647 = phi i1 [ %v1634, %b1090 ], [ %v1635, %b1092 ], [ %v1638, %b1094 ], [ %v1644, %b1098 ]
  br label %b1084
b1090:
  %c387 = zext i8 %v1631 to i64
  %v1634 = icmp ne i64 %c387, 0
  br label %b1089
b1091:
  %gcvolta1091 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1091, ptr %ctxtopo, align 8
  ret i64 0
b1092:
  br label %b1089
b1093:
  %gcvolta1093 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1093, ptr %ctxtopo, align 8
  ret i64 0
b1094:
  br label %b1089
b1095:
  %gcvolta1095 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1095, ptr %ctxtopo, align 8
  ret i64 0
b1096:
  store i64 0, ptr %gcs1
  %v1644 = call i1 @df.unbox_bool(i64 %v1641)
  %v1645 = load i8, ptr %ctx, align 8
  %c388 = zext i8 %v1645 to i64
  %v1646 = icmp ne i64 %c388, 0
  br i1 %v1646, label %b1099, label %b1098
b1097:
  %gcvolta1097 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1097, ptr %ctxtopo, align 8
  ret i64 0
b1098:
  br label %b1089
b1099:
  %gcvolta1099 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1099, ptr %ctxtopo, align 8
  ret i64 0
b1100:
  %v1649 = add i64 0, 0
  br label %b1102
b1101:
  %v1650 = add i64 0, 1
  br label %b1102
b1102:
  %v1651 = phi i64 [ %v1649, %b1100 ], [ %v1650, %b1101 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1652 = call i64 @df.box_int(i64 %v1651)
  %xcp1653 = load i8, ptr %ctx, align 8
  %xcn1653 = icmp ne i8 %xcp1653, 0
  %xce1653 = call i1 @llvm.expect.i1(i1 %xcn1653, i1 false)
  br i1 %xce1653, label %xc1653.limpar, label %xc1653.fim
xc1653.limpar:
  call void @dartforge_exception_clear()
  br label %xc1653.fim
xc1653.fim:
  %gcvolta1102 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1102, ptr %ctxtopo, align 8
  ret i64 %v1652
b1104:
  %v1658 = ashr i64 %v0, 1
  %v1659 = icmp sle i64 %v1658, %v1655
  br label %b1106
b1105:
  %v1660 = call i64 @df.box_int(i64 %v1655)
  store i64 %v1660, ptr %gcs1
  %sa1661_0 = getelementptr [2 x i64], ptr %sargs1661, i64 0, i64 0
  store i64 %v1660, ptr %sa1661_0
  %sat1661 = getelementptr [2 x i64], ptr %sargs1661, i64 0, i64 1
  store i64 0, ptr %sat1661
  %sic1661 = getelementptr i64, ptr %area, i64 228
  %sf1661 = call ptr @df.selector(ptr %sic1661, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1661 = call i64 %sf1661(i64 %v0, ptr %sargs1661, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1661, ptr %gcs1
  %v1662 = load i8, ptr %ctx, align 8
  %c389 = zext i8 %v1662 to i64
  %v1663 = icmp ne i64 %c389, 0
  br i1 %v1663, label %b1108, label %b1107
b1106:
  %v1667 = phi i1 [ %v1659, %b1104 ], [ %v1664, %b1109 ]
  br i1 %v1667, label %b1111, label %b1112
b1107:
  %v1664 = call i1 @df.unbox_bool(i64 %v1661)
  %v1665 = load i8, ptr %ctx, align 8
  %c390 = zext i8 %v1665 to i64
  %v1666 = icmp ne i64 %c390, 0
  br i1 %v1666, label %b1110, label %b1109
b1108:
  %gcvolta1108 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1108, ptr %ctxtopo, align 8
  ret i64 0
b1109:
  br label %b1106
b1110:
  %gcvolta1110 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1110, ptr %ctxtopo, align 8
  ret i64 0
b1111:
  %v1668 = add i64 0, 0
  br label %b1113
b1112:
  store i64 0, ptr %gcs1
  %v1670 = icmp eq i64 %v0, 0
  %v1671 = call i64 @df.class(i64 %v0)
  %v1672 = call i8 @df.subclass(i64 %v1671, i64 674)
  %c391 = zext i8 %v1672 to i64
  %v1673 = icmp ne i64 %c391, 0
  %v1674 = xor i1 %v1670, true
  %c392 = zext i1 %v1673 to i64
  %c393 = zext i1 %v1674 to i64
  %v1675 = and i64 %c392, %c393
  %v1676 = icmp ne i64 %v1675, 0
  br i1 %v1676, label %b1115, label %b1114
b1113:
  %v1690 = phi i64 [ %v1668, %b1111 ], [ %v1689, %b1121 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1691 = call i64 @df.box_int(i64 %v1690)
  %xcp1692 = load i8, ptr %ctx, align 8
  %xcn1692 = icmp ne i8 %xcp1692, 0
  %xce1692 = call i1 @llvm.expect.i1(i1 %xcn1692, i1 false)
  br i1 %xce1692, label %xc1692.limpar, label %xc1692.fim
xc1692.limpar:
  call void @dartforge_exception_clear()
  br label %xc1692.fim
xc1692.fim:
  %gcvolta1113 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1113, ptr %ctxtopo, align 8
  ret i64 %v1691
b1114:
  %ga1677 = getelementptr i64, ptr %area, i64 7
  %v1677 = load i8, ptr %ga1677
  %c394 = zext i8 %v1677 to i64
  %v1678 = icmp ne i64 %c394, 0
  br i1 %v1678, label %b1116, label %b1117
b1115:
  %v1685 = call i64 @df.unbox_int(i64 %v0)
  %v1686 = load i8, ptr %ctx, align 8
  %c395 = zext i8 %v1686 to i64
  %v1687 = icmp ne i64 %c395, 0
  br i1 %v1687, label %b1122, label %b1121
b1116:
  %ga1679 = getelementptr i64, ptr %area, i64 6
  %v1679 = load i64, ptr %ga1679
  br label %b1118
b1117:
  %v1680 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1118
b1118:
  %v1681 = phi i64 [ %v1679, %b1116 ], [ %v1680, %b1117 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1681, i64 0, i64 0)
  %v1683 = load i8, ptr %ctx, align 8
  %c396 = zext i8 %v1683 to i64
  %v1684 = icmp ne i64 %c396, 0
  br i1 %v1684, label %b1120, label %b1119
b1119:
  br label %b1115
b1120:
  %gcvolta1120 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1120, ptr %ctxtopo, align 8
  ret i64 0
b1121:
  %v1688 = add i64 0, 5
  %v1689 = ashr i64 %v1685, %v1688
  br label %b1113
b1122:
  %gcvolta1122 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1122, ptr %ctxtopo, align 8
  ret i64 0
b1124:
  %v1697 = ashr i64 %v0, 1
  %v1698 = icmp sle i64 %v1697, %v1694
  br label %b1126
b1125:
  %v1699 = call i64 @df.box_int(i64 %v1694)
  store i64 %v1699, ptr %gcs1
  %sa1700_0 = getelementptr [2 x i64], ptr %sargs1700, i64 0, i64 0
  store i64 %v1699, ptr %sa1700_0
  %sat1700 = getelementptr [2 x i64], ptr %sargs1700, i64 0, i64 1
  store i64 0, ptr %sat1700
  %sic1700 = getelementptr i64, ptr %area, i64 230
  %sf1700 = call ptr @df.selector(ptr %sic1700, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1700 = call i64 %sf1700(i64 %v0, ptr %sargs1700, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1700, ptr %gcs1
  %v1701 = load i8, ptr %ctx, align 8
  %c397 = zext i8 %v1701 to i64
  %v1702 = icmp ne i64 %c397, 0
  br i1 %v1702, label %b1128, label %b1127
b1126:
  %v1706 = phi i1 [ %v1698, %b1124 ], [ %v1703, %b1129 ]
  br i1 %v1706, label %b1131, label %b1132
b1127:
  %v1703 = call i1 @df.unbox_bool(i64 %v1700)
  %v1704 = load i8, ptr %ctx, align 8
  %c398 = zext i8 %v1704 to i64
  %v1705 = icmp ne i64 %c398, 0
  br i1 %v1705, label %b1130, label %b1129
b1128:
  %gcvolta1128 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1128, ptr %ctxtopo, align 8
  ret i64 0
b1129:
  br label %b1126
b1130:
  %gcvolta1130 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1130, ptr %ctxtopo, align 8
  ret i64 0
b1131:
  %v1707 = add i64 0, 0
  br label %b1133
b1132:
  store i64 0, ptr %gcs1
  %v1709 = icmp eq i64 %v0, 0
  %v1710 = call i64 @df.class(i64 %v0)
  %v1711 = call i8 @df.subclass(i64 %v1710, i64 674)
  %c399 = zext i8 %v1711 to i64
  %v1712 = icmp ne i64 %c399, 0
  %v1713 = xor i1 %v1709, true
  %c400 = zext i1 %v1712 to i64
  %c401 = zext i1 %v1713 to i64
  %v1714 = and i64 %c400, %c401
  %v1715 = icmp ne i64 %v1714, 0
  br i1 %v1715, label %b1135, label %b1134
b1133:
  %v1729 = phi i64 [ %v1707, %b1131 ], [ %v1728, %b1141 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1730 = call i64 @df.box_int(i64 %v1729)
  %xcp1731 = load i8, ptr %ctx, align 8
  %xcn1731 = icmp ne i8 %xcp1731, 0
  %xce1731 = call i1 @llvm.expect.i1(i1 %xcn1731, i1 false)
  br i1 %xce1731, label %xc1731.limpar, label %xc1731.fim
xc1731.limpar:
  call void @dartforge_exception_clear()
  br label %xc1731.fim
xc1731.fim:
  %gcvolta1133 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1133, ptr %ctxtopo, align 8
  ret i64 %v1730
b1134:
  %ga1716 = getelementptr i64, ptr %area, i64 7
  %v1716 = load i8, ptr %ga1716
  %c402 = zext i8 %v1716 to i64
  %v1717 = icmp ne i64 %c402, 0
  br i1 %v1717, label %b1136, label %b1137
b1135:
  %v1724 = call i64 @df.unbox_int(i64 %v0)
  %v1725 = load i8, ptr %ctx, align 8
  %c403 = zext i8 %v1725 to i64
  %v1726 = icmp ne i64 %c403, 0
  br i1 %v1726, label %b1142, label %b1141
b1136:
  %ga1718 = getelementptr i64, ptr %area, i64 6
  %v1718 = load i64, ptr %ga1718
  br label %b1138
b1137:
  %v1719 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1138
b1138:
  %v1720 = phi i64 [ %v1718, %b1136 ], [ %v1719, %b1137 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1720, i64 0, i64 0)
  %v1722 = load i8, ptr %ctx, align 8
  %c404 = zext i8 %v1722 to i64
  %v1723 = icmp ne i64 %c404, 0
  br i1 %v1723, label %b1140, label %b1139
b1139:
  br label %b1135
b1140:
  %gcvolta1140 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1140, ptr %ctxtopo, align 8
  ret i64 0
b1141:
  %v1727 = add i64 0, 3
  %v1728 = ashr i64 %v1724, %v1727
  br label %b1133
b1142:
  %gcvolta1142 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1142, ptr %ctxtopo, align 8
  ret i64 0
b1144:
  %v1736 = ashr i64 %v0, 1
  %v1737 = icmp sle i64 %v1736, %v1733
  br label %b1146
b1145:
  %v1738 = call i64 @df.box_int(i64 %v1733)
  store i64 %v1738, ptr %gcs1
  %sa1739_0 = getelementptr [2 x i64], ptr %sargs1739, i64 0, i64 0
  store i64 %v1738, ptr %sa1739_0
  %sat1739 = getelementptr [2 x i64], ptr %sargs1739, i64 0, i64 1
  store i64 0, ptr %sat1739
  %sic1739 = getelementptr i64, ptr %area, i64 232
  %sf1739 = call ptr @df.selector(ptr %sic1739, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1739 = call i64 %sf1739(i64 %v0, ptr %sargs1739, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1739, ptr %gcs1
  %v1740 = load i8, ptr %ctx, align 8
  %c405 = zext i8 %v1740 to i64
  %v1741 = icmp ne i64 %c405, 0
  br i1 %v1741, label %b1148, label %b1147
b1146:
  %v1745 = phi i1 [ %v1737, %b1144 ], [ %v1742, %b1149 ]
  br i1 %v1745, label %b1151, label %b1152
b1147:
  %v1742 = call i1 @df.unbox_bool(i64 %v1739)
  %v1743 = load i8, ptr %ctx, align 8
  %c406 = zext i8 %v1743 to i64
  %v1744 = icmp ne i64 %c406, 0
  br i1 %v1744, label %b1150, label %b1149
b1148:
  %gcvolta1148 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1148, ptr %ctxtopo, align 8
  ret i64 0
b1149:
  br label %b1146
b1150:
  %gcvolta1150 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1150, ptr %ctxtopo, align 8
  ret i64 0
b1151:
  %v1746 = add i64 0, 0
  br label %b1153
b1152:
  store i64 0, ptr %gcs1
  %v1748 = icmp eq i64 %v0, 0
  %v1749 = call i64 @df.class(i64 %v0)
  %v1750 = call i8 @df.subclass(i64 %v1749, i64 674)
  %c407 = zext i8 %v1750 to i64
  %v1751 = icmp ne i64 %c407, 0
  %v1752 = xor i1 %v1748, true
  %c408 = zext i1 %v1751 to i64
  %c409 = zext i1 %v1752 to i64
  %v1753 = and i64 %c408, %c409
  %v1754 = icmp ne i64 %v1753, 0
  br i1 %v1754, label %b1155, label %b1154
b1153:
  %v1768 = phi i64 [ %v1746, %b1151 ], [ %v1767, %b1161 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1769 = call i64 @df.box_int(i64 %v1768)
  %xcp1770 = load i8, ptr %ctx, align 8
  %xcn1770 = icmp ne i8 %xcp1770, 0
  %xce1770 = call i1 @llvm.expect.i1(i1 %xcn1770, i1 false)
  br i1 %xce1770, label %xc1770.limpar, label %xc1770.fim
xc1770.limpar:
  call void @dartforge_exception_clear()
  br label %xc1770.fim
xc1770.fim:
  %gcvolta1153 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1153, ptr %ctxtopo, align 8
  ret i64 %v1769
b1154:
  %ga1755 = getelementptr i64, ptr %area, i64 7
  %v1755 = load i8, ptr %ga1755
  %c410 = zext i8 %v1755 to i64
  %v1756 = icmp ne i64 %c410, 0
  br i1 %v1756, label %b1156, label %b1157
b1155:
  %v1763 = call i64 @df.unbox_int(i64 %v0)
  %v1764 = load i8, ptr %ctx, align 8
  %c411 = zext i8 %v1764 to i64
  %v1765 = icmp ne i64 %c411, 0
  br i1 %v1765, label %b1162, label %b1161
b1156:
  %ga1757 = getelementptr i64, ptr %area, i64 6
  %v1757 = load i64, ptr %ga1757
  br label %b1158
b1157:
  %v1758 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1158
b1158:
  %v1759 = phi i64 [ %v1757, %b1156 ], [ %v1758, %b1157 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1759, i64 0, i64 0)
  %v1761 = load i8, ptr %ctx, align 8
  %c412 = zext i8 %v1761 to i64
  %v1762 = icmp ne i64 %c412, 0
  br i1 %v1762, label %b1160, label %b1159
b1159:
  br label %b1155
b1160:
  %gcvolta1160 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1160, ptr %ctxtopo, align 8
  ret i64 0
b1161:
  %v1766 = add i64 0, 1
  %v1767 = shl i64 %v1763, %v1766
  br label %b1153
b1162:
  %gcvolta1162 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1162, ptr %ctxtopo, align 8
  ret i64 0
b1164:
  %v1775 = ashr i64 %v0, 1
  %v1776 = icmp sle i64 %v1775, %v1772
  br label %b1166
b1165:
  %v1777 = call i64 @df.box_int(i64 %v1772)
  store i64 %v1777, ptr %gcs1
  %sa1778_0 = getelementptr [2 x i64], ptr %sargs1778, i64 0, i64 0
  store i64 %v1777, ptr %sa1778_0
  %sat1778 = getelementptr [2 x i64], ptr %sargs1778, i64 0, i64 1
  store i64 0, ptr %sat1778
  %sic1778 = getelementptr i64, ptr %area, i64 234
  %sf1778 = call ptr @df.selector(ptr %sic1778, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1778 = call i64 %sf1778(i64 %v0, ptr %sargs1778, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1778, ptr %gcs1
  %v1779 = load i8, ptr %ctx, align 8
  %c413 = zext i8 %v1779 to i64
  %v1780 = icmp ne i64 %c413, 0
  br i1 %v1780, label %b1168, label %b1167
b1166:
  %v1784 = phi i1 [ %v1776, %b1164 ], [ %v1781, %b1169 ]
  br i1 %v1784, label %b1171, label %b1172
b1167:
  %v1781 = call i1 @df.unbox_bool(i64 %v1778)
  %v1782 = load i8, ptr %ctx, align 8
  %c414 = zext i8 %v1782 to i64
  %v1783 = icmp ne i64 %c414, 0
  br i1 %v1783, label %b1170, label %b1169
b1168:
  %gcvolta1168 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1168, ptr %ctxtopo, align 8
  ret i64 0
b1169:
  br label %b1166
b1170:
  %gcvolta1170 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1170, ptr %ctxtopo, align 8
  ret i64 0
b1171:
  %v1785 = add i64 0, 0
  br label %b1173
b1172:
  store i64 0, ptr %gcs1
  %v1787 = icmp eq i64 %v0, 0
  %v1788 = call i64 @df.class(i64 %v0)
  %v1789 = call i8 @df.subclass(i64 %v1788, i64 674)
  %c415 = zext i8 %v1789 to i64
  %v1790 = icmp ne i64 %c415, 0
  %v1791 = xor i1 %v1787, true
  %c416 = zext i1 %v1790 to i64
  %c417 = zext i1 %v1791 to i64
  %v1792 = and i64 %c416, %c417
  %v1793 = icmp ne i64 %v1792, 0
  br i1 %v1793, label %b1175, label %b1174
b1173:
  %v1807 = phi i64 [ %v1785, %b1171 ], [ %v1806, %b1181 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1808 = call i64 @df.box_int(i64 %v1807)
  %xcp1809 = load i8, ptr %ctx, align 8
  %xcn1809 = icmp ne i8 %xcp1809, 0
  %xce1809 = call i1 @llvm.expect.i1(i1 %xcn1809, i1 false)
  br i1 %xce1809, label %xc1809.limpar, label %xc1809.fim
xc1809.limpar:
  call void @dartforge_exception_clear()
  br label %xc1809.fim
xc1809.fim:
  %gcvolta1173 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1173, ptr %ctxtopo, align 8
  ret i64 %v1808
b1174:
  %ga1794 = getelementptr i64, ptr %area, i64 7
  %v1794 = load i8, ptr %ga1794
  %c418 = zext i8 %v1794 to i64
  %v1795 = icmp ne i64 %c418, 0
  br i1 %v1795, label %b1176, label %b1177
b1175:
  %v1802 = call i64 @df.unbox_int(i64 %v0)
  %v1803 = load i8, ptr %ctx, align 8
  %c419 = zext i8 %v1803 to i64
  %v1804 = icmp ne i64 %c419, 0
  br i1 %v1804, label %b1182, label %b1181
b1176:
  %ga1796 = getelementptr i64, ptr %area, i64 6
  %v1796 = load i64, ptr %ga1796
  br label %b1178
b1177:
  %v1797 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1178
b1178:
  %v1798 = phi i64 [ %v1796, %b1176 ], [ %v1797, %b1177 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1798, i64 0, i64 0)
  %v1800 = load i8, ptr %ctx, align 8
  %c420 = zext i8 %v1800 to i64
  %v1801 = icmp ne i64 %c420, 0
  br i1 %v1801, label %b1180, label %b1179
b1179:
  br label %b1175
b1180:
  %gcvolta1180 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1180, ptr %ctxtopo, align 8
  ret i64 0
b1181:
  %v1805 = add i64 0, 516
  %v1806 = mul i64 %v1802, %v1805
  br label %b1173
b1182:
  %gcvolta1182 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1182, ptr %ctxtopo, align 8
  ret i64 0
b1184:
  %v1814 = ashr i64 %v0, 1
  %v1815 = icmp sle i64 %v1814, %v1811
  br label %b1186
b1185:
  %v1816 = call i64 @df.box_int(i64 %v1811)
  store i64 %v1816, ptr %gcs1
  %sa1817_0 = getelementptr [2 x i64], ptr %sargs1817, i64 0, i64 0
  store i64 %v1816, ptr %sa1817_0
  %sat1817 = getelementptr [2 x i64], ptr %sargs1817, i64 0, i64 1
  store i64 0, ptr %sat1817
  %sic1817 = getelementptr i64, ptr %area, i64 236
  %sf1817 = call ptr @df.selector(ptr %sic1817, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1817 = call i64 %sf1817(i64 %v0, ptr %sargs1817, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1817, ptr %gcs1
  %v1818 = load i8, ptr %ctx, align 8
  %c421 = zext i8 %v1818 to i64
  %v1819 = icmp ne i64 %c421, 0
  br i1 %v1819, label %b1188, label %b1187
b1186:
  %v1823 = phi i1 [ %v1815, %b1184 ], [ %v1820, %b1189 ]
  br i1 %v1823, label %b1191, label %b1192
b1187:
  %v1820 = call i1 @df.unbox_bool(i64 %v1817)
  %v1821 = load i8, ptr %ctx, align 8
  %c422 = zext i8 %v1821 to i64
  %v1822 = icmp ne i64 %c422, 0
  br i1 %v1822, label %b1190, label %b1189
b1188:
  %gcvolta1188 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1188, ptr %ctxtopo, align 8
  ret i64 0
b1189:
  br label %b1186
b1190:
  %gcvolta1190 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1190, ptr %ctxtopo, align 8
  ret i64 0
b1191:
  %v1824 = add i64 0, 0
  br label %b1193
b1192:
  store i64 0, ptr %gcs1
  %v1826 = icmp eq i64 %v0, 0
  %v1827 = call i64 @df.class(i64 %v0)
  %v1828 = call i8 @df.subclass(i64 %v1827, i64 674)
  %c423 = zext i8 %v1828 to i64
  %v1829 = icmp ne i64 %c423, 0
  %v1830 = xor i1 %v1826, true
  %c424 = zext i1 %v1829 to i64
  %c425 = zext i1 %v1830 to i64
  %v1831 = and i64 %c424, %c425
  %v1832 = icmp ne i64 %v1831, 0
  br i1 %v1832, label %b1195, label %b1194
b1193:
  %v1846 = phi i64 [ %v1824, %b1191 ], [ %v1845, %b1201 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1847 = call i64 @df.box_int(i64 %v1846)
  %xcp1848 = load i8, ptr %ctx, align 8
  %xcn1848 = icmp ne i8 %xcp1848, 0
  %xce1848 = call i1 @llvm.expect.i1(i1 %xcn1848, i1 false)
  br i1 %xce1848, label %xc1848.limpar, label %xc1848.fim
xc1848.limpar:
  call void @dartforge_exception_clear()
  br label %xc1848.fim
xc1848.fim:
  %gcvolta1193 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1193, ptr %ctxtopo, align 8
  ret i64 %v1847
b1194:
  %ga1833 = getelementptr i64, ptr %area, i64 7
  %v1833 = load i8, ptr %ga1833
  %c426 = zext i8 %v1833 to i64
  %v1834 = icmp ne i64 %c426, 0
  br i1 %v1834, label %b1196, label %b1197
b1195:
  %v1841 = call i64 @df.unbox_int(i64 %v0)
  %v1842 = load i8, ptr %ctx, align 8
  %c427 = zext i8 %v1842 to i64
  %v1843 = icmp ne i64 %c427, 0
  br i1 %v1843, label %b1202, label %b1201
b1196:
  %ga1835 = getelementptr i64, ptr %area, i64 6
  %v1835 = load i64, ptr %ga1835
  br label %b1198
b1197:
  %v1836 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1198
b1198:
  %v1837 = phi i64 [ %v1835, %b1196 ], [ %v1836, %b1197 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1837, i64 0, i64 0)
  %v1839 = load i8, ptr %ctx, align 8
  %c428 = zext i8 %v1839 to i64
  %v1840 = icmp ne i64 %c428, 0
  br i1 %v1840, label %b1200, label %b1199
b1199:
  br label %b1195
b1200:
  %gcvolta1200 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1200, ptr %ctxtopo, align 8
  ret i64 0
b1201:
  %v1844 = add i64 0, 33818640
  %v1845 = mul i64 %v1841, %v1844
  br label %b1193
b1202:
  %gcvolta1202 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1202, ptr %ctxtopo, align 8
  ret i64 0
b1205:
  %v1855 = ashr i64 %v0, 1
  %v1856 = mul i64 %v1855, %v1852
  store i64 0, ptr %gcs0
  %v1857 = call i64 @df.box_int(i64 %v1856)
  br label %b1207
b1206:
  %v1858 = call i64 @df.class(i64 %v0)
  %v1859 = icmp eq i64 %v1858, 2
  %v1860 = icmp eq i64 %v1858, 3
  %v1861 = icmp eq i64 %v1858, 4
  %c429 = zext i1 %v1859 to i64
  %c430 = zext i1 %v1860 to i64
  %v1862 = or i64 %c429, %c430
  %c431 = zext i1 %v1861 to i64
  %v1863 = or i64 %v1862, %c431
  %c432 = trunc i64 %v1863 to i1
  br i1 %c432, label %b1208, label %b1209
b1207:
  %v1882 = phi i64 [ %v1857, %b1205 ], [ %v1881, %b1210 ]
  %xcp1883 = load i8, ptr %ctx, align 8
  %xcn1883 = icmp ne i8 %xcp1883, 0
  %xce1883 = call i1 @llvm.expect.i1(i1 %xcn1883, i1 false)
  br i1 %xce1883, label %xc1883.limpar, label %xc1883.fim
xc1883.limpar:
  call void @dartforge_exception_clear()
  br label %xc1883.fim
xc1883.fim:
  %gcvolta1207 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1207, ptr %ctxtopo, align 8
  ret i64 %v1882
b1208:
  br i1 %v1861, label %b1212, label %b1211
b1209:
  %v1877 = call i64 @df.box_int(i64 %v1852)
  store i64 %v1877, ptr %gcs1
  %sa1878_0 = getelementptr [2 x i64], ptr %sargs1878, i64 0, i64 0
  store i64 %v1877, ptr %sa1878_0
  %sat1878 = getelementptr [2 x i64], ptr %sargs1878, i64 0, i64 1
  store i64 0, ptr %sat1878
  %sic1878 = getelementptr i64, ptr %area, i64 238
  %sf1878 = call ptr @df.selector(ptr %sic1878, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v1878 = call i64 %sf1878(i64 %v0, ptr %sargs1878, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1879 = load i8, ptr %ctx, align 8
  %c433 = zext i8 %v1879 to i64
  %v1880 = icmp ne i64 %c433, 0
  br i1 %v1880, label %b1219, label %b1218
b1210:
  %v1881 = phi i64 [ %v1876, %b1213 ], [ %v1878, %b1218 ]
  br label %b1207
b1211:
  %v1865 = call i64 @df.unbox_int(i64 %v0)
  %v1866 = load i8, ptr %ctx, align 8
  %c434 = zext i8 %v1866 to i64
  %v1867 = icmp ne i64 %c434, 0
  br i1 %v1867, label %b1215, label %b1214
b1212:
  %v1870 = call double @df.num_to_double(i64 %v0)
  %v1871 = load i8, ptr %ctx, align 8
  %c435 = zext i8 %v1871 to i64
  %v1872 = icmp ne i64 %c435, 0
  br i1 %v1872, label %b1217, label %b1216
b1213:
  %v1876 = phi i64 [ %v1869, %b1214 ], [ %v1875, %b1216 ]
  br label %b1210
b1214:
  %v1868 = mul i64 %v1865, %v1852
  store i64 0, ptr %gcs0
  %v1869 = call i64 @df.box_int(i64 %v1868)
  br label %b1213
b1215:
  %gcvolta1215 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1215, ptr %ctxtopo, align 8
  ret i64 0
b1216:
  %v1873 = sitofp i64 %v1852 to double
  %v1874 = fmul double %v1870, %v1873
  store i64 0, ptr %gcs0
  %v1875 = call i64 @df.box_double(double %v1874)
  br label %b1213
b1217:
  %gcvolta1217 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1217, ptr %ctxtopo, align 8
  ret i64 0
b1218:
  br label %b1210
b1219:
  %gcvolta1219 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1219, ptr %ctxtopo, align 8
  ret i64 0
b1221:
  %v1888 = ashr i64 %v0, 1
  %v1889 = mul i64 %v1888, %v1885
  store i64 0, ptr %gcs0
  %v1890 = call i64 @df.box_int(i64 %v1889)
  br label %b1223
b1222:
  %v1891 = call i64 @df.class(i64 %v0)
  %v1892 = icmp eq i64 %v1891, 2
  %v1893 = icmp eq i64 %v1891, 3
  %v1894 = icmp eq i64 %v1891, 4
  %c436 = zext i1 %v1892 to i64
  %c437 = zext i1 %v1893 to i64
  %v1895 = or i64 %c436, %c437
  %c438 = zext i1 %v1894 to i64
  %v1896 = or i64 %v1895, %c438
  %c439 = trunc i64 %v1896 to i1
  br i1 %c439, label %b1224, label %b1225
b1223:
  %v1915 = phi i64 [ %v1890, %b1221 ], [ %v1914, %b1226 ]
  %xcp1916 = load i8, ptr %ctx, align 8
  %xcn1916 = icmp ne i8 %xcp1916, 0
  %xce1916 = call i1 @llvm.expect.i1(i1 %xcn1916, i1 false)
  br i1 %xce1916, label %xc1916.limpar, label %xc1916.fim
xc1916.limpar:
  call void @dartforge_exception_clear()
  br label %xc1916.fim
xc1916.fim:
  %gcvolta1223 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1223, ptr %ctxtopo, align 8
  ret i64 %v1915
b1224:
  br i1 %v1894, label %b1228, label %b1227
b1225:
  %v1910 = call i64 @df.box_int(i64 %v1885)
  store i64 %v1910, ptr %gcs1
  %sa1911_0 = getelementptr [2 x i64], ptr %sargs1911, i64 0, i64 0
  store i64 %v1910, ptr %sa1911_0
  %sat1911 = getelementptr [2 x i64], ptr %sargs1911, i64 0, i64 1
  store i64 0, ptr %sat1911
  %sic1911 = getelementptr i64, ptr %area, i64 240
  %sf1911 = call ptr @df.selector(ptr %sic1911, i64 %v0, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v1911 = call i64 %sf1911(i64 %v0, ptr %sargs1911, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1912 = load i8, ptr %ctx, align 8
  %c440 = zext i8 %v1912 to i64
  %v1913 = icmp ne i64 %c440, 0
  br i1 %v1913, label %b1235, label %b1234
b1226:
  %v1914 = phi i64 [ %v1909, %b1229 ], [ %v1911, %b1234 ]
  br label %b1223
b1227:
  %v1898 = call i64 @df.unbox_int(i64 %v0)
  %v1899 = load i8, ptr %ctx, align 8
  %c441 = zext i8 %v1899 to i64
  %v1900 = icmp ne i64 %c441, 0
  br i1 %v1900, label %b1231, label %b1230
b1228:
  %v1903 = call double @df.num_to_double(i64 %v0)
  %v1904 = load i8, ptr %ctx, align 8
  %c442 = zext i8 %v1904 to i64
  %v1905 = icmp ne i64 %c442, 0
  br i1 %v1905, label %b1233, label %b1232
b1229:
  %v1909 = phi i64 [ %v1902, %b1230 ], [ %v1908, %b1232 ]
  br label %b1226
b1230:
  %v1901 = mul i64 %v1898, %v1885
  store i64 0, ptr %gcs0
  %v1902 = call i64 @df.box_int(i64 %v1901)
  br label %b1229
b1231:
  %gcvolta1231 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1231, ptr %ctxtopo, align 8
  ret i64 0
b1232:
  %v1906 = sitofp i64 %v1885 to double
  %v1907 = fmul double %v1903, %v1906
  store i64 0, ptr %gcs0
  %v1908 = call i64 @df.box_double(double %v1907)
  br label %b1229
b1233:
  %gcvolta1233 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1233, ptr %ctxtopo, align 8
  ret i64 0
b1234:
  br label %b1226
b1235:
  %gcvolta1235 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1235, ptr %ctxtopo, align 8
  ret i64 0
b1237:
  store i64 0, ptr %gcs1
  %v1923 = call double @df.unbox_double(i64 %v1920)
  %v1924 = load i8, ptr %ctx, align 8
  %c443 = zext i8 %v1924 to i64
  %v1925 = icmp ne i64 %c443, 0
  br i1 %v1925, label %b1240, label %b1239
b1238:
  %gcvolta1238 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1238, ptr %ctxtopo, align 8
  ret i64 0
b1239:
  %xcp1927 = load i8, ptr %ctx, align 8
  %xcn1927 = icmp ne i8 %xcp1927, 0
  %xce1927 = call i1 @llvm.expect.i1(i1 %xcn1927, i1 false)
  br i1 %xce1927, label %xc1927.limpar, label %xc1927.fim
xc1927.limpar:
  call void @dartforge_exception_clear()
  br label %xc1927.fim
xc1927.fim:
  %gcvolta1239 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1239, ptr %ctxtopo, align 8
  ret i64 %v1920
b1240:
  %gcvolta1240 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1240, ptr %ctxtopo, align 8
  ret i64 0
b1254:
  %v2253 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2254 = call i64 @df.box_int(i64 %v2253)
  store i64 %v2254, ptr %gcs1
  %sa2255_0 = getelementptr [2 x i64], ptr %sargs2255, i64 0, i64 0
  store i64 %v2254, ptr %sa2255_0
  %sat2255 = getelementptr [2 x i64], ptr %sargs2255, i64 0, i64 1
  store i64 0, ptr %sat2255
  %sic2255 = getelementptr i64, ptr %area, i64 242
  %sf2255 = call ptr @df.selector(ptr %sic2255, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v2255 = call i64 %sf2255(i64 %v0, ptr %sargs2255, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2255, ptr %gcs0
  %v2256 = load i8, ptr %ctx, align 8
  %c444 = zext i8 %v2256 to i64
  %v2257 = icmp ne i64 %c444, 0
  br i1 %v2257, label %b1449, label %b1448
b1255:
  store i64 0, ptr %gcs1
  %v1933 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1933, ptr %gcs1
  %v1934 = load i8, ptr %ctx, align 8
  %c445 = zext i8 %v1934 to i64
  %v1935 = icmp ne i64 %c445, 0
  br i1 %v1935, label %b1261, label %b1260
b1256:
  %v1932 = icmp eq i64 %v1929, %v6
  br i1 %v1932, label %b1258, label %b1255
b1257:
  %gcvolta1257 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1257, ptr %ctxtopo, align 8
  ret i64 0
b1258:
  %v1978 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1979 = call i64 @df.box_int(i64 %v1978)
  store i64 %v1979, ptr %gcs1
  %v1980 = icmp eq i64 %v0, 0
  %v1981 = icmp eq i64 %v1979, 0
  %c446 = zext i1 %v1980 to i64
  %c447 = zext i1 %v1981 to i64
  %v1982 = or i64 %c446, %c447
  %v1983 = icmp ne i64 %v1982, 0
  br i1 %v1983, label %b1305, label %b1306
b1259:
  store i64 0, ptr %gcs1
  %v1937 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1937, ptr %gcs1
  %v1938 = load i8, ptr %ctx, align 8
  %c448 = zext i8 %v1938 to i64
  %v1939 = icmp ne i64 %c448, 0
  br i1 %v1939, label %b1265, label %b1264
b1260:
  %v1936 = icmp eq i64 %v1933, %v6
  br i1 %v1936, label %b1262, label %b1259
b1261:
  %gcvolta1261 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1261, ptr %ctxtopo, align 8
  ret i64 0
b1262:
  %v2010 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2011 = and i64 %v0, 1
  %v2012 = icmp ne i64 %v2011, 0
  br i1 %v2012, label %b1327, label %b1328
b1263:
  store i64 0, ptr %gcs1
  %v1941 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1941, ptr %gcs1
  %v1942 = load i8, ptr %ctx, align 8
  %c449 = zext i8 %v1942 to i64
  %v1943 = icmp ne i64 %c449, 0
  br i1 %v1943, label %b1269, label %b1268
b1264:
  %v1940 = icmp eq i64 %v1937, %v6
  br i1 %v1940, label %b1266, label %b1263
b1265:
  %gcvolta1265 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1265, ptr %ctxtopo, align 8
  ret i64 0
b1266:
  %v2049 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2050 = and i64 %v0, 1
  %v2051 = icmp ne i64 %v2050, 0
  br i1 %v2051, label %b1347, label %b1348
b1267:
  store i64 0, ptr %gcs1
  %v1945 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1945, ptr %gcs1
  %v1946 = load i8, ptr %ctx, align 8
  %c450 = zext i8 %v1946 to i64
  %v1947 = icmp ne i64 %c450, 0
  br i1 %v1947, label %b1273, label %b1272
b1268:
  %v1944 = icmp eq i64 %v1941, %v6
  br i1 %v1944, label %b1270, label %b1267
b1269:
  %gcvolta1269 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1269, ptr %ctxtopo, align 8
  ret i64 0
b1270:
  %v2088 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2089 = and i64 %v0, 1
  %v2090 = icmp ne i64 %v2089, 0
  br i1 %v2090, label %b1367, label %b1368
b1271:
  store i64 0, ptr %gcs1
  %v1949 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1949, ptr %gcs1
  %v1950 = load i8, ptr %ctx, align 8
  %c451 = zext i8 %v1950 to i64
  %v1951 = icmp ne i64 %c451, 0
  br i1 %v1951, label %b1277, label %b1276
b1272:
  %v1948 = icmp eq i64 %v1945, %v6
  br i1 %v1948, label %b1274, label %b1271
b1273:
  %gcvolta1273 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1273, ptr %ctxtopo, align 8
  ret i64 0
b1274:
  %v2127 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2128 = and i64 %v0, 1
  %v2129 = icmp ne i64 %v2128, 0
  br i1 %v2129, label %b1387, label %b1388
b1275:
  store i64 0, ptr %gcs1
  %v1953 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1953, ptr %gcs1
  %v1954 = load i8, ptr %ctx, align 8
  %c452 = zext i8 %v1954 to i64
  %v1955 = icmp ne i64 %c452, 0
  br i1 %v1955, label %b1281, label %b1280
b1276:
  %v1952 = icmp eq i64 %v1949, %v6
  br i1 %v1952, label %b1278, label %b1275
b1277:
  %gcvolta1277 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1277, ptr %ctxtopo, align 8
  ret i64 0
b1278:
  %v2166 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2167 = and i64 %v0, 1
  %v2168 = icmp ne i64 %v2167, 0
  br i1 %v2168, label %b1407, label %b1408
b1279:
  store i64 0, ptr %gcs1
  %v1957 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1957, ptr %gcs1
  %v1958 = load i8, ptr %ctx, align 8
  %c453 = zext i8 %v1958 to i64
  %v1959 = icmp ne i64 %c453, 0
  br i1 %v1959, label %b1285, label %b1284
b1280:
  %v1956 = icmp eq i64 %v1953, %v6
  br i1 %v1956, label %b1282, label %b1279
b1281:
  %gcvolta1281 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1281, ptr %ctxtopo, align 8
  ret i64 0
b1282:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2205 = icmp eq i64 %v0, 0
  %v2206 = call i64 @df.class(i64 %v0)
  %v2207 = call i8 @df.subclass(i64 %v2206, i64 674)
  %c454 = zext i8 %v2207 to i64
  %v2208 = icmp ne i64 %c454, 0
  %v2209 = xor i1 %v2205, true
  %c455 = zext i1 %v2208 to i64
  %c456 = zext i1 %v2209 to i64
  %v2210 = and i64 %c455, %c456
  %v2211 = icmp ne i64 %v2210, 0
  br i1 %v2211, label %b1428, label %b1427
b1283:
  store i64 0, ptr %gcs1
  %v1961 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1961, ptr %gcs1
  %v1962 = load i8, ptr %ctx, align 8
  %c457 = zext i8 %v1962 to i64
  %v1963 = icmp ne i64 %c457, 0
  br i1 %v1963, label %b1289, label %b1288
b1284:
  %v1960 = icmp eq i64 %v1957, %v6
  br i1 %v1960, label %b1286, label %b1283
b1285:
  %gcvolta1285 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1285, ptr %ctxtopo, align 8
  ret i64 0
b1286:
  %xcp2228 = load i8, ptr %ctx, align 8
  %xcn2228 = icmp ne i8 %xcp2228, 0
  %xce2228 = call i1 @llvm.expect.i1(i1 %xcn2228, i1 false)
  br i1 %xce2228, label %xc2228.limpar, label %xc2228.fim
xc2228.limpar:
  call void @dartforge_exception_clear()
  br label %xc2228.fim
xc2228.fim:
  %gcvolta1286 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1286, ptr %ctxtopo, align 8
  ret i64 %v0
b1287:
  store i64 0, ptr %gcs1
  %v1965 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1965, ptr %gcs1
  %v1966 = load i8, ptr %ctx, align 8
  %c458 = zext i8 %v1966 to i64
  %v1967 = icmp ne i64 %c458, 0
  br i1 %v1967, label %b1293, label %b1292
b1288:
  %v1964 = icmp eq i64 %v1961, %v6
  br i1 %v1964, label %b1290, label %b1287
b1289:
  %gcvolta1289 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1289, ptr %ctxtopo, align 8
  ret i64 0
b1290:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2230 = icmp eq i64 %v0, 0
  %v2231 = call i64 @df.class(i64 %v0)
  %v2232 = call i8 @df.subclass(i64 %v2231, i64 674)
  %c459 = zext i8 %v2232 to i64
  %v2233 = icmp ne i64 %c459, 0
  %v2234 = xor i1 %v2230, true
  %c460 = zext i1 %v2233 to i64
  %c461 = zext i1 %v2234 to i64
  %v2235 = and i64 %c460, %c461
  %v2236 = icmp ne i64 %v2235, 0
  br i1 %v2236, label %b1439, label %b1438
b1291:
  store i64 0, ptr %gcs1
  %v1969 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1969, ptr %gcs1
  %v1970 = load i8, ptr %ctx, align 8
  %c462 = zext i8 %v1970 to i64
  %v1971 = icmp ne i64 %c462, 0
  br i1 %v1971, label %b1298, label %b1297
b1292:
  %v1968 = icmp eq i64 %v1965, %v6
  br i1 %v1968, label %b1294, label %b1291
b1293:
  %gcvolta1293 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1293, ptr %ctxtopo, align 8
  ret i64 0
b1294:
  br label %b1254
b1296:
  store i64 0, ptr %gcs1
  %v1973 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1973, ptr %gcs1
  %v1974 = load i8, ptr %ctx, align 8
  %c463 = zext i8 %v1974 to i64
  %v1975 = icmp ne i64 %c463, 0
  br i1 %v1975, label %b1303, label %b1302
b1297:
  %v1972 = icmp eq i64 %v1969, %v6
  br i1 %v1972, label %b1299, label %b1296
b1298:
  %gcvolta1298 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1298, ptr %ctxtopo, align 8
  ret i64 0
b1299:
  br label %b1254
b1301:
  br label %b12
b1302:
  %v1976 = icmp eq i64 %v1973, %v6
  br i1 %v1976, label %b1304, label %b1301
b1303:
  %gcvolta1303 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1303, ptr %ctxtopo, align 8
  ret i64 0
b1304:
  br label %b1254
b1305:
  %v1984 = icmp eq i64 %v0, %v1979
  br label %b1307
b1306:
  %v1985 = call i64 @df.class(i64 %v0)
  switch i64 %v1985, label %b1311 [ i64 675, label %b1308 i64 4, label %b1309 i64 2, label %b1310 i64 3, label %b1310 ]
b1307:
  %v2003 = phi i1 [ %v1984, %b1305 ], [ %v2002, %b1312 ]
  br i1 %v2003, label %b1323, label %b1324
b1308:
  %v1986 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1979)
  %v1987 = load i8, ptr %ctx, align 8
  %c464 = zext i8 %v1987 to i64
  %v1988 = icmp ne i64 %c464, 0
  br i1 %v1988, label %b1314, label %b1313
b1309:
  %v1990 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1979)
  %v1991 = load i8, ptr %ctx, align 8
  %c465 = zext i8 %v1991 to i64
  %v1992 = icmp ne i64 %c465, 0
  br i1 %v1992, label %b1316, label %b1315
b1310:
  %v1993 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1979)
  %v1994 = load i8, ptr %ctx, align 8
  %c466 = zext i8 %v1994 to i64
  %v1995 = icmp ne i64 %c466, 0
  br i1 %v1995, label %b1318, label %b1317
b1311:
  %sa1996_0 = getelementptr [2 x i64], ptr %sargs1996, i64 0, i64 0
  store i64 %v1979, ptr %sa1996_0
  %sat1996 = getelementptr [2 x i64], ptr %sargs1996, i64 0, i64 1
  store i64 0, ptr %sat1996
  %sic1996 = getelementptr i64, ptr %area, i64 244
  %sf1996 = call ptr @df.selector(ptr %sic1996, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1996 = call i64 %sf1996(i64 %v0, ptr %sargs1996, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1996, ptr %gcs0
  %v1997 = load i8, ptr %ctx, align 8
  %c467 = zext i8 %v1997 to i64
  %v1998 = icmp ne i64 %c467, 0
  br i1 %v1998, label %b1320, label %b1319
b1312:
  %v2002 = phi i1 [ %v1989, %b1313 ], [ %v1990, %b1315 ], [ %v1993, %b1317 ], [ %v1999, %b1321 ]
  br label %b1307
b1313:
  %c468 = zext i8 %v1986 to i64
  %v1989 = icmp ne i64 %c468, 0
  br label %b1312
b1314:
  %gcvolta1314 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1314, ptr %ctxtopo, align 8
  ret i64 0
b1315:
  br label %b1312
b1316:
  %gcvolta1316 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1316, ptr %ctxtopo, align 8
  ret i64 0
b1317:
  br label %b1312
b1318:
  %gcvolta1318 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1318, ptr %ctxtopo, align 8
  ret i64 0
b1319:
  store i64 0, ptr %gcs1
  %v1999 = call i1 @df.unbox_bool(i64 %v1996)
  %v2000 = load i8, ptr %ctx, align 8
  %c469 = zext i8 %v2000 to i64
  %v2001 = icmp ne i64 %c469, 0
  br i1 %v2001, label %b1322, label %b1321
b1320:
  %gcvolta1320 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1320, ptr %ctxtopo, align 8
  ret i64 0
b1321:
  br label %b1312
b1322:
  %gcvolta1322 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1322, ptr %ctxtopo, align 8
  ret i64 0
b1323:
  %v2004 = add i64 0, 0
  br label %b1325
b1324:
  %v2005 = add i64 0, 1
  br label %b1325
b1325:
  %v2006 = phi i64 [ %v2004, %b1323 ], [ %v2005, %b1324 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2007 = call i64 @df.box_int(i64 %v2006)
  %xcp2008 = load i8, ptr %ctx, align 8
  %xcn2008 = icmp ne i8 %xcp2008, 0
  %xce2008 = call i1 @llvm.expect.i1(i1 %xcn2008, i1 false)
  br i1 %xce2008, label %xc2008.limpar, label %xc2008.fim
xc2008.limpar:
  call void @dartforge_exception_clear()
  br label %xc2008.fim
xc2008.fim:
  %gcvolta1325 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1325, ptr %ctxtopo, align 8
  ret i64 %v2007
b1327:
  %v2013 = ashr i64 %v0, 1
  %v2014 = icmp sle i64 %v2013, %v2010
  br label %b1329
b1328:
  %v2015 = call i64 @df.box_int(i64 %v2010)
  store i64 %v2015, ptr %gcs1
  %sa2016_0 = getelementptr [2 x i64], ptr %sargs2016, i64 0, i64 0
  store i64 %v2015, ptr %sa2016_0
  %sat2016 = getelementptr [2 x i64], ptr %sargs2016, i64 0, i64 1
  store i64 0, ptr %sat2016
  %sic2016 = getelementptr i64, ptr %area, i64 246
  %sf2016 = call ptr @df.selector(ptr %sic2016, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2016 = call i64 %sf2016(i64 %v0, ptr %sargs2016, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2016, ptr %gcs1
  %v2017 = load i8, ptr %ctx, align 8
  %c470 = zext i8 %v2017 to i64
  %v2018 = icmp ne i64 %c470, 0
  br i1 %v2018, label %b1331, label %b1330
b1329:
  %v2022 = phi i1 [ %v2014, %b1327 ], [ %v2019, %b1332 ]
  br i1 %v2022, label %b1334, label %b1335
b1330:
  %v2019 = call i1 @df.unbox_bool(i64 %v2016)
  %v2020 = load i8, ptr %ctx, align 8
  %c471 = zext i8 %v2020 to i64
  %v2021 = icmp ne i64 %c471, 0
  br i1 %v2021, label %b1333, label %b1332
b1331:
  %gcvolta1331 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1331, ptr %ctxtopo, align 8
  ret i64 0
b1332:
  br label %b1329
b1333:
  %gcvolta1333 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1333, ptr %ctxtopo, align 8
  ret i64 0
b1334:
  %v2023 = add i64 0, 0
  br label %b1336
b1335:
  store i64 0, ptr %gcs1
  %v2025 = icmp eq i64 %v0, 0
  %v2026 = call i64 @df.class(i64 %v0)
  %v2027 = call i8 @df.subclass(i64 %v2026, i64 674)
  %c472 = zext i8 %v2027 to i64
  %v2028 = icmp ne i64 %c472, 0
  %v2029 = xor i1 %v2025, true
  %c473 = zext i1 %v2028 to i64
  %c474 = zext i1 %v2029 to i64
  %v2030 = and i64 %c473, %c474
  %v2031 = icmp ne i64 %v2030, 0
  br i1 %v2031, label %b1338, label %b1337
b1336:
  %v2045 = phi i64 [ %v2023, %b1334 ], [ %v2044, %b1344 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2046 = call i64 @df.box_int(i64 %v2045)
  %xcp2047 = load i8, ptr %ctx, align 8
  %xcn2047 = icmp ne i8 %xcp2047, 0
  %xce2047 = call i1 @llvm.expect.i1(i1 %xcn2047, i1 false)
  br i1 %xce2047, label %xc2047.limpar, label %xc2047.fim
xc2047.limpar:
  call void @dartforge_exception_clear()
  br label %xc2047.fim
xc2047.fim:
  %gcvolta1336 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1336, ptr %ctxtopo, align 8
  ret i64 %v2046
b1337:
  %ga2032 = getelementptr i64, ptr %area, i64 7
  %v2032 = load i8, ptr %ga2032
  %c475 = zext i8 %v2032 to i64
  %v2033 = icmp ne i64 %c475, 0
  br i1 %v2033, label %b1339, label %b1340
b1338:
  %v2040 = call i64 @df.unbox_int(i64 %v0)
  %v2041 = load i8, ptr %ctx, align 8
  %c476 = zext i8 %v2041 to i64
  %v2042 = icmp ne i64 %c476, 0
  br i1 %v2042, label %b1345, label %b1344
b1339:
  %ga2034 = getelementptr i64, ptr %area, i64 6
  %v2034 = load i64, ptr %ga2034
  br label %b1341
b1340:
  %v2035 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1341
b1341:
  %v2036 = phi i64 [ %v2034, %b1339 ], [ %v2035, %b1340 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2036, i64 0, i64 0)
  %v2038 = load i8, ptr %ctx, align 8
  %c477 = zext i8 %v2038 to i64
  %v2039 = icmp ne i64 %c477, 0
  br i1 %v2039, label %b1343, label %b1342
b1342:
  br label %b1338
b1343:
  %gcvolta1343 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1343, ptr %ctxtopo, align 8
  ret i64 0
b1344:
  %v2043 = add i64 0, 15
  %v2044 = ashr i64 %v2040, %v2043
  br label %b1336
b1345:
  %gcvolta1345 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1345, ptr %ctxtopo, align 8
  ret i64 0
b1347:
  %v2052 = ashr i64 %v0, 1
  %v2053 = icmp sle i64 %v2052, %v2049
  br label %b1349
b1348:
  %v2054 = call i64 @df.box_int(i64 %v2049)
  store i64 %v2054, ptr %gcs1
  %sa2055_0 = getelementptr [2 x i64], ptr %sargs2055, i64 0, i64 0
  store i64 %v2054, ptr %sa2055_0
  %sat2055 = getelementptr [2 x i64], ptr %sargs2055, i64 0, i64 1
  store i64 0, ptr %sat2055
  %sic2055 = getelementptr i64, ptr %area, i64 248
  %sf2055 = call ptr @df.selector(ptr %sic2055, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2055 = call i64 %sf2055(i64 %v0, ptr %sargs2055, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2055, ptr %gcs1
  %v2056 = load i8, ptr %ctx, align 8
  %c478 = zext i8 %v2056 to i64
  %v2057 = icmp ne i64 %c478, 0
  br i1 %v2057, label %b1351, label %b1350
b1349:
  %v2061 = phi i1 [ %v2053, %b1347 ], [ %v2058, %b1352 ]
  br i1 %v2061, label %b1354, label %b1355
b1350:
  %v2058 = call i1 @df.unbox_bool(i64 %v2055)
  %v2059 = load i8, ptr %ctx, align 8
  %c479 = zext i8 %v2059 to i64
  %v2060 = icmp ne i64 %c479, 0
  br i1 %v2060, label %b1353, label %b1352
b1351:
  %gcvolta1351 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1351, ptr %ctxtopo, align 8
  ret i64 0
b1352:
  br label %b1349
b1353:
  %gcvolta1353 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1353, ptr %ctxtopo, align 8
  ret i64 0
b1354:
  %v2062 = add i64 0, 0
  br label %b1356
b1355:
  store i64 0, ptr %gcs1
  %v2064 = icmp eq i64 %v0, 0
  %v2065 = call i64 @df.class(i64 %v0)
  %v2066 = call i8 @df.subclass(i64 %v2065, i64 674)
  %c480 = zext i8 %v2066 to i64
  %v2067 = icmp ne i64 %c480, 0
  %v2068 = xor i1 %v2064, true
  %c481 = zext i1 %v2067 to i64
  %c482 = zext i1 %v2068 to i64
  %v2069 = and i64 %c481, %c482
  %v2070 = icmp ne i64 %v2069, 0
  br i1 %v2070, label %b1358, label %b1357
b1356:
  %v2084 = phi i64 [ %v2062, %b1354 ], [ %v2083, %b1364 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2085 = call i64 @df.box_int(i64 %v2084)
  %xcp2086 = load i8, ptr %ctx, align 8
  %xcn2086 = icmp ne i8 %xcp2086, 0
  %xce2086 = call i1 @llvm.expect.i1(i1 %xcn2086, i1 false)
  br i1 %xce2086, label %xc2086.limpar, label %xc2086.fim
xc2086.limpar:
  call void @dartforge_exception_clear()
  br label %xc2086.fim
xc2086.fim:
  %gcvolta1356 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1356, ptr %ctxtopo, align 8
  ret i64 %v2085
b1357:
  %ga2071 = getelementptr i64, ptr %area, i64 7
  %v2071 = load i8, ptr %ga2071
  %c483 = zext i8 %v2071 to i64
  %v2072 = icmp ne i64 %c483, 0
  br i1 %v2072, label %b1359, label %b1360
b1358:
  %v2079 = call i64 @df.unbox_int(i64 %v0)
  %v2080 = load i8, ptr %ctx, align 8
  %c484 = zext i8 %v2080 to i64
  %v2081 = icmp ne i64 %c484, 0
  br i1 %v2081, label %b1365, label %b1364
b1359:
  %ga2073 = getelementptr i64, ptr %area, i64 6
  %v2073 = load i64, ptr %ga2073
  br label %b1361
b1360:
  %v2074 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1361
b1361:
  %v2075 = phi i64 [ %v2073, %b1359 ], [ %v2074, %b1360 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2075, i64 0, i64 0)
  %v2077 = load i8, ptr %ctx, align 8
  %c485 = zext i8 %v2077 to i64
  %v2078 = icmp ne i64 %c485, 0
  br i1 %v2078, label %b1363, label %b1362
b1362:
  br label %b1358
b1363:
  %gcvolta1363 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1363, ptr %ctxtopo, align 8
  ret i64 0
b1364:
  %v2082 = add i64 0, 11
  %v2083 = ashr i64 %v2079, %v2082
  br label %b1356
b1365:
  %gcvolta1365 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1365, ptr %ctxtopo, align 8
  ret i64 0
b1367:
  %v2091 = ashr i64 %v0, 1
  %v2092 = icmp sle i64 %v2091, %v2088
  br label %b1369
b1368:
  %v2093 = call i64 @df.box_int(i64 %v2088)
  store i64 %v2093, ptr %gcs1
  %sa2094_0 = getelementptr [2 x i64], ptr %sargs2094, i64 0, i64 0
  store i64 %v2093, ptr %sa2094_0
  %sat2094 = getelementptr [2 x i64], ptr %sargs2094, i64 0, i64 1
  store i64 0, ptr %sat2094
  %sic2094 = getelementptr i64, ptr %area, i64 250
  %sf2094 = call ptr @df.selector(ptr %sic2094, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2094 = call i64 %sf2094(i64 %v0, ptr %sargs2094, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2094, ptr %gcs1
  %v2095 = load i8, ptr %ctx, align 8
  %c486 = zext i8 %v2095 to i64
  %v2096 = icmp ne i64 %c486, 0
  br i1 %v2096, label %b1371, label %b1370
b1369:
  %v2100 = phi i1 [ %v2092, %b1367 ], [ %v2097, %b1372 ]
  br i1 %v2100, label %b1374, label %b1375
b1370:
  %v2097 = call i1 @df.unbox_bool(i64 %v2094)
  %v2098 = load i8, ptr %ctx, align 8
  %c487 = zext i8 %v2098 to i64
  %v2099 = icmp ne i64 %c487, 0
  br i1 %v2099, label %b1373, label %b1372
b1371:
  %gcvolta1371 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1371, ptr %ctxtopo, align 8
  ret i64 0
b1372:
  br label %b1369
b1373:
  %gcvolta1373 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1373, ptr %ctxtopo, align 8
  ret i64 0
b1374:
  %v2101 = add i64 0, 0
  br label %b1376
b1375:
  store i64 0, ptr %gcs1
  %v2103 = icmp eq i64 %v0, 0
  %v2104 = call i64 @df.class(i64 %v0)
  %v2105 = call i8 @df.subclass(i64 %v2104, i64 674)
  %c488 = zext i8 %v2105 to i64
  %v2106 = icmp ne i64 %c488, 0
  %v2107 = xor i1 %v2103, true
  %c489 = zext i1 %v2106 to i64
  %c490 = zext i1 %v2107 to i64
  %v2108 = and i64 %c489, %c490
  %v2109 = icmp ne i64 %v2108, 0
  br i1 %v2109, label %b1378, label %b1377
b1376:
  %v2123 = phi i64 [ %v2101, %b1374 ], [ %v2122, %b1384 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2124 = call i64 @df.box_int(i64 %v2123)
  %xcp2125 = load i8, ptr %ctx, align 8
  %xcn2125 = icmp ne i8 %xcp2125, 0
  %xce2125 = call i1 @llvm.expect.i1(i1 %xcn2125, i1 false)
  br i1 %xce2125, label %xc2125.limpar, label %xc2125.fim
xc2125.limpar:
  call void @dartforge_exception_clear()
  br label %xc2125.fim
xc2125.fim:
  %gcvolta1376 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1376, ptr %ctxtopo, align 8
  ret i64 %v2124
b1377:
  %ga2110 = getelementptr i64, ptr %area, i64 7
  %v2110 = load i8, ptr %ga2110
  %c491 = zext i8 %v2110 to i64
  %v2111 = icmp ne i64 %c491, 0
  br i1 %v2111, label %b1379, label %b1380
b1378:
  %v2118 = call i64 @df.unbox_int(i64 %v0)
  %v2119 = load i8, ptr %ctx, align 8
  %c492 = zext i8 %v2119 to i64
  %v2120 = icmp ne i64 %c492, 0
  br i1 %v2120, label %b1385, label %b1384
b1379:
  %ga2112 = getelementptr i64, ptr %area, i64 6
  %v2112 = load i64, ptr %ga2112
  br label %b1381
b1380:
  %v2113 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1381
b1381:
  %v2114 = phi i64 [ %v2112, %b1379 ], [ %v2113, %b1380 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2114, i64 0, i64 0)
  %v2116 = load i8, ptr %ctx, align 8
  %c493 = zext i8 %v2116 to i64
  %v2117 = icmp ne i64 %c493, 0
  br i1 %v2117, label %b1383, label %b1382
b1382:
  br label %b1378
b1383:
  %gcvolta1383 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1383, ptr %ctxtopo, align 8
  ret i64 0
b1384:
  %v2121 = add i64 0, 7
  %v2122 = ashr i64 %v2118, %v2121
  br label %b1376
b1385:
  %gcvolta1385 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1385, ptr %ctxtopo, align 8
  ret i64 0
b1387:
  %v2130 = ashr i64 %v0, 1
  %v2131 = icmp sle i64 %v2130, %v2127
  br label %b1389
b1388:
  %v2132 = call i64 @df.box_int(i64 %v2127)
  store i64 %v2132, ptr %gcs1
  %sa2133_0 = getelementptr [2 x i64], ptr %sargs2133, i64 0, i64 0
  store i64 %v2132, ptr %sa2133_0
  %sat2133 = getelementptr [2 x i64], ptr %sargs2133, i64 0, i64 1
  store i64 0, ptr %sat2133
  %sic2133 = getelementptr i64, ptr %area, i64 252
  %sf2133 = call ptr @df.selector(ptr %sic2133, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2133 = call i64 %sf2133(i64 %v0, ptr %sargs2133, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2133, ptr %gcs1
  %v2134 = load i8, ptr %ctx, align 8
  %c494 = zext i8 %v2134 to i64
  %v2135 = icmp ne i64 %c494, 0
  br i1 %v2135, label %b1391, label %b1390
b1389:
  %v2139 = phi i1 [ %v2131, %b1387 ], [ %v2136, %b1392 ]
  br i1 %v2139, label %b1394, label %b1395
b1390:
  %v2136 = call i1 @df.unbox_bool(i64 %v2133)
  %v2137 = load i8, ptr %ctx, align 8
  %c495 = zext i8 %v2137 to i64
  %v2138 = icmp ne i64 %c495, 0
  br i1 %v2138, label %b1393, label %b1392
b1391:
  %gcvolta1391 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1391, ptr %ctxtopo, align 8
  ret i64 0
b1392:
  br label %b1389
b1393:
  %gcvolta1393 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1393, ptr %ctxtopo, align 8
  ret i64 0
b1394:
  %v2140 = add i64 0, 0
  br label %b1396
b1395:
  store i64 0, ptr %gcs1
  %v2142 = icmp eq i64 %v0, 0
  %v2143 = call i64 @df.class(i64 %v0)
  %v2144 = call i8 @df.subclass(i64 %v2143, i64 674)
  %c496 = zext i8 %v2144 to i64
  %v2145 = icmp ne i64 %c496, 0
  %v2146 = xor i1 %v2142, true
  %c497 = zext i1 %v2145 to i64
  %c498 = zext i1 %v2146 to i64
  %v2147 = and i64 %c497, %c498
  %v2148 = icmp ne i64 %v2147, 0
  br i1 %v2148, label %b1398, label %b1397
b1396:
  %v2162 = phi i64 [ %v2140, %b1394 ], [ %v2161, %b1404 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2163 = call i64 @df.box_int(i64 %v2162)
  %xcp2164 = load i8, ptr %ctx, align 8
  %xcn2164 = icmp ne i8 %xcp2164, 0
  %xce2164 = call i1 @llvm.expect.i1(i1 %xcn2164, i1 false)
  br i1 %xce2164, label %xc2164.limpar, label %xc2164.fim
xc2164.limpar:
  call void @dartforge_exception_clear()
  br label %xc2164.fim
xc2164.fim:
  %gcvolta1396 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1396, ptr %ctxtopo, align 8
  ret i64 %v2163
b1397:
  %ga2149 = getelementptr i64, ptr %area, i64 7
  %v2149 = load i8, ptr %ga2149
  %c499 = zext i8 %v2149 to i64
  %v2150 = icmp ne i64 %c499, 0
  br i1 %v2150, label %b1399, label %b1400
b1398:
  %v2157 = call i64 @df.unbox_int(i64 %v0)
  %v2158 = load i8, ptr %ctx, align 8
  %c500 = zext i8 %v2158 to i64
  %v2159 = icmp ne i64 %c500, 0
  br i1 %v2159, label %b1405, label %b1404
b1399:
  %ga2151 = getelementptr i64, ptr %area, i64 6
  %v2151 = load i64, ptr %ga2151
  br label %b1401
b1400:
  %v2152 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1401
b1401:
  %v2153 = phi i64 [ %v2151, %b1399 ], [ %v2152, %b1400 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2153, i64 0, i64 0)
  %v2155 = load i8, ptr %ctx, align 8
  %c501 = zext i8 %v2155 to i64
  %v2156 = icmp ne i64 %c501, 0
  br i1 %v2156, label %b1403, label %b1402
b1402:
  br label %b1398
b1403:
  %gcvolta1403 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1403, ptr %ctxtopo, align 8
  ret i64 0
b1404:
  %v2160 = add i64 0, 1
  %v2161 = shl i64 %v2157, %v2160
  br label %b1396
b1405:
  %gcvolta1405 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1405, ptr %ctxtopo, align 8
  ret i64 0
b1407:
  %v2169 = ashr i64 %v0, 1
  %v2170 = icmp sle i64 %v2169, %v2166
  br label %b1409
b1408:
  %v2171 = call i64 @df.box_int(i64 %v2166)
  store i64 %v2171, ptr %gcs1
  %sa2172_0 = getelementptr [2 x i64], ptr %sargs2172, i64 0, i64 0
  store i64 %v2171, ptr %sa2172_0
  %sat2172 = getelementptr [2 x i64], ptr %sargs2172, i64 0, i64 1
  store i64 0, ptr %sat2172
  %sic2172 = getelementptr i64, ptr %area, i64 254
  %sf2172 = call ptr @df.selector(ptr %sic2172, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2172 = call i64 %sf2172(i64 %v0, ptr %sargs2172, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2172, ptr %gcs1
  %v2173 = load i8, ptr %ctx, align 8
  %c502 = zext i8 %v2173 to i64
  %v2174 = icmp ne i64 %c502, 0
  br i1 %v2174, label %b1411, label %b1410
b1409:
  %v2178 = phi i1 [ %v2170, %b1407 ], [ %v2175, %b1412 ]
  br i1 %v2178, label %b1414, label %b1415
b1410:
  %v2175 = call i1 @df.unbox_bool(i64 %v2172)
  %v2176 = load i8, ptr %ctx, align 8
  %c503 = zext i8 %v2176 to i64
  %v2177 = icmp ne i64 %c503, 0
  br i1 %v2177, label %b1413, label %b1412
b1411:
  %gcvolta1411 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1411, ptr %ctxtopo, align 8
  ret i64 0
b1412:
  br label %b1409
b1413:
  %gcvolta1413 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1413, ptr %ctxtopo, align 8
  ret i64 0
b1414:
  %v2179 = add i64 0, 0
  br label %b1416
b1415:
  store i64 0, ptr %gcs1
  %v2181 = icmp eq i64 %v0, 0
  %v2182 = call i64 @df.class(i64 %v0)
  %v2183 = call i8 @df.subclass(i64 %v2182, i64 674)
  %c504 = zext i8 %v2183 to i64
  %v2184 = icmp ne i64 %c504, 0
  %v2185 = xor i1 %v2181, true
  %c505 = zext i1 %v2184 to i64
  %c506 = zext i1 %v2185 to i64
  %v2186 = and i64 %c505, %c506
  %v2187 = icmp ne i64 %v2186, 0
  br i1 %v2187, label %b1418, label %b1417
b1416:
  %v2201 = phi i64 [ %v2179, %b1414 ], [ %v2200, %b1424 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2202 = call i64 @df.box_int(i64 %v2201)
  %xcp2203 = load i8, ptr %ctx, align 8
  %xcn2203 = icmp ne i8 %xcp2203, 0
  %xce2203 = call i1 @llvm.expect.i1(i1 %xcn2203, i1 false)
  br i1 %xce2203, label %xc2203.limpar, label %xc2203.fim
xc2203.limpar:
  call void @dartforge_exception_clear()
  br label %xc2203.fim
xc2203.fim:
  %gcvolta1416 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1416, ptr %ctxtopo, align 8
  ret i64 %v2202
b1417:
  %ga2188 = getelementptr i64, ptr %area, i64 7
  %v2188 = load i8, ptr %ga2188
  %c507 = zext i8 %v2188 to i64
  %v2189 = icmp ne i64 %c507, 0
  br i1 %v2189, label %b1419, label %b1420
b1418:
  %v2196 = call i64 @df.unbox_int(i64 %v0)
  %v2197 = load i8, ptr %ctx, align 8
  %c508 = zext i8 %v2197 to i64
  %v2198 = icmp ne i64 %c508, 0
  br i1 %v2198, label %b1425, label %b1424
b1419:
  %ga2190 = getelementptr i64, ptr %area, i64 6
  %v2190 = load i64, ptr %ga2190
  br label %b1421
b1420:
  %v2191 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1421
b1421:
  %v2192 = phi i64 [ %v2190, %b1419 ], [ %v2191, %b1420 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2192, i64 0, i64 0)
  %v2194 = load i8, ptr %ctx, align 8
  %c509 = zext i8 %v2194 to i64
  %v2195 = icmp ne i64 %c509, 0
  br i1 %v2195, label %b1423, label %b1422
b1422:
  br label %b1418
b1423:
  %gcvolta1423 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1423, ptr %ctxtopo, align 8
  ret i64 0
b1424:
  %v2199 = add i64 0, 131076
  %v2200 = mul i64 %v2196, %v2199
  br label %b1416
b1425:
  %gcvolta1425 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1425, ptr %ctxtopo, align 8
  ret i64 0
b1427:
  %ga2212 = getelementptr i64, ptr %area, i64 7
  %v2212 = load i8, ptr %ga2212
  %c510 = zext i8 %v2212 to i64
  %v2213 = icmp ne i64 %c510, 0
  br i1 %v2213, label %b1429, label %b1430
b1428:
  %v2220 = call i64 @df.unbox_int(i64 %v0)
  %v2221 = load i8, ptr %ctx, align 8
  %c511 = zext i8 %v2221 to i64
  %v2222 = icmp ne i64 %c511, 0
  br i1 %v2222, label %b1435, label %b1434
b1429:
  %ga2214 = getelementptr i64, ptr %area, i64 6
  %v2214 = load i64, ptr %ga2214
  br label %b1431
b1430:
  %v2215 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1431
b1431:
  %v2216 = phi i64 [ %v2214, %b1429 ], [ %v2215, %b1430 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2216, i64 0, i64 0)
  %v2218 = load i8, ptr %ctx, align 8
  %c512 = zext i8 %v2218 to i64
  %v2219 = icmp ne i64 %c512, 0
  br i1 %v2219, label %b1433, label %b1432
b1432:
  br label %b1428
b1433:
  %gcvolta1433 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1433, ptr %ctxtopo, align 8
  ret i64 0
b1434:
  %v2223 = add i64 0, 8
  %v2224 = ashr i64 %v2220, %v2223
  store i64 0, ptr %gcs0
  %v2225 = call i64 @df.box_int(i64 %v2224)
  %xcp2226 = load i8, ptr %ctx, align 8
  %xcn2226 = icmp ne i8 %xcp2226, 0
  %xce2226 = call i1 @llvm.expect.i1(i1 %xcn2226, i1 false)
  br i1 %xce2226, label %xc2226.limpar, label %xc2226.fim
xc2226.limpar:
  call void @dartforge_exception_clear()
  br label %xc2226.fim
xc2226.fim:
  %gcvolta1434 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1434, ptr %ctxtopo, align 8
  ret i64 %v2225
b1435:
  %gcvolta1435 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1435, ptr %ctxtopo, align 8
  ret i64 0
b1438:
  %ga2237 = getelementptr i64, ptr %area, i64 7
  %v2237 = load i8, ptr %ga2237
  %c513 = zext i8 %v2237 to i64
  %v2238 = icmp ne i64 %c513, 0
  br i1 %v2238, label %b1440, label %b1441
b1439:
  %v2245 = call i64 @df.unbox_int(i64 %v0)
  %v2246 = load i8, ptr %ctx, align 8
  %c514 = zext i8 %v2246 to i64
  %v2247 = icmp ne i64 %c514, 0
  br i1 %v2247, label %b1446, label %b1445
b1440:
  %ga2239 = getelementptr i64, ptr %area, i64 6
  %v2239 = load i64, ptr %ga2239
  br label %b1442
b1441:
  %v2240 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1442
b1442:
  %v2241 = phi i64 [ %v2239, %b1440 ], [ %v2240, %b1441 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2241, i64 0, i64 0)
  %v2243 = load i8, ptr %ctx, align 8
  %c515 = zext i8 %v2243 to i64
  %v2244 = icmp ne i64 %c515, 0
  br i1 %v2244, label %b1444, label %b1443
b1443:
  br label %b1439
b1444:
  %gcvolta1444 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1444, ptr %ctxtopo, align 8
  ret i64 0
b1445:
  %v2248 = add i64 0, 65538
  %v2249 = mul i64 %v2245, %v2248
  store i64 0, ptr %gcs0
  %v2250 = call i64 @df.box_int(i64 %v2249)
  %xcp2251 = load i8, ptr %ctx, align 8
  %xcn2251 = icmp ne i8 %xcp2251, 0
  %xce2251 = call i1 @llvm.expect.i1(i1 %xcn2251, i1 false)
  br i1 %xce2251, label %xc2251.limpar, label %xc2251.fim
xc2251.limpar:
  call void @dartforge_exception_clear()
  br label %xc2251.fim
xc2251.fim:
  %gcvolta1445 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1445, ptr %ctxtopo, align 8
  ret i64 %v2250
b1446:
  %gcvolta1446 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1446, ptr %ctxtopo, align 8
  ret i64 0
b1448:
  store i64 0, ptr %gcs1
  %v2258 = call double @df.unbox_double(i64 %v2255)
  %v2259 = load i8, ptr %ctx, align 8
  %c516 = zext i8 %v2259 to i64
  %v2260 = icmp ne i64 %c516, 0
  br i1 %v2260, label %b1451, label %b1450
b1449:
  %gcvolta1449 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1449, ptr %ctxtopo, align 8
  ret i64 0
b1450:
  %xcp2262 = load i8, ptr %ctx, align 8
  %xcn2262 = icmp ne i8 %xcp2262, 0
  %xce2262 = call i1 @llvm.expect.i1(i1 %xcn2262, i1 false)
  br i1 %xce2262, label %xc2262.limpar, label %xc2262.fim
xc2262.limpar:
  call void @dartforge_exception_clear()
  br label %xc2262.fim
xc2262.fim:
  %gcvolta1450 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1450, ptr %ctxtopo, align 8
  ret i64 %v2255
b1451:
  %gcvolta1451 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1451, ptr %ctxtopo, align 8
  ret i64 0
b1465:
  %v2588 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2589 = call i64 @df.box_int(i64 %v2588)
  store i64 %v2589, ptr %gcs1
  %sa2590_0 = getelementptr [2 x i64], ptr %sargs2590, i64 0, i64 0
  store i64 %v2589, ptr %sa2590_0
  %sat2590 = getelementptr [2 x i64], ptr %sargs2590, i64 0, i64 1
  store i64 0, ptr %sat2590
  %sic2590 = getelementptr i64, ptr %area, i64 256
  %sf2590 = call ptr @df.selector(ptr %sic2590, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v2590 = call i64 %sf2590(i64 %v0, ptr %sargs2590, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2590, ptr %gcs0
  %v2591 = load i8, ptr %ctx, align 8
  %c517 = zext i8 %v2591 to i64
  %v2592 = icmp ne i64 %c517, 0
  br i1 %v2592, label %b1660, label %b1659
b1466:
  store i64 0, ptr %gcs1
  %v2268 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v2268, ptr %gcs1
  %v2269 = load i8, ptr %ctx, align 8
  %c518 = zext i8 %v2269 to i64
  %v2270 = icmp ne i64 %c518, 0
  br i1 %v2270, label %b1472, label %b1471
b1467:
  %v2267 = icmp eq i64 %v2264, %v6
  br i1 %v2267, label %b1469, label %b1466
b1468:
  %gcvolta1468 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1468, ptr %ctxtopo, align 8
  ret i64 0
b1469:
  %v2313 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2314 = call i64 @df.box_int(i64 %v2313)
  store i64 %v2314, ptr %gcs1
  %v2315 = icmp eq i64 %v0, 0
  %v2316 = icmp eq i64 %v2314, 0
  %c519 = zext i1 %v2315 to i64
  %c520 = zext i1 %v2316 to i64
  %v2317 = or i64 %c519, %c520
  %v2318 = icmp ne i64 %v2317, 0
  br i1 %v2318, label %b1516, label %b1517
b1470:
  store i64 0, ptr %gcs1
  %v2272 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v2272, ptr %gcs1
  %v2273 = load i8, ptr %ctx, align 8
  %c521 = zext i8 %v2273 to i64
  %v2274 = icmp ne i64 %c521, 0
  br i1 %v2274, label %b1476, label %b1475
b1471:
  %v2271 = icmp eq i64 %v2268, %v6
  br i1 %v2271, label %b1473, label %b1470
b1472:
  %gcvolta1472 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1472, ptr %ctxtopo, align 8
  ret i64 0
b1473:
  %v2345 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2346 = and i64 %v0, 1
  %v2347 = icmp ne i64 %v2346, 0
  br i1 %v2347, label %b1538, label %b1539
b1474:
  store i64 0, ptr %gcs1
  %v2276 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v2276, ptr %gcs1
  %v2277 = load i8, ptr %ctx, align 8
  %c522 = zext i8 %v2277 to i64
  %v2278 = icmp ne i64 %c522, 0
  br i1 %v2278, label %b1480, label %b1479
b1475:
  %v2275 = icmp eq i64 %v2272, %v6
  br i1 %v2275, label %b1477, label %b1474
b1476:
  %gcvolta1476 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1476, ptr %ctxtopo, align 8
  ret i64 0
b1477:
  %v2384 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2385 = and i64 %v0, 1
  %v2386 = icmp ne i64 %v2385, 0
  br i1 %v2386, label %b1558, label %b1559
b1478:
  store i64 0, ptr %gcs1
  %v2280 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v2280, ptr %gcs1
  %v2281 = load i8, ptr %ctx, align 8
  %c523 = zext i8 %v2281 to i64
  %v2282 = icmp ne i64 %c523, 0
  br i1 %v2282, label %b1484, label %b1483
b1479:
  %v2279 = icmp eq i64 %v2276, %v6
  br i1 %v2279, label %b1481, label %b1478
b1480:
  %gcvolta1480 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1480, ptr %ctxtopo, align 8
  ret i64 0
b1481:
  %v2423 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2424 = and i64 %v0, 1
  %v2425 = icmp ne i64 %v2424, 0
  br i1 %v2425, label %b1578, label %b1579
b1482:
  store i64 0, ptr %gcs1
  %v2284 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v2284, ptr %gcs1
  %v2285 = load i8, ptr %ctx, align 8
  %c524 = zext i8 %v2285 to i64
  %v2286 = icmp ne i64 %c524, 0
  br i1 %v2286, label %b1488, label %b1487
b1483:
  %v2283 = icmp eq i64 %v2280, %v6
  br i1 %v2283, label %b1485, label %b1482
b1484:
  %gcvolta1484 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1484, ptr %ctxtopo, align 8
  ret i64 0
b1485:
  %v2462 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2463 = and i64 %v0, 1
  %v2464 = icmp ne i64 %v2463, 0
  br i1 %v2464, label %b1598, label %b1599
b1486:
  store i64 0, ptr %gcs1
  %v2288 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v2288, ptr %gcs1
  %v2289 = load i8, ptr %ctx, align 8
  %c525 = zext i8 %v2289 to i64
  %v2290 = icmp ne i64 %c525, 0
  br i1 %v2290, label %b1492, label %b1491
b1487:
  %v2287 = icmp eq i64 %v2284, %v6
  br i1 %v2287, label %b1489, label %b1486
b1488:
  %gcvolta1488 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1488, ptr %ctxtopo, align 8
  ret i64 0
b1489:
  %v2501 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2502 = and i64 %v0, 1
  %v2503 = icmp ne i64 %v2502, 0
  br i1 %v2503, label %b1618, label %b1619
b1490:
  store i64 0, ptr %gcs1
  %v2292 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v2292, ptr %gcs1
  %v2293 = load i8, ptr %ctx, align 8
  %c526 = zext i8 %v2293 to i64
  %v2294 = icmp ne i64 %c526, 0
  br i1 %v2294, label %b1496, label %b1495
b1491:
  %v2291 = icmp eq i64 %v2288, %v6
  br i1 %v2291, label %b1493, label %b1490
b1492:
  %gcvolta1492 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1492, ptr %ctxtopo, align 8
  ret i64 0
b1493:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2540 = icmp eq i64 %v0, 0
  %v2541 = call i64 @df.class(i64 %v0)
  %v2542 = call i8 @df.subclass(i64 %v2541, i64 674)
  %c527 = zext i8 %v2542 to i64
  %v2543 = icmp ne i64 %c527, 0
  %v2544 = xor i1 %v2540, true
  %c528 = zext i1 %v2543 to i64
  %c529 = zext i1 %v2544 to i64
  %v2545 = and i64 %c528, %c529
  %v2546 = icmp ne i64 %v2545, 0
  br i1 %v2546, label %b1639, label %b1638
b1494:
  store i64 0, ptr %gcs1
  %v2296 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v2296, ptr %gcs1
  %v2297 = load i8, ptr %ctx, align 8
  %c530 = zext i8 %v2297 to i64
  %v2298 = icmp ne i64 %c530, 0
  br i1 %v2298, label %b1500, label %b1499
b1495:
  %v2295 = icmp eq i64 %v2292, %v6
  br i1 %v2295, label %b1497, label %b1494
b1496:
  %gcvolta1496 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1496, ptr %ctxtopo, align 8
  ret i64 0
b1497:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2563 = icmp eq i64 %v0, 0
  %v2564 = call i64 @df.class(i64 %v0)
  %v2565 = call i8 @df.subclass(i64 %v2564, i64 674)
  %c531 = zext i8 %v2565 to i64
  %v2566 = icmp ne i64 %c531, 0
  %v2567 = xor i1 %v2563, true
  %c532 = zext i1 %v2566 to i64
  %c533 = zext i1 %v2567 to i64
  %v2568 = and i64 %c532, %c533
  %v2569 = icmp ne i64 %v2568, 0
  br i1 %v2569, label %b1649, label %b1648
b1498:
  store i64 0, ptr %gcs1
  %v2300 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v2300, ptr %gcs1
  %v2301 = load i8, ptr %ctx, align 8
  %c534 = zext i8 %v2301 to i64
  %v2302 = icmp ne i64 %c534, 0
  br i1 %v2302, label %b1504, label %b1503
b1499:
  %v2299 = icmp eq i64 %v2296, %v6
  br i1 %v2299, label %b1501, label %b1498
b1500:
  %gcvolta1500 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1500, ptr %ctxtopo, align 8
  ret i64 0
b1501:
  %xcp2586 = load i8, ptr %ctx, align 8
  %xcn2586 = icmp ne i8 %xcp2586, 0
  %xce2586 = call i1 @llvm.expect.i1(i1 %xcn2586, i1 false)
  br i1 %xce2586, label %xc2586.limpar, label %xc2586.fim
xc2586.limpar:
  call void @dartforge_exception_clear()
  br label %xc2586.fim
xc2586.fim:
  %gcvolta1501 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1501, ptr %ctxtopo, align 8
  ret i64 %v0
b1502:
  store i64 0, ptr %gcs1
  %v2304 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v2304, ptr %gcs1
  %v2305 = load i8, ptr %ctx, align 8
  %c535 = zext i8 %v2305 to i64
  %v2306 = icmp ne i64 %c535, 0
  br i1 %v2306, label %b1509, label %b1508
b1503:
  %v2303 = icmp eq i64 %v2300, %v6
  br i1 %v2303, label %b1505, label %b1502
b1504:
  %gcvolta1504 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1504, ptr %ctxtopo, align 8
  ret i64 0
b1505:
  br label %b1465
b1507:
  store i64 0, ptr %gcs1
  %v2308 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v2308, ptr %gcs1
  %v2309 = load i8, ptr %ctx, align 8
  %c536 = zext i8 %v2309 to i64
  %v2310 = icmp ne i64 %c536, 0
  br i1 %v2310, label %b1514, label %b1513
b1508:
  %v2307 = icmp eq i64 %v2304, %v6
  br i1 %v2307, label %b1510, label %b1507
b1509:
  %gcvolta1509 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1509, ptr %ctxtopo, align 8
  ret i64 0
b1510:
  br label %b1465
b1512:
  br label %b12
b1513:
  %v2311 = icmp eq i64 %v2308, %v6
  br i1 %v2311, label %b1515, label %b1512
b1514:
  %gcvolta1514 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1514, ptr %ctxtopo, align 8
  ret i64 0
b1515:
  br label %b1465
b1516:
  %v2319 = icmp eq i64 %v0, %v2314
  br label %b1518
b1517:
  %v2320 = call i64 @df.class(i64 %v0)
  switch i64 %v2320, label %b1522 [ i64 675, label %b1519 i64 4, label %b1520 i64 2, label %b1521 i64 3, label %b1521 ]
b1518:
  %v2338 = phi i1 [ %v2319, %b1516 ], [ %v2337, %b1523 ]
  br i1 %v2338, label %b1534, label %b1535
b1519:
  %v2321 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v2314)
  %v2322 = load i8, ptr %ctx, align 8
  %c537 = zext i8 %v2322 to i64
  %v2323 = icmp ne i64 %c537, 0
  br i1 %v2323, label %b1525, label %b1524
b1520:
  %v2325 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v2314)
  %v2326 = load i8, ptr %ctx, align 8
  %c538 = zext i8 %v2326 to i64
  %v2327 = icmp ne i64 %c538, 0
  br i1 %v2327, label %b1527, label %b1526
b1521:
  %v2328 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v2314)
  %v2329 = load i8, ptr %ctx, align 8
  %c539 = zext i8 %v2329 to i64
  %v2330 = icmp ne i64 %c539, 0
  br i1 %v2330, label %b1529, label %b1528
b1522:
  %sa2331_0 = getelementptr [2 x i64], ptr %sargs2331, i64 0, i64 0
  store i64 %v2314, ptr %sa2331_0
  %sat2331 = getelementptr [2 x i64], ptr %sargs2331, i64 0, i64 1
  store i64 0, ptr %sat2331
  %sic2331 = getelementptr i64, ptr %area, i64 258
  %sf2331 = call ptr @df.selector(ptr %sic2331, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v2331 = call i64 %sf2331(i64 %v0, ptr %sargs2331, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2331, ptr %gcs0
  %v2332 = load i8, ptr %ctx, align 8
  %c540 = zext i8 %v2332 to i64
  %v2333 = icmp ne i64 %c540, 0
  br i1 %v2333, label %b1531, label %b1530
b1523:
  %v2337 = phi i1 [ %v2324, %b1524 ], [ %v2325, %b1526 ], [ %v2328, %b1528 ], [ %v2334, %b1532 ]
  br label %b1518
b1524:
  %c541 = zext i8 %v2321 to i64
  %v2324 = icmp ne i64 %c541, 0
  br label %b1523
b1525:
  %gcvolta1525 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1525, ptr %ctxtopo, align 8
  ret i64 0
b1526:
  br label %b1523
b1527:
  %gcvolta1527 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1527, ptr %ctxtopo, align 8
  ret i64 0
b1528:
  br label %b1523
b1529:
  %gcvolta1529 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1529, ptr %ctxtopo, align 8
  ret i64 0
b1530:
  store i64 0, ptr %gcs1
  %v2334 = call i1 @df.unbox_bool(i64 %v2331)
  %v2335 = load i8, ptr %ctx, align 8
  %c542 = zext i8 %v2335 to i64
  %v2336 = icmp ne i64 %c542, 0
  br i1 %v2336, label %b1533, label %b1532
b1531:
  %gcvolta1531 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1531, ptr %ctxtopo, align 8
  ret i64 0
b1532:
  br label %b1523
b1533:
  %gcvolta1533 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1533, ptr %ctxtopo, align 8
  ret i64 0
b1534:
  %v2339 = add i64 0, 0
  br label %b1536
b1535:
  %v2340 = add i64 0, 1
  br label %b1536
b1536:
  %v2341 = phi i64 [ %v2339, %b1534 ], [ %v2340, %b1535 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2342 = call i64 @df.box_int(i64 %v2341)
  %xcp2343 = load i8, ptr %ctx, align 8
  %xcn2343 = icmp ne i8 %xcp2343, 0
  %xce2343 = call i1 @llvm.expect.i1(i1 %xcn2343, i1 false)
  br i1 %xce2343, label %xc2343.limpar, label %xc2343.fim
xc2343.limpar:
  call void @dartforge_exception_clear()
  br label %xc2343.fim
xc2343.fim:
  %gcvolta1536 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1536, ptr %ctxtopo, align 8
  ret i64 %v2342
b1538:
  %v2348 = ashr i64 %v0, 1
  %v2349 = icmp sle i64 %v2348, %v2345
  br label %b1540
b1539:
  %v2350 = call i64 @df.box_int(i64 %v2345)
  store i64 %v2350, ptr %gcs1
  %sa2351_0 = getelementptr [2 x i64], ptr %sargs2351, i64 0, i64 0
  store i64 %v2350, ptr %sa2351_0
  %sat2351 = getelementptr [2 x i64], ptr %sargs2351, i64 0, i64 1
  store i64 0, ptr %sat2351
  %sic2351 = getelementptr i64, ptr %area, i64 260
  %sf2351 = call ptr @df.selector(ptr %sic2351, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2351 = call i64 %sf2351(i64 %v0, ptr %sargs2351, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2351, ptr %gcs1
  %v2352 = load i8, ptr %ctx, align 8
  %c543 = zext i8 %v2352 to i64
  %v2353 = icmp ne i64 %c543, 0
  br i1 %v2353, label %b1542, label %b1541
b1540:
  %v2357 = phi i1 [ %v2349, %b1538 ], [ %v2354, %b1543 ]
  br i1 %v2357, label %b1545, label %b1546
b1541:
  %v2354 = call i1 @df.unbox_bool(i64 %v2351)
  %v2355 = load i8, ptr %ctx, align 8
  %c544 = zext i8 %v2355 to i64
  %v2356 = icmp ne i64 %c544, 0
  br i1 %v2356, label %b1544, label %b1543
b1542:
  %gcvolta1542 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1542, ptr %ctxtopo, align 8
  ret i64 0
b1543:
  br label %b1540
b1544:
  %gcvolta1544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1544, ptr %ctxtopo, align 8
  ret i64 0
b1545:
  %v2358 = add i64 0, 0
  br label %b1547
b1546:
  store i64 0, ptr %gcs1
  %v2360 = icmp eq i64 %v0, 0
  %v2361 = call i64 @df.class(i64 %v0)
  %v2362 = call i8 @df.subclass(i64 %v2361, i64 674)
  %c545 = zext i8 %v2362 to i64
  %v2363 = icmp ne i64 %c545, 0
  %v2364 = xor i1 %v2360, true
  %c546 = zext i1 %v2363 to i64
  %c547 = zext i1 %v2364 to i64
  %v2365 = and i64 %c546, %c547
  %v2366 = icmp ne i64 %v2365, 0
  br i1 %v2366, label %b1549, label %b1548
b1547:
  %v2380 = phi i64 [ %v2358, %b1545 ], [ %v2379, %b1555 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2381 = call i64 @df.box_int(i64 %v2380)
  %xcp2382 = load i8, ptr %ctx, align 8
  %xcn2382 = icmp ne i8 %xcp2382, 0
  %xce2382 = call i1 @llvm.expect.i1(i1 %xcn2382, i1 false)
  br i1 %xce2382, label %xc2382.limpar, label %xc2382.fim
xc2382.limpar:
  call void @dartforge_exception_clear()
  br label %xc2382.fim
xc2382.fim:
  %gcvolta1547 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1547, ptr %ctxtopo, align 8
  ret i64 %v2381
b1548:
  %ga2367 = getelementptr i64, ptr %area, i64 7
  %v2367 = load i8, ptr %ga2367
  %c548 = zext i8 %v2367 to i64
  %v2368 = icmp ne i64 %c548, 0
  br i1 %v2368, label %b1550, label %b1551
b1549:
  %v2375 = call i64 @df.unbox_int(i64 %v0)
  %v2376 = load i8, ptr %ctx, align 8
  %c549 = zext i8 %v2376 to i64
  %v2377 = icmp ne i64 %c549, 0
  br i1 %v2377, label %b1556, label %b1555
b1550:
  %ga2369 = getelementptr i64, ptr %area, i64 6
  %v2369 = load i64, ptr %ga2369
  br label %b1552
b1551:
  %v2370 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1552
b1552:
  %v2371 = phi i64 [ %v2369, %b1550 ], [ %v2370, %b1551 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2371, i64 0, i64 0)
  %v2373 = load i8, ptr %ctx, align 8
  %c550 = zext i8 %v2373 to i64
  %v2374 = icmp ne i64 %c550, 0
  br i1 %v2374, label %b1554, label %b1553
b1553:
  br label %b1549
b1554:
  %gcvolta1554 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1554, ptr %ctxtopo, align 8
  ret i64 0
b1555:
  %v2378 = add i64 0, 29
  %v2379 = ashr i64 %v2375, %v2378
  br label %b1547
b1556:
  %gcvolta1556 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1556, ptr %ctxtopo, align 8
  ret i64 0
b1558:
  %v2387 = ashr i64 %v0, 1
  %v2388 = icmp sle i64 %v2387, %v2384
  br label %b1560
b1559:
  %v2389 = call i64 @df.box_int(i64 %v2384)
  store i64 %v2389, ptr %gcs1
  %sa2390_0 = getelementptr [2 x i64], ptr %sargs2390, i64 0, i64 0
  store i64 %v2389, ptr %sa2390_0
  %sat2390 = getelementptr [2 x i64], ptr %sargs2390, i64 0, i64 1
  store i64 0, ptr %sat2390
  %sic2390 = getelementptr i64, ptr %area, i64 262
  %sf2390 = call ptr @df.selector(ptr %sic2390, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2390 = call i64 %sf2390(i64 %v0, ptr %sargs2390, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2390, ptr %gcs1
  %v2391 = load i8, ptr %ctx, align 8
  %c551 = zext i8 %v2391 to i64
  %v2392 = icmp ne i64 %c551, 0
  br i1 %v2392, label %b1562, label %b1561
b1560:
  %v2396 = phi i1 [ %v2388, %b1558 ], [ %v2393, %b1563 ]
  br i1 %v2396, label %b1565, label %b1566
b1561:
  %v2393 = call i1 @df.unbox_bool(i64 %v2390)
  %v2394 = load i8, ptr %ctx, align 8
  %c552 = zext i8 %v2394 to i64
  %v2395 = icmp ne i64 %c552, 0
  br i1 %v2395, label %b1564, label %b1563
b1562:
  %gcvolta1562 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1562, ptr %ctxtopo, align 8
  ret i64 0
b1563:
  br label %b1560
b1564:
  %gcvolta1564 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1564, ptr %ctxtopo, align 8
  ret i64 0
b1565:
  %v2397 = add i64 0, 0
  br label %b1567
b1566:
  store i64 0, ptr %gcs1
  %v2399 = icmp eq i64 %v0, 0
  %v2400 = call i64 @df.class(i64 %v0)
  %v2401 = call i8 @df.subclass(i64 %v2400, i64 674)
  %c553 = zext i8 %v2401 to i64
  %v2402 = icmp ne i64 %c553, 0
  %v2403 = xor i1 %v2399, true
  %c554 = zext i1 %v2402 to i64
  %c555 = zext i1 %v2403 to i64
  %v2404 = and i64 %c554, %c555
  %v2405 = icmp ne i64 %v2404, 0
  br i1 %v2405, label %b1569, label %b1568
b1567:
  %v2419 = phi i64 [ %v2397, %b1565 ], [ %v2418, %b1575 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2420 = call i64 @df.box_int(i64 %v2419)
  %xcp2421 = load i8, ptr %ctx, align 8
  %xcn2421 = icmp ne i8 %xcp2421, 0
  %xce2421 = call i1 @llvm.expect.i1(i1 %xcn2421, i1 false)
  br i1 %xce2421, label %xc2421.limpar, label %xc2421.fim
xc2421.limpar:
  call void @dartforge_exception_clear()
  br label %xc2421.fim
xc2421.fim:
  %gcvolta1567 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1567, ptr %ctxtopo, align 8
  ret i64 %v2420
b1568:
  %ga2406 = getelementptr i64, ptr %area, i64 7
  %v2406 = load i8, ptr %ga2406
  %c556 = zext i8 %v2406 to i64
  %v2407 = icmp ne i64 %c556, 0
  br i1 %v2407, label %b1570, label %b1571
b1569:
  %v2414 = call i64 @df.unbox_int(i64 %v0)
  %v2415 = load i8, ptr %ctx, align 8
  %c557 = zext i8 %v2415 to i64
  %v2416 = icmp ne i64 %c557, 0
  br i1 %v2416, label %b1576, label %b1575
b1570:
  %ga2408 = getelementptr i64, ptr %area, i64 6
  %v2408 = load i64, ptr %ga2408
  br label %b1572
b1571:
  %v2409 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1572
b1572:
  %v2410 = phi i64 [ %v2408, %b1570 ], [ %v2409, %b1571 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2410, i64 0, i64 0)
  %v2412 = load i8, ptr %ctx, align 8
  %c558 = zext i8 %v2412 to i64
  %v2413 = icmp ne i64 %c558, 0
  br i1 %v2413, label %b1574, label %b1573
b1573:
  br label %b1569
b1574:
  %gcvolta1574 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1574, ptr %ctxtopo, align 8
  ret i64 0
b1575:
  %v2417 = add i64 0, 27
  %v2418 = ashr i64 %v2414, %v2417
  br label %b1567
b1576:
  %gcvolta1576 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1576, ptr %ctxtopo, align 8
  ret i64 0
b1578:
  %v2426 = ashr i64 %v0, 1
  %v2427 = icmp sle i64 %v2426, %v2423
  br label %b1580
b1579:
  %v2428 = call i64 @df.box_int(i64 %v2423)
  store i64 %v2428, ptr %gcs1
  %sa2429_0 = getelementptr [2 x i64], ptr %sargs2429, i64 0, i64 0
  store i64 %v2428, ptr %sa2429_0
  %sat2429 = getelementptr [2 x i64], ptr %sargs2429, i64 0, i64 1
  store i64 0, ptr %sat2429
  %sic2429 = getelementptr i64, ptr %area, i64 264
  %sf2429 = call ptr @df.selector(ptr %sic2429, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2429 = call i64 %sf2429(i64 %v0, ptr %sargs2429, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2429, ptr %gcs1
  %v2430 = load i8, ptr %ctx, align 8
  %c559 = zext i8 %v2430 to i64
  %v2431 = icmp ne i64 %c559, 0
  br i1 %v2431, label %b1582, label %b1581
b1580:
  %v2435 = phi i1 [ %v2427, %b1578 ], [ %v2432, %b1583 ]
  br i1 %v2435, label %b1585, label %b1586
b1581:
  %v2432 = call i1 @df.unbox_bool(i64 %v2429)
  %v2433 = load i8, ptr %ctx, align 8
  %c560 = zext i8 %v2433 to i64
  %v2434 = icmp ne i64 %c560, 0
  br i1 %v2434, label %b1584, label %b1583
b1582:
  %gcvolta1582 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1582, ptr %ctxtopo, align 8
  ret i64 0
b1583:
  br label %b1580
b1584:
  %gcvolta1584 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1584, ptr %ctxtopo, align 8
  ret i64 0
b1585:
  %v2436 = add i64 0, 0
  br label %b1587
b1586:
  store i64 0, ptr %gcs1
  %v2438 = icmp eq i64 %v0, 0
  %v2439 = call i64 @df.class(i64 %v0)
  %v2440 = call i8 @df.subclass(i64 %v2439, i64 674)
  %c561 = zext i8 %v2440 to i64
  %v2441 = icmp ne i64 %c561, 0
  %v2442 = xor i1 %v2438, true
  %c562 = zext i1 %v2441 to i64
  %c563 = zext i1 %v2442 to i64
  %v2443 = and i64 %c562, %c563
  %v2444 = icmp ne i64 %v2443, 0
  br i1 %v2444, label %b1589, label %b1588
b1587:
  %v2458 = phi i64 [ %v2436, %b1585 ], [ %v2457, %b1595 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2459 = call i64 @df.box_int(i64 %v2458)
  %xcp2460 = load i8, ptr %ctx, align 8
  %xcn2460 = icmp ne i8 %xcp2460, 0
  %xce2460 = call i1 @llvm.expect.i1(i1 %xcn2460, i1 false)
  br i1 %xce2460, label %xc2460.limpar, label %xc2460.fim
xc2460.limpar:
  call void @dartforge_exception_clear()
  br label %xc2460.fim
xc2460.fim:
  %gcvolta1587 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1587, ptr %ctxtopo, align 8
  ret i64 %v2459
b1588:
  %ga2445 = getelementptr i64, ptr %area, i64 7
  %v2445 = load i8, ptr %ga2445
  %c564 = zext i8 %v2445 to i64
  %v2446 = icmp ne i64 %c564, 0
  br i1 %v2446, label %b1590, label %b1591
b1589:
  %v2453 = call i64 @df.unbox_int(i64 %v0)
  %v2454 = load i8, ptr %ctx, align 8
  %c565 = zext i8 %v2454 to i64
  %v2455 = icmp ne i64 %c565, 0
  br i1 %v2455, label %b1596, label %b1595
b1590:
  %ga2447 = getelementptr i64, ptr %area, i64 6
  %v2447 = load i64, ptr %ga2447
  br label %b1592
b1591:
  %v2448 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1592
b1592:
  %v2449 = phi i64 [ %v2447, %b1590 ], [ %v2448, %b1591 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2449, i64 0, i64 0)
  %v2451 = load i8, ptr %ctx, align 8
  %c566 = zext i8 %v2451 to i64
  %v2452 = icmp ne i64 %c566, 0
  br i1 %v2452, label %b1594, label %b1593
b1593:
  br label %b1589
b1594:
  %gcvolta1594 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1594, ptr %ctxtopo, align 8
  ret i64 0
b1595:
  %v2456 = add i64 0, 23
  %v2457 = ashr i64 %v2453, %v2456
  br label %b1587
b1596:
  %gcvolta1596 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1596, ptr %ctxtopo, align 8
  ret i64 0
b1598:
  %v2465 = ashr i64 %v0, 1
  %v2466 = icmp sle i64 %v2465, %v2462
  br label %b1600
b1599:
  %v2467 = call i64 @df.box_int(i64 %v2462)
  store i64 %v2467, ptr %gcs1
  %sa2468_0 = getelementptr [2 x i64], ptr %sargs2468, i64 0, i64 0
  store i64 %v2467, ptr %sa2468_0
  %sat2468 = getelementptr [2 x i64], ptr %sargs2468, i64 0, i64 1
  store i64 0, ptr %sat2468
  %sic2468 = getelementptr i64, ptr %area, i64 266
  %sf2468 = call ptr @df.selector(ptr %sic2468, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2468 = call i64 %sf2468(i64 %v0, ptr %sargs2468, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2468, ptr %gcs1
  %v2469 = load i8, ptr %ctx, align 8
  %c567 = zext i8 %v2469 to i64
  %v2470 = icmp ne i64 %c567, 0
  br i1 %v2470, label %b1602, label %b1601
b1600:
  %v2474 = phi i1 [ %v2466, %b1598 ], [ %v2471, %b1603 ]
  br i1 %v2474, label %b1605, label %b1606
b1601:
  %v2471 = call i1 @df.unbox_bool(i64 %v2468)
  %v2472 = load i8, ptr %ctx, align 8
  %c568 = zext i8 %v2472 to i64
  %v2473 = icmp ne i64 %c568, 0
  br i1 %v2473, label %b1604, label %b1603
b1602:
  %gcvolta1602 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1602, ptr %ctxtopo, align 8
  ret i64 0
b1603:
  br label %b1600
b1604:
  %gcvolta1604 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1604, ptr %ctxtopo, align 8
  ret i64 0
b1605:
  %v2475 = add i64 0, 0
  br label %b1607
b1606:
  store i64 0, ptr %gcs1
  %v2477 = icmp eq i64 %v0, 0
  %v2478 = call i64 @df.class(i64 %v0)
  %v2479 = call i8 @df.subclass(i64 %v2478, i64 674)
  %c569 = zext i8 %v2479 to i64
  %v2480 = icmp ne i64 %c569, 0
  %v2481 = xor i1 %v2477, true
  %c570 = zext i1 %v2480 to i64
  %c571 = zext i1 %v2481 to i64
  %v2482 = and i64 %c570, %c571
  %v2483 = icmp ne i64 %v2482, 0
  br i1 %v2483, label %b1609, label %b1608
b1607:
  %v2497 = phi i64 [ %v2475, %b1605 ], [ %v2496, %b1615 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2498 = call i64 @df.box_int(i64 %v2497)
  %xcp2499 = load i8, ptr %ctx, align 8
  %xcn2499 = icmp ne i8 %xcp2499, 0
  %xce2499 = call i1 @llvm.expect.i1(i1 %xcn2499, i1 false)
  br i1 %xce2499, label %xc2499.limpar, label %xc2499.fim
xc2499.limpar:
  call void @dartforge_exception_clear()
  br label %xc2499.fim
xc2499.fim:
  %gcvolta1607 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1607, ptr %ctxtopo, align 8
  ret i64 %v2498
b1608:
  %ga2484 = getelementptr i64, ptr %area, i64 7
  %v2484 = load i8, ptr %ga2484
  %c572 = zext i8 %v2484 to i64
  %v2485 = icmp ne i64 %c572, 0
  br i1 %v2485, label %b1610, label %b1611
b1609:
  %v2492 = call i64 @df.unbox_int(i64 %v0)
  %v2493 = load i8, ptr %ctx, align 8
  %c573 = zext i8 %v2493 to i64
  %v2494 = icmp ne i64 %c573, 0
  br i1 %v2494, label %b1616, label %b1615
b1610:
  %ga2486 = getelementptr i64, ptr %area, i64 6
  %v2486 = load i64, ptr %ga2486
  br label %b1612
b1611:
  %v2487 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1612
b1612:
  %v2488 = phi i64 [ %v2486, %b1610 ], [ %v2487, %b1611 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2488, i64 0, i64 0)
  %v2490 = load i8, ptr %ctx, align 8
  %c574 = zext i8 %v2490 to i64
  %v2491 = icmp ne i64 %c574, 0
  br i1 %v2491, label %b1614, label %b1613
b1613:
  br label %b1609
b1614:
  %gcvolta1614 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1614, ptr %ctxtopo, align 8
  ret i64 0
b1615:
  %v2495 = add i64 0, 16
  %v2496 = ashr i64 %v2492, %v2495
  br label %b1607
b1616:
  %gcvolta1616 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1616, ptr %ctxtopo, align 8
  ret i64 0
b1618:
  %v2504 = ashr i64 %v0, 1
  %v2505 = icmp sle i64 %v2504, %v2501
  br label %b1620
b1619:
  %v2506 = call i64 @df.box_int(i64 %v2501)
  store i64 %v2506, ptr %gcs1
  %sa2507_0 = getelementptr [2 x i64], ptr %sargs2507, i64 0, i64 0
  store i64 %v2506, ptr %sa2507_0
  %sat2507 = getelementptr [2 x i64], ptr %sargs2507, i64 0, i64 1
  store i64 0, ptr %sat2507
  %sic2507 = getelementptr i64, ptr %area, i64 268
  %sf2507 = call ptr @df.selector(ptr %sic2507, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2507 = call i64 %sf2507(i64 %v0, ptr %sargs2507, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2507, ptr %gcs1
  %v2508 = load i8, ptr %ctx, align 8
  %c575 = zext i8 %v2508 to i64
  %v2509 = icmp ne i64 %c575, 0
  br i1 %v2509, label %b1622, label %b1621
b1620:
  %v2513 = phi i1 [ %v2505, %b1618 ], [ %v2510, %b1623 ]
  br i1 %v2513, label %b1625, label %b1626
b1621:
  %v2510 = call i1 @df.unbox_bool(i64 %v2507)
  %v2511 = load i8, ptr %ctx, align 8
  %c576 = zext i8 %v2511 to i64
  %v2512 = icmp ne i64 %c576, 0
  br i1 %v2512, label %b1624, label %b1623
b1622:
  %gcvolta1622 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1622, ptr %ctxtopo, align 8
  ret i64 0
b1623:
  br label %b1620
b1624:
  %gcvolta1624 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1624, ptr %ctxtopo, align 8
  ret i64 0
b1625:
  %v2514 = add i64 0, 0
  br label %b1627
b1626:
  store i64 0, ptr %gcs1
  %v2516 = icmp eq i64 %v0, 0
  %v2517 = call i64 @df.class(i64 %v0)
  %v2518 = call i8 @df.subclass(i64 %v2517, i64 674)
  %c577 = zext i8 %v2518 to i64
  %v2519 = icmp ne i64 %c577, 0
  %v2520 = xor i1 %v2516, true
  %c578 = zext i1 %v2519 to i64
  %c579 = zext i1 %v2520 to i64
  %v2521 = and i64 %c578, %c579
  %v2522 = icmp ne i64 %v2521, 0
  br i1 %v2522, label %b1629, label %b1628
b1627:
  %v2536 = phi i64 [ %v2514, %b1625 ], [ %v2535, %b1635 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2537 = call i64 @df.box_int(i64 %v2536)
  %xcp2538 = load i8, ptr %ctx, align 8
  %xcn2538 = icmp ne i8 %xcp2538, 0
  %xce2538 = call i1 @llvm.expect.i1(i1 %xcn2538, i1 false)
  br i1 %xce2538, label %xc2538.limpar, label %xc2538.fim
xc2538.limpar:
  call void @dartforge_exception_clear()
  br label %xc2538.fim
xc2538.fim:
  %gcvolta1627 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1627, ptr %ctxtopo, align 8
  ret i64 %v2537
b1628:
  %ga2523 = getelementptr i64, ptr %area, i64 7
  %v2523 = load i8, ptr %ga2523
  %c580 = zext i8 %v2523 to i64
  %v2524 = icmp ne i64 %c580, 0
  br i1 %v2524, label %b1630, label %b1631
b1629:
  %v2531 = call i64 @df.unbox_int(i64 %v0)
  %v2532 = load i8, ptr %ctx, align 8
  %c581 = zext i8 %v2532 to i64
  %v2533 = icmp ne i64 %c581, 0
  br i1 %v2533, label %b1636, label %b1635
b1630:
  %ga2525 = getelementptr i64, ptr %area, i64 6
  %v2525 = load i64, ptr %ga2525
  br label %b1632
b1631:
  %v2526 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1632
b1632:
  %v2527 = phi i64 [ %v2525, %b1630 ], [ %v2526, %b1631 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2527, i64 0, i64 0)
  %v2529 = load i8, ptr %ctx, align 8
  %c582 = zext i8 %v2529 to i64
  %v2530 = icmp ne i64 %c582, 0
  br i1 %v2530, label %b1634, label %b1633
b1633:
  br label %b1629
b1634:
  %gcvolta1634 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1634, ptr %ctxtopo, align 8
  ret i64 0
b1635:
  %v2534 = add i64 0, 1
  %v2535 = shl i64 %v2531, %v2534
  br label %b1627
b1636:
  %gcvolta1636 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1636, ptr %ctxtopo, align 8
  ret i64 0
b1638:
  %ga2547 = getelementptr i64, ptr %area, i64 7
  %v2547 = load i8, ptr %ga2547
  %c583 = zext i8 %v2547 to i64
  %v2548 = icmp ne i64 %c583, 0
  br i1 %v2548, label %b1640, label %b1641
b1639:
  %v2555 = call i64 @df.unbox_int(i64 %v0)
  %v2556 = load i8, ptr %ctx, align 8
  %c584 = zext i8 %v2556 to i64
  %v2557 = icmp ne i64 %c584, 0
  br i1 %v2557, label %b1646, label %b1645
b1640:
  %ga2549 = getelementptr i64, ptr %area, i64 6
  %v2549 = load i64, ptr %ga2549
  br label %b1642
b1641:
  %v2550 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1642
b1642:
  %v2551 = phi i64 [ %v2549, %b1640 ], [ %v2550, %b1641 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2551, i64 0, i64 0)
  %v2553 = load i8, ptr %ctx, align 8
  %c585 = zext i8 %v2553 to i64
  %v2554 = icmp ne i64 %c585, 0
  br i1 %v2554, label %b1644, label %b1643
b1643:
  br label %b1639
b1644:
  %gcvolta1644 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1644, ptr %ctxtopo, align 8
  ret i64 0
b1645:
  %v2558 = add i64 0, 24
  %v2559 = ashr i64 %v2555, %v2558
  store i64 0, ptr %gcs0
  %v2560 = call i64 @df.box_int(i64 %v2559)
  %xcp2561 = load i8, ptr %ctx, align 8
  %xcn2561 = icmp ne i8 %xcp2561, 0
  %xce2561 = call i1 @llvm.expect.i1(i1 %xcn2561, i1 false)
  br i1 %xce2561, label %xc2561.limpar, label %xc2561.fim
xc2561.limpar:
  call void @dartforge_exception_clear()
  br label %xc2561.fim
xc2561.fim:
  %gcvolta1645 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1645, ptr %ctxtopo, align 8
  ret i64 %v2560
b1646:
  %gcvolta1646 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1646, ptr %ctxtopo, align 8
  ret i64 0
b1648:
  %ga2570 = getelementptr i64, ptr %area, i64 7
  %v2570 = load i8, ptr %ga2570
  %c586 = zext i8 %v2570 to i64
  %v2571 = icmp ne i64 %c586, 0
  br i1 %v2571, label %b1650, label %b1651
b1649:
  %v2578 = call i64 @df.unbox_int(i64 %v0)
  %v2579 = load i8, ptr %ctx, align 8
  %c587 = zext i8 %v2579 to i64
  %v2580 = icmp ne i64 %c587, 0
  br i1 %v2580, label %b1656, label %b1655
b1650:
  %ga2572 = getelementptr i64, ptr %area, i64 6
  %v2572 = load i64, ptr %ga2572
  br label %b1652
b1651:
  %v2573 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1652
b1652:
  %v2574 = phi i64 [ %v2572, %b1650 ], [ %v2573, %b1651 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2574, i64 0, i64 0)
  %v2576 = load i8, ptr %ctx, align 8
  %c588 = zext i8 %v2576 to i64
  %v2577 = icmp ne i64 %c588, 0
  br i1 %v2577, label %b1654, label %b1653
b1653:
  br label %b1649
b1654:
  %gcvolta1654 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1654, ptr %ctxtopo, align 8
  ret i64 0
b1655:
  %v2581 = add i64 0, 16
  %v2582 = ashr i64 %v2578, %v2581
  store i64 0, ptr %gcs0
  %v2583 = call i64 @df.box_int(i64 %v2582)
  %xcp2584 = load i8, ptr %ctx, align 8
  %xcn2584 = icmp ne i8 %xcp2584, 0
  %xce2584 = call i1 @llvm.expect.i1(i1 %xcn2584, i1 false)
  br i1 %xce2584, label %xc2584.limpar, label %xc2584.fim
xc2584.limpar:
  call void @dartforge_exception_clear()
  br label %xc2584.fim
xc2584.fim:
  %gcvolta1655 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1655, ptr %ctxtopo, align 8
  ret i64 %v2583
b1656:
  %gcvolta1656 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1656, ptr %ctxtopo, align 8
  ret i64 0
b1659:
  store i64 0, ptr %gcs1
  %v2593 = call double @df.unbox_double(i64 %v2590)
  %v2594 = load i8, ptr %ctx, align 8
  %c589 = zext i8 %v2594 to i64
  %v2595 = icmp ne i64 %c589, 0
  br i1 %v2595, label %b1662, label %b1661
b1660:
  %gcvolta1660 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1660, ptr %ctxtopo, align 8
  ret i64 0
b1661:
  %xcp2597 = load i8, ptr %ctx, align 8
  %xcn2597 = icmp ne i8 %xcp2597, 0
  %xce2597 = call i1 @llvm.expect.i1(i1 %xcn2597, i1 false)
  br i1 %xce2597, label %xc2597.limpar, label %xc2597.fim
xc2597.limpar:
  call void @dartforge_exception_clear()
  br label %xc2597.fim
xc2597.fim:
  %gcvolta1661 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1661, ptr %ctxtopo, align 8
  ret i64 %v2590
b1662:
  %gcvolta1662 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1662, ptr %ctxtopo, align 8
  ret i64 0
b1676:
  %xcp3444 = load i8, ptr %ctx, align 8
  %xcn3444 = icmp ne i8 %xcp3444, 0
  %xce3444 = call i1 @llvm.expect.i1(i1 %xcn3444, i1 false)
  br i1 %xce3444, label %xc3444.limpar, label %xc3444.fim
xc3444.limpar:
  call void @dartforge_exception_clear()
  br label %xc3444.fim
xc3444.fim:
  %gcvolta1676 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1676, ptr %ctxtopo, align 8
  ret i64 %v0
b1677:
  store i64 0, ptr %gcs1
  %v2603 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v2603, ptr %gcs1
  %v2604 = load i8, ptr %ctx, align 8
  %c590 = zext i8 %v2604 to i64
  %v2605 = icmp ne i64 %c590, 0
  br i1 %v2605, label %b1683, label %b1682
b1678:
  %v2602 = icmp eq i64 %v2599, %v6
  br i1 %v2602, label %b1680, label %b1677
b1679:
  %gcvolta1679 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1679, ptr %ctxtopo, align 8
  ret i64 0
b1680:
  %v2648 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2649 = call i64 @df.box_int(i64 %v2648)
  store i64 %v2649, ptr %gcs1
  %v2650 = icmp eq i64 %v0, 0
  %v2651 = icmp eq i64 %v2649, 0
  %c591 = zext i1 %v2650 to i64
  %c592 = zext i1 %v2651 to i64
  %v2652 = or i64 %c591, %c592
  %v2653 = icmp ne i64 %v2652, 0
  br i1 %v2653, label %b1727, label %b1728
b1681:
  store i64 0, ptr %gcs1
  %v2607 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v2607, ptr %gcs1
  %v2608 = load i8, ptr %ctx, align 8
  %c593 = zext i8 %v2608 to i64
  %v2609 = icmp ne i64 %c593, 0
  br i1 %v2609, label %b1687, label %b1686
b1682:
  %v2606 = icmp eq i64 %v2603, %v6
  br i1 %v2606, label %b1684, label %b1681
b1683:
  %gcvolta1683 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1683, ptr %ctxtopo, align 8
  ret i64 0
b1684:
  %v2680 = add i64 0, 0
  %v2681 = add i64 0, 1
  %v2682 = call i64 @df.class(i64 %v0)
  switch i64 %v2682, label %b1751 [ i64 4, label %b1749 i64 2, label %b1750 i64 3, label %b1750 ]
b1685:
  store i64 0, ptr %gcs1
  %v2611 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v2611, ptr %gcs1
  %v2612 = load i8, ptr %ctx, align 8
  %c594 = zext i8 %v2612 to i64
  %v2613 = icmp ne i64 %c594, 0
  br i1 %v2613, label %b1691, label %b1690
b1686:
  %v2610 = icmp eq i64 %v2607, %v6
  br i1 %v2610, label %b1688, label %b1685
b1687:
  %gcvolta1687 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1687, ptr %ctxtopo, align 8
  ret i64 0
b1688:
  %v2750 = add i64 0, 0
  %v2751 = add i64 0, 1
  %v2752 = call i64 @df.class(i64 %v0)
  switch i64 %v2752, label %b1791 [ i64 4, label %b1789 i64 2, label %b1790 i64 3, label %b1790 ]
b1689:
  store i64 0, ptr %gcs1
  %v2615 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v2615, ptr %gcs1
  %v2616 = load i8, ptr %ctx, align 8
  %c595 = zext i8 %v2616 to i64
  %v2617 = icmp ne i64 %c595, 0
  br i1 %v2617, label %b1695, label %b1694
b1690:
  %v2614 = icmp eq i64 %v2611, %v6
  br i1 %v2614, label %b1692, label %b1689
b1691:
  %gcvolta1691 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1691, ptr %ctxtopo, align 8
  ret i64 0
b1692:
  %v2820 = add i64 0, 0
  %v2821 = add i64 0, 1
  %v2822 = call i64 @df.class(i64 %v0)
  switch i64 %v2822, label %b1831 [ i64 4, label %b1829 i64 2, label %b1830 i64 3, label %b1830 ]
b1693:
  store i64 0, ptr %gcs1
  %v2619 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v2619, ptr %gcs1
  %v2620 = load i8, ptr %ctx, align 8
  %c596 = zext i8 %v2620 to i64
  %v2621 = icmp ne i64 %c596, 0
  br i1 %v2621, label %b1699, label %b1698
b1694:
  %v2618 = icmp eq i64 %v2615, %v6
  br i1 %v2618, label %b1696, label %b1693
b1695:
  %gcvolta1695 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1695, ptr %ctxtopo, align 8
  ret i64 0
b1696:
  %v2890 = add i64 0, 0
  %v2891 = add i64 0, 1
  %v2892 = call i64 @df.class(i64 %v0)
  switch i64 %v2892, label %b1871 [ i64 4, label %b1869 i64 2, label %b1870 i64 3, label %b1870 ]
b1697:
  store i64 0, ptr %gcs1
  %v2623 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v2623, ptr %gcs1
  %v2624 = load i8, ptr %ctx, align 8
  %c597 = zext i8 %v2624 to i64
  %v2625 = icmp ne i64 %c597, 0
  br i1 %v2625, label %b1703, label %b1702
b1698:
  %v2622 = icmp eq i64 %v2619, %v6
  br i1 %v2622, label %b1700, label %b1697
b1699:
  %gcvolta1699 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1699, ptr %ctxtopo, align 8
  ret i64 0
b1700:
  %v2960 = add i64 0, 0
  %v2961 = add i64 0, 1
  %v2962 = call i64 @df.class(i64 %v0)
  switch i64 %v2962, label %b1911 [ i64 4, label %b1909 i64 2, label %b1910 i64 3, label %b1910 ]
b1701:
  store i64 0, ptr %gcs1
  %v2627 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v2627, ptr %gcs1
  %v2628 = load i8, ptr %ctx, align 8
  %c598 = zext i8 %v2628 to i64
  %v2629 = icmp ne i64 %c598, 0
  br i1 %v2629, label %b1707, label %b1706
b1702:
  %v2626 = icmp eq i64 %v2623, %v6
  br i1 %v2626, label %b1704, label %b1701
b1703:
  %gcvolta1703 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1703, ptr %ctxtopo, align 8
  ret i64 0
b1704:
  %v3030 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3031 = and i64 %v0, 1
  %v3032 = icmp ne i64 %v3031, 0
  br i1 %v3032, label %b1949, label %b1950
b1705:
  store i64 0, ptr %gcs1
  %v2631 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v2631, ptr %gcs1
  %v2632 = load i8, ptr %ctx, align 8
  %c599 = zext i8 %v2632 to i64
  %v2633 = icmp ne i64 %c599, 0
  br i1 %v2633, label %b1711, label %b1710
b1706:
  %v2630 = icmp eq i64 %v2627, %v6
  br i1 %v2630, label %b1708, label %b1705
b1707:
  %gcvolta1707 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1707, ptr %ctxtopo, align 8
  ret i64 0
b1708:
  %v3168 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3169 = and i64 %v0, 1
  %v3170 = icmp ne i64 %v3169, 0
  br i1 %v3170, label %b2024, label %b2025
b1709:
  store i64 0, ptr %gcs1
  %v2635 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v2635, ptr %gcs1
  %v2636 = load i8, ptr %ctx, align 8
  %c600 = zext i8 %v2636 to i64
  %v2637 = icmp ne i64 %c600, 0
  br i1 %v2637, label %b1715, label %b1714
b1710:
  %v2634 = icmp eq i64 %v2631, %v6
  br i1 %v2634, label %b1712, label %b1709
b1711:
  %gcvolta1711 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1711, ptr %ctxtopo, align 8
  ret i64 0
b1712:
  %v3306 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3307 = and i64 %v0, 1
  %v3308 = icmp ne i64 %v3307, 0
  br i1 %v3308, label %b2099, label %b2100
b1713:
  store i64 0, ptr %gcs1
  %v2639 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v2639, ptr %gcs1
  %v2640 = load i8, ptr %ctx, align 8
  %c601 = zext i8 %v2640 to i64
  %v2641 = icmp ne i64 %c601, 0
  br i1 %v2641, label %b1720, label %b1719
b1714:
  %v2638 = icmp eq i64 %v2635, %v6
  br i1 %v2638, label %b1716, label %b1713
b1715:
  %gcvolta1715 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1715, ptr %ctxtopo, align 8
  ret i64 0
b1716:
  br label %b1676
b1718:
  store i64 0, ptr %gcs1
  %v2643 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v2643, ptr %gcs1
  %v2644 = load i8, ptr %ctx, align 8
  %c602 = zext i8 %v2644 to i64
  %v2645 = icmp ne i64 %c602, 0
  br i1 %v2645, label %b1725, label %b1724
b1719:
  %v2642 = icmp eq i64 %v2639, %v6
  br i1 %v2642, label %b1721, label %b1718
b1720:
  %gcvolta1720 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1720, ptr %ctxtopo, align 8
  ret i64 0
b1721:
  br label %b1676
b1723:
  br label %b12
b1724:
  %v2646 = icmp eq i64 %v2643, %v6
  br i1 %v2646, label %b1726, label %b1723
b1725:
  %gcvolta1725 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1725, ptr %ctxtopo, align 8
  ret i64 0
b1726:
  br label %b1676
b1727:
  %v2654 = icmp eq i64 %v0, %v2649
  br label %b1729
b1728:
  %v2655 = call i64 @df.class(i64 %v0)
  switch i64 %v2655, label %b1733 [ i64 675, label %b1730 i64 4, label %b1731 i64 2, label %b1732 i64 3, label %b1732 ]
b1729:
  %v2673 = phi i1 [ %v2654, %b1727 ], [ %v2672, %b1734 ]
  br i1 %v2673, label %b1745, label %b1746
b1730:
  %v2656 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v2649)
  %v2657 = load i8, ptr %ctx, align 8
  %c603 = zext i8 %v2657 to i64
  %v2658 = icmp ne i64 %c603, 0
  br i1 %v2658, label %b1736, label %b1735
b1731:
  %v2660 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v2649)
  %v2661 = load i8, ptr %ctx, align 8
  %c604 = zext i8 %v2661 to i64
  %v2662 = icmp ne i64 %c604, 0
  br i1 %v2662, label %b1738, label %b1737
b1732:
  %v2663 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v2649)
  %v2664 = load i8, ptr %ctx, align 8
  %c605 = zext i8 %v2664 to i64
  %v2665 = icmp ne i64 %c605, 0
  br i1 %v2665, label %b1740, label %b1739
b1733:
  %sa2666_0 = getelementptr [2 x i64], ptr %sargs2666, i64 0, i64 0
  store i64 %v2649, ptr %sa2666_0
  %sat2666 = getelementptr [2 x i64], ptr %sargs2666, i64 0, i64 1
  store i64 0, ptr %sat2666
  %sic2666 = getelementptr i64, ptr %area, i64 270
  %sf2666 = call ptr @df.selector(ptr %sic2666, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v2666 = call i64 %sf2666(i64 %v0, ptr %sargs2666, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2666, ptr %gcs0
  %v2667 = load i8, ptr %ctx, align 8
  %c606 = zext i8 %v2667 to i64
  %v2668 = icmp ne i64 %c606, 0
  br i1 %v2668, label %b1742, label %b1741
b1734:
  %v2672 = phi i1 [ %v2659, %b1735 ], [ %v2660, %b1737 ], [ %v2663, %b1739 ], [ %v2669, %b1743 ]
  br label %b1729
b1735:
  %c607 = zext i8 %v2656 to i64
  %v2659 = icmp ne i64 %c607, 0
  br label %b1734
b1736:
  %gcvolta1736 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1736, ptr %ctxtopo, align 8
  ret i64 0
b1737:
  br label %b1734
b1738:
  %gcvolta1738 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1738, ptr %ctxtopo, align 8
  ret i64 0
b1739:
  br label %b1734
b1740:
  %gcvolta1740 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1740, ptr %ctxtopo, align 8
  ret i64 0
b1741:
  store i64 0, ptr %gcs1
  %v2669 = call i1 @df.unbox_bool(i64 %v2666)
  %v2670 = load i8, ptr %ctx, align 8
  %c608 = zext i8 %v2670 to i64
  %v2671 = icmp ne i64 %c608, 0
  br i1 %v2671, label %b1744, label %b1743
b1742:
  %gcvolta1742 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1742, ptr %ctxtopo, align 8
  ret i64 0
b1743:
  br label %b1734
b1744:
  %gcvolta1744 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1744, ptr %ctxtopo, align 8
  ret i64 0
b1745:
  %v2674 = add i64 0, 0
  br label %b1747
b1746:
  %v2675 = add i64 0, 1
  br label %b1747
b1747:
  %v2676 = phi i64 [ %v2674, %b1745 ], [ %v2675, %b1746 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2677 = call i64 @df.box_int(i64 %v2676)
  %xcp2678 = load i8, ptr %ctx, align 8
  %xcn2678 = icmp ne i8 %xcp2678, 0
  %xce2678 = call i1 @llvm.expect.i1(i1 %xcn2678, i1 false)
  br i1 %xce2678, label %xc2678.limpar, label %xc2678.fim
xc2678.limpar:
  call void @dartforge_exception_clear()
  br label %xc2678.fim
xc2678.fim:
  %gcvolta1747 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1747, ptr %ctxtopo, align 8
  ret i64 %v2677
b1749:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2683 = call i64 @df.box_int(i64 %v2680)
  store i64 %v2683, ptr %gcs1
  %v2684 = call i64 @df.box_int(i64 %v2681)
  store i64 %v2684, ptr %gcs2
  %v2685 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2683, i64 %v2684)
  %v2686 = load i8, ptr %ctx, align 8
  %c609 = zext i8 %v2686 to i64
  %v2687 = icmp ne i64 %c609, 0
  br i1 %v2687, label %b1754, label %b1753
b1750:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2688 = call i64 @df.box_int(i64 %v2680)
  store i64 %v2688, ptr %gcs1
  %v2689 = call i64 @df.box_int(i64 %v2681)
  store i64 %v2689, ptr %gcs2
  %v2690 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2688, i64 %v2689)
  %v2691 = load i8, ptr %ctx, align 8
  %c610 = zext i8 %v2691 to i64
  %v2692 = icmp ne i64 %c610, 0
  br i1 %v2692, label %b1756, label %b1755
b1751:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2693 = call i64 @df.box_int(i64 %v2680)
  store i64 %v2693, ptr %gcs1
  %v2694 = call i64 @df.box_int(i64 %v2681)
  store i64 %v2694, ptr %gcs2
  %sa2695_0 = getelementptr [3 x i64], ptr %sargs2695, i64 0, i64 0
  store i64 %v2693, ptr %sa2695_0
  %sa2695_1 = getelementptr [3 x i64], ptr %sargs2695, i64 0, i64 1
  store i64 %v2694, ptr %sa2695_1
  %sat2695 = getelementptr [3 x i64], ptr %sargs2695, i64 0, i64 2
  store i64 0, ptr %sat2695
  %sic2695 = getelementptr i64, ptr %area, i64 272
  %sf2695 = call ptr @df.selector(ptr %sic2695, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2695 = call i64 %sf2695(i64 %v0, ptr %sargs2695, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2696 = load i8, ptr %ctx, align 8
  %c611 = zext i8 %v2696 to i64
  %v2697 = icmp ne i64 %c611, 0
  br i1 %v2697, label %b1758, label %b1757
b1752:
  %v2698 = phi i64 [ %v2685, %b1753 ], [ %v2690, %b1755 ], [ %v2695, %b1757 ]
  store i64 %v2698, ptr %gcs0
  %v2699 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2700 = and i64 %v2698, 1
  %v2701 = icmp ne i64 %v2700, 0
  br i1 %v2701, label %b1759, label %b1760
b1753:
  br label %b1752
b1754:
  %gcvolta1754 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1754, ptr %ctxtopo, align 8
  ret i64 0
b1755:
  br label %b1752
b1756:
  %gcvolta1756 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1756, ptr %ctxtopo, align 8
  ret i64 0
b1757:
  br label %b1752
b1758:
  %gcvolta1758 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1758, ptr %ctxtopo, align 8
  ret i64 0
b1759:
  %v2702 = ashr i64 %v2698, 1
  %v2703 = mul i64 %v2702, %v2699
  store i64 0, ptr %gcs0
  %v2704 = call i64 @df.box_int(i64 %v2703)
  br label %b1761
b1760:
  %v2705 = call i64 @df.class(i64 %v2698)
  %v2706 = icmp eq i64 %v2705, 2
  %v2707 = icmp eq i64 %v2705, 3
  %v2708 = icmp eq i64 %v2705, 4
  %c612 = zext i1 %v2706 to i64
  %c613 = zext i1 %v2707 to i64
  %v2709 = or i64 %c612, %c613
  %c614 = zext i1 %v2708 to i64
  %v2710 = or i64 %v2709, %c614
  %c615 = trunc i64 %v2710 to i1
  br i1 %c615, label %b1762, label %b1763
b1761:
  %v2729 = phi i64 [ %v2704, %b1759 ], [ %v2728, %b1764 ]
  store i64 %v2729, ptr %gcs0
  %v2730 = call i64 @df.class(i64 %v2729)
  switch i64 %v2730, label %b1776 [ i64 4, label %b1774 i64 2, label %b1775 i64 3, label %b1775 ]
b1762:
  br i1 %v2708, label %b1766, label %b1765
b1763:
  %v2724 = call i64 @df.box_int(i64 %v2699)
  store i64 %v2724, ptr %gcs1
  %sa2725_0 = getelementptr [2 x i64], ptr %sargs2725, i64 0, i64 0
  store i64 %v2724, ptr %sa2725_0
  %sat2725 = getelementptr [2 x i64], ptr %sargs2725, i64 0, i64 1
  store i64 0, ptr %sat2725
  %sic2725 = getelementptr i64, ptr %area, i64 274
  %sf2725 = call ptr @df.selector(ptr %sic2725, i64 %v2698, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v2725 = call i64 %sf2725(i64 %v2698, ptr %sargs2725, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2726 = load i8, ptr %ctx, align 8
  %c616 = zext i8 %v2726 to i64
  %v2727 = icmp ne i64 %c616, 0
  br i1 %v2727, label %b1773, label %b1772
b1764:
  %v2728 = phi i64 [ %v2723, %b1767 ], [ %v2725, %b1772 ]
  br label %b1761
b1765:
  %v2712 = call i64 @df.unbox_int(i64 %v2698)
  %v2713 = load i8, ptr %ctx, align 8
  %c617 = zext i8 %v2713 to i64
  %v2714 = icmp ne i64 %c617, 0
  br i1 %v2714, label %b1769, label %b1768
b1766:
  %v2717 = call double @df.num_to_double(i64 %v2698)
  %v2718 = load i8, ptr %ctx, align 8
  %c618 = zext i8 %v2718 to i64
  %v2719 = icmp ne i64 %c618, 0
  br i1 %v2719, label %b1771, label %b1770
b1767:
  %v2723 = phi i64 [ %v2716, %b1768 ], [ %v2722, %b1770 ]
  br label %b1764
b1768:
  %v2715 = mul i64 %v2712, %v2699
  store i64 0, ptr %gcs0
  %v2716 = call i64 @df.box_int(i64 %v2715)
  br label %b1767
b1769:
  %gcvolta1769 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1769, ptr %ctxtopo, align 8
  ret i64 0
b1770:
  %v2720 = sitofp i64 %v2699 to double
  %v2721 = fmul double %v2717, %v2720
  store i64 0, ptr %gcs0
  %v2722 = call i64 @df.box_double(double %v2721)
  br label %b1767
b1771:
  %gcvolta1771 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1771, ptr %ctxtopo, align 8
  ret i64 0
b1772:
  br label %b1764
b1773:
  %gcvolta1773 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1773, ptr %ctxtopo, align 8
  ret i64 0
b1774:
  store i64 0, ptr %gcs1
  %v2731 = call double @df.unbox_double(i64 %v2729)
  %v2732 = load i8, ptr %ctx, align 8
  %c619 = zext i8 %v2732 to i64
  %v2733 = icmp ne i64 %c619, 0
  br i1 %v2733, label %b1779, label %b1778
b1775:
  store i64 0, ptr %gcs1
  %v2737 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2729)
  %v2738 = load i8, ptr %ctx, align 8
  %c620 = zext i8 %v2738 to i64
  %v2739 = icmp ne i64 %c620, 0
  br i1 %v2739, label %b1783, label %b1782
b1776:
  store i64 0, ptr %gcs1
  %sat2740 = getelementptr [1 x i64], ptr %sargs2740, i64 0, i64 0
  store i64 0, ptr %sat2740
  %sic2740 = getelementptr i64, ptr %area, i64 276
  %sf2740 = call ptr @df.selector(ptr %sic2740, i64 %v2729, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2740 = call i64 %sf2740(i64 %v2729, ptr %sargs2740, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2740, ptr %gcs0
  %v2741 = load i8, ptr %ctx, align 8
  %c621 = zext i8 %v2741 to i64
  %v2742 = icmp ne i64 %c621, 0
  br i1 %v2742, label %b1785, label %b1784
b1777:
  %v2746 = phi i64 [ %v2734, %b1780 ], [ %v2737, %b1782 ], [ %v2743, %b1786 ]
  store i64 0, ptr %gcs0
  %v2747 = call i64 @df.box_int(i64 %v2746)
  %xcp2748 = load i8, ptr %ctx, align 8
  %xcn2748 = icmp ne i8 %xcp2748, 0
  %xce2748 = call i1 @llvm.expect.i1(i1 %xcn2748, i1 false)
  br i1 %xce2748, label %xc2748.limpar, label %xc2748.fim
xc2748.limpar:
  call void @dartforge_exception_clear()
  br label %xc2748.fim
xc2748.fim:
  %gcvolta1777 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1777, ptr %ctxtopo, align 8
  ret i64 %v2747
b1778:
  store i64 0, ptr %gcs0
  %v2734 = call i64 @dartforge_native_DartForge_double_toInt(double %v2731)
  %v2735 = load i8, ptr %ctx, align 8
  %c622 = zext i8 %v2735 to i64
  %v2736 = icmp ne i64 %c622, 0
  br i1 %v2736, label %b1781, label %b1780
b1779:
  %gcvolta1779 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1779, ptr %ctxtopo, align 8
  ret i64 0
b1780:
  br label %b1777
b1781:
  %gcvolta1781 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1781, ptr %ctxtopo, align 8
  ret i64 0
b1782:
  br label %b1777
b1783:
  %gcvolta1783 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1783, ptr %ctxtopo, align 8
  ret i64 0
b1784:
  %v2743 = call i64 @df.unbox_int(i64 %v2740)
  %v2744 = load i8, ptr %ctx, align 8
  %c623 = zext i8 %v2744 to i64
  %v2745 = icmp ne i64 %c623, 0
  br i1 %v2745, label %b1787, label %b1786
b1785:
  %gcvolta1785 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1785, ptr %ctxtopo, align 8
  ret i64 0
b1786:
  br label %b1777
b1787:
  %gcvolta1787 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1787, ptr %ctxtopo, align 8
  ret i64 0
b1789:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2753 = call i64 @df.box_int(i64 %v2750)
  store i64 %v2753, ptr %gcs1
  %v2754 = call i64 @df.box_int(i64 %v2751)
  store i64 %v2754, ptr %gcs2
  %v2755 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2753, i64 %v2754)
  %v2756 = load i8, ptr %ctx, align 8
  %c624 = zext i8 %v2756 to i64
  %v2757 = icmp ne i64 %c624, 0
  br i1 %v2757, label %b1794, label %b1793
b1790:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2758 = call i64 @df.box_int(i64 %v2750)
  store i64 %v2758, ptr %gcs1
  %v2759 = call i64 @df.box_int(i64 %v2751)
  store i64 %v2759, ptr %gcs2
  %v2760 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2758, i64 %v2759)
  %v2761 = load i8, ptr %ctx, align 8
  %c625 = zext i8 %v2761 to i64
  %v2762 = icmp ne i64 %c625, 0
  br i1 %v2762, label %b1796, label %b1795
b1791:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2763 = call i64 @df.box_int(i64 %v2750)
  store i64 %v2763, ptr %gcs1
  %v2764 = call i64 @df.box_int(i64 %v2751)
  store i64 %v2764, ptr %gcs2
  %sa2765_0 = getelementptr [3 x i64], ptr %sargs2765, i64 0, i64 0
  store i64 %v2763, ptr %sa2765_0
  %sa2765_1 = getelementptr [3 x i64], ptr %sargs2765, i64 0, i64 1
  store i64 %v2764, ptr %sa2765_1
  %sat2765 = getelementptr [3 x i64], ptr %sargs2765, i64 0, i64 2
  store i64 0, ptr %sat2765
  %sic2765 = getelementptr i64, ptr %area, i64 278
  %sf2765 = call ptr @df.selector(ptr %sic2765, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2765 = call i64 %sf2765(i64 %v0, ptr %sargs2765, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2766 = load i8, ptr %ctx, align 8
  %c626 = zext i8 %v2766 to i64
  %v2767 = icmp ne i64 %c626, 0
  br i1 %v2767, label %b1798, label %b1797
b1792:
  %v2768 = phi i64 [ %v2755, %b1793 ], [ %v2760, %b1795 ], [ %v2765, %b1797 ]
  store i64 %v2768, ptr %gcs0
  %v2769 = add i64 0, 15
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2770 = and i64 %v2768, 1
  %v2771 = icmp ne i64 %v2770, 0
  br i1 %v2771, label %b1799, label %b1800
b1793:
  br label %b1792
b1794:
  %gcvolta1794 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1794, ptr %ctxtopo, align 8
  ret i64 0
b1795:
  br label %b1792
b1796:
  %gcvolta1796 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1796, ptr %ctxtopo, align 8
  ret i64 0
b1797:
  br label %b1792
b1798:
  %gcvolta1798 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1798, ptr %ctxtopo, align 8
  ret i64 0
b1799:
  %v2772 = ashr i64 %v2768, 1
  %v2773 = mul i64 %v2772, %v2769
  store i64 0, ptr %gcs0
  %v2774 = call i64 @df.box_int(i64 %v2773)
  br label %b1801
b1800:
  %v2775 = call i64 @df.class(i64 %v2768)
  %v2776 = icmp eq i64 %v2775, 2
  %v2777 = icmp eq i64 %v2775, 3
  %v2778 = icmp eq i64 %v2775, 4
  %c627 = zext i1 %v2776 to i64
  %c628 = zext i1 %v2777 to i64
  %v2779 = or i64 %c627, %c628
  %c629 = zext i1 %v2778 to i64
  %v2780 = or i64 %v2779, %c629
  %c630 = trunc i64 %v2780 to i1
  br i1 %c630, label %b1802, label %b1803
b1801:
  %v2799 = phi i64 [ %v2774, %b1799 ], [ %v2798, %b1804 ]
  store i64 %v2799, ptr %gcs0
  %v2800 = call i64 @df.class(i64 %v2799)
  switch i64 %v2800, label %b1816 [ i64 4, label %b1814 i64 2, label %b1815 i64 3, label %b1815 ]
b1802:
  br i1 %v2778, label %b1806, label %b1805
b1803:
  %v2794 = call i64 @df.box_int(i64 %v2769)
  store i64 %v2794, ptr %gcs1
  %sa2795_0 = getelementptr [2 x i64], ptr %sargs2795, i64 0, i64 0
  store i64 %v2794, ptr %sa2795_0
  %sat2795 = getelementptr [2 x i64], ptr %sargs2795, i64 0, i64 1
  store i64 0, ptr %sat2795
  %sic2795 = getelementptr i64, ptr %area, i64 280
  %sf2795 = call ptr @df.selector(ptr %sic2795, i64 %v2768, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v2795 = call i64 %sf2795(i64 %v2768, ptr %sargs2795, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2796 = load i8, ptr %ctx, align 8
  %c631 = zext i8 %v2796 to i64
  %v2797 = icmp ne i64 %c631, 0
  br i1 %v2797, label %b1813, label %b1812
b1804:
  %v2798 = phi i64 [ %v2793, %b1807 ], [ %v2795, %b1812 ]
  br label %b1801
b1805:
  %v2782 = call i64 @df.unbox_int(i64 %v2768)
  %v2783 = load i8, ptr %ctx, align 8
  %c632 = zext i8 %v2783 to i64
  %v2784 = icmp ne i64 %c632, 0
  br i1 %v2784, label %b1809, label %b1808
b1806:
  %v2787 = call double @df.num_to_double(i64 %v2768)
  %v2788 = load i8, ptr %ctx, align 8
  %c633 = zext i8 %v2788 to i64
  %v2789 = icmp ne i64 %c633, 0
  br i1 %v2789, label %b1811, label %b1810
b1807:
  %v2793 = phi i64 [ %v2786, %b1808 ], [ %v2792, %b1810 ]
  br label %b1804
b1808:
  %v2785 = mul i64 %v2782, %v2769
  store i64 0, ptr %gcs0
  %v2786 = call i64 @df.box_int(i64 %v2785)
  br label %b1807
b1809:
  %gcvolta1809 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1809, ptr %ctxtopo, align 8
  ret i64 0
b1810:
  %v2790 = sitofp i64 %v2769 to double
  %v2791 = fmul double %v2787, %v2790
  store i64 0, ptr %gcs0
  %v2792 = call i64 @df.box_double(double %v2791)
  br label %b1807
b1811:
  %gcvolta1811 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1811, ptr %ctxtopo, align 8
  ret i64 0
b1812:
  br label %b1804
b1813:
  %gcvolta1813 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1813, ptr %ctxtopo, align 8
  ret i64 0
b1814:
  store i64 0, ptr %gcs1
  %v2801 = call double @df.unbox_double(i64 %v2799)
  %v2802 = load i8, ptr %ctx, align 8
  %c634 = zext i8 %v2802 to i64
  %v2803 = icmp ne i64 %c634, 0
  br i1 %v2803, label %b1819, label %b1818
b1815:
  store i64 0, ptr %gcs1
  %v2807 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2799)
  %v2808 = load i8, ptr %ctx, align 8
  %c635 = zext i8 %v2808 to i64
  %v2809 = icmp ne i64 %c635, 0
  br i1 %v2809, label %b1823, label %b1822
b1816:
  store i64 0, ptr %gcs1
  %sat2810 = getelementptr [1 x i64], ptr %sargs2810, i64 0, i64 0
  store i64 0, ptr %sat2810
  %sic2810 = getelementptr i64, ptr %area, i64 282
  %sf2810 = call ptr @df.selector(ptr %sic2810, i64 %v2799, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2810 = call i64 %sf2810(i64 %v2799, ptr %sargs2810, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2810, ptr %gcs0
  %v2811 = load i8, ptr %ctx, align 8
  %c636 = zext i8 %v2811 to i64
  %v2812 = icmp ne i64 %c636, 0
  br i1 %v2812, label %b1825, label %b1824
b1817:
  %v2816 = phi i64 [ %v2804, %b1820 ], [ %v2807, %b1822 ], [ %v2813, %b1826 ]
  store i64 0, ptr %gcs0
  %v2817 = call i64 @df.box_int(i64 %v2816)
  %xcp2818 = load i8, ptr %ctx, align 8
  %xcn2818 = icmp ne i8 %xcp2818, 0
  %xce2818 = call i1 @llvm.expect.i1(i1 %xcn2818, i1 false)
  br i1 %xce2818, label %xc2818.limpar, label %xc2818.fim
xc2818.limpar:
  call void @dartforge_exception_clear()
  br label %xc2818.fim
xc2818.fim:
  %gcvolta1817 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1817, ptr %ctxtopo, align 8
  ret i64 %v2817
b1818:
  store i64 0, ptr %gcs0
  %v2804 = call i64 @dartforge_native_DartForge_double_toInt(double %v2801)
  %v2805 = load i8, ptr %ctx, align 8
  %c637 = zext i8 %v2805 to i64
  %v2806 = icmp ne i64 %c637, 0
  br i1 %v2806, label %b1821, label %b1820
b1819:
  %gcvolta1819 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1819, ptr %ctxtopo, align 8
  ret i64 0
b1820:
  br label %b1817
b1821:
  %gcvolta1821 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1821, ptr %ctxtopo, align 8
  ret i64 0
b1822:
  br label %b1817
b1823:
  %gcvolta1823 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1823, ptr %ctxtopo, align 8
  ret i64 0
b1824:
  %v2813 = call i64 @df.unbox_int(i64 %v2810)
  %v2814 = load i8, ptr %ctx, align 8
  %c638 = zext i8 %v2814 to i64
  %v2815 = icmp ne i64 %c638, 0
  br i1 %v2815, label %b1827, label %b1826
b1825:
  %gcvolta1825 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1825, ptr %ctxtopo, align 8
  ret i64 0
b1826:
  br label %b1817
b1827:
  %gcvolta1827 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1827, ptr %ctxtopo, align 8
  ret i64 0
b1829:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2823 = call i64 @df.box_int(i64 %v2820)
  store i64 %v2823, ptr %gcs1
  %v2824 = call i64 @df.box_int(i64 %v2821)
  store i64 %v2824, ptr %gcs2
  %v2825 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2823, i64 %v2824)
  %v2826 = load i8, ptr %ctx, align 8
  %c639 = zext i8 %v2826 to i64
  %v2827 = icmp ne i64 %c639, 0
  br i1 %v2827, label %b1834, label %b1833
b1830:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2828 = call i64 @df.box_int(i64 %v2820)
  store i64 %v2828, ptr %gcs1
  %v2829 = call i64 @df.box_int(i64 %v2821)
  store i64 %v2829, ptr %gcs2
  %v2830 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2828, i64 %v2829)
  %v2831 = load i8, ptr %ctx, align 8
  %c640 = zext i8 %v2831 to i64
  %v2832 = icmp ne i64 %c640, 0
  br i1 %v2832, label %b1836, label %b1835
b1831:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2833 = call i64 @df.box_int(i64 %v2820)
  store i64 %v2833, ptr %gcs1
  %v2834 = call i64 @df.box_int(i64 %v2821)
  store i64 %v2834, ptr %gcs2
  %sa2835_0 = getelementptr [3 x i64], ptr %sargs2835, i64 0, i64 0
  store i64 %v2833, ptr %sa2835_0
  %sa2835_1 = getelementptr [3 x i64], ptr %sargs2835, i64 0, i64 1
  store i64 %v2834, ptr %sa2835_1
  %sat2835 = getelementptr [3 x i64], ptr %sargs2835, i64 0, i64 2
  store i64 0, ptr %sat2835
  %sic2835 = getelementptr i64, ptr %area, i64 284
  %sf2835 = call ptr @df.selector(ptr %sic2835, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2835 = call i64 %sf2835(i64 %v0, ptr %sargs2835, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2836 = load i8, ptr %ctx, align 8
  %c641 = zext i8 %v2836 to i64
  %v2837 = icmp ne i64 %c641, 0
  br i1 %v2837, label %b1838, label %b1837
b1832:
  %v2838 = phi i64 [ %v2825, %b1833 ], [ %v2830, %b1835 ], [ %v2835, %b1837 ]
  store i64 %v2838, ptr %gcs0
  %v2839 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2840 = and i64 %v2838, 1
  %v2841 = icmp ne i64 %v2840, 0
  br i1 %v2841, label %b1839, label %b1840
b1833:
  br label %b1832
b1834:
  %gcvolta1834 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1834, ptr %ctxtopo, align 8
  ret i64 0
b1835:
  br label %b1832
b1836:
  %gcvolta1836 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1836, ptr %ctxtopo, align 8
  ret i64 0
b1837:
  br label %b1832
b1838:
  %gcvolta1838 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1838, ptr %ctxtopo, align 8
  ret i64 0
b1839:
  %v2842 = ashr i64 %v2838, 1
  %v2843 = mul i64 %v2842, %v2839
  store i64 0, ptr %gcs0
  %v2844 = call i64 @df.box_int(i64 %v2843)
  br label %b1841
b1840:
  %v2845 = call i64 @df.class(i64 %v2838)
  %v2846 = icmp eq i64 %v2845, 2
  %v2847 = icmp eq i64 %v2845, 3
  %v2848 = icmp eq i64 %v2845, 4
  %c642 = zext i1 %v2846 to i64
  %c643 = zext i1 %v2847 to i64
  %v2849 = or i64 %c642, %c643
  %c644 = zext i1 %v2848 to i64
  %v2850 = or i64 %v2849, %c644
  %c645 = trunc i64 %v2850 to i1
  br i1 %c645, label %b1842, label %b1843
b1841:
  %v2869 = phi i64 [ %v2844, %b1839 ], [ %v2868, %b1844 ]
  store i64 %v2869, ptr %gcs0
  %v2870 = call i64 @df.class(i64 %v2869)
  switch i64 %v2870, label %b1856 [ i64 4, label %b1854 i64 2, label %b1855 i64 3, label %b1855 ]
b1842:
  br i1 %v2848, label %b1846, label %b1845
b1843:
  %v2864 = call i64 @df.box_int(i64 %v2839)
  store i64 %v2864, ptr %gcs1
  %sa2865_0 = getelementptr [2 x i64], ptr %sargs2865, i64 0, i64 0
  store i64 %v2864, ptr %sa2865_0
  %sat2865 = getelementptr [2 x i64], ptr %sargs2865, i64 0, i64 1
  store i64 0, ptr %sat2865
  %sic2865 = getelementptr i64, ptr %area, i64 286
  %sf2865 = call ptr @df.selector(ptr %sic2865, i64 %v2838, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v2865 = call i64 %sf2865(i64 %v2838, ptr %sargs2865, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2866 = load i8, ptr %ctx, align 8
  %c646 = zext i8 %v2866 to i64
  %v2867 = icmp ne i64 %c646, 0
  br i1 %v2867, label %b1853, label %b1852
b1844:
  %v2868 = phi i64 [ %v2863, %b1847 ], [ %v2865, %b1852 ]
  br label %b1841
b1845:
  %v2852 = call i64 @df.unbox_int(i64 %v2838)
  %v2853 = load i8, ptr %ctx, align 8
  %c647 = zext i8 %v2853 to i64
  %v2854 = icmp ne i64 %c647, 0
  br i1 %v2854, label %b1849, label %b1848
b1846:
  %v2857 = call double @df.num_to_double(i64 %v2838)
  %v2858 = load i8, ptr %ctx, align 8
  %c648 = zext i8 %v2858 to i64
  %v2859 = icmp ne i64 %c648, 0
  br i1 %v2859, label %b1851, label %b1850
b1847:
  %v2863 = phi i64 [ %v2856, %b1848 ], [ %v2862, %b1850 ]
  br label %b1844
b1848:
  %v2855 = mul i64 %v2852, %v2839
  store i64 0, ptr %gcs0
  %v2856 = call i64 @df.box_int(i64 %v2855)
  br label %b1847
b1849:
  %gcvolta1849 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1849, ptr %ctxtopo, align 8
  ret i64 0
b1850:
  %v2860 = sitofp i64 %v2839 to double
  %v2861 = fmul double %v2857, %v2860
  store i64 0, ptr %gcs0
  %v2862 = call i64 @df.box_double(double %v2861)
  br label %b1847
b1851:
  %gcvolta1851 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1851, ptr %ctxtopo, align 8
  ret i64 0
b1852:
  br label %b1844
b1853:
  %gcvolta1853 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1853, ptr %ctxtopo, align 8
  ret i64 0
b1854:
  store i64 0, ptr %gcs1
  %v2871 = call double @df.unbox_double(i64 %v2869)
  %v2872 = load i8, ptr %ctx, align 8
  %c649 = zext i8 %v2872 to i64
  %v2873 = icmp ne i64 %c649, 0
  br i1 %v2873, label %b1859, label %b1858
b1855:
  store i64 0, ptr %gcs1
  %v2877 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2869)
  %v2878 = load i8, ptr %ctx, align 8
  %c650 = zext i8 %v2878 to i64
  %v2879 = icmp ne i64 %c650, 0
  br i1 %v2879, label %b1863, label %b1862
b1856:
  store i64 0, ptr %gcs1
  %sat2880 = getelementptr [1 x i64], ptr %sargs2880, i64 0, i64 0
  store i64 0, ptr %sat2880
  %sic2880 = getelementptr i64, ptr %area, i64 288
  %sf2880 = call ptr @df.selector(ptr %sic2880, i64 %v2869, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2880 = call i64 %sf2880(i64 %v2869, ptr %sargs2880, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2880, ptr %gcs0
  %v2881 = load i8, ptr %ctx, align 8
  %c651 = zext i8 %v2881 to i64
  %v2882 = icmp ne i64 %c651, 0
  br i1 %v2882, label %b1865, label %b1864
b1857:
  %v2886 = phi i64 [ %v2874, %b1860 ], [ %v2877, %b1862 ], [ %v2883, %b1866 ]
  store i64 0, ptr %gcs0
  %v2887 = call i64 @df.box_int(i64 %v2886)
  %xcp2888 = load i8, ptr %ctx, align 8
  %xcn2888 = icmp ne i8 %xcp2888, 0
  %xce2888 = call i1 @llvm.expect.i1(i1 %xcn2888, i1 false)
  br i1 %xce2888, label %xc2888.limpar, label %xc2888.fim
xc2888.limpar:
  call void @dartforge_exception_clear()
  br label %xc2888.fim
xc2888.fim:
  %gcvolta1857 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1857, ptr %ctxtopo, align 8
  ret i64 %v2887
b1858:
  store i64 0, ptr %gcs0
  %v2874 = call i64 @dartforge_native_DartForge_double_toInt(double %v2871)
  %v2875 = load i8, ptr %ctx, align 8
  %c652 = zext i8 %v2875 to i64
  %v2876 = icmp ne i64 %c652, 0
  br i1 %v2876, label %b1861, label %b1860
b1859:
  %gcvolta1859 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1859, ptr %ctxtopo, align 8
  ret i64 0
b1860:
  br label %b1857
b1861:
  %gcvolta1861 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1861, ptr %ctxtopo, align 8
  ret i64 0
b1862:
  br label %b1857
b1863:
  %gcvolta1863 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1863, ptr %ctxtopo, align 8
  ret i64 0
b1864:
  %v2883 = call i64 @df.unbox_int(i64 %v2880)
  %v2884 = load i8, ptr %ctx, align 8
  %c653 = zext i8 %v2884 to i64
  %v2885 = icmp ne i64 %c653, 0
  br i1 %v2885, label %b1867, label %b1866
b1865:
  %gcvolta1865 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1865, ptr %ctxtopo, align 8
  ret i64 0
b1866:
  br label %b1857
b1867:
  %gcvolta1867 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1867, ptr %ctxtopo, align 8
  ret i64 0
b1869:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2893 = call i64 @df.box_int(i64 %v2890)
  store i64 %v2893, ptr %gcs1
  %v2894 = call i64 @df.box_int(i64 %v2891)
  store i64 %v2894, ptr %gcs2
  %v2895 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2893, i64 %v2894)
  %v2896 = load i8, ptr %ctx, align 8
  %c654 = zext i8 %v2896 to i64
  %v2897 = icmp ne i64 %c654, 0
  br i1 %v2897, label %b1874, label %b1873
b1870:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2898 = call i64 @df.box_int(i64 %v2890)
  store i64 %v2898, ptr %gcs1
  %v2899 = call i64 @df.box_int(i64 %v2891)
  store i64 %v2899, ptr %gcs2
  %v2900 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2898, i64 %v2899)
  %v2901 = load i8, ptr %ctx, align 8
  %c655 = zext i8 %v2901 to i64
  %v2902 = icmp ne i64 %c655, 0
  br i1 %v2902, label %b1876, label %b1875
b1871:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2903 = call i64 @df.box_int(i64 %v2890)
  store i64 %v2903, ptr %gcs1
  %v2904 = call i64 @df.box_int(i64 %v2891)
  store i64 %v2904, ptr %gcs2
  %sa2905_0 = getelementptr [3 x i64], ptr %sargs2905, i64 0, i64 0
  store i64 %v2903, ptr %sa2905_0
  %sa2905_1 = getelementptr [3 x i64], ptr %sargs2905, i64 0, i64 1
  store i64 %v2904, ptr %sa2905_1
  %sat2905 = getelementptr [3 x i64], ptr %sargs2905, i64 0, i64 2
  store i64 0, ptr %sat2905
  %sic2905 = getelementptr i64, ptr %area, i64 290
  %sf2905 = call ptr @df.selector(ptr %sic2905, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2905 = call i64 %sf2905(i64 %v0, ptr %sargs2905, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2906 = load i8, ptr %ctx, align 8
  %c656 = zext i8 %v2906 to i64
  %v2907 = icmp ne i64 %c656, 0
  br i1 %v2907, label %b1878, label %b1877
b1872:
  %v2908 = phi i64 [ %v2895, %b1873 ], [ %v2900, %b1875 ], [ %v2905, %b1877 ]
  store i64 %v2908, ptr %gcs0
  %v2909 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2910 = and i64 %v2908, 1
  %v2911 = icmp ne i64 %v2910, 0
  br i1 %v2911, label %b1879, label %b1880
b1873:
  br label %b1872
b1874:
  %gcvolta1874 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1874, ptr %ctxtopo, align 8
  ret i64 0
b1875:
  br label %b1872
b1876:
  %gcvolta1876 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1876, ptr %ctxtopo, align 8
  ret i64 0
b1877:
  br label %b1872
b1878:
  %gcvolta1878 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1878, ptr %ctxtopo, align 8
  ret i64 0
b1879:
  %v2912 = ashr i64 %v2908, 1
  %v2913 = mul i64 %v2912, %v2909
  store i64 0, ptr %gcs0
  %v2914 = call i64 @df.box_int(i64 %v2913)
  br label %b1881
b1880:
  %v2915 = call i64 @df.class(i64 %v2908)
  %v2916 = icmp eq i64 %v2915, 2
  %v2917 = icmp eq i64 %v2915, 3
  %v2918 = icmp eq i64 %v2915, 4
  %c657 = zext i1 %v2916 to i64
  %c658 = zext i1 %v2917 to i64
  %v2919 = or i64 %c657, %c658
  %c659 = zext i1 %v2918 to i64
  %v2920 = or i64 %v2919, %c659
  %c660 = trunc i64 %v2920 to i1
  br i1 %c660, label %b1882, label %b1883
b1881:
  %v2939 = phi i64 [ %v2914, %b1879 ], [ %v2938, %b1884 ]
  store i64 %v2939, ptr %gcs0
  %v2940 = call i64 @df.class(i64 %v2939)
  switch i64 %v2940, label %b1896 [ i64 4, label %b1894 i64 2, label %b1895 i64 3, label %b1895 ]
b1882:
  br i1 %v2918, label %b1886, label %b1885
b1883:
  %v2934 = call i64 @df.box_int(i64 %v2909)
  store i64 %v2934, ptr %gcs1
  %sa2935_0 = getelementptr [2 x i64], ptr %sargs2935, i64 0, i64 0
  store i64 %v2934, ptr %sa2935_0
  %sat2935 = getelementptr [2 x i64], ptr %sargs2935, i64 0, i64 1
  store i64 0, ptr %sat2935
  %sic2935 = getelementptr i64, ptr %area, i64 292
  %sf2935 = call ptr @df.selector(ptr %sic2935, i64 %v2908, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v2935 = call i64 %sf2935(i64 %v2908, ptr %sargs2935, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2936 = load i8, ptr %ctx, align 8
  %c661 = zext i8 %v2936 to i64
  %v2937 = icmp ne i64 %c661, 0
  br i1 %v2937, label %b1893, label %b1892
b1884:
  %v2938 = phi i64 [ %v2933, %b1887 ], [ %v2935, %b1892 ]
  br label %b1881
b1885:
  %v2922 = call i64 @df.unbox_int(i64 %v2908)
  %v2923 = load i8, ptr %ctx, align 8
  %c662 = zext i8 %v2923 to i64
  %v2924 = icmp ne i64 %c662, 0
  br i1 %v2924, label %b1889, label %b1888
b1886:
  %v2927 = call double @df.num_to_double(i64 %v2908)
  %v2928 = load i8, ptr %ctx, align 8
  %c663 = zext i8 %v2928 to i64
  %v2929 = icmp ne i64 %c663, 0
  br i1 %v2929, label %b1891, label %b1890
b1887:
  %v2933 = phi i64 [ %v2926, %b1888 ], [ %v2932, %b1890 ]
  br label %b1884
b1888:
  %v2925 = mul i64 %v2922, %v2909
  store i64 0, ptr %gcs0
  %v2926 = call i64 @df.box_int(i64 %v2925)
  br label %b1887
b1889:
  %gcvolta1889 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1889, ptr %ctxtopo, align 8
  ret i64 0
b1890:
  %v2930 = sitofp i64 %v2909 to double
  %v2931 = fmul double %v2927, %v2930
  store i64 0, ptr %gcs0
  %v2932 = call i64 @df.box_double(double %v2931)
  br label %b1887
b1891:
  %gcvolta1891 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1891, ptr %ctxtopo, align 8
  ret i64 0
b1892:
  br label %b1884
b1893:
  %gcvolta1893 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1893, ptr %ctxtopo, align 8
  ret i64 0
b1894:
  store i64 0, ptr %gcs1
  %v2941 = call double @df.unbox_double(i64 %v2939)
  %v2942 = load i8, ptr %ctx, align 8
  %c664 = zext i8 %v2942 to i64
  %v2943 = icmp ne i64 %c664, 0
  br i1 %v2943, label %b1899, label %b1898
b1895:
  store i64 0, ptr %gcs1
  %v2947 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2939)
  %v2948 = load i8, ptr %ctx, align 8
  %c665 = zext i8 %v2948 to i64
  %v2949 = icmp ne i64 %c665, 0
  br i1 %v2949, label %b1903, label %b1902
b1896:
  store i64 0, ptr %gcs1
  %sat2950 = getelementptr [1 x i64], ptr %sargs2950, i64 0, i64 0
  store i64 0, ptr %sat2950
  %sic2950 = getelementptr i64, ptr %area, i64 294
  %sf2950 = call ptr @df.selector(ptr %sic2950, i64 %v2939, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2950 = call i64 %sf2950(i64 %v2939, ptr %sargs2950, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2950, ptr %gcs0
  %v2951 = load i8, ptr %ctx, align 8
  %c666 = zext i8 %v2951 to i64
  %v2952 = icmp ne i64 %c666, 0
  br i1 %v2952, label %b1905, label %b1904
b1897:
  %v2956 = phi i64 [ %v2944, %b1900 ], [ %v2947, %b1902 ], [ %v2953, %b1906 ]
  store i64 0, ptr %gcs0
  %v2957 = call i64 @df.box_int(i64 %v2956)
  %xcp2958 = load i8, ptr %ctx, align 8
  %xcn2958 = icmp ne i8 %xcp2958, 0
  %xce2958 = call i1 @llvm.expect.i1(i1 %xcn2958, i1 false)
  br i1 %xce2958, label %xc2958.limpar, label %xc2958.fim
xc2958.limpar:
  call void @dartforge_exception_clear()
  br label %xc2958.fim
xc2958.fim:
  %gcvolta1897 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1897, ptr %ctxtopo, align 8
  ret i64 %v2957
b1898:
  store i64 0, ptr %gcs0
  %v2944 = call i64 @dartforge_native_DartForge_double_toInt(double %v2941)
  %v2945 = load i8, ptr %ctx, align 8
  %c667 = zext i8 %v2945 to i64
  %v2946 = icmp ne i64 %c667, 0
  br i1 %v2946, label %b1901, label %b1900
b1899:
  %gcvolta1899 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1899, ptr %ctxtopo, align 8
  ret i64 0
b1900:
  br label %b1897
b1901:
  %gcvolta1901 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1901, ptr %ctxtopo, align 8
  ret i64 0
b1902:
  br label %b1897
b1903:
  %gcvolta1903 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1903, ptr %ctxtopo, align 8
  ret i64 0
b1904:
  %v2953 = call i64 @df.unbox_int(i64 %v2950)
  %v2954 = load i8, ptr %ctx, align 8
  %c668 = zext i8 %v2954 to i64
  %v2955 = icmp ne i64 %c668, 0
  br i1 %v2955, label %b1907, label %b1906
b1905:
  %gcvolta1905 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1905, ptr %ctxtopo, align 8
  ret i64 0
b1906:
  br label %b1897
b1907:
  %gcvolta1907 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1907, ptr %ctxtopo, align 8
  ret i64 0
b1909:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2963 = call i64 @df.box_int(i64 %v2960)
  store i64 %v2963, ptr %gcs1
  %v2964 = call i64 @df.box_int(i64 %v2961)
  store i64 %v2964, ptr %gcs2
  %v2965 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2963, i64 %v2964)
  %v2966 = load i8, ptr %ctx, align 8
  %c669 = zext i8 %v2966 to i64
  %v2967 = icmp ne i64 %c669, 0
  br i1 %v2967, label %b1914, label %b1913
b1910:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2968 = call i64 @df.box_int(i64 %v2960)
  store i64 %v2968, ptr %gcs1
  %v2969 = call i64 @df.box_int(i64 %v2961)
  store i64 %v2969, ptr %gcs2
  %v2970 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2968, i64 %v2969)
  %v2971 = load i8, ptr %ctx, align 8
  %c670 = zext i8 %v2971 to i64
  %v2972 = icmp ne i64 %c670, 0
  br i1 %v2972, label %b1916, label %b1915
b1911:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2973 = call i64 @df.box_int(i64 %v2960)
  store i64 %v2973, ptr %gcs1
  %v2974 = call i64 @df.box_int(i64 %v2961)
  store i64 %v2974, ptr %gcs2
  %sa2975_0 = getelementptr [3 x i64], ptr %sargs2975, i64 0, i64 0
  store i64 %v2973, ptr %sa2975_0
  %sa2975_1 = getelementptr [3 x i64], ptr %sargs2975, i64 0, i64 1
  store i64 %v2974, ptr %sa2975_1
  %sat2975 = getelementptr [3 x i64], ptr %sargs2975, i64 0, i64 2
  store i64 0, ptr %sat2975
  %sic2975 = getelementptr i64, ptr %area, i64 296
  %sf2975 = call ptr @df.selector(ptr %sic2975, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2975 = call i64 %sf2975(i64 %v0, ptr %sargs2975, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2976 = load i8, ptr %ctx, align 8
  %c671 = zext i8 %v2976 to i64
  %v2977 = icmp ne i64 %c671, 0
  br i1 %v2977, label %b1918, label %b1917
b1912:
  %v2978 = phi i64 [ %v2965, %b1913 ], [ %v2970, %b1915 ], [ %v2975, %b1917 ]
  store i64 %v2978, ptr %gcs0
  %v2979 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2980 = and i64 %v2978, 1
  %v2981 = icmp ne i64 %v2980, 0
  br i1 %v2981, label %b1919, label %b1920
b1913:
  br label %b1912
b1914:
  %gcvolta1914 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1914, ptr %ctxtopo, align 8
  ret i64 0
b1915:
  br label %b1912
b1916:
  %gcvolta1916 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1916, ptr %ctxtopo, align 8
  ret i64 0
b1917:
  br label %b1912
b1918:
  %gcvolta1918 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1918, ptr %ctxtopo, align 8
  ret i64 0
b1919:
  %v2982 = ashr i64 %v2978, 1
  %v2983 = mul i64 %v2982, %v2979
  store i64 0, ptr %gcs0
  %v2984 = call i64 @df.box_int(i64 %v2983)
  br label %b1921
b1920:
  %v2985 = call i64 @df.class(i64 %v2978)
  %v2986 = icmp eq i64 %v2985, 2
  %v2987 = icmp eq i64 %v2985, 3
  %v2988 = icmp eq i64 %v2985, 4
  %c672 = zext i1 %v2986 to i64
  %c673 = zext i1 %v2987 to i64
  %v2989 = or i64 %c672, %c673
  %c674 = zext i1 %v2988 to i64
  %v2990 = or i64 %v2989, %c674
  %c675 = trunc i64 %v2990 to i1
  br i1 %c675, label %b1922, label %b1923
b1921:
  %v3009 = phi i64 [ %v2984, %b1919 ], [ %v3008, %b1924 ]
  store i64 %v3009, ptr %gcs0
  %v3010 = call i64 @df.class(i64 %v3009)
  switch i64 %v3010, label %b1936 [ i64 4, label %b1934 i64 2, label %b1935 i64 3, label %b1935 ]
b1922:
  br i1 %v2988, label %b1926, label %b1925
b1923:
  %v3004 = call i64 @df.box_int(i64 %v2979)
  store i64 %v3004, ptr %gcs1
  %sa3005_0 = getelementptr [2 x i64], ptr %sargs3005, i64 0, i64 0
  store i64 %v3004, ptr %sa3005_0
  %sat3005 = getelementptr [2 x i64], ptr %sargs3005, i64 0, i64 1
  store i64 0, ptr %sat3005
  %sic3005 = getelementptr i64, ptr %area, i64 298
  %sf3005 = call ptr @df.selector(ptr %sic3005, i64 %v2978, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3005 = call i64 %sf3005(i64 %v2978, ptr %sargs3005, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3006 = load i8, ptr %ctx, align 8
  %c676 = zext i8 %v3006 to i64
  %v3007 = icmp ne i64 %c676, 0
  br i1 %v3007, label %b1933, label %b1932
b1924:
  %v3008 = phi i64 [ %v3003, %b1927 ], [ %v3005, %b1932 ]
  br label %b1921
b1925:
  %v2992 = call i64 @df.unbox_int(i64 %v2978)
  %v2993 = load i8, ptr %ctx, align 8
  %c677 = zext i8 %v2993 to i64
  %v2994 = icmp ne i64 %c677, 0
  br i1 %v2994, label %b1929, label %b1928
b1926:
  %v2997 = call double @df.num_to_double(i64 %v2978)
  %v2998 = load i8, ptr %ctx, align 8
  %c678 = zext i8 %v2998 to i64
  %v2999 = icmp ne i64 %c678, 0
  br i1 %v2999, label %b1931, label %b1930
b1927:
  %v3003 = phi i64 [ %v2996, %b1928 ], [ %v3002, %b1930 ]
  br label %b1924
b1928:
  %v2995 = mul i64 %v2992, %v2979
  store i64 0, ptr %gcs0
  %v2996 = call i64 @df.box_int(i64 %v2995)
  br label %b1927
b1929:
  %gcvolta1929 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1929, ptr %ctxtopo, align 8
  ret i64 0
b1930:
  %v3000 = sitofp i64 %v2979 to double
  %v3001 = fmul double %v2997, %v3000
  store i64 0, ptr %gcs0
  %v3002 = call i64 @df.box_double(double %v3001)
  br label %b1927
b1931:
  %gcvolta1931 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1931, ptr %ctxtopo, align 8
  ret i64 0
b1932:
  br label %b1924
b1933:
  %gcvolta1933 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1933, ptr %ctxtopo, align 8
  ret i64 0
b1934:
  store i64 0, ptr %gcs1
  %v3011 = call double @df.unbox_double(i64 %v3009)
  %v3012 = load i8, ptr %ctx, align 8
  %c679 = zext i8 %v3012 to i64
  %v3013 = icmp ne i64 %c679, 0
  br i1 %v3013, label %b1939, label %b1938
b1935:
  store i64 0, ptr %gcs1
  %v3017 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3009)
  %v3018 = load i8, ptr %ctx, align 8
  %c680 = zext i8 %v3018 to i64
  %v3019 = icmp ne i64 %c680, 0
  br i1 %v3019, label %b1943, label %b1942
b1936:
  store i64 0, ptr %gcs1
  %sat3020 = getelementptr [1 x i64], ptr %sargs3020, i64 0, i64 0
  store i64 0, ptr %sat3020
  %sic3020 = getelementptr i64, ptr %area, i64 300
  %sf3020 = call ptr @df.selector(ptr %sic3020, i64 %v3009, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3020 = call i64 %sf3020(i64 %v3009, ptr %sargs3020, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3020, ptr %gcs0
  %v3021 = load i8, ptr %ctx, align 8
  %c681 = zext i8 %v3021 to i64
  %v3022 = icmp ne i64 %c681, 0
  br i1 %v3022, label %b1945, label %b1944
b1937:
  %v3026 = phi i64 [ %v3014, %b1940 ], [ %v3017, %b1942 ], [ %v3023, %b1946 ]
  store i64 0, ptr %gcs0
  %v3027 = call i64 @df.box_int(i64 %v3026)
  %xcp3028 = load i8, ptr %ctx, align 8
  %xcn3028 = icmp ne i8 %xcp3028, 0
  %xce3028 = call i1 @llvm.expect.i1(i1 %xcn3028, i1 false)
  br i1 %xce3028, label %xc3028.limpar, label %xc3028.fim
xc3028.limpar:
  call void @dartforge_exception_clear()
  br label %xc3028.fim
xc3028.fim:
  %gcvolta1937 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1937, ptr %ctxtopo, align 8
  ret i64 %v3027
b1938:
  store i64 0, ptr %gcs0
  %v3014 = call i64 @dartforge_native_DartForge_double_toInt(double %v3011)
  %v3015 = load i8, ptr %ctx, align 8
  %c682 = zext i8 %v3015 to i64
  %v3016 = icmp ne i64 %c682, 0
  br i1 %v3016, label %b1941, label %b1940
b1939:
  %gcvolta1939 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1939, ptr %ctxtopo, align 8
  ret i64 0
b1940:
  br label %b1937
b1941:
  %gcvolta1941 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1941, ptr %ctxtopo, align 8
  ret i64 0
b1942:
  br label %b1937
b1943:
  %gcvolta1943 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1943, ptr %ctxtopo, align 8
  ret i64 0
b1944:
  %v3023 = call i64 @df.unbox_int(i64 %v3020)
  %v3024 = load i8, ptr %ctx, align 8
  %c683 = zext i8 %v3024 to i64
  %v3025 = icmp ne i64 %c683, 0
  br i1 %v3025, label %b1947, label %b1946
b1945:
  %gcvolta1945 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1945, ptr %ctxtopo, align 8
  ret i64 0
b1946:
  br label %b1937
b1947:
  %gcvolta1947 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1947, ptr %ctxtopo, align 8
  ret i64 0
b1949:
  %v3033 = ashr i64 %v0, 1
  %v3034 = icmp slt i64 %v3033, %v3030
  br label %b1951
b1950:
  %v3035 = call i64 @df.box_int(i64 %v3030)
  store i64 %v3035, ptr %gcs1
  %sa3036_0 = getelementptr [2 x i64], ptr %sargs3036, i64 0, i64 0
  store i64 %v3035, ptr %sa3036_0
  %sat3036 = getelementptr [2 x i64], ptr %sargs3036, i64 0, i64 1
  store i64 0, ptr %sat3036
  %sic3036 = getelementptr i64, ptr %area, i64 302
  %sf3036 = call ptr @df.selector(ptr %sic3036, i64 %v0, i64 -4020809642220582218, ptr @df.seln.15, i64 4)
  %v3036 = call i64 %sf3036(i64 %v0, ptr %sargs3036, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v3036, ptr %gcs1
  %v3037 = load i8, ptr %ctx, align 8
  %c684 = zext i8 %v3037 to i64
  %v3038 = icmp ne i64 %c684, 0
  br i1 %v3038, label %b1953, label %b1952
b1951:
  %v3042 = phi i1 [ %v3034, %b1949 ], [ %v3039, %b1954 ]
  br i1 %v3042, label %b1956, label %b1957
b1952:
  %v3039 = call i1 @df.unbox_bool(i64 %v3036)
  %v3040 = load i8, ptr %ctx, align 8
  %c685 = zext i8 %v3040 to i64
  %v3041 = icmp ne i64 %c685, 0
  br i1 %v3041, label %b1955, label %b1954
b1953:
  %gcvolta1953 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1953, ptr %ctxtopo, align 8
  ret i64 0
b1954:
  br label %b1951
b1955:
  %gcvolta1955 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1955, ptr %ctxtopo, align 8
  ret i64 0
b1956:
  %v3044 = add i64 0, 1
  %v3045 = sub i64 0, %v3044
  %v3046 = add i64 0, 1
  %v3047 = call i64 @df.class(i64 %v0)
  switch i64 %v3047, label %b1961 [ i64 4, label %b1959 i64 2, label %b1960 i64 3, label %b1960 ]
b1957:
  %v3096 = add i64 0, 1
  %v3097 = sub i64 0, %v3096
  %v3098 = add i64 0, 1
  %v3099 = call i64 @df.class(i64 %v0)
  switch i64 %v3099, label %b1986 [ i64 4, label %b1984 i64 2, label %b1985 i64 3, label %b1985 ]
b1958:
  %v3147 = phi i64 [ %v3094, %b1971 ], [ %v3146, %b1996 ]
  store i64 %v3147, ptr %gcs0
  %v3148 = call i64 @df.class(i64 %v3147)
  switch i64 %v3148, label %b2011 [ i64 4, label %b2009 i64 2, label %b2010 i64 3, label %b2010 ]
b1959:
  store i64 0, ptr %gcs1
  %v3048 = call i64 @df.box_int(i64 %v3045)
  store i64 %v3048, ptr %gcs1
  %v3049 = call i64 @df.box_int(i64 %v3046)
  store i64 %v3049, ptr %gcs2
  %v3050 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3048, i64 %v3049)
  %v3051 = load i8, ptr %ctx, align 8
  %c686 = zext i8 %v3051 to i64
  %v3052 = icmp ne i64 %c686, 0
  br i1 %v3052, label %b1964, label %b1963
b1960:
  store i64 0, ptr %gcs1
  %v3053 = call i64 @df.box_int(i64 %v3045)
  store i64 %v3053, ptr %gcs1
  %v3054 = call i64 @df.box_int(i64 %v3046)
  store i64 %v3054, ptr %gcs2
  %v3055 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3053, i64 %v3054)
  %v3056 = load i8, ptr %ctx, align 8
  %c687 = zext i8 %v3056 to i64
  %v3057 = icmp ne i64 %c687, 0
  br i1 %v3057, label %b1966, label %b1965
b1961:
  store i64 0, ptr %gcs1
  %v3058 = call i64 @df.box_int(i64 %v3045)
  store i64 %v3058, ptr %gcs1
  %v3059 = call i64 @df.box_int(i64 %v3046)
  store i64 %v3059, ptr %gcs2
  %sa3060_0 = getelementptr [3 x i64], ptr %sargs3060, i64 0, i64 0
  store i64 %v3058, ptr %sa3060_0
  %sa3060_1 = getelementptr [3 x i64], ptr %sargs3060, i64 0, i64 1
  store i64 %v3059, ptr %sa3060_1
  %sat3060 = getelementptr [3 x i64], ptr %sargs3060, i64 0, i64 2
  store i64 0, ptr %sat3060
  %sic3060 = getelementptr i64, ptr %area, i64 304
  %sf3060 = call ptr @df.selector(ptr %sic3060, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3060 = call i64 %sf3060(i64 %v0, ptr %sargs3060, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3061 = load i8, ptr %ctx, align 8
  %c688 = zext i8 %v3061 to i64
  %v3062 = icmp ne i64 %c688, 0
  br i1 %v3062, label %b1968, label %b1967
b1962:
  %v3063 = phi i64 [ %v3050, %b1963 ], [ %v3055, %b1965 ], [ %v3060, %b1967 ]
  store i64 %v3063, ptr %gcs0
  %v3064 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3065 = and i64 %v3063, 1
  %v3066 = icmp ne i64 %v3065, 0
  br i1 %v3066, label %b1969, label %b1970
b1963:
  br label %b1962
b1964:
  %gcvolta1964 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1964, ptr %ctxtopo, align 8
  ret i64 0
b1965:
  br label %b1962
b1966:
  %gcvolta1966 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1966, ptr %ctxtopo, align 8
  ret i64 0
b1967:
  br label %b1962
b1968:
  %gcvolta1968 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1968, ptr %ctxtopo, align 8
  ret i64 0
b1969:
  %v3067 = ashr i64 %v3063, 1
  %v3068 = mul i64 %v3067, %v3064
  store i64 0, ptr %gcs0
  %v3069 = call i64 @df.box_int(i64 %v3068)
  br label %b1971
b1970:
  %v3070 = call i64 @df.class(i64 %v3063)
  %v3071 = icmp eq i64 %v3070, 2
  %v3072 = icmp eq i64 %v3070, 3
  %v3073 = icmp eq i64 %v3070, 4
  %c689 = zext i1 %v3071 to i64
  %c690 = zext i1 %v3072 to i64
  %v3074 = or i64 %c689, %c690
  %c691 = zext i1 %v3073 to i64
  %v3075 = or i64 %v3074, %c691
  %c692 = trunc i64 %v3075 to i1
  br i1 %c692, label %b1972, label %b1973
b1971:
  %v3094 = phi i64 [ %v3069, %b1969 ], [ %v3093, %b1974 ]
  br label %b1958
b1972:
  br i1 %v3073, label %b1976, label %b1975
b1973:
  %v3089 = call i64 @df.box_int(i64 %v3064)
  store i64 %v3089, ptr %gcs1
  %sa3090_0 = getelementptr [2 x i64], ptr %sargs3090, i64 0, i64 0
  store i64 %v3089, ptr %sa3090_0
  %sat3090 = getelementptr [2 x i64], ptr %sargs3090, i64 0, i64 1
  store i64 0, ptr %sat3090
  %sic3090 = getelementptr i64, ptr %area, i64 306
  %sf3090 = call ptr @df.selector(ptr %sic3090, i64 %v3063, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3090 = call i64 %sf3090(i64 %v3063, ptr %sargs3090, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3091 = load i8, ptr %ctx, align 8
  %c693 = zext i8 %v3091 to i64
  %v3092 = icmp ne i64 %c693, 0
  br i1 %v3092, label %b1983, label %b1982
b1974:
  %v3093 = phi i64 [ %v3088, %b1977 ], [ %v3090, %b1982 ]
  br label %b1971
b1975:
  %v3077 = call i64 @df.unbox_int(i64 %v3063)
  %v3078 = load i8, ptr %ctx, align 8
  %c694 = zext i8 %v3078 to i64
  %v3079 = icmp ne i64 %c694, 0
  br i1 %v3079, label %b1979, label %b1978
b1976:
  %v3082 = call double @df.num_to_double(i64 %v3063)
  %v3083 = load i8, ptr %ctx, align 8
  %c695 = zext i8 %v3083 to i64
  %v3084 = icmp ne i64 %c695, 0
  br i1 %v3084, label %b1981, label %b1980
b1977:
  %v3088 = phi i64 [ %v3081, %b1978 ], [ %v3087, %b1980 ]
  br label %b1974
b1978:
  %v3080 = mul i64 %v3077, %v3064
  store i64 0, ptr %gcs0
  %v3081 = call i64 @df.box_int(i64 %v3080)
  br label %b1977
b1979:
  %gcvolta1979 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1979, ptr %ctxtopo, align 8
  ret i64 0
b1980:
  %v3085 = sitofp i64 %v3064 to double
  %v3086 = fmul double %v3082, %v3085
  store i64 0, ptr %gcs0
  %v3087 = call i64 @df.box_double(double %v3086)
  br label %b1977
b1981:
  %gcvolta1981 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1981, ptr %ctxtopo, align 8
  ret i64 0
b1982:
  br label %b1974
b1983:
  %gcvolta1983 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1983, ptr %ctxtopo, align 8
  ret i64 0
b1984:
  store i64 0, ptr %gcs1
  %v3100 = call i64 @df.box_int(i64 %v3097)
  store i64 %v3100, ptr %gcs1
  %v3101 = call i64 @df.box_int(i64 %v3098)
  store i64 %v3101, ptr %gcs2
  %v3102 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3100, i64 %v3101)
  %v3103 = load i8, ptr %ctx, align 8
  %c696 = zext i8 %v3103 to i64
  %v3104 = icmp ne i64 %c696, 0
  br i1 %v3104, label %b1989, label %b1988
b1985:
  store i64 0, ptr %gcs1
  %v3105 = call i64 @df.box_int(i64 %v3097)
  store i64 %v3105, ptr %gcs1
  %v3106 = call i64 @df.box_int(i64 %v3098)
  store i64 %v3106, ptr %gcs2
  %v3107 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3105, i64 %v3106)
  %v3108 = load i8, ptr %ctx, align 8
  %c697 = zext i8 %v3108 to i64
  %v3109 = icmp ne i64 %c697, 0
  br i1 %v3109, label %b1991, label %b1990
b1986:
  store i64 0, ptr %gcs1
  %v3110 = call i64 @df.box_int(i64 %v3097)
  store i64 %v3110, ptr %gcs1
  %v3111 = call i64 @df.box_int(i64 %v3098)
  store i64 %v3111, ptr %gcs2
  %sa3112_0 = getelementptr [3 x i64], ptr %sargs3112, i64 0, i64 0
  store i64 %v3110, ptr %sa3112_0
  %sa3112_1 = getelementptr [3 x i64], ptr %sargs3112, i64 0, i64 1
  store i64 %v3111, ptr %sa3112_1
  %sat3112 = getelementptr [3 x i64], ptr %sargs3112, i64 0, i64 2
  store i64 0, ptr %sat3112
  %sic3112 = getelementptr i64, ptr %area, i64 308
  %sf3112 = call ptr @df.selector(ptr %sic3112, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3112 = call i64 %sf3112(i64 %v0, ptr %sargs3112, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3113 = load i8, ptr %ctx, align 8
  %c698 = zext i8 %v3113 to i64
  %v3114 = icmp ne i64 %c698, 0
  br i1 %v3114, label %b1993, label %b1992
b1987:
  %v3115 = phi i64 [ %v3102, %b1988 ], [ %v3107, %b1990 ], [ %v3112, %b1992 ]
  store i64 %v3115, ptr %gcs0
  %v3116 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3117 = and i64 %v3115, 1
  %v3118 = icmp ne i64 %v3117, 0
  br i1 %v3118, label %b1994, label %b1995
b1988:
  br label %b1987
b1989:
  %gcvolta1989 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1989, ptr %ctxtopo, align 8
  ret i64 0
b1990:
  br label %b1987
b1991:
  %gcvolta1991 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1991, ptr %ctxtopo, align 8
  ret i64 0
b1992:
  br label %b1987
b1993:
  %gcvolta1993 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1993, ptr %ctxtopo, align 8
  ret i64 0
b1994:
  %v3119 = ashr i64 %v3115, 1
  %v3120 = mul i64 %v3119, %v3116
  store i64 0, ptr %gcs0
  %v3121 = call i64 @df.box_int(i64 %v3120)
  br label %b1996
b1995:
  %v3122 = call i64 @df.class(i64 %v3115)
  %v3123 = icmp eq i64 %v3122, 2
  %v3124 = icmp eq i64 %v3122, 3
  %v3125 = icmp eq i64 %v3122, 4
  %c699 = zext i1 %v3123 to i64
  %c700 = zext i1 %v3124 to i64
  %v3126 = or i64 %c699, %c700
  %c701 = zext i1 %v3125 to i64
  %v3127 = or i64 %v3126, %c701
  %c702 = trunc i64 %v3127 to i1
  br i1 %c702, label %b1997, label %b1998
b1996:
  %v3146 = phi i64 [ %v3121, %b1994 ], [ %v3145, %b1999 ]
  br label %b1958
b1997:
  br i1 %v3125, label %b2001, label %b2000
b1998:
  %v3141 = call i64 @df.box_int(i64 %v3116)
  store i64 %v3141, ptr %gcs1
  %sa3142_0 = getelementptr [2 x i64], ptr %sargs3142, i64 0, i64 0
  store i64 %v3141, ptr %sa3142_0
  %sat3142 = getelementptr [2 x i64], ptr %sargs3142, i64 0, i64 1
  store i64 0, ptr %sat3142
  %sic3142 = getelementptr i64, ptr %area, i64 310
  %sf3142 = call ptr @df.selector(ptr %sic3142, i64 %v3115, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3142 = call i64 %sf3142(i64 %v3115, ptr %sargs3142, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3143 = load i8, ptr %ctx, align 8
  %c703 = zext i8 %v3143 to i64
  %v3144 = icmp ne i64 %c703, 0
  br i1 %v3144, label %b2008, label %b2007
b1999:
  %v3145 = phi i64 [ %v3140, %b2002 ], [ %v3142, %b2007 ]
  br label %b1996
b2000:
  %v3129 = call i64 @df.unbox_int(i64 %v3115)
  %v3130 = load i8, ptr %ctx, align 8
  %c704 = zext i8 %v3130 to i64
  %v3131 = icmp ne i64 %c704, 0
  br i1 %v3131, label %b2004, label %b2003
b2001:
  %v3134 = call double @df.num_to_double(i64 %v3115)
  %v3135 = load i8, ptr %ctx, align 8
  %c705 = zext i8 %v3135 to i64
  %v3136 = icmp ne i64 %c705, 0
  br i1 %v3136, label %b2006, label %b2005
b2002:
  %v3140 = phi i64 [ %v3133, %b2003 ], [ %v3139, %b2005 ]
  br label %b1999
b2003:
  %v3132 = mul i64 %v3129, %v3116
  store i64 0, ptr %gcs0
  %v3133 = call i64 @df.box_int(i64 %v3132)
  br label %b2002
b2004:
  %gcvolta2004 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2004, ptr %ctxtopo, align 8
  ret i64 0
b2005:
  %v3137 = sitofp i64 %v3116 to double
  %v3138 = fmul double %v3134, %v3137
  store i64 0, ptr %gcs0
  %v3139 = call i64 @df.box_double(double %v3138)
  br label %b2002
b2006:
  %gcvolta2006 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2006, ptr %ctxtopo, align 8
  ret i64 0
b2007:
  br label %b1999
b2008:
  %gcvolta2008 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2008, ptr %ctxtopo, align 8
  ret i64 0
b2009:
  store i64 0, ptr %gcs1
  %v3149 = call double @df.unbox_double(i64 %v3147)
  %v3150 = load i8, ptr %ctx, align 8
  %c706 = zext i8 %v3150 to i64
  %v3151 = icmp ne i64 %c706, 0
  br i1 %v3151, label %b2014, label %b2013
b2010:
  store i64 0, ptr %gcs1
  %v3155 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3147)
  %v3156 = load i8, ptr %ctx, align 8
  %c707 = zext i8 %v3156 to i64
  %v3157 = icmp ne i64 %c707, 0
  br i1 %v3157, label %b2018, label %b2017
b2011:
  store i64 0, ptr %gcs1
  %sat3158 = getelementptr [1 x i64], ptr %sargs3158, i64 0, i64 0
  store i64 0, ptr %sat3158
  %sic3158 = getelementptr i64, ptr %area, i64 312
  %sf3158 = call ptr @df.selector(ptr %sic3158, i64 %v3147, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3158 = call i64 %sf3158(i64 %v3147, ptr %sargs3158, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3158, ptr %gcs0
  %v3159 = load i8, ptr %ctx, align 8
  %c708 = zext i8 %v3159 to i64
  %v3160 = icmp ne i64 %c708, 0
  br i1 %v3160, label %b2020, label %b2019
b2012:
  %v3164 = phi i64 [ %v3152, %b2015 ], [ %v3155, %b2017 ], [ %v3161, %b2021 ]
  store i64 0, ptr %gcs0
  %v3165 = call i64 @df.box_int(i64 %v3164)
  %xcp3166 = load i8, ptr %ctx, align 8
  %xcn3166 = icmp ne i8 %xcp3166, 0
  %xce3166 = call i1 @llvm.expect.i1(i1 %xcn3166, i1 false)
  br i1 %xce3166, label %xc3166.limpar, label %xc3166.fim
xc3166.limpar:
  call void @dartforge_exception_clear()
  br label %xc3166.fim
xc3166.fim:
  %gcvolta2012 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2012, ptr %ctxtopo, align 8
  ret i64 %v3165
b2013:
  store i64 0, ptr %gcs0
  %v3152 = call i64 @dartforge_native_DartForge_double_toInt(double %v3149)
  %v3153 = load i8, ptr %ctx, align 8
  %c709 = zext i8 %v3153 to i64
  %v3154 = icmp ne i64 %c709, 0
  br i1 %v3154, label %b2016, label %b2015
b2014:
  %gcvolta2014 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2014, ptr %ctxtopo, align 8
  ret i64 0
b2015:
  br label %b2012
b2016:
  %gcvolta2016 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2016, ptr %ctxtopo, align 8
  ret i64 0
b2017:
  br label %b2012
b2018:
  %gcvolta2018 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2018, ptr %ctxtopo, align 8
  ret i64 0
b2019:
  %v3161 = call i64 @df.unbox_int(i64 %v3158)
  %v3162 = load i8, ptr %ctx, align 8
  %c710 = zext i8 %v3162 to i64
  %v3163 = icmp ne i64 %c710, 0
  br i1 %v3163, label %b2022, label %b2021
b2020:
  %gcvolta2020 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2020, ptr %ctxtopo, align 8
  ret i64 0
b2021:
  br label %b2012
b2022:
  %gcvolta2022 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2022, ptr %ctxtopo, align 8
  ret i64 0
b2024:
  %v3171 = ashr i64 %v0, 1
  %v3172 = icmp slt i64 %v3171, %v3168
  br label %b2026
b2025:
  %v3173 = call i64 @df.box_int(i64 %v3168)
  store i64 %v3173, ptr %gcs1
  %sa3174_0 = getelementptr [2 x i64], ptr %sargs3174, i64 0, i64 0
  store i64 %v3173, ptr %sa3174_0
  %sat3174 = getelementptr [2 x i64], ptr %sargs3174, i64 0, i64 1
  store i64 0, ptr %sat3174
  %sic3174 = getelementptr i64, ptr %area, i64 314
  %sf3174 = call ptr @df.selector(ptr %sic3174, i64 %v0, i64 -4020809642220582218, ptr @df.seln.15, i64 4)
  %v3174 = call i64 %sf3174(i64 %v0, ptr %sargs3174, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v3174, ptr %gcs1
  %v3175 = load i8, ptr %ctx, align 8
  %c711 = zext i8 %v3175 to i64
  %v3176 = icmp ne i64 %c711, 0
  br i1 %v3176, label %b2028, label %b2027
b2026:
  %v3180 = phi i1 [ %v3172, %b2024 ], [ %v3177, %b2029 ]
  br i1 %v3180, label %b2031, label %b2032
b2027:
  %v3177 = call i1 @df.unbox_bool(i64 %v3174)
  %v3178 = load i8, ptr %ctx, align 8
  %c712 = zext i8 %v3178 to i64
  %v3179 = icmp ne i64 %c712, 0
  br i1 %v3179, label %b2030, label %b2029
b2028:
  %gcvolta2028 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2028, ptr %ctxtopo, align 8
  ret i64 0
b2029:
  br label %b2026
b2030:
  %gcvolta2030 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2030, ptr %ctxtopo, align 8
  ret i64 0
b2031:
  %v3182 = add i64 0, 1
  %v3183 = sub i64 0, %v3182
  %v3184 = add i64 0, 1
  %v3185 = call i64 @df.class(i64 %v0)
  switch i64 %v3185, label %b2036 [ i64 4, label %b2034 i64 2, label %b2035 i64 3, label %b2035 ]
b2032:
  %v3234 = add i64 0, 1
  %v3235 = sub i64 0, %v3234
  %v3236 = add i64 0, 1
  %v3237 = call i64 @df.class(i64 %v0)
  switch i64 %v3237, label %b2061 [ i64 4, label %b2059 i64 2, label %b2060 i64 3, label %b2060 ]
b2033:
  %v3285 = phi i64 [ %v3232, %b2046 ], [ %v3284, %b2071 ]
  store i64 %v3285, ptr %gcs0
  %v3286 = call i64 @df.class(i64 %v3285)
  switch i64 %v3286, label %b2086 [ i64 4, label %b2084 i64 2, label %b2085 i64 3, label %b2085 ]
b2034:
  store i64 0, ptr %gcs1
  %v3186 = call i64 @df.box_int(i64 %v3183)
  store i64 %v3186, ptr %gcs1
  %v3187 = call i64 @df.box_int(i64 %v3184)
  store i64 %v3187, ptr %gcs2
  %v3188 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3186, i64 %v3187)
  %v3189 = load i8, ptr %ctx, align 8
  %c713 = zext i8 %v3189 to i64
  %v3190 = icmp ne i64 %c713, 0
  br i1 %v3190, label %b2039, label %b2038
b2035:
  store i64 0, ptr %gcs1
  %v3191 = call i64 @df.box_int(i64 %v3183)
  store i64 %v3191, ptr %gcs1
  %v3192 = call i64 @df.box_int(i64 %v3184)
  store i64 %v3192, ptr %gcs2
  %v3193 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3191, i64 %v3192)
  %v3194 = load i8, ptr %ctx, align 8
  %c714 = zext i8 %v3194 to i64
  %v3195 = icmp ne i64 %c714, 0
  br i1 %v3195, label %b2041, label %b2040
b2036:
  store i64 0, ptr %gcs1
  %v3196 = call i64 @df.box_int(i64 %v3183)
  store i64 %v3196, ptr %gcs1
  %v3197 = call i64 @df.box_int(i64 %v3184)
  store i64 %v3197, ptr %gcs2
  %sa3198_0 = getelementptr [3 x i64], ptr %sargs3198, i64 0, i64 0
  store i64 %v3196, ptr %sa3198_0
  %sa3198_1 = getelementptr [3 x i64], ptr %sargs3198, i64 0, i64 1
  store i64 %v3197, ptr %sa3198_1
  %sat3198 = getelementptr [3 x i64], ptr %sargs3198, i64 0, i64 2
  store i64 0, ptr %sat3198
  %sic3198 = getelementptr i64, ptr %area, i64 316
  %sf3198 = call ptr @df.selector(ptr %sic3198, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3198 = call i64 %sf3198(i64 %v0, ptr %sargs3198, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3199 = load i8, ptr %ctx, align 8
  %c715 = zext i8 %v3199 to i64
  %v3200 = icmp ne i64 %c715, 0
  br i1 %v3200, label %b2043, label %b2042
b2037:
  %v3201 = phi i64 [ %v3188, %b2038 ], [ %v3193, %b2040 ], [ %v3198, %b2042 ]
  store i64 %v3201, ptr %gcs0
  %v3202 = add i64 0, 32768
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3203 = and i64 %v3201, 1
  %v3204 = icmp ne i64 %v3203, 0
  br i1 %v3204, label %b2044, label %b2045
b2038:
  br label %b2037
b2039:
  %gcvolta2039 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2039, ptr %ctxtopo, align 8
  ret i64 0
b2040:
  br label %b2037
b2041:
  %gcvolta2041 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2041, ptr %ctxtopo, align 8
  ret i64 0
b2042:
  br label %b2037
b2043:
  %gcvolta2043 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2043, ptr %ctxtopo, align 8
  ret i64 0
b2044:
  %v3205 = ashr i64 %v3201, 1
  %v3206 = mul i64 %v3205, %v3202
  store i64 0, ptr %gcs0
  %v3207 = call i64 @df.box_int(i64 %v3206)
  br label %b2046
b2045:
  %v3208 = call i64 @df.class(i64 %v3201)
  %v3209 = icmp eq i64 %v3208, 2
  %v3210 = icmp eq i64 %v3208, 3
  %v3211 = icmp eq i64 %v3208, 4
  %c716 = zext i1 %v3209 to i64
  %c717 = zext i1 %v3210 to i64
  %v3212 = or i64 %c716, %c717
  %c718 = zext i1 %v3211 to i64
  %v3213 = or i64 %v3212, %c718
  %c719 = trunc i64 %v3213 to i1
  br i1 %c719, label %b2047, label %b2048
b2046:
  %v3232 = phi i64 [ %v3207, %b2044 ], [ %v3231, %b2049 ]
  br label %b2033
b2047:
  br i1 %v3211, label %b2051, label %b2050
b2048:
  %v3227 = call i64 @df.box_int(i64 %v3202)
  store i64 %v3227, ptr %gcs1
  %sa3228_0 = getelementptr [2 x i64], ptr %sargs3228, i64 0, i64 0
  store i64 %v3227, ptr %sa3228_0
  %sat3228 = getelementptr [2 x i64], ptr %sargs3228, i64 0, i64 1
  store i64 0, ptr %sat3228
  %sic3228 = getelementptr i64, ptr %area, i64 318
  %sf3228 = call ptr @df.selector(ptr %sic3228, i64 %v3201, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3228 = call i64 %sf3228(i64 %v3201, ptr %sargs3228, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3229 = load i8, ptr %ctx, align 8
  %c720 = zext i8 %v3229 to i64
  %v3230 = icmp ne i64 %c720, 0
  br i1 %v3230, label %b2058, label %b2057
b2049:
  %v3231 = phi i64 [ %v3226, %b2052 ], [ %v3228, %b2057 ]
  br label %b2046
b2050:
  %v3215 = call i64 @df.unbox_int(i64 %v3201)
  %v3216 = load i8, ptr %ctx, align 8
  %c721 = zext i8 %v3216 to i64
  %v3217 = icmp ne i64 %c721, 0
  br i1 %v3217, label %b2054, label %b2053
b2051:
  %v3220 = call double @df.num_to_double(i64 %v3201)
  %v3221 = load i8, ptr %ctx, align 8
  %c722 = zext i8 %v3221 to i64
  %v3222 = icmp ne i64 %c722, 0
  br i1 %v3222, label %b2056, label %b2055
b2052:
  %v3226 = phi i64 [ %v3219, %b2053 ], [ %v3225, %b2055 ]
  br label %b2049
b2053:
  %v3218 = mul i64 %v3215, %v3202
  store i64 0, ptr %gcs0
  %v3219 = call i64 @df.box_int(i64 %v3218)
  br label %b2052
b2054:
  %gcvolta2054 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2054, ptr %ctxtopo, align 8
  ret i64 0
b2055:
  %v3223 = sitofp i64 %v3202 to double
  %v3224 = fmul double %v3220, %v3223
  store i64 0, ptr %gcs0
  %v3225 = call i64 @df.box_double(double %v3224)
  br label %b2052
b2056:
  %gcvolta2056 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2056, ptr %ctxtopo, align 8
  ret i64 0
b2057:
  br label %b2049
b2058:
  %gcvolta2058 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2058, ptr %ctxtopo, align 8
  ret i64 0
b2059:
  store i64 0, ptr %gcs1
  %v3238 = call i64 @df.box_int(i64 %v3235)
  store i64 %v3238, ptr %gcs1
  %v3239 = call i64 @df.box_int(i64 %v3236)
  store i64 %v3239, ptr %gcs2
  %v3240 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3238, i64 %v3239)
  %v3241 = load i8, ptr %ctx, align 8
  %c723 = zext i8 %v3241 to i64
  %v3242 = icmp ne i64 %c723, 0
  br i1 %v3242, label %b2064, label %b2063
b2060:
  store i64 0, ptr %gcs1
  %v3243 = call i64 @df.box_int(i64 %v3235)
  store i64 %v3243, ptr %gcs1
  %v3244 = call i64 @df.box_int(i64 %v3236)
  store i64 %v3244, ptr %gcs2
  %v3245 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3243, i64 %v3244)
  %v3246 = load i8, ptr %ctx, align 8
  %c724 = zext i8 %v3246 to i64
  %v3247 = icmp ne i64 %c724, 0
  br i1 %v3247, label %b2066, label %b2065
b2061:
  store i64 0, ptr %gcs1
  %v3248 = call i64 @df.box_int(i64 %v3235)
  store i64 %v3248, ptr %gcs1
  %v3249 = call i64 @df.box_int(i64 %v3236)
  store i64 %v3249, ptr %gcs2
  %sa3250_0 = getelementptr [3 x i64], ptr %sargs3250, i64 0, i64 0
  store i64 %v3248, ptr %sa3250_0
  %sa3250_1 = getelementptr [3 x i64], ptr %sargs3250, i64 0, i64 1
  store i64 %v3249, ptr %sa3250_1
  %sat3250 = getelementptr [3 x i64], ptr %sargs3250, i64 0, i64 2
  store i64 0, ptr %sat3250
  %sic3250 = getelementptr i64, ptr %area, i64 320
  %sf3250 = call ptr @df.selector(ptr %sic3250, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3250 = call i64 %sf3250(i64 %v0, ptr %sargs3250, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3251 = load i8, ptr %ctx, align 8
  %c725 = zext i8 %v3251 to i64
  %v3252 = icmp ne i64 %c725, 0
  br i1 %v3252, label %b2068, label %b2067
b2062:
  %v3253 = phi i64 [ %v3240, %b2063 ], [ %v3245, %b2065 ], [ %v3250, %b2067 ]
  store i64 %v3253, ptr %gcs0
  %v3254 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3255 = and i64 %v3253, 1
  %v3256 = icmp ne i64 %v3255, 0
  br i1 %v3256, label %b2069, label %b2070
b2063:
  br label %b2062
b2064:
  %gcvolta2064 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2064, ptr %ctxtopo, align 8
  ret i64 0
b2065:
  br label %b2062
b2066:
  %gcvolta2066 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2066, ptr %ctxtopo, align 8
  ret i64 0
b2067:
  br label %b2062
b2068:
  %gcvolta2068 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2068, ptr %ctxtopo, align 8
  ret i64 0
b2069:
  %v3257 = ashr i64 %v3253, 1
  %v3258 = mul i64 %v3257, %v3254
  store i64 0, ptr %gcs0
  %v3259 = call i64 @df.box_int(i64 %v3258)
  br label %b2071
b2070:
  %v3260 = call i64 @df.class(i64 %v3253)
  %v3261 = icmp eq i64 %v3260, 2
  %v3262 = icmp eq i64 %v3260, 3
  %v3263 = icmp eq i64 %v3260, 4
  %c726 = zext i1 %v3261 to i64
  %c727 = zext i1 %v3262 to i64
  %v3264 = or i64 %c726, %c727
  %c728 = zext i1 %v3263 to i64
  %v3265 = or i64 %v3264, %c728
  %c729 = trunc i64 %v3265 to i1
  br i1 %c729, label %b2072, label %b2073
b2071:
  %v3284 = phi i64 [ %v3259, %b2069 ], [ %v3283, %b2074 ]
  br label %b2033
b2072:
  br i1 %v3263, label %b2076, label %b2075
b2073:
  %v3279 = call i64 @df.box_int(i64 %v3254)
  store i64 %v3279, ptr %gcs1
  %sa3280_0 = getelementptr [2 x i64], ptr %sargs3280, i64 0, i64 0
  store i64 %v3279, ptr %sa3280_0
  %sat3280 = getelementptr [2 x i64], ptr %sargs3280, i64 0, i64 1
  store i64 0, ptr %sat3280
  %sic3280 = getelementptr i64, ptr %area, i64 322
  %sf3280 = call ptr @df.selector(ptr %sic3280, i64 %v3253, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3280 = call i64 %sf3280(i64 %v3253, ptr %sargs3280, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3281 = load i8, ptr %ctx, align 8
  %c730 = zext i8 %v3281 to i64
  %v3282 = icmp ne i64 %c730, 0
  br i1 %v3282, label %b2083, label %b2082
b2074:
  %v3283 = phi i64 [ %v3278, %b2077 ], [ %v3280, %b2082 ]
  br label %b2071
b2075:
  %v3267 = call i64 @df.unbox_int(i64 %v3253)
  %v3268 = load i8, ptr %ctx, align 8
  %c731 = zext i8 %v3268 to i64
  %v3269 = icmp ne i64 %c731, 0
  br i1 %v3269, label %b2079, label %b2078
b2076:
  %v3272 = call double @df.num_to_double(i64 %v3253)
  %v3273 = load i8, ptr %ctx, align 8
  %c732 = zext i8 %v3273 to i64
  %v3274 = icmp ne i64 %c732, 0
  br i1 %v3274, label %b2081, label %b2080
b2077:
  %v3278 = phi i64 [ %v3271, %b2078 ], [ %v3277, %b2080 ]
  br label %b2074
b2078:
  %v3270 = mul i64 %v3267, %v3254
  store i64 0, ptr %gcs0
  %v3271 = call i64 @df.box_int(i64 %v3270)
  br label %b2077
b2079:
  %gcvolta2079 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2079, ptr %ctxtopo, align 8
  ret i64 0
b2080:
  %v3275 = sitofp i64 %v3254 to double
  %v3276 = fmul double %v3272, %v3275
  store i64 0, ptr %gcs0
  %v3277 = call i64 @df.box_double(double %v3276)
  br label %b2077
b2081:
  %gcvolta2081 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2081, ptr %ctxtopo, align 8
  ret i64 0
b2082:
  br label %b2074
b2083:
  %gcvolta2083 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2083, ptr %ctxtopo, align 8
  ret i64 0
b2084:
  store i64 0, ptr %gcs1
  %v3287 = call double @df.unbox_double(i64 %v3285)
  %v3288 = load i8, ptr %ctx, align 8
  %c733 = zext i8 %v3288 to i64
  %v3289 = icmp ne i64 %c733, 0
  br i1 %v3289, label %b2089, label %b2088
b2085:
  store i64 0, ptr %gcs1
  %v3293 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3285)
  %v3294 = load i8, ptr %ctx, align 8
  %c734 = zext i8 %v3294 to i64
  %v3295 = icmp ne i64 %c734, 0
  br i1 %v3295, label %b2093, label %b2092
b2086:
  store i64 0, ptr %gcs1
  %sat3296 = getelementptr [1 x i64], ptr %sargs3296, i64 0, i64 0
  store i64 0, ptr %sat3296
  %sic3296 = getelementptr i64, ptr %area, i64 324
  %sf3296 = call ptr @df.selector(ptr %sic3296, i64 %v3285, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3296 = call i64 %sf3296(i64 %v3285, ptr %sargs3296, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3296, ptr %gcs0
  %v3297 = load i8, ptr %ctx, align 8
  %c735 = zext i8 %v3297 to i64
  %v3298 = icmp ne i64 %c735, 0
  br i1 %v3298, label %b2095, label %b2094
b2087:
  %v3302 = phi i64 [ %v3290, %b2090 ], [ %v3293, %b2092 ], [ %v3299, %b2096 ]
  store i64 0, ptr %gcs0
  %v3303 = call i64 @df.box_int(i64 %v3302)
  %xcp3304 = load i8, ptr %ctx, align 8
  %xcn3304 = icmp ne i8 %xcp3304, 0
  %xce3304 = call i1 @llvm.expect.i1(i1 %xcn3304, i1 false)
  br i1 %xce3304, label %xc3304.limpar, label %xc3304.fim
xc3304.limpar:
  call void @dartforge_exception_clear()
  br label %xc3304.fim
xc3304.fim:
  %gcvolta2087 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2087, ptr %ctxtopo, align 8
  ret i64 %v3303
b2088:
  store i64 0, ptr %gcs0
  %v3290 = call i64 @dartforge_native_DartForge_double_toInt(double %v3287)
  %v3291 = load i8, ptr %ctx, align 8
  %c736 = zext i8 %v3291 to i64
  %v3292 = icmp ne i64 %c736, 0
  br i1 %v3292, label %b2091, label %b2090
b2089:
  %gcvolta2089 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2089, ptr %ctxtopo, align 8
  ret i64 0
b2090:
  br label %b2087
b2091:
  %gcvolta2091 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2091, ptr %ctxtopo, align 8
  ret i64 0
b2092:
  br label %b2087
b2093:
  %gcvolta2093 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2093, ptr %ctxtopo, align 8
  ret i64 0
b2094:
  %v3299 = call i64 @df.unbox_int(i64 %v3296)
  %v3300 = load i8, ptr %ctx, align 8
  %c737 = zext i8 %v3300 to i64
  %v3301 = icmp ne i64 %c737, 0
  br i1 %v3301, label %b2097, label %b2096
b2095:
  %gcvolta2095 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2095, ptr %ctxtopo, align 8
  ret i64 0
b2096:
  br label %b2087
b2097:
  %gcvolta2097 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2097, ptr %ctxtopo, align 8
  ret i64 0
b2099:
  %v3309 = ashr i64 %v0, 1
  %v3310 = icmp slt i64 %v3309, %v3306
  br label %b2101
b2100:
  %v3311 = call i64 @df.box_int(i64 %v3306)
  store i64 %v3311, ptr %gcs1
  %sa3312_0 = getelementptr [2 x i64], ptr %sargs3312, i64 0, i64 0
  store i64 %v3311, ptr %sa3312_0
  %sat3312 = getelementptr [2 x i64], ptr %sargs3312, i64 0, i64 1
  store i64 0, ptr %sat3312
  %sic3312 = getelementptr i64, ptr %area, i64 326
  %sf3312 = call ptr @df.selector(ptr %sic3312, i64 %v0, i64 -4020809642220582218, ptr @df.seln.15, i64 4)
  %v3312 = call i64 %sf3312(i64 %v0, ptr %sargs3312, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v3312, ptr %gcs1
  %v3313 = load i8, ptr %ctx, align 8
  %c738 = zext i8 %v3313 to i64
  %v3314 = icmp ne i64 %c738, 0
  br i1 %v3314, label %b2103, label %b2102
b2101:
  %v3318 = phi i1 [ %v3310, %b2099 ], [ %v3315, %b2104 ]
  br i1 %v3318, label %b2106, label %b2107
b2102:
  %v3315 = call i1 @df.unbox_bool(i64 %v3312)
  %v3316 = load i8, ptr %ctx, align 8
  %c739 = zext i8 %v3316 to i64
  %v3317 = icmp ne i64 %c739, 0
  br i1 %v3317, label %b2105, label %b2104
b2103:
  %gcvolta2103 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2103, ptr %ctxtopo, align 8
  ret i64 0
b2104:
  br label %b2101
b2105:
  %gcvolta2105 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2105, ptr %ctxtopo, align 8
  ret i64 0
b2106:
  %v3320 = add i64 0, 1
  %v3321 = sub i64 0, %v3320
  %v3322 = add i64 0, 1
  %v3323 = call i64 @df.class(i64 %v0)
  switch i64 %v3323, label %b2111 [ i64 4, label %b2109 i64 2, label %b2110 i64 3, label %b2110 ]
b2107:
  %v3372 = add i64 0, 1
  %v3373 = sub i64 0, %v3372
  %v3374 = add i64 0, 1
  %v3375 = call i64 @df.class(i64 %v0)
  switch i64 %v3375, label %b2136 [ i64 4, label %b2134 i64 2, label %b2135 i64 3, label %b2135 ]
b2108:
  %v3423 = phi i64 [ %v3370, %b2121 ], [ %v3422, %b2146 ]
  store i64 %v3423, ptr %gcs0
  %v3424 = call i64 @df.class(i64 %v3423)
  switch i64 %v3424, label %b2161 [ i64 4, label %b2159 i64 2, label %b2160 i64 3, label %b2160 ]
b2109:
  store i64 0, ptr %gcs1
  %v3324 = call i64 @df.box_int(i64 %v3321)
  store i64 %v3324, ptr %gcs1
  %v3325 = call i64 @df.box_int(i64 %v3322)
  store i64 %v3325, ptr %gcs2
  %v3326 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3324, i64 %v3325)
  %v3327 = load i8, ptr %ctx, align 8
  %c740 = zext i8 %v3327 to i64
  %v3328 = icmp ne i64 %c740, 0
  br i1 %v3328, label %b2114, label %b2113
b2110:
  store i64 0, ptr %gcs1
  %v3329 = call i64 @df.box_int(i64 %v3321)
  store i64 %v3329, ptr %gcs1
  %v3330 = call i64 @df.box_int(i64 %v3322)
  store i64 %v3330, ptr %gcs2
  %v3331 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3329, i64 %v3330)
  %v3332 = load i8, ptr %ctx, align 8
  %c741 = zext i8 %v3332 to i64
  %v3333 = icmp ne i64 %c741, 0
  br i1 %v3333, label %b2116, label %b2115
b2111:
  store i64 0, ptr %gcs1
  %v3334 = call i64 @df.box_int(i64 %v3321)
  store i64 %v3334, ptr %gcs1
  %v3335 = call i64 @df.box_int(i64 %v3322)
  store i64 %v3335, ptr %gcs2
  %sa3336_0 = getelementptr [3 x i64], ptr %sargs3336, i64 0, i64 0
  store i64 %v3334, ptr %sa3336_0
  %sa3336_1 = getelementptr [3 x i64], ptr %sargs3336, i64 0, i64 1
  store i64 %v3335, ptr %sa3336_1
  %sat3336 = getelementptr [3 x i64], ptr %sargs3336, i64 0, i64 2
  store i64 0, ptr %sat3336
  %sic3336 = getelementptr i64, ptr %area, i64 328
  %sf3336 = call ptr @df.selector(ptr %sic3336, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3336 = call i64 %sf3336(i64 %v0, ptr %sargs3336, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3337 = load i8, ptr %ctx, align 8
  %c742 = zext i8 %v3337 to i64
  %v3338 = icmp ne i64 %c742, 0
  br i1 %v3338, label %b2118, label %b2117
b2112:
  %v3339 = phi i64 [ %v3326, %b2113 ], [ %v3331, %b2115 ], [ %v3336, %b2117 ]
  store i64 %v3339, ptr %gcs0
  %v3340 = add i64 0, 2147483648
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3341 = and i64 %v3339, 1
  %v3342 = icmp ne i64 %v3341, 0
  br i1 %v3342, label %b2119, label %b2120
b2113:
  br label %b2112
b2114:
  %gcvolta2114 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2114, ptr %ctxtopo, align 8
  ret i64 0
b2115:
  br label %b2112
b2116:
  %gcvolta2116 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2116, ptr %ctxtopo, align 8
  ret i64 0
b2117:
  br label %b2112
b2118:
  %gcvolta2118 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2118, ptr %ctxtopo, align 8
  ret i64 0
b2119:
  %v3343 = ashr i64 %v3339, 1
  %v3344 = mul i64 %v3343, %v3340
  store i64 0, ptr %gcs0
  %v3345 = call i64 @df.box_int(i64 %v3344)
  br label %b2121
b2120:
  %v3346 = call i64 @df.class(i64 %v3339)
  %v3347 = icmp eq i64 %v3346, 2
  %v3348 = icmp eq i64 %v3346, 3
  %v3349 = icmp eq i64 %v3346, 4
  %c743 = zext i1 %v3347 to i64
  %c744 = zext i1 %v3348 to i64
  %v3350 = or i64 %c743, %c744
  %c745 = zext i1 %v3349 to i64
  %v3351 = or i64 %v3350, %c745
  %c746 = trunc i64 %v3351 to i1
  br i1 %c746, label %b2122, label %b2123
b2121:
  %v3370 = phi i64 [ %v3345, %b2119 ], [ %v3369, %b2124 ]
  br label %b2108
b2122:
  br i1 %v3349, label %b2126, label %b2125
b2123:
  %v3365 = call i64 @df.box_int(i64 %v3340)
  store i64 %v3365, ptr %gcs1
  %sa3366_0 = getelementptr [2 x i64], ptr %sargs3366, i64 0, i64 0
  store i64 %v3365, ptr %sa3366_0
  %sat3366 = getelementptr [2 x i64], ptr %sargs3366, i64 0, i64 1
  store i64 0, ptr %sat3366
  %sic3366 = getelementptr i64, ptr %area, i64 330
  %sf3366 = call ptr @df.selector(ptr %sic3366, i64 %v3339, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3366 = call i64 %sf3366(i64 %v3339, ptr %sargs3366, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3367 = load i8, ptr %ctx, align 8
  %c747 = zext i8 %v3367 to i64
  %v3368 = icmp ne i64 %c747, 0
  br i1 %v3368, label %b2133, label %b2132
b2124:
  %v3369 = phi i64 [ %v3364, %b2127 ], [ %v3366, %b2132 ]
  br label %b2121
b2125:
  %v3353 = call i64 @df.unbox_int(i64 %v3339)
  %v3354 = load i8, ptr %ctx, align 8
  %c748 = zext i8 %v3354 to i64
  %v3355 = icmp ne i64 %c748, 0
  br i1 %v3355, label %b2129, label %b2128
b2126:
  %v3358 = call double @df.num_to_double(i64 %v3339)
  %v3359 = load i8, ptr %ctx, align 8
  %c749 = zext i8 %v3359 to i64
  %v3360 = icmp ne i64 %c749, 0
  br i1 %v3360, label %b2131, label %b2130
b2127:
  %v3364 = phi i64 [ %v3357, %b2128 ], [ %v3363, %b2130 ]
  br label %b2124
b2128:
  %v3356 = mul i64 %v3353, %v3340
  store i64 0, ptr %gcs0
  %v3357 = call i64 @df.box_int(i64 %v3356)
  br label %b2127
b2129:
  %gcvolta2129 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2129, ptr %ctxtopo, align 8
  ret i64 0
b2130:
  %v3361 = sitofp i64 %v3340 to double
  %v3362 = fmul double %v3358, %v3361
  store i64 0, ptr %gcs0
  %v3363 = call i64 @df.box_double(double %v3362)
  br label %b2127
b2131:
  %gcvolta2131 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2131, ptr %ctxtopo, align 8
  ret i64 0
b2132:
  br label %b2124
b2133:
  %gcvolta2133 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2133, ptr %ctxtopo, align 8
  ret i64 0
b2134:
  store i64 0, ptr %gcs1
  %v3376 = call i64 @df.box_int(i64 %v3373)
  store i64 %v3376, ptr %gcs1
  %v3377 = call i64 @df.box_int(i64 %v3374)
  store i64 %v3377, ptr %gcs2
  %v3378 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3376, i64 %v3377)
  %v3379 = load i8, ptr %ctx, align 8
  %c750 = zext i8 %v3379 to i64
  %v3380 = icmp ne i64 %c750, 0
  br i1 %v3380, label %b2139, label %b2138
b2135:
  store i64 0, ptr %gcs1
  %v3381 = call i64 @df.box_int(i64 %v3373)
  store i64 %v3381, ptr %gcs1
  %v3382 = call i64 @df.box_int(i64 %v3374)
  store i64 %v3382, ptr %gcs2
  %v3383 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3381, i64 %v3382)
  %v3384 = load i8, ptr %ctx, align 8
  %c751 = zext i8 %v3384 to i64
  %v3385 = icmp ne i64 %c751, 0
  br i1 %v3385, label %b2141, label %b2140
b2136:
  store i64 0, ptr %gcs1
  %v3386 = call i64 @df.box_int(i64 %v3373)
  store i64 %v3386, ptr %gcs1
  %v3387 = call i64 @df.box_int(i64 %v3374)
  store i64 %v3387, ptr %gcs2
  %sa3388_0 = getelementptr [3 x i64], ptr %sargs3388, i64 0, i64 0
  store i64 %v3386, ptr %sa3388_0
  %sa3388_1 = getelementptr [3 x i64], ptr %sargs3388, i64 0, i64 1
  store i64 %v3387, ptr %sa3388_1
  %sat3388 = getelementptr [3 x i64], ptr %sargs3388, i64 0, i64 2
  store i64 0, ptr %sat3388
  %sic3388 = getelementptr i64, ptr %area, i64 332
  %sf3388 = call ptr @df.selector(ptr %sic3388, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3388 = call i64 %sf3388(i64 %v0, ptr %sargs3388, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3389 = load i8, ptr %ctx, align 8
  %c752 = zext i8 %v3389 to i64
  %v3390 = icmp ne i64 %c752, 0
  br i1 %v3390, label %b2143, label %b2142
b2137:
  %v3391 = phi i64 [ %v3378, %b2138 ], [ %v3383, %b2140 ], [ %v3388, %b2142 ]
  store i64 %v3391, ptr %gcs0
  %v3392 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3393 = and i64 %v3391, 1
  %v3394 = icmp ne i64 %v3393, 0
  br i1 %v3394, label %b2144, label %b2145
b2138:
  br label %b2137
b2139:
  %gcvolta2139 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2139, ptr %ctxtopo, align 8
  ret i64 0
b2140:
  br label %b2137
b2141:
  %gcvolta2141 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2141, ptr %ctxtopo, align 8
  ret i64 0
b2142:
  br label %b2137
b2143:
  %gcvolta2143 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2143, ptr %ctxtopo, align 8
  ret i64 0
b2144:
  %v3395 = ashr i64 %v3391, 1
  %v3396 = mul i64 %v3395, %v3392
  store i64 0, ptr %gcs0
  %v3397 = call i64 @df.box_int(i64 %v3396)
  br label %b2146
b2145:
  %v3398 = call i64 @df.class(i64 %v3391)
  %v3399 = icmp eq i64 %v3398, 2
  %v3400 = icmp eq i64 %v3398, 3
  %v3401 = icmp eq i64 %v3398, 4
  %c753 = zext i1 %v3399 to i64
  %c754 = zext i1 %v3400 to i64
  %v3402 = or i64 %c753, %c754
  %c755 = zext i1 %v3401 to i64
  %v3403 = or i64 %v3402, %c755
  %c756 = trunc i64 %v3403 to i1
  br i1 %c756, label %b2147, label %b2148
b2146:
  %v3422 = phi i64 [ %v3397, %b2144 ], [ %v3421, %b2149 ]
  br label %b2108
b2147:
  br i1 %v3401, label %b2151, label %b2150
b2148:
  %v3417 = call i64 @df.box_int(i64 %v3392)
  store i64 %v3417, ptr %gcs1
  %sa3418_0 = getelementptr [2 x i64], ptr %sargs3418, i64 0, i64 0
  store i64 %v3417, ptr %sa3418_0
  %sat3418 = getelementptr [2 x i64], ptr %sargs3418, i64 0, i64 1
  store i64 0, ptr %sat3418
  %sic3418 = getelementptr i64, ptr %area, i64 334
  %sf3418 = call ptr @df.selector(ptr %sic3418, i64 %v3391, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v3418 = call i64 %sf3418(i64 %v3391, ptr %sargs3418, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3419 = load i8, ptr %ctx, align 8
  %c757 = zext i8 %v3419 to i64
  %v3420 = icmp ne i64 %c757, 0
  br i1 %v3420, label %b2158, label %b2157
b2149:
  %v3421 = phi i64 [ %v3416, %b2152 ], [ %v3418, %b2157 ]
  br label %b2146
b2150:
  %v3405 = call i64 @df.unbox_int(i64 %v3391)
  %v3406 = load i8, ptr %ctx, align 8
  %c758 = zext i8 %v3406 to i64
  %v3407 = icmp ne i64 %c758, 0
  br i1 %v3407, label %b2154, label %b2153
b2151:
  %v3410 = call double @df.num_to_double(i64 %v3391)
  %v3411 = load i8, ptr %ctx, align 8
  %c759 = zext i8 %v3411 to i64
  %v3412 = icmp ne i64 %c759, 0
  br i1 %v3412, label %b2156, label %b2155
b2152:
  %v3416 = phi i64 [ %v3409, %b2153 ], [ %v3415, %b2155 ]
  br label %b2149
b2153:
  %v3408 = mul i64 %v3405, %v3392
  store i64 0, ptr %gcs0
  %v3409 = call i64 @df.box_int(i64 %v3408)
  br label %b2152
b2154:
  %gcvolta2154 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2154, ptr %ctxtopo, align 8
  ret i64 0
b2155:
  %v3413 = sitofp i64 %v3392 to double
  %v3414 = fmul double %v3410, %v3413
  store i64 0, ptr %gcs0
  %v3415 = call i64 @df.box_double(double %v3414)
  br label %b2152
b2156:
  %gcvolta2156 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2156, ptr %ctxtopo, align 8
  ret i64 0
b2157:
  br label %b2149
b2158:
  %gcvolta2158 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2158, ptr %ctxtopo, align 8
  ret i64 0
b2159:
  store i64 0, ptr %gcs1
  %v3425 = call double @df.unbox_double(i64 %v3423)
  %v3426 = load i8, ptr %ctx, align 8
  %c760 = zext i8 %v3426 to i64
  %v3427 = icmp ne i64 %c760, 0
  br i1 %v3427, label %b2164, label %b2163
b2160:
  store i64 0, ptr %gcs1
  %v3431 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3423)
  %v3432 = load i8, ptr %ctx, align 8
  %c761 = zext i8 %v3432 to i64
  %v3433 = icmp ne i64 %c761, 0
  br i1 %v3433, label %b2168, label %b2167
b2161:
  store i64 0, ptr %gcs1
  %sat3434 = getelementptr [1 x i64], ptr %sargs3434, i64 0, i64 0
  store i64 0, ptr %sat3434
  %sic3434 = getelementptr i64, ptr %area, i64 336
  %sf3434 = call ptr @df.selector(ptr %sic3434, i64 %v3423, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3434 = call i64 %sf3434(i64 %v3423, ptr %sargs3434, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3434, ptr %gcs0
  %v3435 = load i8, ptr %ctx, align 8
  %c762 = zext i8 %v3435 to i64
  %v3436 = icmp ne i64 %c762, 0
  br i1 %v3436, label %b2170, label %b2169
b2162:
  %v3440 = phi i64 [ %v3428, %b2165 ], [ %v3431, %b2167 ], [ %v3437, %b2171 ]
  store i64 0, ptr %gcs0
  %v3441 = call i64 @df.box_int(i64 %v3440)
  %xcp3442 = load i8, ptr %ctx, align 8
  %xcn3442 = icmp ne i8 %xcp3442, 0
  %xce3442 = call i1 @llvm.expect.i1(i1 %xcn3442, i1 false)
  br i1 %xce3442, label %xc3442.limpar, label %xc3442.fim
xc3442.limpar:
  call void @dartforge_exception_clear()
  br label %xc3442.fim
xc3442.fim:
  %gcvolta2162 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2162, ptr %ctxtopo, align 8
  ret i64 %v3441
b2163:
  store i64 0, ptr %gcs0
  %v3428 = call i64 @dartforge_native_DartForge_double_toInt(double %v3425)
  %v3429 = load i8, ptr %ctx, align 8
  %c763 = zext i8 %v3429 to i64
  %v3430 = icmp ne i64 %c763, 0
  br i1 %v3430, label %b2166, label %b2165
b2164:
  %gcvolta2164 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2164, ptr %ctxtopo, align 8
  ret i64 0
b2165:
  br label %b2162
b2166:
  %gcvolta2166 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2166, ptr %ctxtopo, align 8
  ret i64 0
b2167:
  br label %b2162
b2168:
  %gcvolta2168 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2168, ptr %ctxtopo, align 8
  ret i64 0
b2169:
  %v3437 = call i64 @df.unbox_int(i64 %v3434)
  %v3438 = load i8, ptr %ctx, align 8
  %c764 = zext i8 %v3438 to i64
  %v3439 = icmp ne i64 %c764, 0
  br i1 %v3439, label %b2172, label %b2171
b2170:
  %gcvolta2170 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2170, ptr %ctxtopo, align 8
  ret i64 0
b2171:
  br label %b2162
b2172:
  %gcvolta2172 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2172, ptr %ctxtopo, align 8
  ret i64 0
}
