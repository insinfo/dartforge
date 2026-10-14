define i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..convertFormatValue(i64 %v0, i64 %v3, i64 %v6) nounwind {
b0:
  %sargs16 = alloca [2 x i64]
  %sargs93 = alloca [2 x i64]
  %sargs106 = alloca [2 x i64]
  %sargs414 = alloca [2 x i64]
  %sargs186 = alloca [2 x i64]
  %sargs225 = alloca [2 x i64]
  %sargs255 = alloca [2 x i64]
  %sargs285 = alloca [2 x i64]
  %sargs315 = alloca [2 x i64]
  %sargs345 = alloca [2 x i64]
  %sargs375 = alloca [2 x i64]
  %sargs405 = alloca [2 x i64]
  %sargs711 = alloca [2 x i64]
  %sargs490 = alloca [2 x i64]
  %sargs552 = alloca [2 x i64]
  %sargs582 = alloca [2 x i64]
  %sargs612 = alloca [2 x i64]
  %sargs642 = alloca [2 x i64]
  %sargs672 = alloca [2 x i64]
  %sargs702 = alloca [2 x i64]
  %sargs994 = alloca [2 x i64]
  %sargs787 = alloca [2 x i64]
  %sargs872 = alloca [2 x i64]
  %sargs902 = alloca [2 x i64]
  %sargs955 = alloca [2 x i64]
  %sargs985 = alloca [2 x i64]
  %sargs1256 = alloca [2 x i64]
  %sargs1070 = alloca [2 x i64]
  %sargs1247 = alloca [2 x i64]
  %sargs1511 = alloca [2 x i64]
  %sargs1332 = alloca [2 x i64]
  %sargs1860 = alloca [2 x i64]
  %sargs1587 = alloca [2 x i64]
  %sargs1607 = alloca [2 x i64]
  %sargs1646 = alloca [2 x i64]
  %sargs1685 = alloca [2 x i64]
  %sargs1724 = alloca [2 x i64]
  %sargs1763 = alloca [2 x i64]
  %sargs1821 = alloca [2 x i64]
  %sargs1851 = alloca [2 x i64]
  %sargs2195 = alloca [2 x i64]
  %sargs1936 = alloca [2 x i64]
  %sargs1956 = alloca [2 x i64]
  %sargs1995 = alloca [2 x i64]
  %sargs2034 = alloca [2 x i64]
  %sargs2073 = alloca [2 x i64]
  %sargs2112 = alloca [2 x i64]
  %sargs2530 = alloca [2 x i64]
  %sargs2271 = alloca [2 x i64]
  %sargs2291 = alloca [2 x i64]
  %sargs2330 = alloca [2 x i64]
  %sargs2369 = alloca [2 x i64]
  %sargs2408 = alloca [2 x i64]
  %sargs2447 = alloca [2 x i64]
  %sargs2606 = alloca [2 x i64]
  %sargs2635 = alloca [3 x i64]
  %sargs2662 = alloca [2 x i64]
  %sargs2677 = alloca [1 x i64]
  %sargs2702 = alloca [3 x i64]
  %sargs2729 = alloca [2 x i64]
  %sargs2744 = alloca [1 x i64]
  %sargs2769 = alloca [3 x i64]
  %sargs2796 = alloca [2 x i64]
  %sargs2811 = alloca [1 x i64]
  %sargs2836 = alloca [3 x i64]
  %sargs2863 = alloca [2 x i64]
  %sargs2878 = alloca [1 x i64]
  %sargs2903 = alloca [3 x i64]
  %sargs2930 = alloca [2 x i64]
  %sargs2945 = alloca [1 x i64]
  %sargs2961 = alloca [2 x i64]
  %sargs2985 = alloca [3 x i64]
  %sargs3012 = alloca [2 x i64]
  %sargs3034 = alloca [3 x i64]
  %sargs3061 = alloca [2 x i64]
  %sargs3077 = alloca [1 x i64]
  %sargs3093 = alloca [2 x i64]
  %sargs3117 = alloca [3 x i64]
  %sargs3144 = alloca [2 x i64]
  %sargs3166 = alloca [3 x i64]
  %sargs3193 = alloca [2 x i64]
  %sargs3209 = alloca [1 x i64]
  %sargs3225 = alloca [2 x i64]
  %sargs3249 = alloca [3 x i64]
  %sargs3276 = alloca [2 x i64]
  %sargs3298 = alloca [3 x i64]
  %sargs3325 = alloca [2 x i64]
  %sargs3341 = alloca [1 x i64]
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
  %v2539 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v2539, ptr %gcs1
  %v2540 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v2540 to i64
  %v2541 = icmp ne i64 %c5, 0
  br i1 %v2541, label %b1579, label %b1578
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
  %v423 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v423, ptr %gcs1
  %v424 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v424 to i64
  %v425 = icmp ne i64 %c12, 0
  br i1 %v425, label %b289, label %b288
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
  %v720 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v720, ptr %gcs1
  %v721 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v721 to i64
  %v722 = icmp ne i64 %c14, 0
  br i1 %v722, label %b456, label %b455
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
  %v1003 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1003, ptr %gcs1
  %v1004 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v1004 to i64
  %v1005 = icmp ne i64 %c16, 0
  br i1 %v1005, label %b621, label %b620
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
  %v1265 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1265, ptr %gcs1
  %v1266 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v1266 to i64
  %v1267 = icmp ne i64 %c18, 0
  br i1 %v1267, label %b783, label %b782
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
  %v1520 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1520, ptr %gcs1
  %v1521 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v1521 to i64
  %v1522 = icmp ne i64 %c20, 0
  br i1 %v1522, label %b944, label %b943
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
  %v1869 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1869, ptr %gcs1
  %v1870 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v1870 to i64
  %v1871 = icmp ne i64 %c22, 0
  br i1 %v1871, label %b1157, label %b1156
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
  %v2204 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v2204, ptr %gcs1
  %v2205 = load i8, ptr %ctx, align 8
  %c24 = zext i8 %v2205 to i64
  %v2206 = icmp ne i64 %c24, 0
  br i1 %v2206, label %b1368, label %b1367
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
  %v412 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v413 = call i64 @df.box_int(i64 %v412)
  store i64 %v413, ptr %gcs1
  %sa414_0 = getelementptr [2 x i64], ptr %sargs414, i64 0, i64 0
  store i64 %v413, ptr %sa414_0
  %sat414 = getelementptr [2 x i64], ptr %sargs414, i64 0, i64 1
  store i64 0, ptr %sat414
  %sic414 = getelementptr i64, ptr %area, i64 168
  %sf414 = call ptr @df.selector(ptr %sic414, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v414 = call i64 %sf414(i64 %v0, ptr %sargs414, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v414, ptr %gcs0
  %v415 = load i8, ptr %ctx, align 8
  %c36 = zext i8 %v415 to i64
  %v416 = icmp ne i64 %c36, 0
  br i1 %v416, label %b270, label %b269
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
  %v232 = add i64 0, 75
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v233 = and i64 %v0, 1
  %v234 = icmp ne i64 %v233, 0
  br i1 %v234, label %b203, label %b204
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
  %v262 = add i64 0, 21845
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v263 = and i64 %v0, 1
  %v264 = icmp ne i64 %v263, 0
  br i1 %v264, label %b214, label %b215
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
  %v292 = add i64 0, 1431655765
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v293 = and i64 %v0, 1
  %v294 = icmp ne i64 %v293, 0
  br i1 %v294, label %b225, label %b226
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
  %v322 = add i64 0, 42
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v323 = and i64 %v0, 1
  %v324 = icmp ne i64 %v323, 0
  br i1 %v324, label %b236, label %b237
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
  %v352 = add i64 0, 10922
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v353 = and i64 %v0, 1
  %v354 = icmp ne i64 %v353, 0
  br i1 %v354, label %b247, label %b248
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
  %v382 = add i64 0, 715827882
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v383 = and i64 %v0, 1
  %v384 = icmp ne i64 %v383, 0
  br i1 %v384, label %b258, label %b259
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
  %v208 = call i64 @df.box_int(i64 %v202)
  store i64 %v208, ptr %gcs1
  %v209 = call i64 @df.class(i64 %v0)
  %v210 = icmp eq i64 %v209, 2
  %v211 = icmp eq i64 %v209, 3
  %c56 = zext i1 %v210 to i64
  %c57 = zext i1 %v211 to i64
  %v212 = or i64 %c56, %c57
  %v213 = icmp eq i64 %v209, 4
  %c58 = zext i1 %v213 to i64
  %v214 = or i64 %v212, %c58
  %v215 = call i64 @df.class(i64 %v208)
  %v216 = icmp eq i64 %v215, 2
  %v217 = icmp eq i64 %v215, 3
  %c59 = zext i1 %v216 to i64
  %c60 = zext i1 %v217 to i64
  %v218 = or i64 %c59, %c60
  %v219 = icmp eq i64 %v215, 4
  %c61 = zext i1 %v219 to i64
  %v220 = or i64 %v218, %c61
  %v221 = and i64 %v214, %v220
  %c62 = trunc i64 %v221 to i1
  br i1 %c62, label %b195, label %b196
b194:
  %v229 = phi i64 [ %v207, %b192 ], [ %v228, %b197 ]
  %xcp230 = load i8, ptr %ctx, align 8
  %xcn230 = icmp ne i8 %xcp230, 0
  %xce230 = call i1 @llvm.expect.i1(i1 %xcn230, i1 false)
  br i1 %xce230, label %xc230.limpar, label %xc230.fim
xc230.limpar:
  call void @dartforge_exception_clear()
  br label %xc230.fim
xc230.fim:
  %gcvolta194 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta194, ptr %ctxtopo, align 8
  ret i64 %v229
b195:
  %v222 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v208)
  %v223 = load i8, ptr %ctx, align 8
  %c63 = zext i8 %v223 to i64
  %v224 = icmp ne i64 %c63, 0
  br i1 %v224, label %b199, label %b198
b196:
  %sa225_0 = getelementptr [2 x i64], ptr %sargs225, i64 0, i64 0
  store i64 %v208, ptr %sa225_0
  %sat225 = getelementptr [2 x i64], ptr %sargs225, i64 0, i64 1
  store i64 0, ptr %sat225
  %sic225 = getelementptr i64, ptr %area, i64 172
  %sf225 = call ptr @df.selector(ptr %sic225, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v225 = call i64 %sf225(i64 %v0, ptr %sargs225, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v226 = load i8, ptr %ctx, align 8
  %c64 = zext i8 %v226 to i64
  %v227 = icmp ne i64 %c64, 0
  br i1 %v227, label %b201, label %b200
b197:
  %v228 = phi i64 [ %v222, %b198 ], [ %v225, %b200 ]
  br label %b194
b198:
  br label %b197
b199:
  %gcvolta199 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta199, ptr %ctxtopo, align 8
  ret i64 0
b200:
  br label %b197
b201:
  %gcvolta201 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta201, ptr %ctxtopo, align 8
  ret i64 0
b203:
  %v235 = ashr i64 %v0, 1
  %v236 = mul i64 %v235, %v232
  store i64 0, ptr %gcs0
  %v237 = call i64 @df.box_int(i64 %v236)
  br label %b205
b204:
  %v238 = call i64 @df.box_int(i64 %v232)
  store i64 %v238, ptr %gcs1
  %v239 = call i64 @df.class(i64 %v0)
  %v240 = icmp eq i64 %v239, 2
  %v241 = icmp eq i64 %v239, 3
  %c65 = zext i1 %v240 to i64
  %c66 = zext i1 %v241 to i64
  %v242 = or i64 %c65, %c66
  %v243 = icmp eq i64 %v239, 4
  %c67 = zext i1 %v243 to i64
  %v244 = or i64 %v242, %c67
  %v245 = call i64 @df.class(i64 %v238)
  %v246 = icmp eq i64 %v245, 2
  %v247 = icmp eq i64 %v245, 3
  %c68 = zext i1 %v246 to i64
  %c69 = zext i1 %v247 to i64
  %v248 = or i64 %c68, %c69
  %v249 = icmp eq i64 %v245, 4
  %c70 = zext i1 %v249 to i64
  %v250 = or i64 %v248, %c70
  %v251 = and i64 %v244, %v250
  %c71 = trunc i64 %v251 to i1
  br i1 %c71, label %b206, label %b207
b205:
  %v259 = phi i64 [ %v237, %b203 ], [ %v258, %b208 ]
  %xcp260 = load i8, ptr %ctx, align 8
  %xcn260 = icmp ne i8 %xcp260, 0
  %xce260 = call i1 @llvm.expect.i1(i1 %xcn260, i1 false)
  br i1 %xce260, label %xc260.limpar, label %xc260.fim
xc260.limpar:
  call void @dartforge_exception_clear()
  br label %xc260.fim
xc260.fim:
  %gcvolta205 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta205, ptr %ctxtopo, align 8
  ret i64 %v259
b206:
  %v252 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v238)
  %v253 = load i8, ptr %ctx, align 8
  %c72 = zext i8 %v253 to i64
  %v254 = icmp ne i64 %c72, 0
  br i1 %v254, label %b210, label %b209
b207:
  %sa255_0 = getelementptr [2 x i64], ptr %sargs255, i64 0, i64 0
  store i64 %v238, ptr %sa255_0
  %sat255 = getelementptr [2 x i64], ptr %sargs255, i64 0, i64 1
  store i64 0, ptr %sat255
  %sic255 = getelementptr i64, ptr %area, i64 174
  %sf255 = call ptr @df.selector(ptr %sic255, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v255 = call i64 %sf255(i64 %v0, ptr %sargs255, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v256 = load i8, ptr %ctx, align 8
  %c73 = zext i8 %v256 to i64
  %v257 = icmp ne i64 %c73, 0
  br i1 %v257, label %b212, label %b211
b208:
  %v258 = phi i64 [ %v252, %b209 ], [ %v255, %b211 ]
  br label %b205
b209:
  br label %b208
b210:
  %gcvolta210 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta210, ptr %ctxtopo, align 8
  ret i64 0
b211:
  br label %b208
b212:
  %gcvolta212 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta212, ptr %ctxtopo, align 8
  ret i64 0
b214:
  %v265 = ashr i64 %v0, 1
  %v266 = mul i64 %v265, %v262
  store i64 0, ptr %gcs0
  %v267 = call i64 @df.box_int(i64 %v266)
  br label %b216
b215:
  %v268 = call i64 @df.box_int(i64 %v262)
  store i64 %v268, ptr %gcs1
  %v269 = call i64 @df.class(i64 %v0)
  %v270 = icmp eq i64 %v269, 2
  %v271 = icmp eq i64 %v269, 3
  %c74 = zext i1 %v270 to i64
  %c75 = zext i1 %v271 to i64
  %v272 = or i64 %c74, %c75
  %v273 = icmp eq i64 %v269, 4
  %c76 = zext i1 %v273 to i64
  %v274 = or i64 %v272, %c76
  %v275 = call i64 @df.class(i64 %v268)
  %v276 = icmp eq i64 %v275, 2
  %v277 = icmp eq i64 %v275, 3
  %c77 = zext i1 %v276 to i64
  %c78 = zext i1 %v277 to i64
  %v278 = or i64 %c77, %c78
  %v279 = icmp eq i64 %v275, 4
  %c79 = zext i1 %v279 to i64
  %v280 = or i64 %v278, %c79
  %v281 = and i64 %v274, %v280
  %c80 = trunc i64 %v281 to i1
  br i1 %c80, label %b217, label %b218
b216:
  %v289 = phi i64 [ %v267, %b214 ], [ %v288, %b219 ]
  %xcp290 = load i8, ptr %ctx, align 8
  %xcn290 = icmp ne i8 %xcp290, 0
  %xce290 = call i1 @llvm.expect.i1(i1 %xcn290, i1 false)
  br i1 %xce290, label %xc290.limpar, label %xc290.fim
xc290.limpar:
  call void @dartforge_exception_clear()
  br label %xc290.fim
xc290.fim:
  %gcvolta216 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta216, ptr %ctxtopo, align 8
  ret i64 %v289
b217:
  %v282 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v268)
  %v283 = load i8, ptr %ctx, align 8
  %c81 = zext i8 %v283 to i64
  %v284 = icmp ne i64 %c81, 0
  br i1 %v284, label %b221, label %b220
b218:
  %sa285_0 = getelementptr [2 x i64], ptr %sargs285, i64 0, i64 0
  store i64 %v268, ptr %sa285_0
  %sat285 = getelementptr [2 x i64], ptr %sargs285, i64 0, i64 1
  store i64 0, ptr %sat285
  %sic285 = getelementptr i64, ptr %area, i64 176
  %sf285 = call ptr @df.selector(ptr %sic285, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v285 = call i64 %sf285(i64 %v0, ptr %sargs285, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v286 = load i8, ptr %ctx, align 8
  %c82 = zext i8 %v286 to i64
  %v287 = icmp ne i64 %c82, 0
  br i1 %v287, label %b223, label %b222
b219:
  %v288 = phi i64 [ %v282, %b220 ], [ %v285, %b222 ]
  br label %b216
b220:
  br label %b219
b221:
  %gcvolta221 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta221, ptr %ctxtopo, align 8
  ret i64 0
b222:
  br label %b219
b223:
  %gcvolta223 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta223, ptr %ctxtopo, align 8
  ret i64 0
b225:
  %v295 = ashr i64 %v0, 1
  %v296 = mul i64 %v295, %v292
  store i64 0, ptr %gcs0
  %v297 = call i64 @df.box_int(i64 %v296)
  br label %b227
b226:
  %v298 = call i64 @df.box_int(i64 %v292)
  store i64 %v298, ptr %gcs1
  %v299 = call i64 @df.class(i64 %v0)
  %v300 = icmp eq i64 %v299, 2
  %v301 = icmp eq i64 %v299, 3
  %c83 = zext i1 %v300 to i64
  %c84 = zext i1 %v301 to i64
  %v302 = or i64 %c83, %c84
  %v303 = icmp eq i64 %v299, 4
  %c85 = zext i1 %v303 to i64
  %v304 = or i64 %v302, %c85
  %v305 = call i64 @df.class(i64 %v298)
  %v306 = icmp eq i64 %v305, 2
  %v307 = icmp eq i64 %v305, 3
  %c86 = zext i1 %v306 to i64
  %c87 = zext i1 %v307 to i64
  %v308 = or i64 %c86, %c87
  %v309 = icmp eq i64 %v305, 4
  %c88 = zext i1 %v309 to i64
  %v310 = or i64 %v308, %c88
  %v311 = and i64 %v304, %v310
  %c89 = trunc i64 %v311 to i1
  br i1 %c89, label %b228, label %b229
b227:
  %v319 = phi i64 [ %v297, %b225 ], [ %v318, %b230 ]
  %xcp320 = load i8, ptr %ctx, align 8
  %xcn320 = icmp ne i8 %xcp320, 0
  %xce320 = call i1 @llvm.expect.i1(i1 %xcn320, i1 false)
  br i1 %xce320, label %xc320.limpar, label %xc320.fim
xc320.limpar:
  call void @dartforge_exception_clear()
  br label %xc320.fim
xc320.fim:
  %gcvolta227 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta227, ptr %ctxtopo, align 8
  ret i64 %v319
b228:
  %v312 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v298)
  %v313 = load i8, ptr %ctx, align 8
  %c90 = zext i8 %v313 to i64
  %v314 = icmp ne i64 %c90, 0
  br i1 %v314, label %b232, label %b231
b229:
  %sa315_0 = getelementptr [2 x i64], ptr %sargs315, i64 0, i64 0
  store i64 %v298, ptr %sa315_0
  %sat315 = getelementptr [2 x i64], ptr %sargs315, i64 0, i64 1
  store i64 0, ptr %sat315
  %sic315 = getelementptr i64, ptr %area, i64 178
  %sf315 = call ptr @df.selector(ptr %sic315, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v315 = call i64 %sf315(i64 %v0, ptr %sargs315, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v316 = load i8, ptr %ctx, align 8
  %c91 = zext i8 %v316 to i64
  %v317 = icmp ne i64 %c91, 0
  br i1 %v317, label %b234, label %b233
b230:
  %v318 = phi i64 [ %v312, %b231 ], [ %v315, %b233 ]
  br label %b227
b231:
  br label %b230
b232:
  %gcvolta232 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta232, ptr %ctxtopo, align 8
  ret i64 0
b233:
  br label %b230
b234:
  %gcvolta234 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta234, ptr %ctxtopo, align 8
  ret i64 0
b236:
  %v325 = ashr i64 %v0, 1
  %v326 = mul i64 %v325, %v322
  store i64 0, ptr %gcs0
  %v327 = call i64 @df.box_int(i64 %v326)
  br label %b238
b237:
  %v328 = call i64 @df.box_int(i64 %v322)
  store i64 %v328, ptr %gcs1
  %v329 = call i64 @df.class(i64 %v0)
  %v330 = icmp eq i64 %v329, 2
  %v331 = icmp eq i64 %v329, 3
  %c92 = zext i1 %v330 to i64
  %c93 = zext i1 %v331 to i64
  %v332 = or i64 %c92, %c93
  %v333 = icmp eq i64 %v329, 4
  %c94 = zext i1 %v333 to i64
  %v334 = or i64 %v332, %c94
  %v335 = call i64 @df.class(i64 %v328)
  %v336 = icmp eq i64 %v335, 2
  %v337 = icmp eq i64 %v335, 3
  %c95 = zext i1 %v336 to i64
  %c96 = zext i1 %v337 to i64
  %v338 = or i64 %c95, %c96
  %v339 = icmp eq i64 %v335, 4
  %c97 = zext i1 %v339 to i64
  %v340 = or i64 %v338, %c97
  %v341 = and i64 %v334, %v340
  %c98 = trunc i64 %v341 to i1
  br i1 %c98, label %b239, label %b240
b238:
  %v349 = phi i64 [ %v327, %b236 ], [ %v348, %b241 ]
  %xcp350 = load i8, ptr %ctx, align 8
  %xcn350 = icmp ne i8 %xcp350, 0
  %xce350 = call i1 @llvm.expect.i1(i1 %xcn350, i1 false)
  br i1 %xce350, label %xc350.limpar, label %xc350.fim
xc350.limpar:
  call void @dartforge_exception_clear()
  br label %xc350.fim
xc350.fim:
  %gcvolta238 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta238, ptr %ctxtopo, align 8
  ret i64 %v349
b239:
  %v342 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v328)
  %v343 = load i8, ptr %ctx, align 8
  %c99 = zext i8 %v343 to i64
  %v344 = icmp ne i64 %c99, 0
  br i1 %v344, label %b243, label %b242
b240:
  %sa345_0 = getelementptr [2 x i64], ptr %sargs345, i64 0, i64 0
  store i64 %v328, ptr %sa345_0
  %sat345 = getelementptr [2 x i64], ptr %sargs345, i64 0, i64 1
  store i64 0, ptr %sat345
  %sic345 = getelementptr i64, ptr %area, i64 180
  %sf345 = call ptr @df.selector(ptr %sic345, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v345 = call i64 %sf345(i64 %v0, ptr %sargs345, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v346 = load i8, ptr %ctx, align 8
  %c100 = zext i8 %v346 to i64
  %v347 = icmp ne i64 %c100, 0
  br i1 %v347, label %b245, label %b244
b241:
  %v348 = phi i64 [ %v342, %b242 ], [ %v345, %b244 ]
  br label %b238
b242:
  br label %b241
b243:
  %gcvolta243 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta243, ptr %ctxtopo, align 8
  ret i64 0
b244:
  br label %b241
b245:
  %gcvolta245 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta245, ptr %ctxtopo, align 8
  ret i64 0
b247:
  %v355 = ashr i64 %v0, 1
  %v356 = mul i64 %v355, %v352
  store i64 0, ptr %gcs0
  %v357 = call i64 @df.box_int(i64 %v356)
  br label %b249
b248:
  %v358 = call i64 @df.box_int(i64 %v352)
  store i64 %v358, ptr %gcs1
  %v359 = call i64 @df.class(i64 %v0)
  %v360 = icmp eq i64 %v359, 2
  %v361 = icmp eq i64 %v359, 3
  %c101 = zext i1 %v360 to i64
  %c102 = zext i1 %v361 to i64
  %v362 = or i64 %c101, %c102
  %v363 = icmp eq i64 %v359, 4
  %c103 = zext i1 %v363 to i64
  %v364 = or i64 %v362, %c103
  %v365 = call i64 @df.class(i64 %v358)
  %v366 = icmp eq i64 %v365, 2
  %v367 = icmp eq i64 %v365, 3
  %c104 = zext i1 %v366 to i64
  %c105 = zext i1 %v367 to i64
  %v368 = or i64 %c104, %c105
  %v369 = icmp eq i64 %v365, 4
  %c106 = zext i1 %v369 to i64
  %v370 = or i64 %v368, %c106
  %v371 = and i64 %v364, %v370
  %c107 = trunc i64 %v371 to i1
  br i1 %c107, label %b250, label %b251
b249:
  %v379 = phi i64 [ %v357, %b247 ], [ %v378, %b252 ]
  %xcp380 = load i8, ptr %ctx, align 8
  %xcn380 = icmp ne i8 %xcp380, 0
  %xce380 = call i1 @llvm.expect.i1(i1 %xcn380, i1 false)
  br i1 %xce380, label %xc380.limpar, label %xc380.fim
xc380.limpar:
  call void @dartforge_exception_clear()
  br label %xc380.fim
xc380.fim:
  %gcvolta249 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta249, ptr %ctxtopo, align 8
  ret i64 %v379
b250:
  %v372 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v358)
  %v373 = load i8, ptr %ctx, align 8
  %c108 = zext i8 %v373 to i64
  %v374 = icmp ne i64 %c108, 0
  br i1 %v374, label %b254, label %b253
b251:
  %sa375_0 = getelementptr [2 x i64], ptr %sargs375, i64 0, i64 0
  store i64 %v358, ptr %sa375_0
  %sat375 = getelementptr [2 x i64], ptr %sargs375, i64 0, i64 1
  store i64 0, ptr %sat375
  %sic375 = getelementptr i64, ptr %area, i64 182
  %sf375 = call ptr @df.selector(ptr %sic375, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v375 = call i64 %sf375(i64 %v0, ptr %sargs375, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v376 = load i8, ptr %ctx, align 8
  %c109 = zext i8 %v376 to i64
  %v377 = icmp ne i64 %c109, 0
  br i1 %v377, label %b256, label %b255
b252:
  %v378 = phi i64 [ %v372, %b253 ], [ %v375, %b255 ]
  br label %b249
b253:
  br label %b252
b254:
  %gcvolta254 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta254, ptr %ctxtopo, align 8
  ret i64 0
b255:
  br label %b252
b256:
  %gcvolta256 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta256, ptr %ctxtopo, align 8
  ret i64 0
b258:
  %v385 = ashr i64 %v0, 1
  %v386 = mul i64 %v385, %v382
  store i64 0, ptr %gcs0
  %v387 = call i64 @df.box_int(i64 %v386)
  br label %b260
b259:
  %v388 = call i64 @df.box_int(i64 %v382)
  store i64 %v388, ptr %gcs1
  %v389 = call i64 @df.class(i64 %v0)
  %v390 = icmp eq i64 %v389, 2
  %v391 = icmp eq i64 %v389, 3
  %c110 = zext i1 %v390 to i64
  %c111 = zext i1 %v391 to i64
  %v392 = or i64 %c110, %c111
  %v393 = icmp eq i64 %v389, 4
  %c112 = zext i1 %v393 to i64
  %v394 = or i64 %v392, %c112
  %v395 = call i64 @df.class(i64 %v388)
  %v396 = icmp eq i64 %v395, 2
  %v397 = icmp eq i64 %v395, 3
  %c113 = zext i1 %v396 to i64
  %c114 = zext i1 %v397 to i64
  %v398 = or i64 %c113, %c114
  %v399 = icmp eq i64 %v395, 4
  %c115 = zext i1 %v399 to i64
  %v400 = or i64 %v398, %c115
  %v401 = and i64 %v394, %v400
  %c116 = trunc i64 %v401 to i1
  br i1 %c116, label %b261, label %b262
b260:
  %v409 = phi i64 [ %v387, %b258 ], [ %v408, %b263 ]
  %xcp410 = load i8, ptr %ctx, align 8
  %xcn410 = icmp ne i8 %xcp410, 0
  %xce410 = call i1 @llvm.expect.i1(i1 %xcn410, i1 false)
  br i1 %xce410, label %xc410.limpar, label %xc410.fim
xc410.limpar:
  call void @dartforge_exception_clear()
  br label %xc410.fim
xc410.fim:
  %gcvolta260 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta260, ptr %ctxtopo, align 8
  ret i64 %v409
b261:
  %v402 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v388)
  %v403 = load i8, ptr %ctx, align 8
  %c117 = zext i8 %v403 to i64
  %v404 = icmp ne i64 %c117, 0
  br i1 %v404, label %b265, label %b264
b262:
  %sa405_0 = getelementptr [2 x i64], ptr %sargs405, i64 0, i64 0
  store i64 %v388, ptr %sa405_0
  %sat405 = getelementptr [2 x i64], ptr %sargs405, i64 0, i64 1
  store i64 0, ptr %sat405
  %sic405 = getelementptr i64, ptr %area, i64 184
  %sf405 = call ptr @df.selector(ptr %sic405, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v405 = call i64 %sf405(i64 %v0, ptr %sargs405, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v406 = load i8, ptr %ctx, align 8
  %c118 = zext i8 %v406 to i64
  %v407 = icmp ne i64 %c118, 0
  br i1 %v407, label %b267, label %b266
b263:
  %v408 = phi i64 [ %v402, %b264 ], [ %v405, %b266 ]
  br label %b260
b264:
  br label %b263
b265:
  %gcvolta265 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta265, ptr %ctxtopo, align 8
  ret i64 0
b266:
  br label %b263
b267:
  %gcvolta267 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta267, ptr %ctxtopo, align 8
  ret i64 0
b269:
  store i64 0, ptr %gcs1
  %v417 = call double @df.unbox_double(i64 %v414)
  %v418 = load i8, ptr %ctx, align 8
  %c119 = zext i8 %v418 to i64
  %v419 = icmp ne i64 %c119, 0
  br i1 %v419, label %b272, label %b271
b270:
  %gcvolta270 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta270, ptr %ctxtopo, align 8
  ret i64 0
b271:
  %xcp421 = load i8, ptr %ctx, align 8
  %xcn421 = icmp ne i8 %xcp421, 0
  %xce421 = call i1 @llvm.expect.i1(i1 %xcn421, i1 false)
  br i1 %xce421, label %xc421.limpar, label %xc421.fim
xc421.limpar:
  call void @dartforge_exception_clear()
  br label %xc421.fim
xc421.fim:
  %gcvolta271 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta271, ptr %ctxtopo, align 8
  ret i64 %v414
b272:
  %gcvolta272 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta272, ptr %ctxtopo, align 8
  ret i64 0
b286:
  %v709 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v710 = call i64 @df.box_int(i64 %v709)
  store i64 %v710, ptr %gcs1
  %sa711_0 = getelementptr [2 x i64], ptr %sargs711, i64 0, i64 0
  store i64 %v710, ptr %sa711_0
  %sat711 = getelementptr [2 x i64], ptr %sargs711, i64 0, i64 1
  store i64 0, ptr %sat711
  %sic711 = getelementptr i64, ptr %area, i64 186
  %sf711 = call ptr @df.selector(ptr %sic711, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v711 = call i64 %sf711(i64 %v0, ptr %sargs711, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v711, ptr %gcs0
  %v712 = load i8, ptr %ctx, align 8
  %c120 = zext i8 %v712 to i64
  %v713 = icmp ne i64 %c120, 0
  br i1 %v713, label %b437, label %b436
b287:
  store i64 0, ptr %gcs1
  %v427 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v427, ptr %gcs1
  %v428 = load i8, ptr %ctx, align 8
  %c121 = zext i8 %v428 to i64
  %v429 = icmp ne i64 %c121, 0
  br i1 %v429, label %b293, label %b292
b288:
  %v426 = icmp eq i64 %v423, %v6
  br i1 %v426, label %b290, label %b287
b289:
  %gcvolta289 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta289, ptr %ctxtopo, align 8
  ret i64 0
b290:
  %v472 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v473 = call i64 @df.box_int(i64 %v472)
  store i64 %v473, ptr %gcs1
  %v474 = icmp eq i64 %v0, 0
  %v475 = icmp eq i64 %v473, 0
  %c122 = zext i1 %v474 to i64
  %c123 = zext i1 %v475 to i64
  %v476 = or i64 %c122, %c123
  %v477 = icmp ne i64 %v476, 0
  br i1 %v477, label %b337, label %b338
b291:
  store i64 0, ptr %gcs1
  %v431 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v431, ptr %gcs1
  %v432 = load i8, ptr %ctx, align 8
  %c124 = zext i8 %v432 to i64
  %v433 = icmp ne i64 %c124, 0
  br i1 %v433, label %b297, label %b296
b292:
  %v430 = icmp eq i64 %v427, %v6
  br i1 %v430, label %b294, label %b291
b293:
  %gcvolta293 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta293, ptr %ctxtopo, align 8
  ret i64 0
b294:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v504 = icmp eq i64 %v0, 0
  %v505 = call i64 @df.class(i64 %v0)
  %v506 = call i8 @df.subclass(i64 %v505, i64 674)
  %c125 = zext i8 %v506 to i64
  %v507 = icmp ne i64 %c125, 0
  %v508 = xor i1 %v504, true
  %c126 = zext i1 %v507 to i64
  %c127 = zext i1 %v508 to i64
  %v509 = and i64 %c126, %c127
  %v510 = icmp ne i64 %v509, 0
  br i1 %v510, label %b360, label %b359
b295:
  store i64 0, ptr %gcs1
  %v435 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v435, ptr %gcs1
  %v436 = load i8, ptr %ctx, align 8
  %c128 = zext i8 %v436 to i64
  %v437 = icmp ne i64 %c128, 0
  br i1 %v437, label %b301, label %b300
b296:
  %v434 = icmp eq i64 %v431, %v6
  br i1 %v434, label %b298, label %b295
b297:
  %gcvolta297 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta297, ptr %ctxtopo, align 8
  ret i64 0
b298:
  %xcp527 = load i8, ptr %ctx, align 8
  %xcn527 = icmp ne i8 %xcp527, 0
  %xce527 = call i1 @llvm.expect.i1(i1 %xcn527, i1 false)
  br i1 %xce527, label %xc527.limpar, label %xc527.fim
xc527.limpar:
  call void @dartforge_exception_clear()
  br label %xc527.fim
xc527.fim:
  %gcvolta298 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta298, ptr %ctxtopo, align 8
  ret i64 %v0
b299:
  store i64 0, ptr %gcs1
  %v439 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v439, ptr %gcs1
  %v440 = load i8, ptr %ctx, align 8
  %c129 = zext i8 %v440 to i64
  %v441 = icmp ne i64 %c129, 0
  br i1 %v441, label %b305, label %b304
b300:
  %v438 = icmp eq i64 %v435, %v6
  br i1 %v438, label %b302, label %b299
b301:
  %gcvolta301 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta301, ptr %ctxtopo, align 8
  ret i64 0
b302:
  %v529 = add i64 0, 17
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v530 = and i64 %v0, 1
  %v531 = icmp ne i64 %v530, 0
  br i1 %v531, label %b370, label %b371
b303:
  store i64 0, ptr %gcs1
  %v443 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v443, ptr %gcs1
  %v444 = load i8, ptr %ctx, align 8
  %c130 = zext i8 %v444 to i64
  %v445 = icmp ne i64 %c130, 0
  br i1 %v445, label %b309, label %b308
b304:
  %v442 = icmp eq i64 %v439, %v6
  br i1 %v442, label %b306, label %b303
b305:
  %gcvolta305 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta305, ptr %ctxtopo, align 8
  ret i64 0
b306:
  %v559 = add i64 0, 4369
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v560 = and i64 %v0, 1
  %v561 = icmp ne i64 %v560, 0
  br i1 %v561, label %b381, label %b382
b307:
  store i64 0, ptr %gcs1
  %v447 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v447, ptr %gcs1
  %v448 = load i8, ptr %ctx, align 8
  %c131 = zext i8 %v448 to i64
  %v449 = icmp ne i64 %c131, 0
  br i1 %v449, label %b313, label %b312
b308:
  %v446 = icmp eq i64 %v443, %v6
  br i1 %v446, label %b310, label %b307
b309:
  %gcvolta309 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta309, ptr %ctxtopo, align 8
  ret i64 0
b310:
  %v589 = add i64 0, 286331153
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v590 = and i64 %v0, 1
  %v591 = icmp ne i64 %v590, 0
  br i1 %v591, label %b392, label %b393
b311:
  store i64 0, ptr %gcs1
  %v451 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v451, ptr %gcs1
  %v452 = load i8, ptr %ctx, align 8
  %c132 = zext i8 %v452 to i64
  %v453 = icmp ne i64 %c132, 0
  br i1 %v453, label %b317, label %b316
b312:
  %v450 = icmp eq i64 %v447, %v6
  br i1 %v450, label %b314, label %b311
b313:
  %gcvolta313 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta313, ptr %ctxtopo, align 8
  ret i64 0
b314:
  %v619 = add i64 0, 8
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v620 = and i64 %v0, 1
  %v621 = icmp ne i64 %v620, 0
  br i1 %v621, label %b403, label %b404
b315:
  store i64 0, ptr %gcs1
  %v455 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v455, ptr %gcs1
  %v456 = load i8, ptr %ctx, align 8
  %c133 = zext i8 %v456 to i64
  %v457 = icmp ne i64 %c133, 0
  br i1 %v457, label %b321, label %b320
b316:
  %v454 = icmp eq i64 %v451, %v6
  br i1 %v454, label %b318, label %b315
b317:
  %gcvolta317 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta317, ptr %ctxtopo, align 8
  ret i64 0
b318:
  %v649 = add i64 0, 2184
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v650 = and i64 %v0, 1
  %v651 = icmp ne i64 %v650, 0
  br i1 %v651, label %b414, label %b415
b319:
  store i64 0, ptr %gcs1
  %v459 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v459, ptr %gcs1
  %v460 = load i8, ptr %ctx, align 8
  %c134 = zext i8 %v460 to i64
  %v461 = icmp ne i64 %c134, 0
  br i1 %v461, label %b325, label %b324
b320:
  %v458 = icmp eq i64 %v455, %v6
  br i1 %v458, label %b322, label %b319
b321:
  %gcvolta321 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta321, ptr %ctxtopo, align 8
  ret i64 0
b322:
  %v679 = add i64 0, 143165576
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v680 = and i64 %v0, 1
  %v681 = icmp ne i64 %v680, 0
  br i1 %v681, label %b425, label %b426
b323:
  store i64 0, ptr %gcs1
  %v463 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v463, ptr %gcs1
  %v464 = load i8, ptr %ctx, align 8
  %c135 = zext i8 %v464 to i64
  %v465 = icmp ne i64 %c135, 0
  br i1 %v465, label %b330, label %b329
b324:
  %v462 = icmp eq i64 %v459, %v6
  br i1 %v462, label %b326, label %b323
b325:
  %gcvolta325 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta325, ptr %ctxtopo, align 8
  ret i64 0
b326:
  br label %b286
b328:
  store i64 0, ptr %gcs1
  %v467 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v467, ptr %gcs1
  %v468 = load i8, ptr %ctx, align 8
  %c136 = zext i8 %v468 to i64
  %v469 = icmp ne i64 %c136, 0
  br i1 %v469, label %b335, label %b334
b329:
  %v466 = icmp eq i64 %v463, %v6
  br i1 %v466, label %b331, label %b328
b330:
  %gcvolta330 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta330, ptr %ctxtopo, align 8
  ret i64 0
b331:
  br label %b286
b333:
  br label %b12
b334:
  %v470 = icmp eq i64 %v467, %v6
  br i1 %v470, label %b336, label %b333
b335:
  %gcvolta335 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta335, ptr %ctxtopo, align 8
  ret i64 0
b336:
  br label %b286
b337:
  %v478 = icmp eq i64 %v0, %v473
  br label %b339
b338:
  %v479 = call i64 @df.class(i64 %v0)
  switch i64 %v479, label %b343 [ i64 675, label %b340 i64 4, label %b341 i64 2, label %b342 i64 3, label %b342 ]
b339:
  %v497 = phi i1 [ %v478, %b337 ], [ %v496, %b344 ]
  br i1 %v497, label %b355, label %b356
b340:
  %v480 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v473)
  %v481 = load i8, ptr %ctx, align 8
  %c137 = zext i8 %v481 to i64
  %v482 = icmp ne i64 %c137, 0
  br i1 %v482, label %b346, label %b345
b341:
  %v484 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v473)
  %v485 = load i8, ptr %ctx, align 8
  %c138 = zext i8 %v485 to i64
  %v486 = icmp ne i64 %c138, 0
  br i1 %v486, label %b348, label %b347
b342:
  %v487 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v473)
  %v488 = load i8, ptr %ctx, align 8
  %c139 = zext i8 %v488 to i64
  %v489 = icmp ne i64 %c139, 0
  br i1 %v489, label %b350, label %b349
b343:
  %sa490_0 = getelementptr [2 x i64], ptr %sargs490, i64 0, i64 0
  store i64 %v473, ptr %sa490_0
  %sat490 = getelementptr [2 x i64], ptr %sargs490, i64 0, i64 1
  store i64 0, ptr %sat490
  %sic490 = getelementptr i64, ptr %area, i64 188
  %sf490 = call ptr @df.selector(ptr %sic490, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v490 = call i64 %sf490(i64 %v0, ptr %sargs490, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v490, ptr %gcs0
  %v491 = load i8, ptr %ctx, align 8
  %c140 = zext i8 %v491 to i64
  %v492 = icmp ne i64 %c140, 0
  br i1 %v492, label %b352, label %b351
b344:
  %v496 = phi i1 [ %v483, %b345 ], [ %v484, %b347 ], [ %v487, %b349 ], [ %v493, %b353 ]
  br label %b339
b345:
  %c141 = zext i8 %v480 to i64
  %v483 = icmp ne i64 %c141, 0
  br label %b344
b346:
  %gcvolta346 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta346, ptr %ctxtopo, align 8
  ret i64 0
b347:
  br label %b344
b348:
  %gcvolta348 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta348, ptr %ctxtopo, align 8
  ret i64 0
b349:
  br label %b344
b350:
  %gcvolta350 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta350, ptr %ctxtopo, align 8
  ret i64 0
b351:
  store i64 0, ptr %gcs1
  %v493 = call i1 @df.unbox_bool(i64 %v490)
  %v494 = load i8, ptr %ctx, align 8
  %c142 = zext i8 %v494 to i64
  %v495 = icmp ne i64 %c142, 0
  br i1 %v495, label %b354, label %b353
b352:
  %gcvolta352 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta352, ptr %ctxtopo, align 8
  ret i64 0
b353:
  br label %b344
b354:
  %gcvolta354 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta354, ptr %ctxtopo, align 8
  ret i64 0
b355:
  %v498 = add i64 0, 0
  br label %b357
b356:
  %v499 = add i64 0, 1
  br label %b357
b357:
  %v500 = phi i64 [ %v498, %b355 ], [ %v499, %b356 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v501 = call i64 @df.box_int(i64 %v500)
  %xcp502 = load i8, ptr %ctx, align 8
  %xcn502 = icmp ne i8 %xcp502, 0
  %xce502 = call i1 @llvm.expect.i1(i1 %xcn502, i1 false)
  br i1 %xce502, label %xc502.limpar, label %xc502.fim
xc502.limpar:
  call void @dartforge_exception_clear()
  br label %xc502.fim
xc502.fim:
  %gcvolta357 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta357, ptr %ctxtopo, align 8
  ret i64 %v501
b359:
  %ga511 = getelementptr i64, ptr %area, i64 7
  %v511 = load i8, ptr %ga511
  %c143 = zext i8 %v511 to i64
  %v512 = icmp ne i64 %c143, 0
  br i1 %v512, label %b361, label %b362
b360:
  %v519 = call i64 @df.unbox_int(i64 %v0)
  %v520 = load i8, ptr %ctx, align 8
  %c144 = zext i8 %v520 to i64
  %v521 = icmp ne i64 %c144, 0
  br i1 %v521, label %b367, label %b366
b361:
  %ga513 = getelementptr i64, ptr %area, i64 6
  %v513 = load i64, ptr %ga513
  br label %b363
b362:
  %v514 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b363
b363:
  %v515 = phi i64 [ %v513, %b361 ], [ %v514, %b362 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v515, i64 0, i64 0)
  %v517 = load i8, ptr %ctx, align 8
  %c145 = zext i8 %v517 to i64
  %v518 = icmp ne i64 %c145, 0
  br i1 %v518, label %b365, label %b364
b364:
  br label %b360
b365:
  %gcvolta365 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta365, ptr %ctxtopo, align 8
  ret i64 0
b366:
  %v522 = add i64 0, 1
  %v523 = ashr i64 %v519, %v522
  store i64 0, ptr %gcs0
  %v524 = call i64 @df.box_int(i64 %v523)
  %xcp525 = load i8, ptr %ctx, align 8
  %xcn525 = icmp ne i8 %xcp525, 0
  %xce525 = call i1 @llvm.expect.i1(i1 %xcn525, i1 false)
  br i1 %xce525, label %xc525.limpar, label %xc525.fim
xc525.limpar:
  call void @dartforge_exception_clear()
  br label %xc525.fim
xc525.fim:
  %gcvolta366 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta366, ptr %ctxtopo, align 8
  ret i64 %v524
b367:
  %gcvolta367 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta367, ptr %ctxtopo, align 8
  ret i64 0
b370:
  %v532 = ashr i64 %v0, 1
  %v533 = mul i64 %v532, %v529
  store i64 0, ptr %gcs0
  %v534 = call i64 @df.box_int(i64 %v533)
  br label %b372
b371:
  %v535 = call i64 @df.box_int(i64 %v529)
  store i64 %v535, ptr %gcs1
  %v536 = call i64 @df.class(i64 %v0)
  %v537 = icmp eq i64 %v536, 2
  %v538 = icmp eq i64 %v536, 3
  %c146 = zext i1 %v537 to i64
  %c147 = zext i1 %v538 to i64
  %v539 = or i64 %c146, %c147
  %v540 = icmp eq i64 %v536, 4
  %c148 = zext i1 %v540 to i64
  %v541 = or i64 %v539, %c148
  %v542 = call i64 @df.class(i64 %v535)
  %v543 = icmp eq i64 %v542, 2
  %v544 = icmp eq i64 %v542, 3
  %c149 = zext i1 %v543 to i64
  %c150 = zext i1 %v544 to i64
  %v545 = or i64 %c149, %c150
  %v546 = icmp eq i64 %v542, 4
  %c151 = zext i1 %v546 to i64
  %v547 = or i64 %v545, %c151
  %v548 = and i64 %v541, %v547
  %c152 = trunc i64 %v548 to i1
  br i1 %c152, label %b373, label %b374
b372:
  %v556 = phi i64 [ %v534, %b370 ], [ %v555, %b375 ]
  %xcp557 = load i8, ptr %ctx, align 8
  %xcn557 = icmp ne i8 %xcp557, 0
  %xce557 = call i1 @llvm.expect.i1(i1 %xcn557, i1 false)
  br i1 %xce557, label %xc557.limpar, label %xc557.fim
xc557.limpar:
  call void @dartforge_exception_clear()
  br label %xc557.fim
xc557.fim:
  %gcvolta372 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta372, ptr %ctxtopo, align 8
  ret i64 %v556
b373:
  %v549 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v535)
  %v550 = load i8, ptr %ctx, align 8
  %c153 = zext i8 %v550 to i64
  %v551 = icmp ne i64 %c153, 0
  br i1 %v551, label %b377, label %b376
b374:
  %sa552_0 = getelementptr [2 x i64], ptr %sargs552, i64 0, i64 0
  store i64 %v535, ptr %sa552_0
  %sat552 = getelementptr [2 x i64], ptr %sargs552, i64 0, i64 1
  store i64 0, ptr %sat552
  %sic552 = getelementptr i64, ptr %area, i64 190
  %sf552 = call ptr @df.selector(ptr %sic552, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v552 = call i64 %sf552(i64 %v0, ptr %sargs552, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v553 = load i8, ptr %ctx, align 8
  %c154 = zext i8 %v553 to i64
  %v554 = icmp ne i64 %c154, 0
  br i1 %v554, label %b379, label %b378
b375:
  %v555 = phi i64 [ %v549, %b376 ], [ %v552, %b378 ]
  br label %b372
b376:
  br label %b375
b377:
  %gcvolta377 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta377, ptr %ctxtopo, align 8
  ret i64 0
b378:
  br label %b375
b379:
  %gcvolta379 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta379, ptr %ctxtopo, align 8
  ret i64 0
b381:
  %v562 = ashr i64 %v0, 1
  %v563 = mul i64 %v562, %v559
  store i64 0, ptr %gcs0
  %v564 = call i64 @df.box_int(i64 %v563)
  br label %b383
b382:
  %v565 = call i64 @df.box_int(i64 %v559)
  store i64 %v565, ptr %gcs1
  %v566 = call i64 @df.class(i64 %v0)
  %v567 = icmp eq i64 %v566, 2
  %v568 = icmp eq i64 %v566, 3
  %c155 = zext i1 %v567 to i64
  %c156 = zext i1 %v568 to i64
  %v569 = or i64 %c155, %c156
  %v570 = icmp eq i64 %v566, 4
  %c157 = zext i1 %v570 to i64
  %v571 = or i64 %v569, %c157
  %v572 = call i64 @df.class(i64 %v565)
  %v573 = icmp eq i64 %v572, 2
  %v574 = icmp eq i64 %v572, 3
  %c158 = zext i1 %v573 to i64
  %c159 = zext i1 %v574 to i64
  %v575 = or i64 %c158, %c159
  %v576 = icmp eq i64 %v572, 4
  %c160 = zext i1 %v576 to i64
  %v577 = or i64 %v575, %c160
  %v578 = and i64 %v571, %v577
  %c161 = trunc i64 %v578 to i1
  br i1 %c161, label %b384, label %b385
b383:
  %v586 = phi i64 [ %v564, %b381 ], [ %v585, %b386 ]
  %xcp587 = load i8, ptr %ctx, align 8
  %xcn587 = icmp ne i8 %xcp587, 0
  %xce587 = call i1 @llvm.expect.i1(i1 %xcn587, i1 false)
  br i1 %xce587, label %xc587.limpar, label %xc587.fim
xc587.limpar:
  call void @dartforge_exception_clear()
  br label %xc587.fim
xc587.fim:
  %gcvolta383 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta383, ptr %ctxtopo, align 8
  ret i64 %v586
b384:
  %v579 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v565)
  %v580 = load i8, ptr %ctx, align 8
  %c162 = zext i8 %v580 to i64
  %v581 = icmp ne i64 %c162, 0
  br i1 %v581, label %b388, label %b387
b385:
  %sa582_0 = getelementptr [2 x i64], ptr %sargs582, i64 0, i64 0
  store i64 %v565, ptr %sa582_0
  %sat582 = getelementptr [2 x i64], ptr %sargs582, i64 0, i64 1
  store i64 0, ptr %sat582
  %sic582 = getelementptr i64, ptr %area, i64 192
  %sf582 = call ptr @df.selector(ptr %sic582, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v582 = call i64 %sf582(i64 %v0, ptr %sargs582, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v583 = load i8, ptr %ctx, align 8
  %c163 = zext i8 %v583 to i64
  %v584 = icmp ne i64 %c163, 0
  br i1 %v584, label %b390, label %b389
b386:
  %v585 = phi i64 [ %v579, %b387 ], [ %v582, %b389 ]
  br label %b383
b387:
  br label %b386
b388:
  %gcvolta388 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta388, ptr %ctxtopo, align 8
  ret i64 0
b389:
  br label %b386
b390:
  %gcvolta390 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta390, ptr %ctxtopo, align 8
  ret i64 0
b392:
  %v592 = ashr i64 %v0, 1
  %v593 = mul i64 %v592, %v589
  store i64 0, ptr %gcs0
  %v594 = call i64 @df.box_int(i64 %v593)
  br label %b394
b393:
  %v595 = call i64 @df.box_int(i64 %v589)
  store i64 %v595, ptr %gcs1
  %v596 = call i64 @df.class(i64 %v0)
  %v597 = icmp eq i64 %v596, 2
  %v598 = icmp eq i64 %v596, 3
  %c164 = zext i1 %v597 to i64
  %c165 = zext i1 %v598 to i64
  %v599 = or i64 %c164, %c165
  %v600 = icmp eq i64 %v596, 4
  %c166 = zext i1 %v600 to i64
  %v601 = or i64 %v599, %c166
  %v602 = call i64 @df.class(i64 %v595)
  %v603 = icmp eq i64 %v602, 2
  %v604 = icmp eq i64 %v602, 3
  %c167 = zext i1 %v603 to i64
  %c168 = zext i1 %v604 to i64
  %v605 = or i64 %c167, %c168
  %v606 = icmp eq i64 %v602, 4
  %c169 = zext i1 %v606 to i64
  %v607 = or i64 %v605, %c169
  %v608 = and i64 %v601, %v607
  %c170 = trunc i64 %v608 to i1
  br i1 %c170, label %b395, label %b396
b394:
  %v616 = phi i64 [ %v594, %b392 ], [ %v615, %b397 ]
  %xcp617 = load i8, ptr %ctx, align 8
  %xcn617 = icmp ne i8 %xcp617, 0
  %xce617 = call i1 @llvm.expect.i1(i1 %xcn617, i1 false)
  br i1 %xce617, label %xc617.limpar, label %xc617.fim
xc617.limpar:
  call void @dartforge_exception_clear()
  br label %xc617.fim
xc617.fim:
  %gcvolta394 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta394, ptr %ctxtopo, align 8
  ret i64 %v616
b395:
  %v609 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v595)
  %v610 = load i8, ptr %ctx, align 8
  %c171 = zext i8 %v610 to i64
  %v611 = icmp ne i64 %c171, 0
  br i1 %v611, label %b399, label %b398
b396:
  %sa612_0 = getelementptr [2 x i64], ptr %sargs612, i64 0, i64 0
  store i64 %v595, ptr %sa612_0
  %sat612 = getelementptr [2 x i64], ptr %sargs612, i64 0, i64 1
  store i64 0, ptr %sat612
  %sic612 = getelementptr i64, ptr %area, i64 194
  %sf612 = call ptr @df.selector(ptr %sic612, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v612 = call i64 %sf612(i64 %v0, ptr %sargs612, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v613 = load i8, ptr %ctx, align 8
  %c172 = zext i8 %v613 to i64
  %v614 = icmp ne i64 %c172, 0
  br i1 %v614, label %b401, label %b400
b397:
  %v615 = phi i64 [ %v609, %b398 ], [ %v612, %b400 ]
  br label %b394
b398:
  br label %b397
b399:
  %gcvolta399 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta399, ptr %ctxtopo, align 8
  ret i64 0
b400:
  br label %b397
b401:
  %gcvolta401 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta401, ptr %ctxtopo, align 8
  ret i64 0
b403:
  %v622 = ashr i64 %v0, 1
  %v623 = mul i64 %v622, %v619
  store i64 0, ptr %gcs0
  %v624 = call i64 @df.box_int(i64 %v623)
  br label %b405
b404:
  %v625 = call i64 @df.box_int(i64 %v619)
  store i64 %v625, ptr %gcs1
  %v626 = call i64 @df.class(i64 %v0)
  %v627 = icmp eq i64 %v626, 2
  %v628 = icmp eq i64 %v626, 3
  %c173 = zext i1 %v627 to i64
  %c174 = zext i1 %v628 to i64
  %v629 = or i64 %c173, %c174
  %v630 = icmp eq i64 %v626, 4
  %c175 = zext i1 %v630 to i64
  %v631 = or i64 %v629, %c175
  %v632 = call i64 @df.class(i64 %v625)
  %v633 = icmp eq i64 %v632, 2
  %v634 = icmp eq i64 %v632, 3
  %c176 = zext i1 %v633 to i64
  %c177 = zext i1 %v634 to i64
  %v635 = or i64 %c176, %c177
  %v636 = icmp eq i64 %v632, 4
  %c178 = zext i1 %v636 to i64
  %v637 = or i64 %v635, %c178
  %v638 = and i64 %v631, %v637
  %c179 = trunc i64 %v638 to i1
  br i1 %c179, label %b406, label %b407
b405:
  %v646 = phi i64 [ %v624, %b403 ], [ %v645, %b408 ]
  %xcp647 = load i8, ptr %ctx, align 8
  %xcn647 = icmp ne i8 %xcp647, 0
  %xce647 = call i1 @llvm.expect.i1(i1 %xcn647, i1 false)
  br i1 %xce647, label %xc647.limpar, label %xc647.fim
xc647.limpar:
  call void @dartforge_exception_clear()
  br label %xc647.fim
xc647.fim:
  %gcvolta405 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta405, ptr %ctxtopo, align 8
  ret i64 %v646
b406:
  %v639 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v625)
  %v640 = load i8, ptr %ctx, align 8
  %c180 = zext i8 %v640 to i64
  %v641 = icmp ne i64 %c180, 0
  br i1 %v641, label %b410, label %b409
b407:
  %sa642_0 = getelementptr [2 x i64], ptr %sargs642, i64 0, i64 0
  store i64 %v625, ptr %sa642_0
  %sat642 = getelementptr [2 x i64], ptr %sargs642, i64 0, i64 1
  store i64 0, ptr %sat642
  %sic642 = getelementptr i64, ptr %area, i64 196
  %sf642 = call ptr @df.selector(ptr %sic642, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v642 = call i64 %sf642(i64 %v0, ptr %sargs642, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v643 = load i8, ptr %ctx, align 8
  %c181 = zext i8 %v643 to i64
  %v644 = icmp ne i64 %c181, 0
  br i1 %v644, label %b412, label %b411
b408:
  %v645 = phi i64 [ %v639, %b409 ], [ %v642, %b411 ]
  br label %b405
b409:
  br label %b408
b410:
  %gcvolta410 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta410, ptr %ctxtopo, align 8
  ret i64 0
b411:
  br label %b408
b412:
  %gcvolta412 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta412, ptr %ctxtopo, align 8
  ret i64 0
b414:
  %v652 = ashr i64 %v0, 1
  %v653 = mul i64 %v652, %v649
  store i64 0, ptr %gcs0
  %v654 = call i64 @df.box_int(i64 %v653)
  br label %b416
b415:
  %v655 = call i64 @df.box_int(i64 %v649)
  store i64 %v655, ptr %gcs1
  %v656 = call i64 @df.class(i64 %v0)
  %v657 = icmp eq i64 %v656, 2
  %v658 = icmp eq i64 %v656, 3
  %c182 = zext i1 %v657 to i64
  %c183 = zext i1 %v658 to i64
  %v659 = or i64 %c182, %c183
  %v660 = icmp eq i64 %v656, 4
  %c184 = zext i1 %v660 to i64
  %v661 = or i64 %v659, %c184
  %v662 = call i64 @df.class(i64 %v655)
  %v663 = icmp eq i64 %v662, 2
  %v664 = icmp eq i64 %v662, 3
  %c185 = zext i1 %v663 to i64
  %c186 = zext i1 %v664 to i64
  %v665 = or i64 %c185, %c186
  %v666 = icmp eq i64 %v662, 4
  %c187 = zext i1 %v666 to i64
  %v667 = or i64 %v665, %c187
  %v668 = and i64 %v661, %v667
  %c188 = trunc i64 %v668 to i1
  br i1 %c188, label %b417, label %b418
b416:
  %v676 = phi i64 [ %v654, %b414 ], [ %v675, %b419 ]
  %xcp677 = load i8, ptr %ctx, align 8
  %xcn677 = icmp ne i8 %xcp677, 0
  %xce677 = call i1 @llvm.expect.i1(i1 %xcn677, i1 false)
  br i1 %xce677, label %xc677.limpar, label %xc677.fim
xc677.limpar:
  call void @dartforge_exception_clear()
  br label %xc677.fim
xc677.fim:
  %gcvolta416 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta416, ptr %ctxtopo, align 8
  ret i64 %v676
b417:
  %v669 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v655)
  %v670 = load i8, ptr %ctx, align 8
  %c189 = zext i8 %v670 to i64
  %v671 = icmp ne i64 %c189, 0
  br i1 %v671, label %b421, label %b420
b418:
  %sa672_0 = getelementptr [2 x i64], ptr %sargs672, i64 0, i64 0
  store i64 %v655, ptr %sa672_0
  %sat672 = getelementptr [2 x i64], ptr %sargs672, i64 0, i64 1
  store i64 0, ptr %sat672
  %sic672 = getelementptr i64, ptr %area, i64 198
  %sf672 = call ptr @df.selector(ptr %sic672, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v672 = call i64 %sf672(i64 %v0, ptr %sargs672, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v673 = load i8, ptr %ctx, align 8
  %c190 = zext i8 %v673 to i64
  %v674 = icmp ne i64 %c190, 0
  br i1 %v674, label %b423, label %b422
b419:
  %v675 = phi i64 [ %v669, %b420 ], [ %v672, %b422 ]
  br label %b416
b420:
  br label %b419
b421:
  %gcvolta421 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta421, ptr %ctxtopo, align 8
  ret i64 0
b422:
  br label %b419
b423:
  %gcvolta423 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta423, ptr %ctxtopo, align 8
  ret i64 0
b425:
  %v682 = ashr i64 %v0, 1
  %v683 = mul i64 %v682, %v679
  store i64 0, ptr %gcs0
  %v684 = call i64 @df.box_int(i64 %v683)
  br label %b427
b426:
  %v685 = call i64 @df.box_int(i64 %v679)
  store i64 %v685, ptr %gcs1
  %v686 = call i64 @df.class(i64 %v0)
  %v687 = icmp eq i64 %v686, 2
  %v688 = icmp eq i64 %v686, 3
  %c191 = zext i1 %v687 to i64
  %c192 = zext i1 %v688 to i64
  %v689 = or i64 %c191, %c192
  %v690 = icmp eq i64 %v686, 4
  %c193 = zext i1 %v690 to i64
  %v691 = or i64 %v689, %c193
  %v692 = call i64 @df.class(i64 %v685)
  %v693 = icmp eq i64 %v692, 2
  %v694 = icmp eq i64 %v692, 3
  %c194 = zext i1 %v693 to i64
  %c195 = zext i1 %v694 to i64
  %v695 = or i64 %c194, %c195
  %v696 = icmp eq i64 %v692, 4
  %c196 = zext i1 %v696 to i64
  %v697 = or i64 %v695, %c196
  %v698 = and i64 %v691, %v697
  %c197 = trunc i64 %v698 to i1
  br i1 %c197, label %b428, label %b429
b427:
  %v706 = phi i64 [ %v684, %b425 ], [ %v705, %b430 ]
  %xcp707 = load i8, ptr %ctx, align 8
  %xcn707 = icmp ne i8 %xcp707, 0
  %xce707 = call i1 @llvm.expect.i1(i1 %xcn707, i1 false)
  br i1 %xce707, label %xc707.limpar, label %xc707.fim
xc707.limpar:
  call void @dartforge_exception_clear()
  br label %xc707.fim
xc707.fim:
  %gcvolta427 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta427, ptr %ctxtopo, align 8
  ret i64 %v706
b428:
  %v699 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v685)
  %v700 = load i8, ptr %ctx, align 8
  %c198 = zext i8 %v700 to i64
  %v701 = icmp ne i64 %c198, 0
  br i1 %v701, label %b432, label %b431
b429:
  %sa702_0 = getelementptr [2 x i64], ptr %sargs702, i64 0, i64 0
  store i64 %v685, ptr %sa702_0
  %sat702 = getelementptr [2 x i64], ptr %sargs702, i64 0, i64 1
  store i64 0, ptr %sat702
  %sic702 = getelementptr i64, ptr %area, i64 200
  %sf702 = call ptr @df.selector(ptr %sic702, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v702 = call i64 %sf702(i64 %v0, ptr %sargs702, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v703 = load i8, ptr %ctx, align 8
  %c199 = zext i8 %v703 to i64
  %v704 = icmp ne i64 %c199, 0
  br i1 %v704, label %b434, label %b433
b430:
  %v705 = phi i64 [ %v699, %b431 ], [ %v702, %b433 ]
  br label %b427
b431:
  br label %b430
b432:
  %gcvolta432 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta432, ptr %ctxtopo, align 8
  ret i64 0
b433:
  br label %b430
b434:
  %gcvolta434 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta434, ptr %ctxtopo, align 8
  ret i64 0
b436:
  store i64 0, ptr %gcs1
  %v714 = call double @df.unbox_double(i64 %v711)
  %v715 = load i8, ptr %ctx, align 8
  %c200 = zext i8 %v715 to i64
  %v716 = icmp ne i64 %c200, 0
  br i1 %v716, label %b439, label %b438
b437:
  %gcvolta437 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta437, ptr %ctxtopo, align 8
  ret i64 0
b438:
  %xcp718 = load i8, ptr %ctx, align 8
  %xcn718 = icmp ne i8 %xcp718, 0
  %xce718 = call i1 @llvm.expect.i1(i1 %xcn718, i1 false)
  br i1 %xce718, label %xc718.limpar, label %xc718.fim
xc718.limpar:
  call void @dartforge_exception_clear()
  br label %xc718.fim
xc718.fim:
  %gcvolta438 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta438, ptr %ctxtopo, align 8
  ret i64 %v711
b439:
  %gcvolta439 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta439, ptr %ctxtopo, align 8
  ret i64 0
b453:
  %v992 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v993 = call i64 @df.box_int(i64 %v992)
  store i64 %v993, ptr %gcs1
  %sa994_0 = getelementptr [2 x i64], ptr %sargs994, i64 0, i64 0
  store i64 %v993, ptr %sa994_0
  %sat994 = getelementptr [2 x i64], ptr %sargs994, i64 0, i64 1
  store i64 0, ptr %sat994
  %sic994 = getelementptr i64, ptr %area, i64 202
  %sf994 = call ptr @df.selector(ptr %sic994, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v994 = call i64 %sf994(i64 %v0, ptr %sargs994, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v994, ptr %gcs0
  %v995 = load i8, ptr %ctx, align 8
  %c201 = zext i8 %v995 to i64
  %v996 = icmp ne i64 %c201, 0
  br i1 %v996, label %b602, label %b601
b454:
  store i64 0, ptr %gcs1
  %v724 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v724, ptr %gcs1
  %v725 = load i8, ptr %ctx, align 8
  %c202 = zext i8 %v725 to i64
  %v726 = icmp ne i64 %c202, 0
  br i1 %v726, label %b460, label %b459
b455:
  %v723 = icmp eq i64 %v720, %v6
  br i1 %v723, label %b457, label %b454
b456:
  %gcvolta456 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta456, ptr %ctxtopo, align 8
  ret i64 0
b457:
  %v769 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v770 = call i64 @df.box_int(i64 %v769)
  store i64 %v770, ptr %gcs1
  %v771 = icmp eq i64 %v0, 0
  %v772 = icmp eq i64 %v770, 0
  %c203 = zext i1 %v771 to i64
  %c204 = zext i1 %v772 to i64
  %v773 = or i64 %c203, %c204
  %v774 = icmp ne i64 %v773, 0
  br i1 %v774, label %b504, label %b505
b458:
  store i64 0, ptr %gcs1
  %v728 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v728, ptr %gcs1
  %v729 = load i8, ptr %ctx, align 8
  %c205 = zext i8 %v729 to i64
  %v730 = icmp ne i64 %c205, 0
  br i1 %v730, label %b464, label %b463
b459:
  %v727 = icmp eq i64 %v724, %v6
  br i1 %v727, label %b461, label %b458
b460:
  %gcvolta460 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta460, ptr %ctxtopo, align 8
  ret i64 0
b461:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v801 = icmp eq i64 %v0, 0
  %v802 = call i64 @df.class(i64 %v0)
  %v803 = call i8 @df.subclass(i64 %v802, i64 674)
  %c206 = zext i8 %v803 to i64
  %v804 = icmp ne i64 %c206, 0
  %v805 = xor i1 %v801, true
  %c207 = zext i1 %v804 to i64
  %c208 = zext i1 %v805 to i64
  %v806 = and i64 %c207, %c208
  %v807 = icmp ne i64 %v806, 0
  br i1 %v807, label %b527, label %b526
b462:
  store i64 0, ptr %gcs1
  %v732 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v732, ptr %gcs1
  %v733 = load i8, ptr %ctx, align 8
  %c209 = zext i8 %v733 to i64
  %v734 = icmp ne i64 %c209, 0
  br i1 %v734, label %b468, label %b467
b463:
  %v731 = icmp eq i64 %v728, %v6
  br i1 %v731, label %b465, label %b462
b464:
  %gcvolta464 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta464, ptr %ctxtopo, align 8
  ret i64 0
b465:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v824 = icmp eq i64 %v0, 0
  %v825 = call i64 @df.class(i64 %v0)
  %v826 = call i8 @df.subclass(i64 %v825, i64 674)
  %c210 = zext i8 %v826 to i64
  %v827 = icmp ne i64 %c210, 0
  %v828 = xor i1 %v824, true
  %c211 = zext i1 %v827 to i64
  %c212 = zext i1 %v828 to i64
  %v829 = and i64 %c211, %c212
  %v830 = icmp ne i64 %v829, 0
  br i1 %v830, label %b537, label %b536
b466:
  store i64 0, ptr %gcs1
  %v736 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v736, ptr %gcs1
  %v737 = load i8, ptr %ctx, align 8
  %c213 = zext i8 %v737 to i64
  %v738 = icmp ne i64 %c213, 0
  br i1 %v738, label %b472, label %b471
b467:
  %v735 = icmp eq i64 %v732, %v6
  br i1 %v735, label %b469, label %b466
b468:
  %gcvolta468 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta468, ptr %ctxtopo, align 8
  ret i64 0
b469:
  %xcp847 = load i8, ptr %ctx, align 8
  %xcn847 = icmp ne i8 %xcp847, 0
  %xce847 = call i1 @llvm.expect.i1(i1 %xcn847, i1 false)
  br i1 %xce847, label %xc847.limpar, label %xc847.fim
xc847.limpar:
  call void @dartforge_exception_clear()
  br label %xc847.fim
xc847.fim:
  %gcvolta469 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta469, ptr %ctxtopo, align 8
  ret i64 %v0
b470:
  store i64 0, ptr %gcs1
  %v740 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v740, ptr %gcs1
  %v741 = load i8, ptr %ctx, align 8
  %c214 = zext i8 %v741 to i64
  %v742 = icmp ne i64 %c214, 0
  br i1 %v742, label %b476, label %b475
b471:
  %v739 = icmp eq i64 %v736, %v6
  br i1 %v739, label %b473, label %b470
b472:
  %gcvolta472 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta472, ptr %ctxtopo, align 8
  ret i64 0
b473:
  %v849 = add i64 0, 257
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v850 = and i64 %v0, 1
  %v851 = icmp ne i64 %v850, 0
  br i1 %v851, label %b547, label %b548
b474:
  store i64 0, ptr %gcs1
  %v744 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v744, ptr %gcs1
  %v745 = load i8, ptr %ctx, align 8
  %c215 = zext i8 %v745 to i64
  %v746 = icmp ne i64 %c215, 0
  br i1 %v746, label %b480, label %b479
b475:
  %v743 = icmp eq i64 %v740, %v6
  br i1 %v743, label %b477, label %b474
b476:
  %gcvolta476 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta476, ptr %ctxtopo, align 8
  ret i64 0
b477:
  %v879 = add i64 0, 16843009
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v880 = and i64 %v0, 1
  %v881 = icmp ne i64 %v880, 0
  br i1 %v881, label %b558, label %b559
b478:
  store i64 0, ptr %gcs1
  %v748 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v748, ptr %gcs1
  %v749 = load i8, ptr %ctx, align 8
  %c216 = zext i8 %v749 to i64
  %v750 = icmp ne i64 %c216, 0
  br i1 %v750, label %b484, label %b483
b479:
  %v747 = icmp eq i64 %v744, %v6
  br i1 %v747, label %b481, label %b478
b480:
  %gcvolta480 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta480, ptr %ctxtopo, align 8
  ret i64 0
b481:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v909 = icmp eq i64 %v0, 0
  %v910 = call i64 @df.class(i64 %v0)
  %v911 = call i8 @df.subclass(i64 %v910, i64 674)
  %c217 = zext i8 %v911 to i64
  %v912 = icmp ne i64 %c217, 0
  %v913 = xor i1 %v909, true
  %c218 = zext i1 %v912 to i64
  %c219 = zext i1 %v913 to i64
  %v914 = and i64 %c218, %c219
  %v915 = icmp ne i64 %v914, 0
  br i1 %v915, label %b570, label %b569
b482:
  store i64 0, ptr %gcs1
  %v752 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v752, ptr %gcs1
  %v753 = load i8, ptr %ctx, align 8
  %c220 = zext i8 %v753 to i64
  %v754 = icmp ne i64 %c220, 0
  br i1 %v754, label %b488, label %b487
b483:
  %v751 = icmp eq i64 %v748, %v6
  br i1 %v751, label %b485, label %b482
b484:
  %gcvolta484 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta484, ptr %ctxtopo, align 8
  ret i64 0
b485:
  %v932 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v933 = and i64 %v0, 1
  %v934 = icmp ne i64 %v933, 0
  br i1 %v934, label %b579, label %b580
b486:
  store i64 0, ptr %gcs1
  %v756 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v756, ptr %gcs1
  %v757 = load i8, ptr %ctx, align 8
  %c221 = zext i8 %v757 to i64
  %v758 = icmp ne i64 %c221, 0
  br i1 %v758, label %b492, label %b491
b487:
  %v755 = icmp eq i64 %v752, %v6
  br i1 %v755, label %b489, label %b486
b488:
  %gcvolta488 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta488, ptr %ctxtopo, align 8
  ret i64 0
b489:
  %v962 = add i64 0, 8421504
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v963 = and i64 %v0, 1
  %v964 = icmp ne i64 %v963, 0
  br i1 %v964, label %b590, label %b591
b490:
  store i64 0, ptr %gcs1
  %v760 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v760, ptr %gcs1
  %v761 = load i8, ptr %ctx, align 8
  %c222 = zext i8 %v761 to i64
  %v762 = icmp ne i64 %c222, 0
  br i1 %v762, label %b497, label %b496
b491:
  %v759 = icmp eq i64 %v756, %v6
  br i1 %v759, label %b493, label %b490
b492:
  %gcvolta492 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta492, ptr %ctxtopo, align 8
  ret i64 0
b493:
  br label %b453
b495:
  store i64 0, ptr %gcs1
  %v764 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v764, ptr %gcs1
  %v765 = load i8, ptr %ctx, align 8
  %c223 = zext i8 %v765 to i64
  %v766 = icmp ne i64 %c223, 0
  br i1 %v766, label %b502, label %b501
b496:
  %v763 = icmp eq i64 %v760, %v6
  br i1 %v763, label %b498, label %b495
b497:
  %gcvolta497 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta497, ptr %ctxtopo, align 8
  ret i64 0
b498:
  br label %b453
b500:
  br label %b12
b501:
  %v767 = icmp eq i64 %v764, %v6
  br i1 %v767, label %b503, label %b500
b502:
  %gcvolta502 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta502, ptr %ctxtopo, align 8
  ret i64 0
b503:
  br label %b453
b504:
  %v775 = icmp eq i64 %v0, %v770
  br label %b506
b505:
  %v776 = call i64 @df.class(i64 %v0)
  switch i64 %v776, label %b510 [ i64 675, label %b507 i64 4, label %b508 i64 2, label %b509 i64 3, label %b509 ]
b506:
  %v794 = phi i1 [ %v775, %b504 ], [ %v793, %b511 ]
  br i1 %v794, label %b522, label %b523
b507:
  %v777 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v770)
  %v778 = load i8, ptr %ctx, align 8
  %c224 = zext i8 %v778 to i64
  %v779 = icmp ne i64 %c224, 0
  br i1 %v779, label %b513, label %b512
b508:
  %v781 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v770)
  %v782 = load i8, ptr %ctx, align 8
  %c225 = zext i8 %v782 to i64
  %v783 = icmp ne i64 %c225, 0
  br i1 %v783, label %b515, label %b514
b509:
  %v784 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v770)
  %v785 = load i8, ptr %ctx, align 8
  %c226 = zext i8 %v785 to i64
  %v786 = icmp ne i64 %c226, 0
  br i1 %v786, label %b517, label %b516
b510:
  %sa787_0 = getelementptr [2 x i64], ptr %sargs787, i64 0, i64 0
  store i64 %v770, ptr %sa787_0
  %sat787 = getelementptr [2 x i64], ptr %sargs787, i64 0, i64 1
  store i64 0, ptr %sat787
  %sic787 = getelementptr i64, ptr %area, i64 204
  %sf787 = call ptr @df.selector(ptr %sic787, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v787 = call i64 %sf787(i64 %v0, ptr %sargs787, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v787, ptr %gcs0
  %v788 = load i8, ptr %ctx, align 8
  %c227 = zext i8 %v788 to i64
  %v789 = icmp ne i64 %c227, 0
  br i1 %v789, label %b519, label %b518
b511:
  %v793 = phi i1 [ %v780, %b512 ], [ %v781, %b514 ], [ %v784, %b516 ], [ %v790, %b520 ]
  br label %b506
b512:
  %c228 = zext i8 %v777 to i64
  %v780 = icmp ne i64 %c228, 0
  br label %b511
b513:
  %gcvolta513 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta513, ptr %ctxtopo, align 8
  ret i64 0
b514:
  br label %b511
b515:
  %gcvolta515 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta515, ptr %ctxtopo, align 8
  ret i64 0
b516:
  br label %b511
b517:
  %gcvolta517 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta517, ptr %ctxtopo, align 8
  ret i64 0
b518:
  store i64 0, ptr %gcs1
  %v790 = call i1 @df.unbox_bool(i64 %v787)
  %v791 = load i8, ptr %ctx, align 8
  %c229 = zext i8 %v791 to i64
  %v792 = icmp ne i64 %c229, 0
  br i1 %v792, label %b521, label %b520
b519:
  %gcvolta519 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta519, ptr %ctxtopo, align 8
  ret i64 0
b520:
  br label %b511
b521:
  %gcvolta521 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta521, ptr %ctxtopo, align 8
  ret i64 0
b522:
  %v795 = add i64 0, 0
  br label %b524
b523:
  %v796 = add i64 0, 1
  br label %b524
b524:
  %v797 = phi i64 [ %v795, %b522 ], [ %v796, %b523 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v798 = call i64 @df.box_int(i64 %v797)
  %xcp799 = load i8, ptr %ctx, align 8
  %xcn799 = icmp ne i8 %xcp799, 0
  %xce799 = call i1 @llvm.expect.i1(i1 %xcn799, i1 false)
  br i1 %xce799, label %xc799.limpar, label %xc799.fim
xc799.limpar:
  call void @dartforge_exception_clear()
  br label %xc799.fim
xc799.fim:
  %gcvolta524 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta524, ptr %ctxtopo, align 8
  ret i64 %v798
b526:
  %ga808 = getelementptr i64, ptr %area, i64 7
  %v808 = load i8, ptr %ga808
  %c230 = zext i8 %v808 to i64
  %v809 = icmp ne i64 %c230, 0
  br i1 %v809, label %b528, label %b529
b527:
  %v816 = call i64 @df.unbox_int(i64 %v0)
  %v817 = load i8, ptr %ctx, align 8
  %c231 = zext i8 %v817 to i64
  %v818 = icmp ne i64 %c231, 0
  br i1 %v818, label %b534, label %b533
b528:
  %ga810 = getelementptr i64, ptr %area, i64 6
  %v810 = load i64, ptr %ga810
  br label %b530
b529:
  %v811 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b530
b530:
  %v812 = phi i64 [ %v810, %b528 ], [ %v811, %b529 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v812, i64 0, i64 0)
  %v814 = load i8, ptr %ctx, align 8
  %c232 = zext i8 %v814 to i64
  %v815 = icmp ne i64 %c232, 0
  br i1 %v815, label %b532, label %b531
b531:
  br label %b527
b532:
  %gcvolta532 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta532, ptr %ctxtopo, align 8
  ret i64 0
b533:
  %v819 = add i64 0, 6
  %v820 = ashr i64 %v816, %v819
  store i64 0, ptr %gcs0
  %v821 = call i64 @df.box_int(i64 %v820)
  %xcp822 = load i8, ptr %ctx, align 8
  %xcn822 = icmp ne i8 %xcp822, 0
  %xce822 = call i1 @llvm.expect.i1(i1 %xcn822, i1 false)
  br i1 %xce822, label %xc822.limpar, label %xc822.fim
xc822.limpar:
  call void @dartforge_exception_clear()
  br label %xc822.fim
xc822.fim:
  %gcvolta533 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta533, ptr %ctxtopo, align 8
  ret i64 %v821
b534:
  %gcvolta534 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta534, ptr %ctxtopo, align 8
  ret i64 0
b536:
  %ga831 = getelementptr i64, ptr %area, i64 7
  %v831 = load i8, ptr %ga831
  %c233 = zext i8 %v831 to i64
  %v832 = icmp ne i64 %c233, 0
  br i1 %v832, label %b538, label %b539
b537:
  %v839 = call i64 @df.unbox_int(i64 %v0)
  %v840 = load i8, ptr %ctx, align 8
  %c234 = zext i8 %v840 to i64
  %v841 = icmp ne i64 %c234, 0
  br i1 %v841, label %b544, label %b543
b538:
  %ga833 = getelementptr i64, ptr %area, i64 6
  %v833 = load i64, ptr %ga833
  br label %b540
b539:
  %v834 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b540
b540:
  %v835 = phi i64 [ %v833, %b538 ], [ %v834, %b539 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v835, i64 0, i64 0)
  %v837 = load i8, ptr %ctx, align 8
  %c235 = zext i8 %v837 to i64
  %v838 = icmp ne i64 %c235, 0
  br i1 %v838, label %b542, label %b541
b541:
  br label %b537
b542:
  %gcvolta542 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta542, ptr %ctxtopo, align 8
  ret i64 0
b543:
  %v842 = add i64 0, 4
  %v843 = ashr i64 %v839, %v842
  store i64 0, ptr %gcs0
  %v844 = call i64 @df.box_int(i64 %v843)
  %xcp845 = load i8, ptr %ctx, align 8
  %xcn845 = icmp ne i8 %xcp845, 0
  %xce845 = call i1 @llvm.expect.i1(i1 %xcn845, i1 false)
  br i1 %xce845, label %xc845.limpar, label %xc845.fim
xc845.limpar:
  call void @dartforge_exception_clear()
  br label %xc845.fim
xc845.fim:
  %gcvolta543 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta543, ptr %ctxtopo, align 8
  ret i64 %v844
b544:
  %gcvolta544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta544, ptr %ctxtopo, align 8
  ret i64 0
b547:
  %v852 = ashr i64 %v0, 1
  %v853 = mul i64 %v852, %v849
  store i64 0, ptr %gcs0
  %v854 = call i64 @df.box_int(i64 %v853)
  br label %b549
b548:
  %v855 = call i64 @df.box_int(i64 %v849)
  store i64 %v855, ptr %gcs1
  %v856 = call i64 @df.class(i64 %v0)
  %v857 = icmp eq i64 %v856, 2
  %v858 = icmp eq i64 %v856, 3
  %c236 = zext i1 %v857 to i64
  %c237 = zext i1 %v858 to i64
  %v859 = or i64 %c236, %c237
  %v860 = icmp eq i64 %v856, 4
  %c238 = zext i1 %v860 to i64
  %v861 = or i64 %v859, %c238
  %v862 = call i64 @df.class(i64 %v855)
  %v863 = icmp eq i64 %v862, 2
  %v864 = icmp eq i64 %v862, 3
  %c239 = zext i1 %v863 to i64
  %c240 = zext i1 %v864 to i64
  %v865 = or i64 %c239, %c240
  %v866 = icmp eq i64 %v862, 4
  %c241 = zext i1 %v866 to i64
  %v867 = or i64 %v865, %c241
  %v868 = and i64 %v861, %v867
  %c242 = trunc i64 %v868 to i1
  br i1 %c242, label %b550, label %b551
b549:
  %v876 = phi i64 [ %v854, %b547 ], [ %v875, %b552 ]
  %xcp877 = load i8, ptr %ctx, align 8
  %xcn877 = icmp ne i8 %xcp877, 0
  %xce877 = call i1 @llvm.expect.i1(i1 %xcn877, i1 false)
  br i1 %xce877, label %xc877.limpar, label %xc877.fim
xc877.limpar:
  call void @dartforge_exception_clear()
  br label %xc877.fim
xc877.fim:
  %gcvolta549 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta549, ptr %ctxtopo, align 8
  ret i64 %v876
b550:
  %v869 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v855)
  %v870 = load i8, ptr %ctx, align 8
  %c243 = zext i8 %v870 to i64
  %v871 = icmp ne i64 %c243, 0
  br i1 %v871, label %b554, label %b553
b551:
  %sa872_0 = getelementptr [2 x i64], ptr %sargs872, i64 0, i64 0
  store i64 %v855, ptr %sa872_0
  %sat872 = getelementptr [2 x i64], ptr %sargs872, i64 0, i64 1
  store i64 0, ptr %sat872
  %sic872 = getelementptr i64, ptr %area, i64 206
  %sf872 = call ptr @df.selector(ptr %sic872, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v872 = call i64 %sf872(i64 %v0, ptr %sargs872, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v873 = load i8, ptr %ctx, align 8
  %c244 = zext i8 %v873 to i64
  %v874 = icmp ne i64 %c244, 0
  br i1 %v874, label %b556, label %b555
b552:
  %v875 = phi i64 [ %v869, %b553 ], [ %v872, %b555 ]
  br label %b549
b553:
  br label %b552
b554:
  %gcvolta554 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta554, ptr %ctxtopo, align 8
  ret i64 0
b555:
  br label %b552
b556:
  %gcvolta556 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta556, ptr %ctxtopo, align 8
  ret i64 0
b558:
  %v882 = ashr i64 %v0, 1
  %v883 = mul i64 %v882, %v879
  store i64 0, ptr %gcs0
  %v884 = call i64 @df.box_int(i64 %v883)
  br label %b560
b559:
  %v885 = call i64 @df.box_int(i64 %v879)
  store i64 %v885, ptr %gcs1
  %v886 = call i64 @df.class(i64 %v0)
  %v887 = icmp eq i64 %v886, 2
  %v888 = icmp eq i64 %v886, 3
  %c245 = zext i1 %v887 to i64
  %c246 = zext i1 %v888 to i64
  %v889 = or i64 %c245, %c246
  %v890 = icmp eq i64 %v886, 4
  %c247 = zext i1 %v890 to i64
  %v891 = or i64 %v889, %c247
  %v892 = call i64 @df.class(i64 %v885)
  %v893 = icmp eq i64 %v892, 2
  %v894 = icmp eq i64 %v892, 3
  %c248 = zext i1 %v893 to i64
  %c249 = zext i1 %v894 to i64
  %v895 = or i64 %c248, %c249
  %v896 = icmp eq i64 %v892, 4
  %c250 = zext i1 %v896 to i64
  %v897 = or i64 %v895, %c250
  %v898 = and i64 %v891, %v897
  %c251 = trunc i64 %v898 to i1
  br i1 %c251, label %b561, label %b562
b560:
  %v906 = phi i64 [ %v884, %b558 ], [ %v905, %b563 ]
  %xcp907 = load i8, ptr %ctx, align 8
  %xcn907 = icmp ne i8 %xcp907, 0
  %xce907 = call i1 @llvm.expect.i1(i1 %xcn907, i1 false)
  br i1 %xce907, label %xc907.limpar, label %xc907.fim
xc907.limpar:
  call void @dartforge_exception_clear()
  br label %xc907.fim
xc907.fim:
  %gcvolta560 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta560, ptr %ctxtopo, align 8
  ret i64 %v906
b561:
  %v899 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v885)
  %v900 = load i8, ptr %ctx, align 8
  %c252 = zext i8 %v900 to i64
  %v901 = icmp ne i64 %c252, 0
  br i1 %v901, label %b565, label %b564
b562:
  %sa902_0 = getelementptr [2 x i64], ptr %sargs902, i64 0, i64 0
  store i64 %v885, ptr %sa902_0
  %sat902 = getelementptr [2 x i64], ptr %sargs902, i64 0, i64 1
  store i64 0, ptr %sat902
  %sic902 = getelementptr i64, ptr %area, i64 208
  %sf902 = call ptr @df.selector(ptr %sic902, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v902 = call i64 %sf902(i64 %v0, ptr %sargs902, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v903 = load i8, ptr %ctx, align 8
  %c253 = zext i8 %v903 to i64
  %v904 = icmp ne i64 %c253, 0
  br i1 %v904, label %b567, label %b566
b563:
  %v905 = phi i64 [ %v899, %b564 ], [ %v902, %b566 ]
  br label %b560
b564:
  br label %b563
b565:
  %gcvolta565 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta565, ptr %ctxtopo, align 8
  ret i64 0
b566:
  br label %b563
b567:
  %gcvolta567 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta567, ptr %ctxtopo, align 8
  ret i64 0
b569:
  %ga916 = getelementptr i64, ptr %area, i64 7
  %v916 = load i8, ptr %ga916
  %c254 = zext i8 %v916 to i64
  %v917 = icmp ne i64 %c254, 0
  br i1 %v917, label %b571, label %b572
b570:
  %v924 = call i64 @df.unbox_int(i64 %v0)
  %v925 = load i8, ptr %ctx, align 8
  %c255 = zext i8 %v925 to i64
  %v926 = icmp ne i64 %c255, 0
  br i1 %v926, label %b577, label %b576
b571:
  %ga918 = getelementptr i64, ptr %area, i64 6
  %v918 = load i64, ptr %ga918
  br label %b573
b572:
  %v919 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b573
b573:
  %v920 = phi i64 [ %v918, %b571 ], [ %v919, %b572 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v920, i64 0, i64 0)
  %v922 = load i8, ptr %ctx, align 8
  %c256 = zext i8 %v922 to i64
  %v923 = icmp ne i64 %c256, 0
  br i1 %v923, label %b575, label %b574
b574:
  br label %b570
b575:
  %gcvolta575 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta575, ptr %ctxtopo, align 8
  ret i64 0
b576:
  %v927 = add i64 0, 1
  %v928 = ashr i64 %v924, %v927
  store i64 0, ptr %gcs0
  %v929 = call i64 @df.box_int(i64 %v928)
  %xcp930 = load i8, ptr %ctx, align 8
  %xcn930 = icmp ne i8 %xcp930, 0
  %xce930 = call i1 @llvm.expect.i1(i1 %xcn930, i1 false)
  br i1 %xce930, label %xc930.limpar, label %xc930.fim
xc930.limpar:
  call void @dartforge_exception_clear()
  br label %xc930.fim
xc930.fim:
  %gcvolta576 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta576, ptr %ctxtopo, align 8
  ret i64 %v929
b577:
  %gcvolta577 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta577, ptr %ctxtopo, align 8
  ret i64 0
b579:
  %v935 = ashr i64 %v0, 1
  %v936 = mul i64 %v935, %v932
  store i64 0, ptr %gcs0
  %v937 = call i64 @df.box_int(i64 %v936)
  br label %b581
b580:
  %v938 = call i64 @df.box_int(i64 %v932)
  store i64 %v938, ptr %gcs1
  %v939 = call i64 @df.class(i64 %v0)
  %v940 = icmp eq i64 %v939, 2
  %v941 = icmp eq i64 %v939, 3
  %c257 = zext i1 %v940 to i64
  %c258 = zext i1 %v941 to i64
  %v942 = or i64 %c257, %c258
  %v943 = icmp eq i64 %v939, 4
  %c259 = zext i1 %v943 to i64
  %v944 = or i64 %v942, %c259
  %v945 = call i64 @df.class(i64 %v938)
  %v946 = icmp eq i64 %v945, 2
  %v947 = icmp eq i64 %v945, 3
  %c260 = zext i1 %v946 to i64
  %c261 = zext i1 %v947 to i64
  %v948 = or i64 %c260, %c261
  %v949 = icmp eq i64 %v945, 4
  %c262 = zext i1 %v949 to i64
  %v950 = or i64 %v948, %c262
  %v951 = and i64 %v944, %v950
  %c263 = trunc i64 %v951 to i1
  br i1 %c263, label %b582, label %b583
b581:
  %v959 = phi i64 [ %v937, %b579 ], [ %v958, %b584 ]
  %xcp960 = load i8, ptr %ctx, align 8
  %xcn960 = icmp ne i8 %xcp960, 0
  %xce960 = call i1 @llvm.expect.i1(i1 %xcn960, i1 false)
  br i1 %xce960, label %xc960.limpar, label %xc960.fim
xc960.limpar:
  call void @dartforge_exception_clear()
  br label %xc960.fim
xc960.fim:
  %gcvolta581 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta581, ptr %ctxtopo, align 8
  ret i64 %v959
b582:
  %v952 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v938)
  %v953 = load i8, ptr %ctx, align 8
  %c264 = zext i8 %v953 to i64
  %v954 = icmp ne i64 %c264, 0
  br i1 %v954, label %b586, label %b585
b583:
  %sa955_0 = getelementptr [2 x i64], ptr %sargs955, i64 0, i64 0
  store i64 %v938, ptr %sa955_0
  %sat955 = getelementptr [2 x i64], ptr %sargs955, i64 0, i64 1
  store i64 0, ptr %sat955
  %sic955 = getelementptr i64, ptr %area, i64 210
  %sf955 = call ptr @df.selector(ptr %sic955, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v955 = call i64 %sf955(i64 %v0, ptr %sargs955, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v956 = load i8, ptr %ctx, align 8
  %c265 = zext i8 %v956 to i64
  %v957 = icmp ne i64 %c265, 0
  br i1 %v957, label %b588, label %b587
b584:
  %v958 = phi i64 [ %v952, %b585 ], [ %v955, %b587 ]
  br label %b581
b585:
  br label %b584
b586:
  %gcvolta586 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta586, ptr %ctxtopo, align 8
  ret i64 0
b587:
  br label %b584
b588:
  %gcvolta588 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta588, ptr %ctxtopo, align 8
  ret i64 0
b590:
  %v965 = ashr i64 %v0, 1
  %v966 = mul i64 %v965, %v962
  store i64 0, ptr %gcs0
  %v967 = call i64 @df.box_int(i64 %v966)
  br label %b592
b591:
  %v968 = call i64 @df.box_int(i64 %v962)
  store i64 %v968, ptr %gcs1
  %v969 = call i64 @df.class(i64 %v0)
  %v970 = icmp eq i64 %v969, 2
  %v971 = icmp eq i64 %v969, 3
  %c266 = zext i1 %v970 to i64
  %c267 = zext i1 %v971 to i64
  %v972 = or i64 %c266, %c267
  %v973 = icmp eq i64 %v969, 4
  %c268 = zext i1 %v973 to i64
  %v974 = or i64 %v972, %c268
  %v975 = call i64 @df.class(i64 %v968)
  %v976 = icmp eq i64 %v975, 2
  %v977 = icmp eq i64 %v975, 3
  %c269 = zext i1 %v976 to i64
  %c270 = zext i1 %v977 to i64
  %v978 = or i64 %c269, %c270
  %v979 = icmp eq i64 %v975, 4
  %c271 = zext i1 %v979 to i64
  %v980 = or i64 %v978, %c271
  %v981 = and i64 %v974, %v980
  %c272 = trunc i64 %v981 to i1
  br i1 %c272, label %b593, label %b594
b592:
  %v989 = phi i64 [ %v967, %b590 ], [ %v988, %b595 ]
  %xcp990 = load i8, ptr %ctx, align 8
  %xcn990 = icmp ne i8 %xcp990, 0
  %xce990 = call i1 @llvm.expect.i1(i1 %xcn990, i1 false)
  br i1 %xce990, label %xc990.limpar, label %xc990.fim
xc990.limpar:
  call void @dartforge_exception_clear()
  br label %xc990.fim
xc990.fim:
  %gcvolta592 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta592, ptr %ctxtopo, align 8
  ret i64 %v989
b593:
  %v982 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v968)
  %v983 = load i8, ptr %ctx, align 8
  %c273 = zext i8 %v983 to i64
  %v984 = icmp ne i64 %c273, 0
  br i1 %v984, label %b597, label %b596
b594:
  %sa985_0 = getelementptr [2 x i64], ptr %sargs985, i64 0, i64 0
  store i64 %v968, ptr %sa985_0
  %sat985 = getelementptr [2 x i64], ptr %sargs985, i64 0, i64 1
  store i64 0, ptr %sat985
  %sic985 = getelementptr i64, ptr %area, i64 212
  %sf985 = call ptr @df.selector(ptr %sic985, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v985 = call i64 %sf985(i64 %v0, ptr %sargs985, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v986 = load i8, ptr %ctx, align 8
  %c274 = zext i8 %v986 to i64
  %v987 = icmp ne i64 %c274, 0
  br i1 %v987, label %b599, label %b598
b595:
  %v988 = phi i64 [ %v982, %b596 ], [ %v985, %b598 ]
  br label %b592
b596:
  br label %b595
b597:
  %gcvolta597 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta597, ptr %ctxtopo, align 8
  ret i64 0
b598:
  br label %b595
b599:
  %gcvolta599 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta599, ptr %ctxtopo, align 8
  ret i64 0
b601:
  store i64 0, ptr %gcs1
  %v997 = call double @df.unbox_double(i64 %v994)
  %v998 = load i8, ptr %ctx, align 8
  %c275 = zext i8 %v998 to i64
  %v999 = icmp ne i64 %c275, 0
  br i1 %v999, label %b604, label %b603
b602:
  %gcvolta602 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta602, ptr %ctxtopo, align 8
  ret i64 0
b603:
  %xcp1001 = load i8, ptr %ctx, align 8
  %xcn1001 = icmp ne i8 %xcp1001, 0
  %xce1001 = call i1 @llvm.expect.i1(i1 %xcn1001, i1 false)
  br i1 %xce1001, label %xc1001.limpar, label %xc1001.fim
xc1001.limpar:
  call void @dartforge_exception_clear()
  br label %xc1001.fim
xc1001.fim:
  %gcvolta603 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta603, ptr %ctxtopo, align 8
  ret i64 %v994
b604:
  %gcvolta604 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta604, ptr %ctxtopo, align 8
  ret i64 0
b618:
  %v1254 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1255 = call i64 @df.box_int(i64 %v1254)
  store i64 %v1255, ptr %gcs1
  %sa1256_0 = getelementptr [2 x i64], ptr %sargs1256, i64 0, i64 0
  store i64 %v1255, ptr %sa1256_0
  %sat1256 = getelementptr [2 x i64], ptr %sargs1256, i64 0, i64 1
  store i64 0, ptr %sat1256
  %sic1256 = getelementptr i64, ptr %area, i64 214
  %sf1256 = call ptr @df.selector(ptr %sic1256, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1256 = call i64 %sf1256(i64 %v0, ptr %sargs1256, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1256, ptr %gcs0
  %v1257 = load i8, ptr %ctx, align 8
  %c276 = zext i8 %v1257 to i64
  %v1258 = icmp ne i64 %c276, 0
  br i1 %v1258, label %b764, label %b763
b619:
  store i64 0, ptr %gcs1
  %v1007 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1007, ptr %gcs1
  %v1008 = load i8, ptr %ctx, align 8
  %c277 = zext i8 %v1008 to i64
  %v1009 = icmp ne i64 %c277, 0
  br i1 %v1009, label %b625, label %b624
b620:
  %v1006 = icmp eq i64 %v1003, %v6
  br i1 %v1006, label %b622, label %b619
b621:
  %gcvolta621 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta621, ptr %ctxtopo, align 8
  ret i64 0
b622:
  %v1052 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1053 = call i64 @df.box_int(i64 %v1052)
  store i64 %v1053, ptr %gcs1
  %v1054 = icmp eq i64 %v0, 0
  %v1055 = icmp eq i64 %v1053, 0
  %c278 = zext i1 %v1054 to i64
  %c279 = zext i1 %v1055 to i64
  %v1056 = or i64 %c278, %c279
  %v1057 = icmp ne i64 %v1056, 0
  br i1 %v1057, label %b669, label %b670
b623:
  store i64 0, ptr %gcs1
  %v1011 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1011, ptr %gcs1
  %v1012 = load i8, ptr %ctx, align 8
  %c280 = zext i8 %v1012 to i64
  %v1013 = icmp ne i64 %c280, 0
  br i1 %v1013, label %b629, label %b628
b624:
  %v1010 = icmp eq i64 %v1007, %v6
  br i1 %v1010, label %b626, label %b623
b625:
  %gcvolta625 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta625, ptr %ctxtopo, align 8
  ret i64 0
b626:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1084 = icmp eq i64 %v0, 0
  %v1085 = call i64 @df.class(i64 %v0)
  %v1086 = call i8 @df.subclass(i64 %v1085, i64 674)
  %c281 = zext i8 %v1086 to i64
  %v1087 = icmp ne i64 %c281, 0
  %v1088 = xor i1 %v1084, true
  %c282 = zext i1 %v1087 to i64
  %c283 = zext i1 %v1088 to i64
  %v1089 = and i64 %c282, %c283
  %v1090 = icmp ne i64 %v1089, 0
  br i1 %v1090, label %b692, label %b691
b627:
  store i64 0, ptr %gcs1
  %v1015 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1015, ptr %gcs1
  %v1016 = load i8, ptr %ctx, align 8
  %c284 = zext i8 %v1016 to i64
  %v1017 = icmp ne i64 %c284, 0
  br i1 %v1017, label %b633, label %b632
b628:
  %v1014 = icmp eq i64 %v1011, %v6
  br i1 %v1014, label %b630, label %b627
b629:
  %gcvolta629 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta629, ptr %ctxtopo, align 8
  ret i64 0
b630:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1107 = icmp eq i64 %v0, 0
  %v1108 = call i64 @df.class(i64 %v0)
  %v1109 = call i8 @df.subclass(i64 %v1108, i64 674)
  %c285 = zext i8 %v1109 to i64
  %v1110 = icmp ne i64 %c285, 0
  %v1111 = xor i1 %v1107, true
  %c286 = zext i1 %v1110 to i64
  %c287 = zext i1 %v1111 to i64
  %v1112 = and i64 %c286, %c287
  %v1113 = icmp ne i64 %v1112, 0
  br i1 %v1113, label %b702, label %b701
b631:
  store i64 0, ptr %gcs1
  %v1019 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1019, ptr %gcs1
  %v1020 = load i8, ptr %ctx, align 8
  %c288 = zext i8 %v1020 to i64
  %v1021 = icmp ne i64 %c288, 0
  br i1 %v1021, label %b637, label %b636
b632:
  %v1018 = icmp eq i64 %v1015, %v6
  br i1 %v1018, label %b634, label %b631
b633:
  %gcvolta633 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta633, ptr %ctxtopo, align 8
  ret i64 0
b634:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1130 = icmp eq i64 %v0, 0
  %v1131 = call i64 @df.class(i64 %v0)
  %v1132 = call i8 @df.subclass(i64 %v1131, i64 674)
  %c289 = zext i8 %v1132 to i64
  %v1133 = icmp ne i64 %c289, 0
  %v1134 = xor i1 %v1130, true
  %c290 = zext i1 %v1133 to i64
  %c291 = zext i1 %v1134 to i64
  %v1135 = and i64 %c290, %c291
  %v1136 = icmp ne i64 %v1135, 0
  br i1 %v1136, label %b712, label %b711
b635:
  store i64 0, ptr %gcs1
  %v1023 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1023, ptr %gcs1
  %v1024 = load i8, ptr %ctx, align 8
  %c292 = zext i8 %v1024 to i64
  %v1025 = icmp ne i64 %c292, 0
  br i1 %v1025, label %b641, label %b640
b636:
  %v1022 = icmp eq i64 %v1019, %v6
  br i1 %v1022, label %b638, label %b635
b637:
  %gcvolta637 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta637, ptr %ctxtopo, align 8
  ret i64 0
b638:
  %xcp1153 = load i8, ptr %ctx, align 8
  %xcn1153 = icmp ne i8 %xcp1153, 0
  %xce1153 = call i1 @llvm.expect.i1(i1 %xcn1153, i1 false)
  br i1 %xce1153, label %xc1153.limpar, label %xc1153.fim
xc1153.limpar:
  call void @dartforge_exception_clear()
  br label %xc1153.fim
xc1153.fim:
  %gcvolta638 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta638, ptr %ctxtopo, align 8
  ret i64 %v0
b639:
  store i64 0, ptr %gcs1
  %v1027 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1027, ptr %gcs1
  %v1028 = load i8, ptr %ctx, align 8
  %c293 = zext i8 %v1028 to i64
  %v1029 = icmp ne i64 %c293, 0
  br i1 %v1029, label %b645, label %b644
b640:
  %v1026 = icmp eq i64 %v1023, %v6
  br i1 %v1026, label %b642, label %b639
b641:
  %gcvolta641 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta641, ptr %ctxtopo, align 8
  ret i64 0
b642:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1155 = icmp eq i64 %v0, 0
  %v1156 = call i64 @df.class(i64 %v0)
  %v1157 = call i8 @df.subclass(i64 %v1156, i64 674)
  %c294 = zext i8 %v1157 to i64
  %v1158 = icmp ne i64 %c294, 0
  %v1159 = xor i1 %v1155, true
  %c295 = zext i1 %v1158 to i64
  %c296 = zext i1 %v1159 to i64
  %v1160 = and i64 %c295, %c296
  %v1161 = icmp ne i64 %v1160, 0
  br i1 %v1161, label %b723, label %b722
b643:
  store i64 0, ptr %gcs1
  %v1031 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1031, ptr %gcs1
  %v1032 = load i8, ptr %ctx, align 8
  %c297 = zext i8 %v1032 to i64
  %v1033 = icmp ne i64 %c297, 0
  br i1 %v1033, label %b649, label %b648
b644:
  %v1030 = icmp eq i64 %v1027, %v6
  br i1 %v1030, label %b646, label %b643
b645:
  %gcvolta645 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta645, ptr %ctxtopo, align 8
  ret i64 0
b646:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1178 = icmp eq i64 %v0, 0
  %v1179 = call i64 @df.class(i64 %v0)
  %v1180 = call i8 @df.subclass(i64 %v1179, i64 674)
  %c298 = zext i8 %v1180 to i64
  %v1181 = icmp ne i64 %c298, 0
  %v1182 = xor i1 %v1178, true
  %c299 = zext i1 %v1181 to i64
  %c300 = zext i1 %v1182 to i64
  %v1183 = and i64 %c299, %c300
  %v1184 = icmp ne i64 %v1183, 0
  br i1 %v1184, label %b733, label %b732
b647:
  store i64 0, ptr %gcs1
  %v1035 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1035, ptr %gcs1
  %v1036 = load i8, ptr %ctx, align 8
  %c301 = zext i8 %v1036 to i64
  %v1037 = icmp ne i64 %c301, 0
  br i1 %v1037, label %b653, label %b652
b648:
  %v1034 = icmp eq i64 %v1031, %v6
  br i1 %v1034, label %b650, label %b647
b649:
  %gcvolta649 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta649, ptr %ctxtopo, align 8
  ret i64 0
b650:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1201 = icmp eq i64 %v0, 0
  %v1202 = call i64 @df.class(i64 %v0)
  %v1203 = call i8 @df.subclass(i64 %v1202, i64 674)
  %c302 = zext i8 %v1203 to i64
  %v1204 = icmp ne i64 %c302, 0
  %v1205 = xor i1 %v1201, true
  %c303 = zext i1 %v1204 to i64
  %c304 = zext i1 %v1205 to i64
  %v1206 = and i64 %c303, %c304
  %v1207 = icmp ne i64 %v1206, 0
  br i1 %v1207, label %b743, label %b742
b651:
  store i64 0, ptr %gcs1
  %v1039 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1039, ptr %gcs1
  %v1040 = load i8, ptr %ctx, align 8
  %c305 = zext i8 %v1040 to i64
  %v1041 = icmp ne i64 %c305, 0
  br i1 %v1041, label %b657, label %b656
b652:
  %v1038 = icmp eq i64 %v1035, %v6
  br i1 %v1038, label %b654, label %b651
b653:
  %gcvolta653 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta653, ptr %ctxtopo, align 8
  ret i64 0
b654:
  %v1224 = add i64 0, 524296
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1225 = and i64 %v0, 1
  %v1226 = icmp ne i64 %v1225, 0
  br i1 %v1226, label %b752, label %b753
b655:
  store i64 0, ptr %gcs1
  %v1043 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1043, ptr %gcs1
  %v1044 = load i8, ptr %ctx, align 8
  %c306 = zext i8 %v1044 to i64
  %v1045 = icmp ne i64 %c306, 0
  br i1 %v1045, label %b662, label %b661
b656:
  %v1042 = icmp eq i64 %v1039, %v6
  br i1 %v1042, label %b658, label %b655
b657:
  %gcvolta657 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta657, ptr %ctxtopo, align 8
  ret i64 0
b658:
  br label %b618
b660:
  store i64 0, ptr %gcs1
  %v1047 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1047, ptr %gcs1
  %v1048 = load i8, ptr %ctx, align 8
  %c307 = zext i8 %v1048 to i64
  %v1049 = icmp ne i64 %c307, 0
  br i1 %v1049, label %b667, label %b666
b661:
  %v1046 = icmp eq i64 %v1043, %v6
  br i1 %v1046, label %b663, label %b660
b662:
  %gcvolta662 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta662, ptr %ctxtopo, align 8
  ret i64 0
b663:
  br label %b618
b665:
  br label %b12
b666:
  %v1050 = icmp eq i64 %v1047, %v6
  br i1 %v1050, label %b668, label %b665
b667:
  %gcvolta667 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta667, ptr %ctxtopo, align 8
  ret i64 0
b668:
  br label %b618
b669:
  %v1058 = icmp eq i64 %v0, %v1053
  br label %b671
b670:
  %v1059 = call i64 @df.class(i64 %v0)
  switch i64 %v1059, label %b675 [ i64 675, label %b672 i64 4, label %b673 i64 2, label %b674 i64 3, label %b674 ]
b671:
  %v1077 = phi i1 [ %v1058, %b669 ], [ %v1076, %b676 ]
  br i1 %v1077, label %b687, label %b688
b672:
  %v1060 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1053)
  %v1061 = load i8, ptr %ctx, align 8
  %c308 = zext i8 %v1061 to i64
  %v1062 = icmp ne i64 %c308, 0
  br i1 %v1062, label %b678, label %b677
b673:
  %v1064 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1053)
  %v1065 = load i8, ptr %ctx, align 8
  %c309 = zext i8 %v1065 to i64
  %v1066 = icmp ne i64 %c309, 0
  br i1 %v1066, label %b680, label %b679
b674:
  %v1067 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1053)
  %v1068 = load i8, ptr %ctx, align 8
  %c310 = zext i8 %v1068 to i64
  %v1069 = icmp ne i64 %c310, 0
  br i1 %v1069, label %b682, label %b681
b675:
  %sa1070_0 = getelementptr [2 x i64], ptr %sargs1070, i64 0, i64 0
  store i64 %v1053, ptr %sa1070_0
  %sat1070 = getelementptr [2 x i64], ptr %sargs1070, i64 0, i64 1
  store i64 0, ptr %sat1070
  %sic1070 = getelementptr i64, ptr %area, i64 216
  %sf1070 = call ptr @df.selector(ptr %sic1070, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1070 = call i64 %sf1070(i64 %v0, ptr %sargs1070, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1070, ptr %gcs0
  %v1071 = load i8, ptr %ctx, align 8
  %c311 = zext i8 %v1071 to i64
  %v1072 = icmp ne i64 %c311, 0
  br i1 %v1072, label %b684, label %b683
b676:
  %v1076 = phi i1 [ %v1063, %b677 ], [ %v1064, %b679 ], [ %v1067, %b681 ], [ %v1073, %b685 ]
  br label %b671
b677:
  %c312 = zext i8 %v1060 to i64
  %v1063 = icmp ne i64 %c312, 0
  br label %b676
b678:
  %gcvolta678 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta678, ptr %ctxtopo, align 8
  ret i64 0
b679:
  br label %b676
b680:
  %gcvolta680 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta680, ptr %ctxtopo, align 8
  ret i64 0
b681:
  br label %b676
b682:
  %gcvolta682 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta682, ptr %ctxtopo, align 8
  ret i64 0
b683:
  store i64 0, ptr %gcs1
  %v1073 = call i1 @df.unbox_bool(i64 %v1070)
  %v1074 = load i8, ptr %ctx, align 8
  %c313 = zext i8 %v1074 to i64
  %v1075 = icmp ne i64 %c313, 0
  br i1 %v1075, label %b686, label %b685
b684:
  %gcvolta684 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta684, ptr %ctxtopo, align 8
  ret i64 0
b685:
  br label %b676
b686:
  %gcvolta686 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta686, ptr %ctxtopo, align 8
  ret i64 0
b687:
  %v1078 = add i64 0, 0
  br label %b689
b688:
  %v1079 = add i64 0, 1
  br label %b689
b689:
  %v1080 = phi i64 [ %v1078, %b687 ], [ %v1079, %b688 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1081 = call i64 @df.box_int(i64 %v1080)
  %xcp1082 = load i8, ptr %ctx, align 8
  %xcn1082 = icmp ne i8 %xcp1082, 0
  %xce1082 = call i1 @llvm.expect.i1(i1 %xcn1082, i1 false)
  br i1 %xce1082, label %xc1082.limpar, label %xc1082.fim
xc1082.limpar:
  call void @dartforge_exception_clear()
  br label %xc1082.fim
xc1082.fim:
  %gcvolta689 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta689, ptr %ctxtopo, align 8
  ret i64 %v1081
b691:
  %ga1091 = getelementptr i64, ptr %area, i64 7
  %v1091 = load i8, ptr %ga1091
  %c314 = zext i8 %v1091 to i64
  %v1092 = icmp ne i64 %c314, 0
  br i1 %v1092, label %b693, label %b694
b692:
  %v1099 = call i64 @df.unbox_int(i64 %v0)
  %v1100 = load i8, ptr %ctx, align 8
  %c315 = zext i8 %v1100 to i64
  %v1101 = icmp ne i64 %c315, 0
  br i1 %v1101, label %b699, label %b698
b693:
  %ga1093 = getelementptr i64, ptr %area, i64 6
  %v1093 = load i64, ptr %ga1093
  br label %b695
b694:
  %v1094 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b695
b695:
  %v1095 = phi i64 [ %v1093, %b693 ], [ %v1094, %b694 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1095, i64 0, i64 0)
  %v1097 = load i8, ptr %ctx, align 8
  %c316 = zext i8 %v1097 to i64
  %v1098 = icmp ne i64 %c316, 0
  br i1 %v1098, label %b697, label %b696
b696:
  br label %b692
b697:
  %gcvolta697 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta697, ptr %ctxtopo, align 8
  ret i64 0
b698:
  %v1102 = add i64 0, 14
  %v1103 = ashr i64 %v1099, %v1102
  store i64 0, ptr %gcs0
  %v1104 = call i64 @df.box_int(i64 %v1103)
  %xcp1105 = load i8, ptr %ctx, align 8
  %xcn1105 = icmp ne i8 %xcp1105, 0
  %xce1105 = call i1 @llvm.expect.i1(i1 %xcn1105, i1 false)
  br i1 %xce1105, label %xc1105.limpar, label %xc1105.fim
xc1105.limpar:
  call void @dartforge_exception_clear()
  br label %xc1105.fim
xc1105.fim:
  %gcvolta698 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta698, ptr %ctxtopo, align 8
  ret i64 %v1104
b699:
  %gcvolta699 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta699, ptr %ctxtopo, align 8
  ret i64 0
b701:
  %ga1114 = getelementptr i64, ptr %area, i64 7
  %v1114 = load i8, ptr %ga1114
  %c317 = zext i8 %v1114 to i64
  %v1115 = icmp ne i64 %c317, 0
  br i1 %v1115, label %b703, label %b704
b702:
  %v1122 = call i64 @df.unbox_int(i64 %v0)
  %v1123 = load i8, ptr %ctx, align 8
  %c318 = zext i8 %v1123 to i64
  %v1124 = icmp ne i64 %c318, 0
  br i1 %v1124, label %b709, label %b708
b703:
  %ga1116 = getelementptr i64, ptr %area, i64 6
  %v1116 = load i64, ptr %ga1116
  br label %b705
b704:
  %v1117 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b705
b705:
  %v1118 = phi i64 [ %v1116, %b703 ], [ %v1117, %b704 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1118, i64 0, i64 0)
  %v1120 = load i8, ptr %ctx, align 8
  %c319 = zext i8 %v1120 to i64
  %v1121 = icmp ne i64 %c319, 0
  br i1 %v1121, label %b707, label %b706
b706:
  br label %b702
b707:
  %gcvolta707 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta707, ptr %ctxtopo, align 8
  ret i64 0
b708:
  %v1125 = add i64 0, 12
  %v1126 = ashr i64 %v1122, %v1125
  store i64 0, ptr %gcs0
  %v1127 = call i64 @df.box_int(i64 %v1126)
  %xcp1128 = load i8, ptr %ctx, align 8
  %xcn1128 = icmp ne i8 %xcp1128, 0
  %xce1128 = call i1 @llvm.expect.i1(i1 %xcn1128, i1 false)
  br i1 %xce1128, label %xc1128.limpar, label %xc1128.fim
xc1128.limpar:
  call void @dartforge_exception_clear()
  br label %xc1128.fim
xc1128.fim:
  %gcvolta708 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta708, ptr %ctxtopo, align 8
  ret i64 %v1127
b709:
  %gcvolta709 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta709, ptr %ctxtopo, align 8
  ret i64 0
b711:
  %ga1137 = getelementptr i64, ptr %area, i64 7
  %v1137 = load i8, ptr %ga1137
  %c320 = zext i8 %v1137 to i64
  %v1138 = icmp ne i64 %c320, 0
  br i1 %v1138, label %b713, label %b714
b712:
  %v1145 = call i64 @df.unbox_int(i64 %v0)
  %v1146 = load i8, ptr %ctx, align 8
  %c321 = zext i8 %v1146 to i64
  %v1147 = icmp ne i64 %c321, 0
  br i1 %v1147, label %b719, label %b718
b713:
  %ga1139 = getelementptr i64, ptr %area, i64 6
  %v1139 = load i64, ptr %ga1139
  br label %b715
b714:
  %v1140 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b715
b715:
  %v1141 = phi i64 [ %v1139, %b713 ], [ %v1140, %b714 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1141, i64 0, i64 0)
  %v1143 = load i8, ptr %ctx, align 8
  %c322 = zext i8 %v1143 to i64
  %v1144 = icmp ne i64 %c322, 0
  br i1 %v1144, label %b717, label %b716
b716:
  br label %b712
b717:
  %gcvolta717 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta717, ptr %ctxtopo, align 8
  ret i64 0
b718:
  %v1148 = add i64 0, 8
  %v1149 = ashr i64 %v1145, %v1148
  store i64 0, ptr %gcs0
  %v1150 = call i64 @df.box_int(i64 %v1149)
  %xcp1151 = load i8, ptr %ctx, align 8
  %xcn1151 = icmp ne i8 %xcp1151, 0
  %xce1151 = call i1 @llvm.expect.i1(i1 %xcn1151, i1 false)
  br i1 %xce1151, label %xc1151.limpar, label %xc1151.fim
xc1151.limpar:
  call void @dartforge_exception_clear()
  br label %xc1151.fim
xc1151.fim:
  %gcvolta718 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta718, ptr %ctxtopo, align 8
  ret i64 %v1150
b719:
  %gcvolta719 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta719, ptr %ctxtopo, align 8
  ret i64 0
b722:
  %ga1162 = getelementptr i64, ptr %area, i64 7
  %v1162 = load i8, ptr %ga1162
  %c323 = zext i8 %v1162 to i64
  %v1163 = icmp ne i64 %c323, 0
  br i1 %v1163, label %b724, label %b725
b723:
  %v1170 = call i64 @df.unbox_int(i64 %v0)
  %v1171 = load i8, ptr %ctx, align 8
  %c324 = zext i8 %v1171 to i64
  %v1172 = icmp ne i64 %c324, 0
  br i1 %v1172, label %b730, label %b729
b724:
  %ga1164 = getelementptr i64, ptr %area, i64 6
  %v1164 = load i64, ptr %ga1164
  br label %b726
b725:
  %v1165 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b726
b726:
  %v1166 = phi i64 [ %v1164, %b724 ], [ %v1165, %b725 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1166, i64 0, i64 0)
  %v1168 = load i8, ptr %ctx, align 8
  %c325 = zext i8 %v1168 to i64
  %v1169 = icmp ne i64 %c325, 0
  br i1 %v1169, label %b728, label %b727
b727:
  br label %b723
b728:
  %gcvolta728 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta728, ptr %ctxtopo, align 8
  ret i64 0
b729:
  %v1173 = add i64 0, 8
  %v1174 = shl i64 %v1170, %v1173
  store i64 0, ptr %gcs0
  %v1175 = call i64 @df.box_int(i64 %v1174)
  %xcp1176 = load i8, ptr %ctx, align 8
  %xcn1176 = icmp ne i8 %xcp1176, 0
  %xce1176 = call i1 @llvm.expect.i1(i1 %xcn1176, i1 false)
  br i1 %xce1176, label %xc1176.limpar, label %xc1176.fim
xc1176.limpar:
  call void @dartforge_exception_clear()
  br label %xc1176.fim
xc1176.fim:
  %gcvolta729 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta729, ptr %ctxtopo, align 8
  ret i64 %v1175
b730:
  %gcvolta730 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta730, ptr %ctxtopo, align 8
  ret i64 0
b732:
  %ga1185 = getelementptr i64, ptr %area, i64 7
  %v1185 = load i8, ptr %ga1185
  %c326 = zext i8 %v1185 to i64
  %v1186 = icmp ne i64 %c326, 0
  br i1 %v1186, label %b734, label %b735
b733:
  %v1193 = call i64 @df.unbox_int(i64 %v0)
  %v1194 = load i8, ptr %ctx, align 8
  %c327 = zext i8 %v1194 to i64
  %v1195 = icmp ne i64 %c327, 0
  br i1 %v1195, label %b740, label %b739
b734:
  %ga1187 = getelementptr i64, ptr %area, i64 6
  %v1187 = load i64, ptr %ga1187
  br label %b736
b735:
  %v1188 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b736
b736:
  %v1189 = phi i64 [ %v1187, %b734 ], [ %v1188, %b735 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1189, i64 0, i64 0)
  %v1191 = load i8, ptr %ctx, align 8
  %c328 = zext i8 %v1191 to i64
  %v1192 = icmp ne i64 %c328, 0
  br i1 %v1192, label %b738, label %b737
b737:
  br label %b733
b738:
  %gcvolta738 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta738, ptr %ctxtopo, align 8
  ret i64 0
b739:
  %v1196 = add i64 0, 9
  %v1197 = ashr i64 %v1193, %v1196
  store i64 0, ptr %gcs0
  %v1198 = call i64 @df.box_int(i64 %v1197)
  %xcp1199 = load i8, ptr %ctx, align 8
  %xcn1199 = icmp ne i8 %xcp1199, 0
  %xce1199 = call i1 @llvm.expect.i1(i1 %xcn1199, i1 false)
  br i1 %xce1199, label %xc1199.limpar, label %xc1199.fim
xc1199.limpar:
  call void @dartforge_exception_clear()
  br label %xc1199.fim
xc1199.fim:
  %gcvolta739 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta739, ptr %ctxtopo, align 8
  ret i64 %v1198
b740:
  %gcvolta740 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta740, ptr %ctxtopo, align 8
  ret i64 0
b742:
  %ga1208 = getelementptr i64, ptr %area, i64 7
  %v1208 = load i8, ptr %ga1208
  %c329 = zext i8 %v1208 to i64
  %v1209 = icmp ne i64 %c329, 0
  br i1 %v1209, label %b744, label %b745
b743:
  %v1216 = call i64 @df.unbox_int(i64 %v0)
  %v1217 = load i8, ptr %ctx, align 8
  %c330 = zext i8 %v1217 to i64
  %v1218 = icmp ne i64 %c330, 0
  br i1 %v1218, label %b750, label %b749
b744:
  %ga1210 = getelementptr i64, ptr %area, i64 6
  %v1210 = load i64, ptr %ga1210
  br label %b746
b745:
  %v1211 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b746
b746:
  %v1212 = phi i64 [ %v1210, %b744 ], [ %v1211, %b745 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1212, i64 0, i64 0)
  %v1214 = load i8, ptr %ctx, align 8
  %c331 = zext i8 %v1214 to i64
  %v1215 = icmp ne i64 %c331, 0
  br i1 %v1215, label %b748, label %b747
b747:
  br label %b743
b748:
  %gcvolta748 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta748, ptr %ctxtopo, align 8
  ret i64 0
b749:
  %v1219 = add i64 0, 1
  %v1220 = ashr i64 %v1216, %v1219
  store i64 0, ptr %gcs0
  %v1221 = call i64 @df.box_int(i64 %v1220)
  %xcp1222 = load i8, ptr %ctx, align 8
  %xcn1222 = icmp ne i8 %xcp1222, 0
  %xce1222 = call i1 @llvm.expect.i1(i1 %xcn1222, i1 false)
  br i1 %xce1222, label %xc1222.limpar, label %xc1222.fim
xc1222.limpar:
  call void @dartforge_exception_clear()
  br label %xc1222.fim
xc1222.fim:
  %gcvolta749 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta749, ptr %ctxtopo, align 8
  ret i64 %v1221
b750:
  %gcvolta750 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta750, ptr %ctxtopo, align 8
  ret i64 0
b752:
  %v1227 = ashr i64 %v0, 1
  %v1228 = mul i64 %v1227, %v1224
  store i64 0, ptr %gcs0
  %v1229 = call i64 @df.box_int(i64 %v1228)
  br label %b754
b753:
  %v1230 = call i64 @df.box_int(i64 %v1224)
  store i64 %v1230, ptr %gcs1
  %v1231 = call i64 @df.class(i64 %v0)
  %v1232 = icmp eq i64 %v1231, 2
  %v1233 = icmp eq i64 %v1231, 3
  %c332 = zext i1 %v1232 to i64
  %c333 = zext i1 %v1233 to i64
  %v1234 = or i64 %c332, %c333
  %v1235 = icmp eq i64 %v1231, 4
  %c334 = zext i1 %v1235 to i64
  %v1236 = or i64 %v1234, %c334
  %v1237 = call i64 @df.class(i64 %v1230)
  %v1238 = icmp eq i64 %v1237, 2
  %v1239 = icmp eq i64 %v1237, 3
  %c335 = zext i1 %v1238 to i64
  %c336 = zext i1 %v1239 to i64
  %v1240 = or i64 %c335, %c336
  %v1241 = icmp eq i64 %v1237, 4
  %c337 = zext i1 %v1241 to i64
  %v1242 = or i64 %v1240, %c337
  %v1243 = and i64 %v1236, %v1242
  %c338 = trunc i64 %v1243 to i1
  br i1 %c338, label %b755, label %b756
b754:
  %v1251 = phi i64 [ %v1229, %b752 ], [ %v1250, %b757 ]
  %xcp1252 = load i8, ptr %ctx, align 8
  %xcn1252 = icmp ne i8 %xcp1252, 0
  %xce1252 = call i1 @llvm.expect.i1(i1 %xcn1252, i1 false)
  br i1 %xce1252, label %xc1252.limpar, label %xc1252.fim
xc1252.limpar:
  call void @dartforge_exception_clear()
  br label %xc1252.fim
xc1252.fim:
  %gcvolta754 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta754, ptr %ctxtopo, align 8
  ret i64 %v1251
b755:
  %v1244 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v1230)
  %v1245 = load i8, ptr %ctx, align 8
  %c339 = zext i8 %v1245 to i64
  %v1246 = icmp ne i64 %c339, 0
  br i1 %v1246, label %b759, label %b758
b756:
  %sa1247_0 = getelementptr [2 x i64], ptr %sargs1247, i64 0, i64 0
  store i64 %v1230, ptr %sa1247_0
  %sat1247 = getelementptr [2 x i64], ptr %sargs1247, i64 0, i64 1
  store i64 0, ptr %sat1247
  %sic1247 = getelementptr i64, ptr %area, i64 218
  %sf1247 = call ptr @df.selector(ptr %sic1247, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v1247 = call i64 %sf1247(i64 %v0, ptr %sargs1247, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1248 = load i8, ptr %ctx, align 8
  %c340 = zext i8 %v1248 to i64
  %v1249 = icmp ne i64 %c340, 0
  br i1 %v1249, label %b761, label %b760
b757:
  %v1250 = phi i64 [ %v1244, %b758 ], [ %v1247, %b760 ]
  br label %b754
b758:
  br label %b757
b759:
  %gcvolta759 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta759, ptr %ctxtopo, align 8
  ret i64 0
b760:
  br label %b757
b761:
  %gcvolta761 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta761, ptr %ctxtopo, align 8
  ret i64 0
b763:
  store i64 0, ptr %gcs1
  %v1259 = call double @df.unbox_double(i64 %v1256)
  %v1260 = load i8, ptr %ctx, align 8
  %c341 = zext i8 %v1260 to i64
  %v1261 = icmp ne i64 %c341, 0
  br i1 %v1261, label %b766, label %b765
b764:
  %gcvolta764 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta764, ptr %ctxtopo, align 8
  ret i64 0
b765:
  %xcp1263 = load i8, ptr %ctx, align 8
  %xcn1263 = icmp ne i8 %xcp1263, 0
  %xce1263 = call i1 @llvm.expect.i1(i1 %xcn1263, i1 false)
  br i1 %xce1263, label %xc1263.limpar, label %xc1263.fim
xc1263.limpar:
  call void @dartforge_exception_clear()
  br label %xc1263.fim
xc1263.fim:
  %gcvolta765 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta765, ptr %ctxtopo, align 8
  ret i64 %v1256
b766:
  %gcvolta766 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta766, ptr %ctxtopo, align 8
  ret i64 0
b780:
  %v1509 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1510 = call i64 @df.box_int(i64 %v1509)
  store i64 %v1510, ptr %gcs1
  %sa1511_0 = getelementptr [2 x i64], ptr %sargs1511, i64 0, i64 0
  store i64 %v1510, ptr %sa1511_0
  %sat1511 = getelementptr [2 x i64], ptr %sargs1511, i64 0, i64 1
  store i64 0, ptr %sat1511
  %sic1511 = getelementptr i64, ptr %area, i64 220
  %sf1511 = call ptr @df.selector(ptr %sic1511, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1511 = call i64 %sf1511(i64 %v0, ptr %sargs1511, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1511, ptr %gcs0
  %v1512 = load i8, ptr %ctx, align 8
  %c342 = zext i8 %v1512 to i64
  %v1513 = icmp ne i64 %c342, 0
  br i1 %v1513, label %b925, label %b924
b781:
  store i64 0, ptr %gcs1
  %v1269 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1269, ptr %gcs1
  %v1270 = load i8, ptr %ctx, align 8
  %c343 = zext i8 %v1270 to i64
  %v1271 = icmp ne i64 %c343, 0
  br i1 %v1271, label %b787, label %b786
b782:
  %v1268 = icmp eq i64 %v1265, %v6
  br i1 %v1268, label %b784, label %b781
b783:
  %gcvolta783 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta783, ptr %ctxtopo, align 8
  ret i64 0
b784:
  %v1314 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1315 = call i64 @df.box_int(i64 %v1314)
  store i64 %v1315, ptr %gcs1
  %v1316 = icmp eq i64 %v0, 0
  %v1317 = icmp eq i64 %v1315, 0
  %c344 = zext i1 %v1316 to i64
  %c345 = zext i1 %v1317 to i64
  %v1318 = or i64 %c344, %c345
  %v1319 = icmp ne i64 %v1318, 0
  br i1 %v1319, label %b831, label %b832
b785:
  store i64 0, ptr %gcs1
  %v1273 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1273, ptr %gcs1
  %v1274 = load i8, ptr %ctx, align 8
  %c346 = zext i8 %v1274 to i64
  %v1275 = icmp ne i64 %c346, 0
  br i1 %v1275, label %b791, label %b790
b786:
  %v1272 = icmp eq i64 %v1269, %v6
  br i1 %v1272, label %b788, label %b785
b787:
  %gcvolta787 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta787, ptr %ctxtopo, align 8
  ret i64 0
b788:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1346 = icmp eq i64 %v0, 0
  %v1347 = call i64 @df.class(i64 %v0)
  %v1348 = call i8 @df.subclass(i64 %v1347, i64 674)
  %c347 = zext i8 %v1348 to i64
  %v1349 = icmp ne i64 %c347, 0
  %v1350 = xor i1 %v1346, true
  %c348 = zext i1 %v1349 to i64
  %c349 = zext i1 %v1350 to i64
  %v1351 = and i64 %c348, %c349
  %v1352 = icmp ne i64 %v1351, 0
  br i1 %v1352, label %b854, label %b853
b789:
  store i64 0, ptr %gcs1
  %v1277 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1277, ptr %gcs1
  %v1278 = load i8, ptr %ctx, align 8
  %c350 = zext i8 %v1278 to i64
  %v1279 = icmp ne i64 %c350, 0
  br i1 %v1279, label %b795, label %b794
b790:
  %v1276 = icmp eq i64 %v1273, %v6
  br i1 %v1276, label %b792, label %b789
b791:
  %gcvolta791 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta791, ptr %ctxtopo, align 8
  ret i64 0
b792:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1369 = icmp eq i64 %v0, 0
  %v1370 = call i64 @df.class(i64 %v0)
  %v1371 = call i8 @df.subclass(i64 %v1370, i64 674)
  %c351 = zext i8 %v1371 to i64
  %v1372 = icmp ne i64 %c351, 0
  %v1373 = xor i1 %v1369, true
  %c352 = zext i1 %v1372 to i64
  %c353 = zext i1 %v1373 to i64
  %v1374 = and i64 %c352, %c353
  %v1375 = icmp ne i64 %v1374, 0
  br i1 %v1375, label %b864, label %b863
b793:
  store i64 0, ptr %gcs1
  %v1281 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1281, ptr %gcs1
  %v1282 = load i8, ptr %ctx, align 8
  %c354 = zext i8 %v1282 to i64
  %v1283 = icmp ne i64 %c354, 0
  br i1 %v1283, label %b799, label %b798
b794:
  %v1280 = icmp eq i64 %v1277, %v6
  br i1 %v1280, label %b796, label %b793
b795:
  %gcvolta795 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta795, ptr %ctxtopo, align 8
  ret i64 0
b796:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1392 = icmp eq i64 %v0, 0
  %v1393 = call i64 @df.class(i64 %v0)
  %v1394 = call i8 @df.subclass(i64 %v1393, i64 674)
  %c355 = zext i8 %v1394 to i64
  %v1395 = icmp ne i64 %c355, 0
  %v1396 = xor i1 %v1392, true
  %c356 = zext i1 %v1395 to i64
  %c357 = zext i1 %v1396 to i64
  %v1397 = and i64 %c356, %c357
  %v1398 = icmp ne i64 %v1397, 0
  br i1 %v1398, label %b874, label %b873
b797:
  store i64 0, ptr %gcs1
  %v1285 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1285, ptr %gcs1
  %v1286 = load i8, ptr %ctx, align 8
  %c358 = zext i8 %v1286 to i64
  %v1287 = icmp ne i64 %c358, 0
  br i1 %v1287, label %b803, label %b802
b798:
  %v1284 = icmp eq i64 %v1281, %v6
  br i1 %v1284, label %b800, label %b797
b799:
  %gcvolta799 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta799, ptr %ctxtopo, align 8
  ret i64 0
b800:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1415 = icmp eq i64 %v0, 0
  %v1416 = call i64 @df.class(i64 %v0)
  %v1417 = call i8 @df.subclass(i64 %v1416, i64 674)
  %c359 = zext i8 %v1417 to i64
  %v1418 = icmp ne i64 %c359, 0
  %v1419 = xor i1 %v1415, true
  %c360 = zext i1 %v1418 to i64
  %c361 = zext i1 %v1419 to i64
  %v1420 = and i64 %c360, %c361
  %v1421 = icmp ne i64 %v1420, 0
  br i1 %v1421, label %b884, label %b883
b801:
  store i64 0, ptr %gcs1
  %v1289 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1289, ptr %gcs1
  %v1290 = load i8, ptr %ctx, align 8
  %c362 = zext i8 %v1290 to i64
  %v1291 = icmp ne i64 %c362, 0
  br i1 %v1291, label %b807, label %b806
b802:
  %v1288 = icmp eq i64 %v1285, %v6
  br i1 %v1288, label %b804, label %b801
b803:
  %gcvolta803 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta803, ptr %ctxtopo, align 8
  ret i64 0
b804:
  %xcp1438 = load i8, ptr %ctx, align 8
  %xcn1438 = icmp ne i8 %xcp1438, 0
  %xce1438 = call i1 @llvm.expect.i1(i1 %xcn1438, i1 false)
  br i1 %xce1438, label %xc1438.limpar, label %xc1438.fim
xc1438.limpar:
  call void @dartforge_exception_clear()
  br label %xc1438.fim
xc1438.fim:
  %gcvolta804 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta804, ptr %ctxtopo, align 8
  ret i64 %v0
b805:
  store i64 0, ptr %gcs1
  %v1293 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1293, ptr %gcs1
  %v1294 = load i8, ptr %ctx, align 8
  %c363 = zext i8 %v1294 to i64
  %v1295 = icmp ne i64 %c363, 0
  br i1 %v1295, label %b811, label %b810
b806:
  %v1292 = icmp eq i64 %v1289, %v6
  br i1 %v1292, label %b808, label %b805
b807:
  %gcvolta807 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta807, ptr %ctxtopo, align 8
  ret i64 0
b808:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1440 = icmp eq i64 %v0, 0
  %v1441 = call i64 @df.class(i64 %v0)
  %v1442 = call i8 @df.subclass(i64 %v1441, i64 674)
  %c364 = zext i8 %v1442 to i64
  %v1443 = icmp ne i64 %c364, 0
  %v1444 = xor i1 %v1440, true
  %c365 = zext i1 %v1443 to i64
  %c366 = zext i1 %v1444 to i64
  %v1445 = and i64 %c365, %c366
  %v1446 = icmp ne i64 %v1445, 0
  br i1 %v1446, label %b895, label %b894
b809:
  store i64 0, ptr %gcs1
  %v1297 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1297, ptr %gcs1
  %v1298 = load i8, ptr %ctx, align 8
  %c367 = zext i8 %v1298 to i64
  %v1299 = icmp ne i64 %c367, 0
  br i1 %v1299, label %b815, label %b814
b810:
  %v1296 = icmp eq i64 %v1293, %v6
  br i1 %v1296, label %b812, label %b809
b811:
  %gcvolta811 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta811, ptr %ctxtopo, align 8
  ret i64 0
b812:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1463 = icmp eq i64 %v0, 0
  %v1464 = call i64 @df.class(i64 %v0)
  %v1465 = call i8 @df.subclass(i64 %v1464, i64 674)
  %c368 = zext i8 %v1465 to i64
  %v1466 = icmp ne i64 %c368, 0
  %v1467 = xor i1 %v1463, true
  %c369 = zext i1 %v1466 to i64
  %c370 = zext i1 %v1467 to i64
  %v1468 = and i64 %c369, %c370
  %v1469 = icmp ne i64 %v1468, 0
  br i1 %v1469, label %b905, label %b904
b813:
  store i64 0, ptr %gcs1
  %v1301 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1301, ptr %gcs1
  %v1302 = load i8, ptr %ctx, align 8
  %c371 = zext i8 %v1302 to i64
  %v1303 = icmp ne i64 %c371, 0
  br i1 %v1303, label %b819, label %b818
b814:
  %v1300 = icmp eq i64 %v1297, %v6
  br i1 %v1300, label %b816, label %b813
b815:
  %gcvolta815 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta815, ptr %ctxtopo, align 8
  ret i64 0
b816:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1486 = icmp eq i64 %v0, 0
  %v1487 = call i64 @df.class(i64 %v0)
  %v1488 = call i8 @df.subclass(i64 %v1487, i64 674)
  %c372 = zext i8 %v1488 to i64
  %v1489 = icmp ne i64 %c372, 0
  %v1490 = xor i1 %v1486, true
  %c373 = zext i1 %v1489 to i64
  %c374 = zext i1 %v1490 to i64
  %v1491 = and i64 %c373, %c374
  %v1492 = icmp ne i64 %v1491, 0
  br i1 %v1492, label %b915, label %b914
b817:
  store i64 0, ptr %gcs1
  %v1305 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1305, ptr %gcs1
  %v1306 = load i8, ptr %ctx, align 8
  %c375 = zext i8 %v1306 to i64
  %v1307 = icmp ne i64 %c375, 0
  br i1 %v1307, label %b824, label %b823
b818:
  %v1304 = icmp eq i64 %v1301, %v6
  br i1 %v1304, label %b820, label %b817
b819:
  %gcvolta819 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta819, ptr %ctxtopo, align 8
  ret i64 0
b820:
  br label %b780
b822:
  store i64 0, ptr %gcs1
  %v1309 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1309, ptr %gcs1
  %v1310 = load i8, ptr %ctx, align 8
  %c376 = zext i8 %v1310 to i64
  %v1311 = icmp ne i64 %c376, 0
  br i1 %v1311, label %b829, label %b828
b823:
  %v1308 = icmp eq i64 %v1305, %v6
  br i1 %v1308, label %b825, label %b822
b824:
  %gcvolta824 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta824, ptr %ctxtopo, align 8
  ret i64 0
b825:
  br label %b780
b827:
  br label %b12
b828:
  %v1312 = icmp eq i64 %v1309, %v6
  br i1 %v1312, label %b830, label %b827
b829:
  %gcvolta829 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta829, ptr %ctxtopo, align 8
  ret i64 0
b830:
  br label %b780
b831:
  %v1320 = icmp eq i64 %v0, %v1315
  br label %b833
b832:
  %v1321 = call i64 @df.class(i64 %v0)
  switch i64 %v1321, label %b837 [ i64 675, label %b834 i64 4, label %b835 i64 2, label %b836 i64 3, label %b836 ]
b833:
  %v1339 = phi i1 [ %v1320, %b831 ], [ %v1338, %b838 ]
  br i1 %v1339, label %b849, label %b850
b834:
  %v1322 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1315)
  %v1323 = load i8, ptr %ctx, align 8
  %c377 = zext i8 %v1323 to i64
  %v1324 = icmp ne i64 %c377, 0
  br i1 %v1324, label %b840, label %b839
b835:
  %v1326 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1315)
  %v1327 = load i8, ptr %ctx, align 8
  %c378 = zext i8 %v1327 to i64
  %v1328 = icmp ne i64 %c378, 0
  br i1 %v1328, label %b842, label %b841
b836:
  %v1329 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1315)
  %v1330 = load i8, ptr %ctx, align 8
  %c379 = zext i8 %v1330 to i64
  %v1331 = icmp ne i64 %c379, 0
  br i1 %v1331, label %b844, label %b843
b837:
  %sa1332_0 = getelementptr [2 x i64], ptr %sargs1332, i64 0, i64 0
  store i64 %v1315, ptr %sa1332_0
  %sat1332 = getelementptr [2 x i64], ptr %sargs1332, i64 0, i64 1
  store i64 0, ptr %sat1332
  %sic1332 = getelementptr i64, ptr %area, i64 222
  %sf1332 = call ptr @df.selector(ptr %sic1332, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1332 = call i64 %sf1332(i64 %v0, ptr %sargs1332, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1332, ptr %gcs0
  %v1333 = load i8, ptr %ctx, align 8
  %c380 = zext i8 %v1333 to i64
  %v1334 = icmp ne i64 %c380, 0
  br i1 %v1334, label %b846, label %b845
b838:
  %v1338 = phi i1 [ %v1325, %b839 ], [ %v1326, %b841 ], [ %v1329, %b843 ], [ %v1335, %b847 ]
  br label %b833
b839:
  %c381 = zext i8 %v1322 to i64
  %v1325 = icmp ne i64 %c381, 0
  br label %b838
b840:
  %gcvolta840 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta840, ptr %ctxtopo, align 8
  ret i64 0
b841:
  br label %b838
b842:
  %gcvolta842 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta842, ptr %ctxtopo, align 8
  ret i64 0
b843:
  br label %b838
b844:
  %gcvolta844 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta844, ptr %ctxtopo, align 8
  ret i64 0
b845:
  store i64 0, ptr %gcs1
  %v1335 = call i1 @df.unbox_bool(i64 %v1332)
  %v1336 = load i8, ptr %ctx, align 8
  %c382 = zext i8 %v1336 to i64
  %v1337 = icmp ne i64 %c382, 0
  br i1 %v1337, label %b848, label %b847
b846:
  %gcvolta846 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta846, ptr %ctxtopo, align 8
  ret i64 0
b847:
  br label %b838
b848:
  %gcvolta848 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta848, ptr %ctxtopo, align 8
  ret i64 0
b849:
  %v1340 = add i64 0, 0
  br label %b851
b850:
  %v1341 = add i64 0, 1
  br label %b851
b851:
  %v1342 = phi i64 [ %v1340, %b849 ], [ %v1341, %b850 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1343 = call i64 @df.box_int(i64 %v1342)
  %xcp1344 = load i8, ptr %ctx, align 8
  %xcn1344 = icmp ne i8 %xcp1344, 0
  %xce1344 = call i1 @llvm.expect.i1(i1 %xcn1344, i1 false)
  br i1 %xce1344, label %xc1344.limpar, label %xc1344.fim
xc1344.limpar:
  call void @dartforge_exception_clear()
  br label %xc1344.fim
xc1344.fim:
  %gcvolta851 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta851, ptr %ctxtopo, align 8
  ret i64 %v1343
b853:
  %ga1353 = getelementptr i64, ptr %area, i64 7
  %v1353 = load i8, ptr %ga1353
  %c383 = zext i8 %v1353 to i64
  %v1354 = icmp ne i64 %c383, 0
  br i1 %v1354, label %b855, label %b856
b854:
  %v1361 = call i64 @df.unbox_int(i64 %v0)
  %v1362 = load i8, ptr %ctx, align 8
  %c384 = zext i8 %v1362 to i64
  %v1363 = icmp ne i64 %c384, 0
  br i1 %v1363, label %b861, label %b860
b855:
  %ga1355 = getelementptr i64, ptr %area, i64 6
  %v1355 = load i64, ptr %ga1355
  br label %b857
b856:
  %v1356 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b857
b857:
  %v1357 = phi i64 [ %v1355, %b855 ], [ %v1356, %b856 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1357, i64 0, i64 0)
  %v1359 = load i8, ptr %ctx, align 8
  %c385 = zext i8 %v1359 to i64
  %v1360 = icmp ne i64 %c385, 0
  br i1 %v1360, label %b859, label %b858
b858:
  br label %b854
b859:
  %gcvolta859 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta859, ptr %ctxtopo, align 8
  ret i64 0
b860:
  %v1364 = add i64 0, 30
  %v1365 = ashr i64 %v1361, %v1364
  store i64 0, ptr %gcs0
  %v1366 = call i64 @df.box_int(i64 %v1365)
  %xcp1367 = load i8, ptr %ctx, align 8
  %xcn1367 = icmp ne i8 %xcp1367, 0
  %xce1367 = call i1 @llvm.expect.i1(i1 %xcn1367, i1 false)
  br i1 %xce1367, label %xc1367.limpar, label %xc1367.fim
xc1367.limpar:
  call void @dartforge_exception_clear()
  br label %xc1367.fim
xc1367.fim:
  %gcvolta860 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta860, ptr %ctxtopo, align 8
  ret i64 %v1366
b861:
  %gcvolta861 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta861, ptr %ctxtopo, align 8
  ret i64 0
b863:
  %ga1376 = getelementptr i64, ptr %area, i64 7
  %v1376 = load i8, ptr %ga1376
  %c386 = zext i8 %v1376 to i64
  %v1377 = icmp ne i64 %c386, 0
  br i1 %v1377, label %b865, label %b866
b864:
  %v1384 = call i64 @df.unbox_int(i64 %v0)
  %v1385 = load i8, ptr %ctx, align 8
  %c387 = zext i8 %v1385 to i64
  %v1386 = icmp ne i64 %c387, 0
  br i1 %v1386, label %b871, label %b870
b865:
  %ga1378 = getelementptr i64, ptr %area, i64 6
  %v1378 = load i64, ptr %ga1378
  br label %b867
b866:
  %v1379 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b867
b867:
  %v1380 = phi i64 [ %v1378, %b865 ], [ %v1379, %b866 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1380, i64 0, i64 0)
  %v1382 = load i8, ptr %ctx, align 8
  %c388 = zext i8 %v1382 to i64
  %v1383 = icmp ne i64 %c388, 0
  br i1 %v1383, label %b869, label %b868
b868:
  br label %b864
b869:
  %gcvolta869 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta869, ptr %ctxtopo, align 8
  ret i64 0
b870:
  %v1387 = add i64 0, 28
  %v1388 = ashr i64 %v1384, %v1387
  store i64 0, ptr %gcs0
  %v1389 = call i64 @df.box_int(i64 %v1388)
  %xcp1390 = load i8, ptr %ctx, align 8
  %xcn1390 = icmp ne i8 %xcp1390, 0
  %xce1390 = call i1 @llvm.expect.i1(i1 %xcn1390, i1 false)
  br i1 %xce1390, label %xc1390.limpar, label %xc1390.fim
xc1390.limpar:
  call void @dartforge_exception_clear()
  br label %xc1390.fim
xc1390.fim:
  %gcvolta870 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta870, ptr %ctxtopo, align 8
  ret i64 %v1389
b871:
  %gcvolta871 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta871, ptr %ctxtopo, align 8
  ret i64 0
b873:
  %ga1399 = getelementptr i64, ptr %area, i64 7
  %v1399 = load i8, ptr %ga1399
  %c389 = zext i8 %v1399 to i64
  %v1400 = icmp ne i64 %c389, 0
  br i1 %v1400, label %b875, label %b876
b874:
  %v1407 = call i64 @df.unbox_int(i64 %v0)
  %v1408 = load i8, ptr %ctx, align 8
  %c390 = zext i8 %v1408 to i64
  %v1409 = icmp ne i64 %c390, 0
  br i1 %v1409, label %b881, label %b880
b875:
  %ga1401 = getelementptr i64, ptr %area, i64 6
  %v1401 = load i64, ptr %ga1401
  br label %b877
b876:
  %v1402 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b877
b877:
  %v1403 = phi i64 [ %v1401, %b875 ], [ %v1402, %b876 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1403, i64 0, i64 0)
  %v1405 = load i8, ptr %ctx, align 8
  %c391 = zext i8 %v1405 to i64
  %v1406 = icmp ne i64 %c391, 0
  br i1 %v1406, label %b879, label %b878
b878:
  br label %b874
b879:
  %gcvolta879 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta879, ptr %ctxtopo, align 8
  ret i64 0
b880:
  %v1410 = add i64 0, 24
  %v1411 = ashr i64 %v1407, %v1410
  store i64 0, ptr %gcs0
  %v1412 = call i64 @df.box_int(i64 %v1411)
  %xcp1413 = load i8, ptr %ctx, align 8
  %xcn1413 = icmp ne i8 %xcp1413, 0
  %xce1413 = call i1 @llvm.expect.i1(i1 %xcn1413, i1 false)
  br i1 %xce1413, label %xc1413.limpar, label %xc1413.fim
xc1413.limpar:
  call void @dartforge_exception_clear()
  br label %xc1413.fim
xc1413.fim:
  %gcvolta880 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta880, ptr %ctxtopo, align 8
  ret i64 %v1412
b881:
  %gcvolta881 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta881, ptr %ctxtopo, align 8
  ret i64 0
b883:
  %ga1422 = getelementptr i64, ptr %area, i64 7
  %v1422 = load i8, ptr %ga1422
  %c392 = zext i8 %v1422 to i64
  %v1423 = icmp ne i64 %c392, 0
  br i1 %v1423, label %b885, label %b886
b884:
  %v1430 = call i64 @df.unbox_int(i64 %v0)
  %v1431 = load i8, ptr %ctx, align 8
  %c393 = zext i8 %v1431 to i64
  %v1432 = icmp ne i64 %c393, 0
  br i1 %v1432, label %b891, label %b890
b885:
  %ga1424 = getelementptr i64, ptr %area, i64 6
  %v1424 = load i64, ptr %ga1424
  br label %b887
b886:
  %v1425 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b887
b887:
  %v1426 = phi i64 [ %v1424, %b885 ], [ %v1425, %b886 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1426, i64 0, i64 0)
  %v1428 = load i8, ptr %ctx, align 8
  %c394 = zext i8 %v1428 to i64
  %v1429 = icmp ne i64 %c394, 0
  br i1 %v1429, label %b889, label %b888
b888:
  br label %b884
b889:
  %gcvolta889 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta889, ptr %ctxtopo, align 8
  ret i64 0
b890:
  %v1433 = add i64 0, 16
  %v1434 = ashr i64 %v1430, %v1433
  store i64 0, ptr %gcs0
  %v1435 = call i64 @df.box_int(i64 %v1434)
  %xcp1436 = load i8, ptr %ctx, align 8
  %xcn1436 = icmp ne i8 %xcp1436, 0
  %xce1436 = call i1 @llvm.expect.i1(i1 %xcn1436, i1 false)
  br i1 %xce1436, label %xc1436.limpar, label %xc1436.fim
xc1436.limpar:
  call void @dartforge_exception_clear()
  br label %xc1436.fim
xc1436.fim:
  %gcvolta890 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta890, ptr %ctxtopo, align 8
  ret i64 %v1435
b891:
  %gcvolta891 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta891, ptr %ctxtopo, align 8
  ret i64 0
b894:
  %ga1447 = getelementptr i64, ptr %area, i64 7
  %v1447 = load i8, ptr %ga1447
  %c395 = zext i8 %v1447 to i64
  %v1448 = icmp ne i64 %c395, 0
  br i1 %v1448, label %b896, label %b897
b895:
  %v1455 = call i64 @df.unbox_int(i64 %v0)
  %v1456 = load i8, ptr %ctx, align 8
  %c396 = zext i8 %v1456 to i64
  %v1457 = icmp ne i64 %c396, 0
  br i1 %v1457, label %b902, label %b901
b896:
  %ga1449 = getelementptr i64, ptr %area, i64 6
  %v1449 = load i64, ptr %ga1449
  br label %b898
b897:
  %v1450 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b898
b898:
  %v1451 = phi i64 [ %v1449, %b896 ], [ %v1450, %b897 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1451, i64 0, i64 0)
  %v1453 = load i8, ptr %ctx, align 8
  %c397 = zext i8 %v1453 to i64
  %v1454 = icmp ne i64 %c397, 0
  br i1 %v1454, label %b900, label %b899
b899:
  br label %b895
b900:
  %gcvolta900 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta900, ptr %ctxtopo, align 8
  ret i64 0
b901:
  %v1458 = add i64 0, 25
  %v1459 = ashr i64 %v1455, %v1458
  store i64 0, ptr %gcs0
  %v1460 = call i64 @df.box_int(i64 %v1459)
  %xcp1461 = load i8, ptr %ctx, align 8
  %xcn1461 = icmp ne i8 %xcp1461, 0
  %xce1461 = call i1 @llvm.expect.i1(i1 %xcn1461, i1 false)
  br i1 %xce1461, label %xc1461.limpar, label %xc1461.fim
xc1461.limpar:
  call void @dartforge_exception_clear()
  br label %xc1461.fim
xc1461.fim:
  %gcvolta901 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta901, ptr %ctxtopo, align 8
  ret i64 %v1460
b902:
  %gcvolta902 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta902, ptr %ctxtopo, align 8
  ret i64 0
b904:
  %ga1470 = getelementptr i64, ptr %area, i64 7
  %v1470 = load i8, ptr %ga1470
  %c398 = zext i8 %v1470 to i64
  %v1471 = icmp ne i64 %c398, 0
  br i1 %v1471, label %b906, label %b907
b905:
  %v1478 = call i64 @df.unbox_int(i64 %v0)
  %v1479 = load i8, ptr %ctx, align 8
  %c399 = zext i8 %v1479 to i64
  %v1480 = icmp ne i64 %c399, 0
  br i1 %v1480, label %b912, label %b911
b906:
  %ga1472 = getelementptr i64, ptr %area, i64 6
  %v1472 = load i64, ptr %ga1472
  br label %b908
b907:
  %v1473 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b908
b908:
  %v1474 = phi i64 [ %v1472, %b906 ], [ %v1473, %b907 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1474, i64 0, i64 0)
  %v1476 = load i8, ptr %ctx, align 8
  %c400 = zext i8 %v1476 to i64
  %v1477 = icmp ne i64 %c400, 0
  br i1 %v1477, label %b910, label %b909
b909:
  br label %b905
b910:
  %gcvolta910 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta910, ptr %ctxtopo, align 8
  ret i64 0
b911:
  %v1481 = add i64 0, 17
  %v1482 = ashr i64 %v1478, %v1481
  store i64 0, ptr %gcs0
  %v1483 = call i64 @df.box_int(i64 %v1482)
  %xcp1484 = load i8, ptr %ctx, align 8
  %xcn1484 = icmp ne i8 %xcp1484, 0
  %xce1484 = call i1 @llvm.expect.i1(i1 %xcn1484, i1 false)
  br i1 %xce1484, label %xc1484.limpar, label %xc1484.fim
xc1484.limpar:
  call void @dartforge_exception_clear()
  br label %xc1484.fim
xc1484.fim:
  %gcvolta911 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta911, ptr %ctxtopo, align 8
  ret i64 %v1483
b912:
  %gcvolta912 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta912, ptr %ctxtopo, align 8
  ret i64 0
b914:
  %ga1493 = getelementptr i64, ptr %area, i64 7
  %v1493 = load i8, ptr %ga1493
  %c401 = zext i8 %v1493 to i64
  %v1494 = icmp ne i64 %c401, 0
  br i1 %v1494, label %b916, label %b917
b915:
  %v1501 = call i64 @df.unbox_int(i64 %v0)
  %v1502 = load i8, ptr %ctx, align 8
  %c402 = zext i8 %v1502 to i64
  %v1503 = icmp ne i64 %c402, 0
  br i1 %v1503, label %b922, label %b921
b916:
  %ga1495 = getelementptr i64, ptr %area, i64 6
  %v1495 = load i64, ptr %ga1495
  br label %b918
b917:
  %v1496 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b918
b918:
  %v1497 = phi i64 [ %v1495, %b916 ], [ %v1496, %b917 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1497, i64 0, i64 0)
  %v1499 = load i8, ptr %ctx, align 8
  %c403 = zext i8 %v1499 to i64
  %v1500 = icmp ne i64 %c403, 0
  br i1 %v1500, label %b920, label %b919
b919:
  br label %b915
b920:
  %gcvolta920 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta920, ptr %ctxtopo, align 8
  ret i64 0
b921:
  %v1504 = add i64 0, 1
  %v1505 = ashr i64 %v1501, %v1504
  store i64 0, ptr %gcs0
  %v1506 = call i64 @df.box_int(i64 %v1505)
  %xcp1507 = load i8, ptr %ctx, align 8
  %xcn1507 = icmp ne i8 %xcp1507, 0
  %xce1507 = call i1 @llvm.expect.i1(i1 %xcn1507, i1 false)
  br i1 %xce1507, label %xc1507.limpar, label %xc1507.fim
xc1507.limpar:
  call void @dartforge_exception_clear()
  br label %xc1507.fim
xc1507.fim:
  %gcvolta921 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta921, ptr %ctxtopo, align 8
  ret i64 %v1506
b922:
  %gcvolta922 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta922, ptr %ctxtopo, align 8
  ret i64 0
b924:
  store i64 0, ptr %gcs1
  %v1514 = call double @df.unbox_double(i64 %v1511)
  %v1515 = load i8, ptr %ctx, align 8
  %c404 = zext i8 %v1515 to i64
  %v1516 = icmp ne i64 %c404, 0
  br i1 %v1516, label %b927, label %b926
b925:
  %gcvolta925 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta925, ptr %ctxtopo, align 8
  ret i64 0
b926:
  %xcp1518 = load i8, ptr %ctx, align 8
  %xcn1518 = icmp ne i8 %xcp1518, 0
  %xce1518 = call i1 @llvm.expect.i1(i1 %xcn1518, i1 false)
  br i1 %xce1518, label %xc1518.limpar, label %xc1518.fim
xc1518.limpar:
  call void @dartforge_exception_clear()
  br label %xc1518.fim
xc1518.fim:
  %gcvolta926 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta926, ptr %ctxtopo, align 8
  ret i64 %v1511
b927:
  %gcvolta927 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta927, ptr %ctxtopo, align 8
  ret i64 0
b941:
  %v1858 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1859 = call i64 @df.box_int(i64 %v1858)
  store i64 %v1859, ptr %gcs1
  %sa1860_0 = getelementptr [2 x i64], ptr %sargs1860, i64 0, i64 0
  store i64 %v1859, ptr %sa1860_0
  %sat1860 = getelementptr [2 x i64], ptr %sargs1860, i64 0, i64 1
  store i64 0, ptr %sat1860
  %sic1860 = getelementptr i64, ptr %area, i64 224
  %sf1860 = call ptr @df.selector(ptr %sic1860, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v1860 = call i64 %sf1860(i64 %v0, ptr %sargs1860, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1860, ptr %gcs0
  %v1861 = load i8, ptr %ctx, align 8
  %c405 = zext i8 %v1861 to i64
  %v1862 = icmp ne i64 %c405, 0
  br i1 %v1862, label %b1138, label %b1137
b942:
  store i64 0, ptr %gcs1
  %v1524 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1524, ptr %gcs1
  %v1525 = load i8, ptr %ctx, align 8
  %c406 = zext i8 %v1525 to i64
  %v1526 = icmp ne i64 %c406, 0
  br i1 %v1526, label %b948, label %b947
b943:
  %v1523 = icmp eq i64 %v1520, %v6
  br i1 %v1523, label %b945, label %b942
b944:
  %gcvolta944 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta944, ptr %ctxtopo, align 8
  ret i64 0
b945:
  %v1569 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1570 = call i64 @df.box_int(i64 %v1569)
  store i64 %v1570, ptr %gcs1
  %v1571 = icmp eq i64 %v0, 0
  %v1572 = icmp eq i64 %v1570, 0
  %c407 = zext i1 %v1571 to i64
  %c408 = zext i1 %v1572 to i64
  %v1573 = or i64 %c407, %c408
  %v1574 = icmp ne i64 %v1573, 0
  br i1 %v1574, label %b992, label %b993
b946:
  store i64 0, ptr %gcs1
  %v1528 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1528, ptr %gcs1
  %v1529 = load i8, ptr %ctx, align 8
  %c409 = zext i8 %v1529 to i64
  %v1530 = icmp ne i64 %c409, 0
  br i1 %v1530, label %b952, label %b951
b947:
  %v1527 = icmp eq i64 %v1524, %v6
  br i1 %v1527, label %b949, label %b946
b948:
  %gcvolta948 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta948, ptr %ctxtopo, align 8
  ret i64 0
b949:
  %v1601 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1602 = and i64 %v0, 1
  %v1603 = icmp ne i64 %v1602, 0
  br i1 %v1603, label %b1014, label %b1015
b950:
  store i64 0, ptr %gcs1
  %v1532 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1532, ptr %gcs1
  %v1533 = load i8, ptr %ctx, align 8
  %c410 = zext i8 %v1533 to i64
  %v1534 = icmp ne i64 %c410, 0
  br i1 %v1534, label %b956, label %b955
b951:
  %v1531 = icmp eq i64 %v1528, %v6
  br i1 %v1531, label %b953, label %b950
b952:
  %gcvolta952 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta952, ptr %ctxtopo, align 8
  ret i64 0
b953:
  %v1640 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1641 = and i64 %v0, 1
  %v1642 = icmp ne i64 %v1641, 0
  br i1 %v1642, label %b1034, label %b1035
b954:
  store i64 0, ptr %gcs1
  %v1536 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1536, ptr %gcs1
  %v1537 = load i8, ptr %ctx, align 8
  %c411 = zext i8 %v1537 to i64
  %v1538 = icmp ne i64 %c411, 0
  br i1 %v1538, label %b960, label %b959
b955:
  %v1535 = icmp eq i64 %v1532, %v6
  br i1 %v1535, label %b957, label %b954
b956:
  %gcvolta956 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta956, ptr %ctxtopo, align 8
  ret i64 0
b957:
  %v1679 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1680 = and i64 %v0, 1
  %v1681 = icmp ne i64 %v1680, 0
  br i1 %v1681, label %b1054, label %b1055
b958:
  store i64 0, ptr %gcs1
  %v1540 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1540, ptr %gcs1
  %v1541 = load i8, ptr %ctx, align 8
  %c412 = zext i8 %v1541 to i64
  %v1542 = icmp ne i64 %c412, 0
  br i1 %v1542, label %b964, label %b963
b959:
  %v1539 = icmp eq i64 %v1536, %v6
  br i1 %v1539, label %b961, label %b958
b960:
  %gcvolta960 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta960, ptr %ctxtopo, align 8
  ret i64 0
b961:
  %v1718 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1719 = and i64 %v0, 1
  %v1720 = icmp ne i64 %v1719, 0
  br i1 %v1720, label %b1074, label %b1075
b962:
  store i64 0, ptr %gcs1
  %v1544 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1544, ptr %gcs1
  %v1545 = load i8, ptr %ctx, align 8
  %c413 = zext i8 %v1545 to i64
  %v1546 = icmp ne i64 %c413, 0
  br i1 %v1546, label %b968, label %b967
b963:
  %v1543 = icmp eq i64 %v1540, %v6
  br i1 %v1543, label %b965, label %b962
b964:
  %gcvolta964 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta964, ptr %ctxtopo, align 8
  ret i64 0
b965:
  %v1757 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1758 = and i64 %v0, 1
  %v1759 = icmp ne i64 %v1758, 0
  br i1 %v1759, label %b1094, label %b1095
b966:
  store i64 0, ptr %gcs1
  %v1548 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1548, ptr %gcs1
  %v1549 = load i8, ptr %ctx, align 8
  %c414 = zext i8 %v1549 to i64
  %v1550 = icmp ne i64 %c414, 0
  br i1 %v1550, label %b972, label %b971
b967:
  %v1547 = icmp eq i64 %v1544, %v6
  br i1 %v1547, label %b969, label %b966
b968:
  %gcvolta968 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta968, ptr %ctxtopo, align 8
  ret i64 0
b969:
  %xcp1796 = load i8, ptr %ctx, align 8
  %xcn1796 = icmp ne i8 %xcp1796, 0
  %xce1796 = call i1 @llvm.expect.i1(i1 %xcn1796, i1 false)
  br i1 %xce1796, label %xc1796.limpar, label %xc1796.fim
xc1796.limpar:
  call void @dartforge_exception_clear()
  br label %xc1796.fim
xc1796.fim:
  %gcvolta969 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta969, ptr %ctxtopo, align 8
  ret i64 %v0
b970:
  store i64 0, ptr %gcs1
  %v1552 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1552, ptr %gcs1
  %v1553 = load i8, ptr %ctx, align 8
  %c415 = zext i8 %v1553 to i64
  %v1554 = icmp ne i64 %c415, 0
  br i1 %v1554, label %b976, label %b975
b971:
  %v1551 = icmp eq i64 %v1548, %v6
  br i1 %v1551, label %b973, label %b970
b972:
  %gcvolta972 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta972, ptr %ctxtopo, align 8
  ret i64 0
b973:
  %v1798 = add i64 0, 258
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1799 = and i64 %v0, 1
  %v1800 = icmp ne i64 %v1799, 0
  br i1 %v1800, label %b1115, label %b1116
b974:
  store i64 0, ptr %gcs1
  %v1556 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1556, ptr %gcs1
  %v1557 = load i8, ptr %ctx, align 8
  %c416 = zext i8 %v1557 to i64
  %v1558 = icmp ne i64 %c416, 0
  br i1 %v1558, label %b980, label %b979
b975:
  %v1555 = icmp eq i64 %v1552, %v6
  br i1 %v1555, label %b977, label %b974
b976:
  %gcvolta976 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta976, ptr %ctxtopo, align 8
  ret i64 0
b977:
  %v1828 = add i64 0, 16909320
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1829 = and i64 %v0, 1
  %v1830 = icmp ne i64 %v1829, 0
  br i1 %v1830, label %b1126, label %b1127
b978:
  store i64 0, ptr %gcs1
  %v1560 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1560, ptr %gcs1
  %v1561 = load i8, ptr %ctx, align 8
  %c417 = zext i8 %v1561 to i64
  %v1562 = icmp ne i64 %c417, 0
  br i1 %v1562, label %b985, label %b984
b979:
  %v1559 = icmp eq i64 %v1556, %v6
  br i1 %v1559, label %b981, label %b978
b980:
  %gcvolta980 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta980, ptr %ctxtopo, align 8
  ret i64 0
b981:
  br label %b941
b983:
  store i64 0, ptr %gcs1
  %v1564 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1564, ptr %gcs1
  %v1565 = load i8, ptr %ctx, align 8
  %c418 = zext i8 %v1565 to i64
  %v1566 = icmp ne i64 %c418, 0
  br i1 %v1566, label %b990, label %b989
b984:
  %v1563 = icmp eq i64 %v1560, %v6
  br i1 %v1563, label %b986, label %b983
b985:
  %gcvolta985 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta985, ptr %ctxtopo, align 8
  ret i64 0
b986:
  br label %b941
b988:
  br label %b12
b989:
  %v1567 = icmp eq i64 %v1564, %v6
  br i1 %v1567, label %b991, label %b988
b990:
  %gcvolta990 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta990, ptr %ctxtopo, align 8
  ret i64 0
b991:
  br label %b941
b992:
  %v1575 = icmp eq i64 %v0, %v1570
  br label %b994
b993:
  %v1576 = call i64 @df.class(i64 %v0)
  switch i64 %v1576, label %b998 [ i64 675, label %b995 i64 4, label %b996 i64 2, label %b997 i64 3, label %b997 ]
b994:
  %v1594 = phi i1 [ %v1575, %b992 ], [ %v1593, %b999 ]
  br i1 %v1594, label %b1010, label %b1011
b995:
  %v1577 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1570)
  %v1578 = load i8, ptr %ctx, align 8
  %c419 = zext i8 %v1578 to i64
  %v1579 = icmp ne i64 %c419, 0
  br i1 %v1579, label %b1001, label %b1000
b996:
  %v1581 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1570)
  %v1582 = load i8, ptr %ctx, align 8
  %c420 = zext i8 %v1582 to i64
  %v1583 = icmp ne i64 %c420, 0
  br i1 %v1583, label %b1003, label %b1002
b997:
  %v1584 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1570)
  %v1585 = load i8, ptr %ctx, align 8
  %c421 = zext i8 %v1585 to i64
  %v1586 = icmp ne i64 %c421, 0
  br i1 %v1586, label %b1005, label %b1004
b998:
  %sa1587_0 = getelementptr [2 x i64], ptr %sargs1587, i64 0, i64 0
  store i64 %v1570, ptr %sa1587_0
  %sat1587 = getelementptr [2 x i64], ptr %sargs1587, i64 0, i64 1
  store i64 0, ptr %sat1587
  %sic1587 = getelementptr i64, ptr %area, i64 226
  %sf1587 = call ptr @df.selector(ptr %sic1587, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1587 = call i64 %sf1587(i64 %v0, ptr %sargs1587, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1587, ptr %gcs0
  %v1588 = load i8, ptr %ctx, align 8
  %c422 = zext i8 %v1588 to i64
  %v1589 = icmp ne i64 %c422, 0
  br i1 %v1589, label %b1007, label %b1006
b999:
  %v1593 = phi i1 [ %v1580, %b1000 ], [ %v1581, %b1002 ], [ %v1584, %b1004 ], [ %v1590, %b1008 ]
  br label %b994
b1000:
  %c423 = zext i8 %v1577 to i64
  %v1580 = icmp ne i64 %c423, 0
  br label %b999
b1001:
  %gcvolta1001 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1001, ptr %ctxtopo, align 8
  ret i64 0
b1002:
  br label %b999
b1003:
  %gcvolta1003 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1003, ptr %ctxtopo, align 8
  ret i64 0
b1004:
  br label %b999
b1005:
  %gcvolta1005 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1005, ptr %ctxtopo, align 8
  ret i64 0
b1006:
  store i64 0, ptr %gcs1
  %v1590 = call i1 @df.unbox_bool(i64 %v1587)
  %v1591 = load i8, ptr %ctx, align 8
  %c424 = zext i8 %v1591 to i64
  %v1592 = icmp ne i64 %c424, 0
  br i1 %v1592, label %b1009, label %b1008
b1007:
  %gcvolta1007 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1007, ptr %ctxtopo, align 8
  ret i64 0
b1008:
  br label %b999
b1009:
  %gcvolta1009 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1009, ptr %ctxtopo, align 8
  ret i64 0
b1010:
  %v1595 = add i64 0, 0
  br label %b1012
b1011:
  %v1596 = add i64 0, 1
  br label %b1012
b1012:
  %v1597 = phi i64 [ %v1595, %b1010 ], [ %v1596, %b1011 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1598 = call i64 @df.box_int(i64 %v1597)
  %xcp1599 = load i8, ptr %ctx, align 8
  %xcn1599 = icmp ne i8 %xcp1599, 0
  %xce1599 = call i1 @llvm.expect.i1(i1 %xcn1599, i1 false)
  br i1 %xce1599, label %xc1599.limpar, label %xc1599.fim
xc1599.limpar:
  call void @dartforge_exception_clear()
  br label %xc1599.fim
xc1599.fim:
  %gcvolta1012 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1012, ptr %ctxtopo, align 8
  ret i64 %v1598
b1014:
  %v1604 = ashr i64 %v0, 1
  %v1605 = icmp sle i64 %v1604, %v1601
  br label %b1016
b1015:
  %v1606 = call i64 @df.box_int(i64 %v1601)
  store i64 %v1606, ptr %gcs1
  %sa1607_0 = getelementptr [2 x i64], ptr %sargs1607, i64 0, i64 0
  store i64 %v1606, ptr %sa1607_0
  %sat1607 = getelementptr [2 x i64], ptr %sargs1607, i64 0, i64 1
  store i64 0, ptr %sat1607
  %sic1607 = getelementptr i64, ptr %area, i64 228
  %sf1607 = call ptr @df.selector(ptr %sic1607, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1607 = call i64 %sf1607(i64 %v0, ptr %sargs1607, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1607, ptr %gcs1
  %v1608 = load i8, ptr %ctx, align 8
  %c425 = zext i8 %v1608 to i64
  %v1609 = icmp ne i64 %c425, 0
  br i1 %v1609, label %b1018, label %b1017
b1016:
  %v1613 = phi i1 [ %v1605, %b1014 ], [ %v1610, %b1019 ]
  br i1 %v1613, label %b1021, label %b1022
b1017:
  %v1610 = call i1 @df.unbox_bool(i64 %v1607)
  %v1611 = load i8, ptr %ctx, align 8
  %c426 = zext i8 %v1611 to i64
  %v1612 = icmp ne i64 %c426, 0
  br i1 %v1612, label %b1020, label %b1019
b1018:
  %gcvolta1018 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1018, ptr %ctxtopo, align 8
  ret i64 0
b1019:
  br label %b1016
b1020:
  %gcvolta1020 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1020, ptr %ctxtopo, align 8
  ret i64 0
b1021:
  %v1614 = add i64 0, 0
  br label %b1023
b1022:
  store i64 0, ptr %gcs1
  %v1616 = icmp eq i64 %v0, 0
  %v1617 = call i64 @df.class(i64 %v0)
  %v1618 = call i8 @df.subclass(i64 %v1617, i64 674)
  %c427 = zext i8 %v1618 to i64
  %v1619 = icmp ne i64 %c427, 0
  %v1620 = xor i1 %v1616, true
  %c428 = zext i1 %v1619 to i64
  %c429 = zext i1 %v1620 to i64
  %v1621 = and i64 %c428, %c429
  %v1622 = icmp ne i64 %v1621, 0
  br i1 %v1622, label %b1025, label %b1024
b1023:
  %v1636 = phi i64 [ %v1614, %b1021 ], [ %v1635, %b1031 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1637 = call i64 @df.box_int(i64 %v1636)
  %xcp1638 = load i8, ptr %ctx, align 8
  %xcn1638 = icmp ne i8 %xcp1638, 0
  %xce1638 = call i1 @llvm.expect.i1(i1 %xcn1638, i1 false)
  br i1 %xce1638, label %xc1638.limpar, label %xc1638.fim
xc1638.limpar:
  call void @dartforge_exception_clear()
  br label %xc1638.fim
xc1638.fim:
  %gcvolta1023 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1023, ptr %ctxtopo, align 8
  ret i64 %v1637
b1024:
  %ga1623 = getelementptr i64, ptr %area, i64 7
  %v1623 = load i8, ptr %ga1623
  %c430 = zext i8 %v1623 to i64
  %v1624 = icmp ne i64 %c430, 0
  br i1 %v1624, label %b1026, label %b1027
b1025:
  %v1631 = call i64 @df.unbox_int(i64 %v0)
  %v1632 = load i8, ptr %ctx, align 8
  %c431 = zext i8 %v1632 to i64
  %v1633 = icmp ne i64 %c431, 0
  br i1 %v1633, label %b1032, label %b1031
b1026:
  %ga1625 = getelementptr i64, ptr %area, i64 6
  %v1625 = load i64, ptr %ga1625
  br label %b1028
b1027:
  %v1626 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1028
b1028:
  %v1627 = phi i64 [ %v1625, %b1026 ], [ %v1626, %b1027 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1627, i64 0, i64 0)
  %v1629 = load i8, ptr %ctx, align 8
  %c432 = zext i8 %v1629 to i64
  %v1630 = icmp ne i64 %c432, 0
  br i1 %v1630, label %b1030, label %b1029
b1029:
  br label %b1025
b1030:
  %gcvolta1030 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1030, ptr %ctxtopo, align 8
  ret i64 0
b1031:
  %v1634 = add i64 0, 5
  %v1635 = ashr i64 %v1631, %v1634
  br label %b1023
b1032:
  %gcvolta1032 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1032, ptr %ctxtopo, align 8
  ret i64 0
b1034:
  %v1643 = ashr i64 %v0, 1
  %v1644 = icmp sle i64 %v1643, %v1640
  br label %b1036
b1035:
  %v1645 = call i64 @df.box_int(i64 %v1640)
  store i64 %v1645, ptr %gcs1
  %sa1646_0 = getelementptr [2 x i64], ptr %sargs1646, i64 0, i64 0
  store i64 %v1645, ptr %sa1646_0
  %sat1646 = getelementptr [2 x i64], ptr %sargs1646, i64 0, i64 1
  store i64 0, ptr %sat1646
  %sic1646 = getelementptr i64, ptr %area, i64 230
  %sf1646 = call ptr @df.selector(ptr %sic1646, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1646 = call i64 %sf1646(i64 %v0, ptr %sargs1646, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1646, ptr %gcs1
  %v1647 = load i8, ptr %ctx, align 8
  %c433 = zext i8 %v1647 to i64
  %v1648 = icmp ne i64 %c433, 0
  br i1 %v1648, label %b1038, label %b1037
b1036:
  %v1652 = phi i1 [ %v1644, %b1034 ], [ %v1649, %b1039 ]
  br i1 %v1652, label %b1041, label %b1042
b1037:
  %v1649 = call i1 @df.unbox_bool(i64 %v1646)
  %v1650 = load i8, ptr %ctx, align 8
  %c434 = zext i8 %v1650 to i64
  %v1651 = icmp ne i64 %c434, 0
  br i1 %v1651, label %b1040, label %b1039
b1038:
  %gcvolta1038 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1038, ptr %ctxtopo, align 8
  ret i64 0
b1039:
  br label %b1036
b1040:
  %gcvolta1040 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1040, ptr %ctxtopo, align 8
  ret i64 0
b1041:
  %v1653 = add i64 0, 0
  br label %b1043
b1042:
  store i64 0, ptr %gcs1
  %v1655 = icmp eq i64 %v0, 0
  %v1656 = call i64 @df.class(i64 %v0)
  %v1657 = call i8 @df.subclass(i64 %v1656, i64 674)
  %c435 = zext i8 %v1657 to i64
  %v1658 = icmp ne i64 %c435, 0
  %v1659 = xor i1 %v1655, true
  %c436 = zext i1 %v1658 to i64
  %c437 = zext i1 %v1659 to i64
  %v1660 = and i64 %c436, %c437
  %v1661 = icmp ne i64 %v1660, 0
  br i1 %v1661, label %b1045, label %b1044
b1043:
  %v1675 = phi i64 [ %v1653, %b1041 ], [ %v1674, %b1051 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1676 = call i64 @df.box_int(i64 %v1675)
  %xcp1677 = load i8, ptr %ctx, align 8
  %xcn1677 = icmp ne i8 %xcp1677, 0
  %xce1677 = call i1 @llvm.expect.i1(i1 %xcn1677, i1 false)
  br i1 %xce1677, label %xc1677.limpar, label %xc1677.fim
xc1677.limpar:
  call void @dartforge_exception_clear()
  br label %xc1677.fim
xc1677.fim:
  %gcvolta1043 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1043, ptr %ctxtopo, align 8
  ret i64 %v1676
b1044:
  %ga1662 = getelementptr i64, ptr %area, i64 7
  %v1662 = load i8, ptr %ga1662
  %c438 = zext i8 %v1662 to i64
  %v1663 = icmp ne i64 %c438, 0
  br i1 %v1663, label %b1046, label %b1047
b1045:
  %v1670 = call i64 @df.unbox_int(i64 %v0)
  %v1671 = load i8, ptr %ctx, align 8
  %c439 = zext i8 %v1671 to i64
  %v1672 = icmp ne i64 %c439, 0
  br i1 %v1672, label %b1052, label %b1051
b1046:
  %ga1664 = getelementptr i64, ptr %area, i64 6
  %v1664 = load i64, ptr %ga1664
  br label %b1048
b1047:
  %v1665 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1048
b1048:
  %v1666 = phi i64 [ %v1664, %b1046 ], [ %v1665, %b1047 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1666, i64 0, i64 0)
  %v1668 = load i8, ptr %ctx, align 8
  %c440 = zext i8 %v1668 to i64
  %v1669 = icmp ne i64 %c440, 0
  br i1 %v1669, label %b1050, label %b1049
b1049:
  br label %b1045
b1050:
  %gcvolta1050 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1050, ptr %ctxtopo, align 8
  ret i64 0
b1051:
  %v1673 = add i64 0, 3
  %v1674 = ashr i64 %v1670, %v1673
  br label %b1043
b1052:
  %gcvolta1052 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1052, ptr %ctxtopo, align 8
  ret i64 0
b1054:
  %v1682 = ashr i64 %v0, 1
  %v1683 = icmp sle i64 %v1682, %v1679
  br label %b1056
b1055:
  %v1684 = call i64 @df.box_int(i64 %v1679)
  store i64 %v1684, ptr %gcs1
  %sa1685_0 = getelementptr [2 x i64], ptr %sargs1685, i64 0, i64 0
  store i64 %v1684, ptr %sa1685_0
  %sat1685 = getelementptr [2 x i64], ptr %sargs1685, i64 0, i64 1
  store i64 0, ptr %sat1685
  %sic1685 = getelementptr i64, ptr %area, i64 232
  %sf1685 = call ptr @df.selector(ptr %sic1685, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1685 = call i64 %sf1685(i64 %v0, ptr %sargs1685, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1685, ptr %gcs1
  %v1686 = load i8, ptr %ctx, align 8
  %c441 = zext i8 %v1686 to i64
  %v1687 = icmp ne i64 %c441, 0
  br i1 %v1687, label %b1058, label %b1057
b1056:
  %v1691 = phi i1 [ %v1683, %b1054 ], [ %v1688, %b1059 ]
  br i1 %v1691, label %b1061, label %b1062
b1057:
  %v1688 = call i1 @df.unbox_bool(i64 %v1685)
  %v1689 = load i8, ptr %ctx, align 8
  %c442 = zext i8 %v1689 to i64
  %v1690 = icmp ne i64 %c442, 0
  br i1 %v1690, label %b1060, label %b1059
b1058:
  %gcvolta1058 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1058, ptr %ctxtopo, align 8
  ret i64 0
b1059:
  br label %b1056
b1060:
  %gcvolta1060 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1060, ptr %ctxtopo, align 8
  ret i64 0
b1061:
  %v1692 = add i64 0, 0
  br label %b1063
b1062:
  store i64 0, ptr %gcs1
  %v1694 = icmp eq i64 %v0, 0
  %v1695 = call i64 @df.class(i64 %v0)
  %v1696 = call i8 @df.subclass(i64 %v1695, i64 674)
  %c443 = zext i8 %v1696 to i64
  %v1697 = icmp ne i64 %c443, 0
  %v1698 = xor i1 %v1694, true
  %c444 = zext i1 %v1697 to i64
  %c445 = zext i1 %v1698 to i64
  %v1699 = and i64 %c444, %c445
  %v1700 = icmp ne i64 %v1699, 0
  br i1 %v1700, label %b1065, label %b1064
b1063:
  %v1714 = phi i64 [ %v1692, %b1061 ], [ %v1713, %b1071 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1715 = call i64 @df.box_int(i64 %v1714)
  %xcp1716 = load i8, ptr %ctx, align 8
  %xcn1716 = icmp ne i8 %xcp1716, 0
  %xce1716 = call i1 @llvm.expect.i1(i1 %xcn1716, i1 false)
  br i1 %xce1716, label %xc1716.limpar, label %xc1716.fim
xc1716.limpar:
  call void @dartforge_exception_clear()
  br label %xc1716.fim
xc1716.fim:
  %gcvolta1063 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1063, ptr %ctxtopo, align 8
  ret i64 %v1715
b1064:
  %ga1701 = getelementptr i64, ptr %area, i64 7
  %v1701 = load i8, ptr %ga1701
  %c446 = zext i8 %v1701 to i64
  %v1702 = icmp ne i64 %c446, 0
  br i1 %v1702, label %b1066, label %b1067
b1065:
  %v1709 = call i64 @df.unbox_int(i64 %v0)
  %v1710 = load i8, ptr %ctx, align 8
  %c447 = zext i8 %v1710 to i64
  %v1711 = icmp ne i64 %c447, 0
  br i1 %v1711, label %b1072, label %b1071
b1066:
  %ga1703 = getelementptr i64, ptr %area, i64 6
  %v1703 = load i64, ptr %ga1703
  br label %b1068
b1067:
  %v1704 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1068
b1068:
  %v1705 = phi i64 [ %v1703, %b1066 ], [ %v1704, %b1067 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1705, i64 0, i64 0)
  %v1707 = load i8, ptr %ctx, align 8
  %c448 = zext i8 %v1707 to i64
  %v1708 = icmp ne i64 %c448, 0
  br i1 %v1708, label %b1070, label %b1069
b1069:
  br label %b1065
b1070:
  %gcvolta1070 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1070, ptr %ctxtopo, align 8
  ret i64 0
b1071:
  %v1712 = add i64 0, 1
  %v1713 = shl i64 %v1709, %v1712
  br label %b1063
b1072:
  %gcvolta1072 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1072, ptr %ctxtopo, align 8
  ret i64 0
b1074:
  %v1721 = ashr i64 %v0, 1
  %v1722 = icmp sle i64 %v1721, %v1718
  br label %b1076
b1075:
  %v1723 = call i64 @df.box_int(i64 %v1718)
  store i64 %v1723, ptr %gcs1
  %sa1724_0 = getelementptr [2 x i64], ptr %sargs1724, i64 0, i64 0
  store i64 %v1723, ptr %sa1724_0
  %sat1724 = getelementptr [2 x i64], ptr %sargs1724, i64 0, i64 1
  store i64 0, ptr %sat1724
  %sic1724 = getelementptr i64, ptr %area, i64 234
  %sf1724 = call ptr @df.selector(ptr %sic1724, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1724 = call i64 %sf1724(i64 %v0, ptr %sargs1724, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1724, ptr %gcs1
  %v1725 = load i8, ptr %ctx, align 8
  %c449 = zext i8 %v1725 to i64
  %v1726 = icmp ne i64 %c449, 0
  br i1 %v1726, label %b1078, label %b1077
b1076:
  %v1730 = phi i1 [ %v1722, %b1074 ], [ %v1727, %b1079 ]
  br i1 %v1730, label %b1081, label %b1082
b1077:
  %v1727 = call i1 @df.unbox_bool(i64 %v1724)
  %v1728 = load i8, ptr %ctx, align 8
  %c450 = zext i8 %v1728 to i64
  %v1729 = icmp ne i64 %c450, 0
  br i1 %v1729, label %b1080, label %b1079
b1078:
  %gcvolta1078 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1078, ptr %ctxtopo, align 8
  ret i64 0
b1079:
  br label %b1076
b1080:
  %gcvolta1080 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1080, ptr %ctxtopo, align 8
  ret i64 0
b1081:
  %v1731 = add i64 0, 0
  br label %b1083
b1082:
  store i64 0, ptr %gcs1
  %v1733 = icmp eq i64 %v0, 0
  %v1734 = call i64 @df.class(i64 %v0)
  %v1735 = call i8 @df.subclass(i64 %v1734, i64 674)
  %c451 = zext i8 %v1735 to i64
  %v1736 = icmp ne i64 %c451, 0
  %v1737 = xor i1 %v1733, true
  %c452 = zext i1 %v1736 to i64
  %c453 = zext i1 %v1737 to i64
  %v1738 = and i64 %c452, %c453
  %v1739 = icmp ne i64 %v1738, 0
  br i1 %v1739, label %b1085, label %b1084
b1083:
  %v1753 = phi i64 [ %v1731, %b1081 ], [ %v1752, %b1091 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1754 = call i64 @df.box_int(i64 %v1753)
  %xcp1755 = load i8, ptr %ctx, align 8
  %xcn1755 = icmp ne i8 %xcp1755, 0
  %xce1755 = call i1 @llvm.expect.i1(i1 %xcn1755, i1 false)
  br i1 %xce1755, label %xc1755.limpar, label %xc1755.fim
xc1755.limpar:
  call void @dartforge_exception_clear()
  br label %xc1755.fim
xc1755.fim:
  %gcvolta1083 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1083, ptr %ctxtopo, align 8
  ret i64 %v1754
b1084:
  %ga1740 = getelementptr i64, ptr %area, i64 7
  %v1740 = load i8, ptr %ga1740
  %c454 = zext i8 %v1740 to i64
  %v1741 = icmp ne i64 %c454, 0
  br i1 %v1741, label %b1086, label %b1087
b1085:
  %v1748 = call i64 @df.unbox_int(i64 %v0)
  %v1749 = load i8, ptr %ctx, align 8
  %c455 = zext i8 %v1749 to i64
  %v1750 = icmp ne i64 %c455, 0
  br i1 %v1750, label %b1092, label %b1091
b1086:
  %ga1742 = getelementptr i64, ptr %area, i64 6
  %v1742 = load i64, ptr %ga1742
  br label %b1088
b1087:
  %v1743 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1088
b1088:
  %v1744 = phi i64 [ %v1742, %b1086 ], [ %v1743, %b1087 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1744, i64 0, i64 0)
  %v1746 = load i8, ptr %ctx, align 8
  %c456 = zext i8 %v1746 to i64
  %v1747 = icmp ne i64 %c456, 0
  br i1 %v1747, label %b1090, label %b1089
b1089:
  br label %b1085
b1090:
  %gcvolta1090 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1090, ptr %ctxtopo, align 8
  ret i64 0
b1091:
  %v1751 = add i64 0, 516
  %v1752 = mul i64 %v1748, %v1751
  br label %b1083
b1092:
  %gcvolta1092 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1092, ptr %ctxtopo, align 8
  ret i64 0
b1094:
  %v1760 = ashr i64 %v0, 1
  %v1761 = icmp sle i64 %v1760, %v1757
  br label %b1096
b1095:
  %v1762 = call i64 @df.box_int(i64 %v1757)
  store i64 %v1762, ptr %gcs1
  %sa1763_0 = getelementptr [2 x i64], ptr %sargs1763, i64 0, i64 0
  store i64 %v1762, ptr %sa1763_0
  %sat1763 = getelementptr [2 x i64], ptr %sargs1763, i64 0, i64 1
  store i64 0, ptr %sat1763
  %sic1763 = getelementptr i64, ptr %area, i64 236
  %sf1763 = call ptr @df.selector(ptr %sic1763, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1763 = call i64 %sf1763(i64 %v0, ptr %sargs1763, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1763, ptr %gcs1
  %v1764 = load i8, ptr %ctx, align 8
  %c457 = zext i8 %v1764 to i64
  %v1765 = icmp ne i64 %c457, 0
  br i1 %v1765, label %b1098, label %b1097
b1096:
  %v1769 = phi i1 [ %v1761, %b1094 ], [ %v1766, %b1099 ]
  br i1 %v1769, label %b1101, label %b1102
b1097:
  %v1766 = call i1 @df.unbox_bool(i64 %v1763)
  %v1767 = load i8, ptr %ctx, align 8
  %c458 = zext i8 %v1767 to i64
  %v1768 = icmp ne i64 %c458, 0
  br i1 %v1768, label %b1100, label %b1099
b1098:
  %gcvolta1098 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1098, ptr %ctxtopo, align 8
  ret i64 0
b1099:
  br label %b1096
b1100:
  %gcvolta1100 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1100, ptr %ctxtopo, align 8
  ret i64 0
b1101:
  %v1770 = add i64 0, 0
  br label %b1103
b1102:
  store i64 0, ptr %gcs1
  %v1772 = icmp eq i64 %v0, 0
  %v1773 = call i64 @df.class(i64 %v0)
  %v1774 = call i8 @df.subclass(i64 %v1773, i64 674)
  %c459 = zext i8 %v1774 to i64
  %v1775 = icmp ne i64 %c459, 0
  %v1776 = xor i1 %v1772, true
  %c460 = zext i1 %v1775 to i64
  %c461 = zext i1 %v1776 to i64
  %v1777 = and i64 %c460, %c461
  %v1778 = icmp ne i64 %v1777, 0
  br i1 %v1778, label %b1105, label %b1104
b1103:
  %v1792 = phi i64 [ %v1770, %b1101 ], [ %v1791, %b1111 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1793 = call i64 @df.box_int(i64 %v1792)
  %xcp1794 = load i8, ptr %ctx, align 8
  %xcn1794 = icmp ne i8 %xcp1794, 0
  %xce1794 = call i1 @llvm.expect.i1(i1 %xcn1794, i1 false)
  br i1 %xce1794, label %xc1794.limpar, label %xc1794.fim
xc1794.limpar:
  call void @dartforge_exception_clear()
  br label %xc1794.fim
xc1794.fim:
  %gcvolta1103 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1103, ptr %ctxtopo, align 8
  ret i64 %v1793
b1104:
  %ga1779 = getelementptr i64, ptr %area, i64 7
  %v1779 = load i8, ptr %ga1779
  %c462 = zext i8 %v1779 to i64
  %v1780 = icmp ne i64 %c462, 0
  br i1 %v1780, label %b1106, label %b1107
b1105:
  %v1787 = call i64 @df.unbox_int(i64 %v0)
  %v1788 = load i8, ptr %ctx, align 8
  %c463 = zext i8 %v1788 to i64
  %v1789 = icmp ne i64 %c463, 0
  br i1 %v1789, label %b1112, label %b1111
b1106:
  %ga1781 = getelementptr i64, ptr %area, i64 6
  %v1781 = load i64, ptr %ga1781
  br label %b1108
b1107:
  %v1782 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1108
b1108:
  %v1783 = phi i64 [ %v1781, %b1106 ], [ %v1782, %b1107 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1783, i64 0, i64 0)
  %v1785 = load i8, ptr %ctx, align 8
  %c464 = zext i8 %v1785 to i64
  %v1786 = icmp ne i64 %c464, 0
  br i1 %v1786, label %b1110, label %b1109
b1109:
  br label %b1105
b1110:
  %gcvolta1110 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1110, ptr %ctxtopo, align 8
  ret i64 0
b1111:
  %v1790 = add i64 0, 33818640
  %v1791 = mul i64 %v1787, %v1790
  br label %b1103
b1112:
  %gcvolta1112 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1112, ptr %ctxtopo, align 8
  ret i64 0
b1115:
  %v1801 = ashr i64 %v0, 1
  %v1802 = mul i64 %v1801, %v1798
  store i64 0, ptr %gcs0
  %v1803 = call i64 @df.box_int(i64 %v1802)
  br label %b1117
b1116:
  %v1804 = call i64 @df.box_int(i64 %v1798)
  store i64 %v1804, ptr %gcs1
  %v1805 = call i64 @df.class(i64 %v0)
  %v1806 = icmp eq i64 %v1805, 2
  %v1807 = icmp eq i64 %v1805, 3
  %c465 = zext i1 %v1806 to i64
  %c466 = zext i1 %v1807 to i64
  %v1808 = or i64 %c465, %c466
  %v1809 = icmp eq i64 %v1805, 4
  %c467 = zext i1 %v1809 to i64
  %v1810 = or i64 %v1808, %c467
  %v1811 = call i64 @df.class(i64 %v1804)
  %v1812 = icmp eq i64 %v1811, 2
  %v1813 = icmp eq i64 %v1811, 3
  %c468 = zext i1 %v1812 to i64
  %c469 = zext i1 %v1813 to i64
  %v1814 = or i64 %c468, %c469
  %v1815 = icmp eq i64 %v1811, 4
  %c470 = zext i1 %v1815 to i64
  %v1816 = or i64 %v1814, %c470
  %v1817 = and i64 %v1810, %v1816
  %c471 = trunc i64 %v1817 to i1
  br i1 %c471, label %b1118, label %b1119
b1117:
  %v1825 = phi i64 [ %v1803, %b1115 ], [ %v1824, %b1120 ]
  %xcp1826 = load i8, ptr %ctx, align 8
  %xcn1826 = icmp ne i8 %xcp1826, 0
  %xce1826 = call i1 @llvm.expect.i1(i1 %xcn1826, i1 false)
  br i1 %xce1826, label %xc1826.limpar, label %xc1826.fim
xc1826.limpar:
  call void @dartforge_exception_clear()
  br label %xc1826.fim
xc1826.fim:
  %gcvolta1117 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1117, ptr %ctxtopo, align 8
  ret i64 %v1825
b1118:
  %v1818 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v1804)
  %v1819 = load i8, ptr %ctx, align 8
  %c472 = zext i8 %v1819 to i64
  %v1820 = icmp ne i64 %c472, 0
  br i1 %v1820, label %b1122, label %b1121
b1119:
  %sa1821_0 = getelementptr [2 x i64], ptr %sargs1821, i64 0, i64 0
  store i64 %v1804, ptr %sa1821_0
  %sat1821 = getelementptr [2 x i64], ptr %sargs1821, i64 0, i64 1
  store i64 0, ptr %sat1821
  %sic1821 = getelementptr i64, ptr %area, i64 238
  %sf1821 = call ptr @df.selector(ptr %sic1821, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v1821 = call i64 %sf1821(i64 %v0, ptr %sargs1821, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1822 = load i8, ptr %ctx, align 8
  %c473 = zext i8 %v1822 to i64
  %v1823 = icmp ne i64 %c473, 0
  br i1 %v1823, label %b1124, label %b1123
b1120:
  %v1824 = phi i64 [ %v1818, %b1121 ], [ %v1821, %b1123 ]
  br label %b1117
b1121:
  br label %b1120
b1122:
  %gcvolta1122 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1122, ptr %ctxtopo, align 8
  ret i64 0
b1123:
  br label %b1120
b1124:
  %gcvolta1124 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1124, ptr %ctxtopo, align 8
  ret i64 0
b1126:
  %v1831 = ashr i64 %v0, 1
  %v1832 = mul i64 %v1831, %v1828
  store i64 0, ptr %gcs0
  %v1833 = call i64 @df.box_int(i64 %v1832)
  br label %b1128
b1127:
  %v1834 = call i64 @df.box_int(i64 %v1828)
  store i64 %v1834, ptr %gcs1
  %v1835 = call i64 @df.class(i64 %v0)
  %v1836 = icmp eq i64 %v1835, 2
  %v1837 = icmp eq i64 %v1835, 3
  %c474 = zext i1 %v1836 to i64
  %c475 = zext i1 %v1837 to i64
  %v1838 = or i64 %c474, %c475
  %v1839 = icmp eq i64 %v1835, 4
  %c476 = zext i1 %v1839 to i64
  %v1840 = or i64 %v1838, %c476
  %v1841 = call i64 @df.class(i64 %v1834)
  %v1842 = icmp eq i64 %v1841, 2
  %v1843 = icmp eq i64 %v1841, 3
  %c477 = zext i1 %v1842 to i64
  %c478 = zext i1 %v1843 to i64
  %v1844 = or i64 %c477, %c478
  %v1845 = icmp eq i64 %v1841, 4
  %c479 = zext i1 %v1845 to i64
  %v1846 = or i64 %v1844, %c479
  %v1847 = and i64 %v1840, %v1846
  %c480 = trunc i64 %v1847 to i1
  br i1 %c480, label %b1129, label %b1130
b1128:
  %v1855 = phi i64 [ %v1833, %b1126 ], [ %v1854, %b1131 ]
  %xcp1856 = load i8, ptr %ctx, align 8
  %xcn1856 = icmp ne i8 %xcp1856, 0
  %xce1856 = call i1 @llvm.expect.i1(i1 %xcn1856, i1 false)
  br i1 %xce1856, label %xc1856.limpar, label %xc1856.fim
xc1856.limpar:
  call void @dartforge_exception_clear()
  br label %xc1856.fim
xc1856.fim:
  %gcvolta1128 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1128, ptr %ctxtopo, align 8
  ret i64 %v1855
b1129:
  %v1848 = call i64 @dartforge_dyn_op(i64 2, i64 %v0, i64 %v1834)
  %v1849 = load i8, ptr %ctx, align 8
  %c481 = zext i8 %v1849 to i64
  %v1850 = icmp ne i64 %c481, 0
  br i1 %v1850, label %b1133, label %b1132
b1130:
  %sa1851_0 = getelementptr [2 x i64], ptr %sargs1851, i64 0, i64 0
  store i64 %v1834, ptr %sa1851_0
  %sat1851 = getelementptr [2 x i64], ptr %sargs1851, i64 0, i64 1
  store i64 0, ptr %sat1851
  %sic1851 = getelementptr i64, ptr %area, i64 240
  %sf1851 = call ptr @df.selector(ptr %sic1851, i64 %v0, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v1851 = call i64 %sf1851(i64 %v0, ptr %sargs1851, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1852 = load i8, ptr %ctx, align 8
  %c482 = zext i8 %v1852 to i64
  %v1853 = icmp ne i64 %c482, 0
  br i1 %v1853, label %b1135, label %b1134
b1131:
  %v1854 = phi i64 [ %v1848, %b1132 ], [ %v1851, %b1134 ]
  br label %b1128
b1132:
  br label %b1131
b1133:
  %gcvolta1133 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1133, ptr %ctxtopo, align 8
  ret i64 0
b1134:
  br label %b1131
b1135:
  %gcvolta1135 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1135, ptr %ctxtopo, align 8
  ret i64 0
b1137:
  store i64 0, ptr %gcs1
  %v1863 = call double @df.unbox_double(i64 %v1860)
  %v1864 = load i8, ptr %ctx, align 8
  %c483 = zext i8 %v1864 to i64
  %v1865 = icmp ne i64 %c483, 0
  br i1 %v1865, label %b1140, label %b1139
b1138:
  %gcvolta1138 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1138, ptr %ctxtopo, align 8
  ret i64 0
b1139:
  %xcp1867 = load i8, ptr %ctx, align 8
  %xcn1867 = icmp ne i8 %xcp1867, 0
  %xce1867 = call i1 @llvm.expect.i1(i1 %xcn1867, i1 false)
  br i1 %xce1867, label %xc1867.limpar, label %xc1867.fim
xc1867.limpar:
  call void @dartforge_exception_clear()
  br label %xc1867.fim
xc1867.fim:
  %gcvolta1139 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1139, ptr %ctxtopo, align 8
  ret i64 %v1860
b1140:
  %gcvolta1140 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1140, ptr %ctxtopo, align 8
  ret i64 0
b1154:
  %v2193 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2194 = call i64 @df.box_int(i64 %v2193)
  store i64 %v2194, ptr %gcs1
  %sa2195_0 = getelementptr [2 x i64], ptr %sargs2195, i64 0, i64 0
  store i64 %v2194, ptr %sa2195_0
  %sat2195 = getelementptr [2 x i64], ptr %sargs2195, i64 0, i64 1
  store i64 0, ptr %sat2195
  %sic2195 = getelementptr i64, ptr %area, i64 242
  %sf2195 = call ptr @df.selector(ptr %sic2195, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v2195 = call i64 %sf2195(i64 %v0, ptr %sargs2195, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2195, ptr %gcs0
  %v2196 = load i8, ptr %ctx, align 8
  %c484 = zext i8 %v2196 to i64
  %v2197 = icmp ne i64 %c484, 0
  br i1 %v2197, label %b1349, label %b1348
b1155:
  store i64 0, ptr %gcs1
  %v1873 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1873, ptr %gcs1
  %v1874 = load i8, ptr %ctx, align 8
  %c485 = zext i8 %v1874 to i64
  %v1875 = icmp ne i64 %c485, 0
  br i1 %v1875, label %b1161, label %b1160
b1156:
  %v1872 = icmp eq i64 %v1869, %v6
  br i1 %v1872, label %b1158, label %b1155
b1157:
  %gcvolta1157 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1157, ptr %ctxtopo, align 8
  ret i64 0
b1158:
  %v1918 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1919 = call i64 @df.box_int(i64 %v1918)
  store i64 %v1919, ptr %gcs1
  %v1920 = icmp eq i64 %v0, 0
  %v1921 = icmp eq i64 %v1919, 0
  %c486 = zext i1 %v1920 to i64
  %c487 = zext i1 %v1921 to i64
  %v1922 = or i64 %c486, %c487
  %v1923 = icmp ne i64 %v1922, 0
  br i1 %v1923, label %b1205, label %b1206
b1159:
  store i64 0, ptr %gcs1
  %v1877 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1877, ptr %gcs1
  %v1878 = load i8, ptr %ctx, align 8
  %c488 = zext i8 %v1878 to i64
  %v1879 = icmp ne i64 %c488, 0
  br i1 %v1879, label %b1165, label %b1164
b1160:
  %v1876 = icmp eq i64 %v1873, %v6
  br i1 %v1876, label %b1162, label %b1159
b1161:
  %gcvolta1161 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1161, ptr %ctxtopo, align 8
  ret i64 0
b1162:
  %v1950 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1951 = and i64 %v0, 1
  %v1952 = icmp ne i64 %v1951, 0
  br i1 %v1952, label %b1227, label %b1228
b1163:
  store i64 0, ptr %gcs1
  %v1881 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1881, ptr %gcs1
  %v1882 = load i8, ptr %ctx, align 8
  %c489 = zext i8 %v1882 to i64
  %v1883 = icmp ne i64 %c489, 0
  br i1 %v1883, label %b1169, label %b1168
b1164:
  %v1880 = icmp eq i64 %v1877, %v6
  br i1 %v1880, label %b1166, label %b1163
b1165:
  %gcvolta1165 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1165, ptr %ctxtopo, align 8
  ret i64 0
b1166:
  %v1989 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1990 = and i64 %v0, 1
  %v1991 = icmp ne i64 %v1990, 0
  br i1 %v1991, label %b1247, label %b1248
b1167:
  store i64 0, ptr %gcs1
  %v1885 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1885, ptr %gcs1
  %v1886 = load i8, ptr %ctx, align 8
  %c490 = zext i8 %v1886 to i64
  %v1887 = icmp ne i64 %c490, 0
  br i1 %v1887, label %b1173, label %b1172
b1168:
  %v1884 = icmp eq i64 %v1881, %v6
  br i1 %v1884, label %b1170, label %b1167
b1169:
  %gcvolta1169 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1169, ptr %ctxtopo, align 8
  ret i64 0
b1170:
  %v2028 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2029 = and i64 %v0, 1
  %v2030 = icmp ne i64 %v2029, 0
  br i1 %v2030, label %b1267, label %b1268
b1171:
  store i64 0, ptr %gcs1
  %v1889 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1889, ptr %gcs1
  %v1890 = load i8, ptr %ctx, align 8
  %c491 = zext i8 %v1890 to i64
  %v1891 = icmp ne i64 %c491, 0
  br i1 %v1891, label %b1177, label %b1176
b1172:
  %v1888 = icmp eq i64 %v1885, %v6
  br i1 %v1888, label %b1174, label %b1171
b1173:
  %gcvolta1173 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1173, ptr %ctxtopo, align 8
  ret i64 0
b1174:
  %v2067 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2068 = and i64 %v0, 1
  %v2069 = icmp ne i64 %v2068, 0
  br i1 %v2069, label %b1287, label %b1288
b1175:
  store i64 0, ptr %gcs1
  %v1893 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1893, ptr %gcs1
  %v1894 = load i8, ptr %ctx, align 8
  %c492 = zext i8 %v1894 to i64
  %v1895 = icmp ne i64 %c492, 0
  br i1 %v1895, label %b1181, label %b1180
b1176:
  %v1892 = icmp eq i64 %v1889, %v6
  br i1 %v1892, label %b1178, label %b1175
b1177:
  %gcvolta1177 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1177, ptr %ctxtopo, align 8
  ret i64 0
b1178:
  %v2106 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2107 = and i64 %v0, 1
  %v2108 = icmp ne i64 %v2107, 0
  br i1 %v2108, label %b1307, label %b1308
b1179:
  store i64 0, ptr %gcs1
  %v1897 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1897, ptr %gcs1
  %v1898 = load i8, ptr %ctx, align 8
  %c493 = zext i8 %v1898 to i64
  %v1899 = icmp ne i64 %c493, 0
  br i1 %v1899, label %b1185, label %b1184
b1180:
  %v1896 = icmp eq i64 %v1893, %v6
  br i1 %v1896, label %b1182, label %b1179
b1181:
  %gcvolta1181 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1181, ptr %ctxtopo, align 8
  ret i64 0
b1182:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2145 = icmp eq i64 %v0, 0
  %v2146 = call i64 @df.class(i64 %v0)
  %v2147 = call i8 @df.subclass(i64 %v2146, i64 674)
  %c494 = zext i8 %v2147 to i64
  %v2148 = icmp ne i64 %c494, 0
  %v2149 = xor i1 %v2145, true
  %c495 = zext i1 %v2148 to i64
  %c496 = zext i1 %v2149 to i64
  %v2150 = and i64 %c495, %c496
  %v2151 = icmp ne i64 %v2150, 0
  br i1 %v2151, label %b1328, label %b1327
b1183:
  store i64 0, ptr %gcs1
  %v1901 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1901, ptr %gcs1
  %v1902 = load i8, ptr %ctx, align 8
  %c497 = zext i8 %v1902 to i64
  %v1903 = icmp ne i64 %c497, 0
  br i1 %v1903, label %b1189, label %b1188
b1184:
  %v1900 = icmp eq i64 %v1897, %v6
  br i1 %v1900, label %b1186, label %b1183
b1185:
  %gcvolta1185 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1185, ptr %ctxtopo, align 8
  ret i64 0
b1186:
  %xcp2168 = load i8, ptr %ctx, align 8
  %xcn2168 = icmp ne i8 %xcp2168, 0
  %xce2168 = call i1 @llvm.expect.i1(i1 %xcn2168, i1 false)
  br i1 %xce2168, label %xc2168.limpar, label %xc2168.fim
xc2168.limpar:
  call void @dartforge_exception_clear()
  br label %xc2168.fim
xc2168.fim:
  %gcvolta1186 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1186, ptr %ctxtopo, align 8
  ret i64 %v0
b1187:
  store i64 0, ptr %gcs1
  %v1905 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1905, ptr %gcs1
  %v1906 = load i8, ptr %ctx, align 8
  %c498 = zext i8 %v1906 to i64
  %v1907 = icmp ne i64 %c498, 0
  br i1 %v1907, label %b1193, label %b1192
b1188:
  %v1904 = icmp eq i64 %v1901, %v6
  br i1 %v1904, label %b1190, label %b1187
b1189:
  %gcvolta1189 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1189, ptr %ctxtopo, align 8
  ret i64 0
b1190:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2170 = icmp eq i64 %v0, 0
  %v2171 = call i64 @df.class(i64 %v0)
  %v2172 = call i8 @df.subclass(i64 %v2171, i64 674)
  %c499 = zext i8 %v2172 to i64
  %v2173 = icmp ne i64 %c499, 0
  %v2174 = xor i1 %v2170, true
  %c500 = zext i1 %v2173 to i64
  %c501 = zext i1 %v2174 to i64
  %v2175 = and i64 %c500, %c501
  %v2176 = icmp ne i64 %v2175, 0
  br i1 %v2176, label %b1339, label %b1338
b1191:
  store i64 0, ptr %gcs1
  %v1909 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1909, ptr %gcs1
  %v1910 = load i8, ptr %ctx, align 8
  %c502 = zext i8 %v1910 to i64
  %v1911 = icmp ne i64 %c502, 0
  br i1 %v1911, label %b1198, label %b1197
b1192:
  %v1908 = icmp eq i64 %v1905, %v6
  br i1 %v1908, label %b1194, label %b1191
b1193:
  %gcvolta1193 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1193, ptr %ctxtopo, align 8
  ret i64 0
b1194:
  br label %b1154
b1196:
  store i64 0, ptr %gcs1
  %v1913 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1913, ptr %gcs1
  %v1914 = load i8, ptr %ctx, align 8
  %c503 = zext i8 %v1914 to i64
  %v1915 = icmp ne i64 %c503, 0
  br i1 %v1915, label %b1203, label %b1202
b1197:
  %v1912 = icmp eq i64 %v1909, %v6
  br i1 %v1912, label %b1199, label %b1196
b1198:
  %gcvolta1198 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1198, ptr %ctxtopo, align 8
  ret i64 0
b1199:
  br label %b1154
b1201:
  br label %b12
b1202:
  %v1916 = icmp eq i64 %v1913, %v6
  br i1 %v1916, label %b1204, label %b1201
b1203:
  %gcvolta1203 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1203, ptr %ctxtopo, align 8
  ret i64 0
b1204:
  br label %b1154
b1205:
  %v1924 = icmp eq i64 %v0, %v1919
  br label %b1207
b1206:
  %v1925 = call i64 @df.class(i64 %v0)
  switch i64 %v1925, label %b1211 [ i64 675, label %b1208 i64 4, label %b1209 i64 2, label %b1210 i64 3, label %b1210 ]
b1207:
  %v1943 = phi i1 [ %v1924, %b1205 ], [ %v1942, %b1212 ]
  br i1 %v1943, label %b1223, label %b1224
b1208:
  %v1926 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1919)
  %v1927 = load i8, ptr %ctx, align 8
  %c504 = zext i8 %v1927 to i64
  %v1928 = icmp ne i64 %c504, 0
  br i1 %v1928, label %b1214, label %b1213
b1209:
  %v1930 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1919)
  %v1931 = load i8, ptr %ctx, align 8
  %c505 = zext i8 %v1931 to i64
  %v1932 = icmp ne i64 %c505, 0
  br i1 %v1932, label %b1216, label %b1215
b1210:
  %v1933 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1919)
  %v1934 = load i8, ptr %ctx, align 8
  %c506 = zext i8 %v1934 to i64
  %v1935 = icmp ne i64 %c506, 0
  br i1 %v1935, label %b1218, label %b1217
b1211:
  %sa1936_0 = getelementptr [2 x i64], ptr %sargs1936, i64 0, i64 0
  store i64 %v1919, ptr %sa1936_0
  %sat1936 = getelementptr [2 x i64], ptr %sargs1936, i64 0, i64 1
  store i64 0, ptr %sat1936
  %sic1936 = getelementptr i64, ptr %area, i64 244
  %sf1936 = call ptr @df.selector(ptr %sic1936, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v1936 = call i64 %sf1936(i64 %v0, ptr %sargs1936, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1936, ptr %gcs0
  %v1937 = load i8, ptr %ctx, align 8
  %c507 = zext i8 %v1937 to i64
  %v1938 = icmp ne i64 %c507, 0
  br i1 %v1938, label %b1220, label %b1219
b1212:
  %v1942 = phi i1 [ %v1929, %b1213 ], [ %v1930, %b1215 ], [ %v1933, %b1217 ], [ %v1939, %b1221 ]
  br label %b1207
b1213:
  %c508 = zext i8 %v1926 to i64
  %v1929 = icmp ne i64 %c508, 0
  br label %b1212
b1214:
  %gcvolta1214 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1214, ptr %ctxtopo, align 8
  ret i64 0
b1215:
  br label %b1212
b1216:
  %gcvolta1216 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1216, ptr %ctxtopo, align 8
  ret i64 0
b1217:
  br label %b1212
b1218:
  %gcvolta1218 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1218, ptr %ctxtopo, align 8
  ret i64 0
b1219:
  store i64 0, ptr %gcs1
  %v1939 = call i1 @df.unbox_bool(i64 %v1936)
  %v1940 = load i8, ptr %ctx, align 8
  %c509 = zext i8 %v1940 to i64
  %v1941 = icmp ne i64 %c509, 0
  br i1 %v1941, label %b1222, label %b1221
b1220:
  %gcvolta1220 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1220, ptr %ctxtopo, align 8
  ret i64 0
b1221:
  br label %b1212
b1222:
  %gcvolta1222 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1222, ptr %ctxtopo, align 8
  ret i64 0
b1223:
  %v1944 = add i64 0, 0
  br label %b1225
b1224:
  %v1945 = add i64 0, 1
  br label %b1225
b1225:
  %v1946 = phi i64 [ %v1944, %b1223 ], [ %v1945, %b1224 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1947 = call i64 @df.box_int(i64 %v1946)
  %xcp1948 = load i8, ptr %ctx, align 8
  %xcn1948 = icmp ne i8 %xcp1948, 0
  %xce1948 = call i1 @llvm.expect.i1(i1 %xcn1948, i1 false)
  br i1 %xce1948, label %xc1948.limpar, label %xc1948.fim
xc1948.limpar:
  call void @dartforge_exception_clear()
  br label %xc1948.fim
xc1948.fim:
  %gcvolta1225 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1225, ptr %ctxtopo, align 8
  ret i64 %v1947
b1227:
  %v1953 = ashr i64 %v0, 1
  %v1954 = icmp sle i64 %v1953, %v1950
  br label %b1229
b1228:
  %v1955 = call i64 @df.box_int(i64 %v1950)
  store i64 %v1955, ptr %gcs1
  %sa1956_0 = getelementptr [2 x i64], ptr %sargs1956, i64 0, i64 0
  store i64 %v1955, ptr %sa1956_0
  %sat1956 = getelementptr [2 x i64], ptr %sargs1956, i64 0, i64 1
  store i64 0, ptr %sat1956
  %sic1956 = getelementptr i64, ptr %area, i64 246
  %sf1956 = call ptr @df.selector(ptr %sic1956, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1956 = call i64 %sf1956(i64 %v0, ptr %sargs1956, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1956, ptr %gcs1
  %v1957 = load i8, ptr %ctx, align 8
  %c510 = zext i8 %v1957 to i64
  %v1958 = icmp ne i64 %c510, 0
  br i1 %v1958, label %b1231, label %b1230
b1229:
  %v1962 = phi i1 [ %v1954, %b1227 ], [ %v1959, %b1232 ]
  br i1 %v1962, label %b1234, label %b1235
b1230:
  %v1959 = call i1 @df.unbox_bool(i64 %v1956)
  %v1960 = load i8, ptr %ctx, align 8
  %c511 = zext i8 %v1960 to i64
  %v1961 = icmp ne i64 %c511, 0
  br i1 %v1961, label %b1233, label %b1232
b1231:
  %gcvolta1231 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1231, ptr %ctxtopo, align 8
  ret i64 0
b1232:
  br label %b1229
b1233:
  %gcvolta1233 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1233, ptr %ctxtopo, align 8
  ret i64 0
b1234:
  %v1963 = add i64 0, 0
  br label %b1236
b1235:
  store i64 0, ptr %gcs1
  %v1965 = icmp eq i64 %v0, 0
  %v1966 = call i64 @df.class(i64 %v0)
  %v1967 = call i8 @df.subclass(i64 %v1966, i64 674)
  %c512 = zext i8 %v1967 to i64
  %v1968 = icmp ne i64 %c512, 0
  %v1969 = xor i1 %v1965, true
  %c513 = zext i1 %v1968 to i64
  %c514 = zext i1 %v1969 to i64
  %v1970 = and i64 %c513, %c514
  %v1971 = icmp ne i64 %v1970, 0
  br i1 %v1971, label %b1238, label %b1237
b1236:
  %v1985 = phi i64 [ %v1963, %b1234 ], [ %v1984, %b1244 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1986 = call i64 @df.box_int(i64 %v1985)
  %xcp1987 = load i8, ptr %ctx, align 8
  %xcn1987 = icmp ne i8 %xcp1987, 0
  %xce1987 = call i1 @llvm.expect.i1(i1 %xcn1987, i1 false)
  br i1 %xce1987, label %xc1987.limpar, label %xc1987.fim
xc1987.limpar:
  call void @dartforge_exception_clear()
  br label %xc1987.fim
xc1987.fim:
  %gcvolta1236 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1236, ptr %ctxtopo, align 8
  ret i64 %v1986
b1237:
  %ga1972 = getelementptr i64, ptr %area, i64 7
  %v1972 = load i8, ptr %ga1972
  %c515 = zext i8 %v1972 to i64
  %v1973 = icmp ne i64 %c515, 0
  br i1 %v1973, label %b1239, label %b1240
b1238:
  %v1980 = call i64 @df.unbox_int(i64 %v0)
  %v1981 = load i8, ptr %ctx, align 8
  %c516 = zext i8 %v1981 to i64
  %v1982 = icmp ne i64 %c516, 0
  br i1 %v1982, label %b1245, label %b1244
b1239:
  %ga1974 = getelementptr i64, ptr %area, i64 6
  %v1974 = load i64, ptr %ga1974
  br label %b1241
b1240:
  %v1975 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1241
b1241:
  %v1976 = phi i64 [ %v1974, %b1239 ], [ %v1975, %b1240 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1976, i64 0, i64 0)
  %v1978 = load i8, ptr %ctx, align 8
  %c517 = zext i8 %v1978 to i64
  %v1979 = icmp ne i64 %c517, 0
  br i1 %v1979, label %b1243, label %b1242
b1242:
  br label %b1238
b1243:
  %gcvolta1243 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1243, ptr %ctxtopo, align 8
  ret i64 0
b1244:
  %v1983 = add i64 0, 15
  %v1984 = ashr i64 %v1980, %v1983
  br label %b1236
b1245:
  %gcvolta1245 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1245, ptr %ctxtopo, align 8
  ret i64 0
b1247:
  %v1992 = ashr i64 %v0, 1
  %v1993 = icmp sle i64 %v1992, %v1989
  br label %b1249
b1248:
  %v1994 = call i64 @df.box_int(i64 %v1989)
  store i64 %v1994, ptr %gcs1
  %sa1995_0 = getelementptr [2 x i64], ptr %sargs1995, i64 0, i64 0
  store i64 %v1994, ptr %sa1995_0
  %sat1995 = getelementptr [2 x i64], ptr %sargs1995, i64 0, i64 1
  store i64 0, ptr %sat1995
  %sic1995 = getelementptr i64, ptr %area, i64 248
  %sf1995 = call ptr @df.selector(ptr %sic1995, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v1995 = call i64 %sf1995(i64 %v0, ptr %sargs1995, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1995, ptr %gcs1
  %v1996 = load i8, ptr %ctx, align 8
  %c518 = zext i8 %v1996 to i64
  %v1997 = icmp ne i64 %c518, 0
  br i1 %v1997, label %b1251, label %b1250
b1249:
  %v2001 = phi i1 [ %v1993, %b1247 ], [ %v1998, %b1252 ]
  br i1 %v2001, label %b1254, label %b1255
b1250:
  %v1998 = call i1 @df.unbox_bool(i64 %v1995)
  %v1999 = load i8, ptr %ctx, align 8
  %c519 = zext i8 %v1999 to i64
  %v2000 = icmp ne i64 %c519, 0
  br i1 %v2000, label %b1253, label %b1252
b1251:
  %gcvolta1251 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1251, ptr %ctxtopo, align 8
  ret i64 0
b1252:
  br label %b1249
b1253:
  %gcvolta1253 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1253, ptr %ctxtopo, align 8
  ret i64 0
b1254:
  %v2002 = add i64 0, 0
  br label %b1256
b1255:
  store i64 0, ptr %gcs1
  %v2004 = icmp eq i64 %v0, 0
  %v2005 = call i64 @df.class(i64 %v0)
  %v2006 = call i8 @df.subclass(i64 %v2005, i64 674)
  %c520 = zext i8 %v2006 to i64
  %v2007 = icmp ne i64 %c520, 0
  %v2008 = xor i1 %v2004, true
  %c521 = zext i1 %v2007 to i64
  %c522 = zext i1 %v2008 to i64
  %v2009 = and i64 %c521, %c522
  %v2010 = icmp ne i64 %v2009, 0
  br i1 %v2010, label %b1258, label %b1257
b1256:
  %v2024 = phi i64 [ %v2002, %b1254 ], [ %v2023, %b1264 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2025 = call i64 @df.box_int(i64 %v2024)
  %xcp2026 = load i8, ptr %ctx, align 8
  %xcn2026 = icmp ne i8 %xcp2026, 0
  %xce2026 = call i1 @llvm.expect.i1(i1 %xcn2026, i1 false)
  br i1 %xce2026, label %xc2026.limpar, label %xc2026.fim
xc2026.limpar:
  call void @dartforge_exception_clear()
  br label %xc2026.fim
xc2026.fim:
  %gcvolta1256 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1256, ptr %ctxtopo, align 8
  ret i64 %v2025
b1257:
  %ga2011 = getelementptr i64, ptr %area, i64 7
  %v2011 = load i8, ptr %ga2011
  %c523 = zext i8 %v2011 to i64
  %v2012 = icmp ne i64 %c523, 0
  br i1 %v2012, label %b1259, label %b1260
b1258:
  %v2019 = call i64 @df.unbox_int(i64 %v0)
  %v2020 = load i8, ptr %ctx, align 8
  %c524 = zext i8 %v2020 to i64
  %v2021 = icmp ne i64 %c524, 0
  br i1 %v2021, label %b1265, label %b1264
b1259:
  %ga2013 = getelementptr i64, ptr %area, i64 6
  %v2013 = load i64, ptr %ga2013
  br label %b1261
b1260:
  %v2014 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1261
b1261:
  %v2015 = phi i64 [ %v2013, %b1259 ], [ %v2014, %b1260 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2015, i64 0, i64 0)
  %v2017 = load i8, ptr %ctx, align 8
  %c525 = zext i8 %v2017 to i64
  %v2018 = icmp ne i64 %c525, 0
  br i1 %v2018, label %b1263, label %b1262
b1262:
  br label %b1258
b1263:
  %gcvolta1263 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1263, ptr %ctxtopo, align 8
  ret i64 0
b1264:
  %v2022 = add i64 0, 11
  %v2023 = ashr i64 %v2019, %v2022
  br label %b1256
b1265:
  %gcvolta1265 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1265, ptr %ctxtopo, align 8
  ret i64 0
b1267:
  %v2031 = ashr i64 %v0, 1
  %v2032 = icmp sle i64 %v2031, %v2028
  br label %b1269
b1268:
  %v2033 = call i64 @df.box_int(i64 %v2028)
  store i64 %v2033, ptr %gcs1
  %sa2034_0 = getelementptr [2 x i64], ptr %sargs2034, i64 0, i64 0
  store i64 %v2033, ptr %sa2034_0
  %sat2034 = getelementptr [2 x i64], ptr %sargs2034, i64 0, i64 1
  store i64 0, ptr %sat2034
  %sic2034 = getelementptr i64, ptr %area, i64 250
  %sf2034 = call ptr @df.selector(ptr %sic2034, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2034 = call i64 %sf2034(i64 %v0, ptr %sargs2034, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2034, ptr %gcs1
  %v2035 = load i8, ptr %ctx, align 8
  %c526 = zext i8 %v2035 to i64
  %v2036 = icmp ne i64 %c526, 0
  br i1 %v2036, label %b1271, label %b1270
b1269:
  %v2040 = phi i1 [ %v2032, %b1267 ], [ %v2037, %b1272 ]
  br i1 %v2040, label %b1274, label %b1275
b1270:
  %v2037 = call i1 @df.unbox_bool(i64 %v2034)
  %v2038 = load i8, ptr %ctx, align 8
  %c527 = zext i8 %v2038 to i64
  %v2039 = icmp ne i64 %c527, 0
  br i1 %v2039, label %b1273, label %b1272
b1271:
  %gcvolta1271 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1271, ptr %ctxtopo, align 8
  ret i64 0
b1272:
  br label %b1269
b1273:
  %gcvolta1273 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1273, ptr %ctxtopo, align 8
  ret i64 0
b1274:
  %v2041 = add i64 0, 0
  br label %b1276
b1275:
  store i64 0, ptr %gcs1
  %v2043 = icmp eq i64 %v0, 0
  %v2044 = call i64 @df.class(i64 %v0)
  %v2045 = call i8 @df.subclass(i64 %v2044, i64 674)
  %c528 = zext i8 %v2045 to i64
  %v2046 = icmp ne i64 %c528, 0
  %v2047 = xor i1 %v2043, true
  %c529 = zext i1 %v2046 to i64
  %c530 = zext i1 %v2047 to i64
  %v2048 = and i64 %c529, %c530
  %v2049 = icmp ne i64 %v2048, 0
  br i1 %v2049, label %b1278, label %b1277
b1276:
  %v2063 = phi i64 [ %v2041, %b1274 ], [ %v2062, %b1284 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2064 = call i64 @df.box_int(i64 %v2063)
  %xcp2065 = load i8, ptr %ctx, align 8
  %xcn2065 = icmp ne i8 %xcp2065, 0
  %xce2065 = call i1 @llvm.expect.i1(i1 %xcn2065, i1 false)
  br i1 %xce2065, label %xc2065.limpar, label %xc2065.fim
xc2065.limpar:
  call void @dartforge_exception_clear()
  br label %xc2065.fim
xc2065.fim:
  %gcvolta1276 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1276, ptr %ctxtopo, align 8
  ret i64 %v2064
b1277:
  %ga2050 = getelementptr i64, ptr %area, i64 7
  %v2050 = load i8, ptr %ga2050
  %c531 = zext i8 %v2050 to i64
  %v2051 = icmp ne i64 %c531, 0
  br i1 %v2051, label %b1279, label %b1280
b1278:
  %v2058 = call i64 @df.unbox_int(i64 %v0)
  %v2059 = load i8, ptr %ctx, align 8
  %c532 = zext i8 %v2059 to i64
  %v2060 = icmp ne i64 %c532, 0
  br i1 %v2060, label %b1285, label %b1284
b1279:
  %ga2052 = getelementptr i64, ptr %area, i64 6
  %v2052 = load i64, ptr %ga2052
  br label %b1281
b1280:
  %v2053 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1281
b1281:
  %v2054 = phi i64 [ %v2052, %b1279 ], [ %v2053, %b1280 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2054, i64 0, i64 0)
  %v2056 = load i8, ptr %ctx, align 8
  %c533 = zext i8 %v2056 to i64
  %v2057 = icmp ne i64 %c533, 0
  br i1 %v2057, label %b1283, label %b1282
b1282:
  br label %b1278
b1283:
  %gcvolta1283 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1283, ptr %ctxtopo, align 8
  ret i64 0
b1284:
  %v2061 = add i64 0, 7
  %v2062 = ashr i64 %v2058, %v2061
  br label %b1276
b1285:
  %gcvolta1285 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1285, ptr %ctxtopo, align 8
  ret i64 0
b1287:
  %v2070 = ashr i64 %v0, 1
  %v2071 = icmp sle i64 %v2070, %v2067
  br label %b1289
b1288:
  %v2072 = call i64 @df.box_int(i64 %v2067)
  store i64 %v2072, ptr %gcs1
  %sa2073_0 = getelementptr [2 x i64], ptr %sargs2073, i64 0, i64 0
  store i64 %v2072, ptr %sa2073_0
  %sat2073 = getelementptr [2 x i64], ptr %sargs2073, i64 0, i64 1
  store i64 0, ptr %sat2073
  %sic2073 = getelementptr i64, ptr %area, i64 252
  %sf2073 = call ptr @df.selector(ptr %sic2073, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2073 = call i64 %sf2073(i64 %v0, ptr %sargs2073, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2073, ptr %gcs1
  %v2074 = load i8, ptr %ctx, align 8
  %c534 = zext i8 %v2074 to i64
  %v2075 = icmp ne i64 %c534, 0
  br i1 %v2075, label %b1291, label %b1290
b1289:
  %v2079 = phi i1 [ %v2071, %b1287 ], [ %v2076, %b1292 ]
  br i1 %v2079, label %b1294, label %b1295
b1290:
  %v2076 = call i1 @df.unbox_bool(i64 %v2073)
  %v2077 = load i8, ptr %ctx, align 8
  %c535 = zext i8 %v2077 to i64
  %v2078 = icmp ne i64 %c535, 0
  br i1 %v2078, label %b1293, label %b1292
b1291:
  %gcvolta1291 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1291, ptr %ctxtopo, align 8
  ret i64 0
b1292:
  br label %b1289
b1293:
  %gcvolta1293 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1293, ptr %ctxtopo, align 8
  ret i64 0
b1294:
  %v2080 = add i64 0, 0
  br label %b1296
b1295:
  store i64 0, ptr %gcs1
  %v2082 = icmp eq i64 %v0, 0
  %v2083 = call i64 @df.class(i64 %v0)
  %v2084 = call i8 @df.subclass(i64 %v2083, i64 674)
  %c536 = zext i8 %v2084 to i64
  %v2085 = icmp ne i64 %c536, 0
  %v2086 = xor i1 %v2082, true
  %c537 = zext i1 %v2085 to i64
  %c538 = zext i1 %v2086 to i64
  %v2087 = and i64 %c537, %c538
  %v2088 = icmp ne i64 %v2087, 0
  br i1 %v2088, label %b1298, label %b1297
b1296:
  %v2102 = phi i64 [ %v2080, %b1294 ], [ %v2101, %b1304 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2103 = call i64 @df.box_int(i64 %v2102)
  %xcp2104 = load i8, ptr %ctx, align 8
  %xcn2104 = icmp ne i8 %xcp2104, 0
  %xce2104 = call i1 @llvm.expect.i1(i1 %xcn2104, i1 false)
  br i1 %xce2104, label %xc2104.limpar, label %xc2104.fim
xc2104.limpar:
  call void @dartforge_exception_clear()
  br label %xc2104.fim
xc2104.fim:
  %gcvolta1296 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1296, ptr %ctxtopo, align 8
  ret i64 %v2103
b1297:
  %ga2089 = getelementptr i64, ptr %area, i64 7
  %v2089 = load i8, ptr %ga2089
  %c539 = zext i8 %v2089 to i64
  %v2090 = icmp ne i64 %c539, 0
  br i1 %v2090, label %b1299, label %b1300
b1298:
  %v2097 = call i64 @df.unbox_int(i64 %v0)
  %v2098 = load i8, ptr %ctx, align 8
  %c540 = zext i8 %v2098 to i64
  %v2099 = icmp ne i64 %c540, 0
  br i1 %v2099, label %b1305, label %b1304
b1299:
  %ga2091 = getelementptr i64, ptr %area, i64 6
  %v2091 = load i64, ptr %ga2091
  br label %b1301
b1300:
  %v2092 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1301
b1301:
  %v2093 = phi i64 [ %v2091, %b1299 ], [ %v2092, %b1300 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2093, i64 0, i64 0)
  %v2095 = load i8, ptr %ctx, align 8
  %c541 = zext i8 %v2095 to i64
  %v2096 = icmp ne i64 %c541, 0
  br i1 %v2096, label %b1303, label %b1302
b1302:
  br label %b1298
b1303:
  %gcvolta1303 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1303, ptr %ctxtopo, align 8
  ret i64 0
b1304:
  %v2100 = add i64 0, 1
  %v2101 = shl i64 %v2097, %v2100
  br label %b1296
b1305:
  %gcvolta1305 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1305, ptr %ctxtopo, align 8
  ret i64 0
b1307:
  %v2109 = ashr i64 %v0, 1
  %v2110 = icmp sle i64 %v2109, %v2106
  br label %b1309
b1308:
  %v2111 = call i64 @df.box_int(i64 %v2106)
  store i64 %v2111, ptr %gcs1
  %sa2112_0 = getelementptr [2 x i64], ptr %sargs2112, i64 0, i64 0
  store i64 %v2111, ptr %sa2112_0
  %sat2112 = getelementptr [2 x i64], ptr %sargs2112, i64 0, i64 1
  store i64 0, ptr %sat2112
  %sic2112 = getelementptr i64, ptr %area, i64 254
  %sf2112 = call ptr @df.selector(ptr %sic2112, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2112 = call i64 %sf2112(i64 %v0, ptr %sargs2112, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2112, ptr %gcs1
  %v2113 = load i8, ptr %ctx, align 8
  %c542 = zext i8 %v2113 to i64
  %v2114 = icmp ne i64 %c542, 0
  br i1 %v2114, label %b1311, label %b1310
b1309:
  %v2118 = phi i1 [ %v2110, %b1307 ], [ %v2115, %b1312 ]
  br i1 %v2118, label %b1314, label %b1315
b1310:
  %v2115 = call i1 @df.unbox_bool(i64 %v2112)
  %v2116 = load i8, ptr %ctx, align 8
  %c543 = zext i8 %v2116 to i64
  %v2117 = icmp ne i64 %c543, 0
  br i1 %v2117, label %b1313, label %b1312
b1311:
  %gcvolta1311 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1311, ptr %ctxtopo, align 8
  ret i64 0
b1312:
  br label %b1309
b1313:
  %gcvolta1313 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1313, ptr %ctxtopo, align 8
  ret i64 0
b1314:
  %v2119 = add i64 0, 0
  br label %b1316
b1315:
  store i64 0, ptr %gcs1
  %v2121 = icmp eq i64 %v0, 0
  %v2122 = call i64 @df.class(i64 %v0)
  %v2123 = call i8 @df.subclass(i64 %v2122, i64 674)
  %c544 = zext i8 %v2123 to i64
  %v2124 = icmp ne i64 %c544, 0
  %v2125 = xor i1 %v2121, true
  %c545 = zext i1 %v2124 to i64
  %c546 = zext i1 %v2125 to i64
  %v2126 = and i64 %c545, %c546
  %v2127 = icmp ne i64 %v2126, 0
  br i1 %v2127, label %b1318, label %b1317
b1316:
  %v2141 = phi i64 [ %v2119, %b1314 ], [ %v2140, %b1324 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2142 = call i64 @df.box_int(i64 %v2141)
  %xcp2143 = load i8, ptr %ctx, align 8
  %xcn2143 = icmp ne i8 %xcp2143, 0
  %xce2143 = call i1 @llvm.expect.i1(i1 %xcn2143, i1 false)
  br i1 %xce2143, label %xc2143.limpar, label %xc2143.fim
xc2143.limpar:
  call void @dartforge_exception_clear()
  br label %xc2143.fim
xc2143.fim:
  %gcvolta1316 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1316, ptr %ctxtopo, align 8
  ret i64 %v2142
b1317:
  %ga2128 = getelementptr i64, ptr %area, i64 7
  %v2128 = load i8, ptr %ga2128
  %c547 = zext i8 %v2128 to i64
  %v2129 = icmp ne i64 %c547, 0
  br i1 %v2129, label %b1319, label %b1320
b1318:
  %v2136 = call i64 @df.unbox_int(i64 %v0)
  %v2137 = load i8, ptr %ctx, align 8
  %c548 = zext i8 %v2137 to i64
  %v2138 = icmp ne i64 %c548, 0
  br i1 %v2138, label %b1325, label %b1324
b1319:
  %ga2130 = getelementptr i64, ptr %area, i64 6
  %v2130 = load i64, ptr %ga2130
  br label %b1321
b1320:
  %v2131 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1321
b1321:
  %v2132 = phi i64 [ %v2130, %b1319 ], [ %v2131, %b1320 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2132, i64 0, i64 0)
  %v2134 = load i8, ptr %ctx, align 8
  %c549 = zext i8 %v2134 to i64
  %v2135 = icmp ne i64 %c549, 0
  br i1 %v2135, label %b1323, label %b1322
b1322:
  br label %b1318
b1323:
  %gcvolta1323 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1323, ptr %ctxtopo, align 8
  ret i64 0
b1324:
  %v2139 = add i64 0, 131076
  %v2140 = mul i64 %v2136, %v2139
  br label %b1316
b1325:
  %gcvolta1325 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1325, ptr %ctxtopo, align 8
  ret i64 0
b1327:
  %ga2152 = getelementptr i64, ptr %area, i64 7
  %v2152 = load i8, ptr %ga2152
  %c550 = zext i8 %v2152 to i64
  %v2153 = icmp ne i64 %c550, 0
  br i1 %v2153, label %b1329, label %b1330
b1328:
  %v2160 = call i64 @df.unbox_int(i64 %v0)
  %v2161 = load i8, ptr %ctx, align 8
  %c551 = zext i8 %v2161 to i64
  %v2162 = icmp ne i64 %c551, 0
  br i1 %v2162, label %b1335, label %b1334
b1329:
  %ga2154 = getelementptr i64, ptr %area, i64 6
  %v2154 = load i64, ptr %ga2154
  br label %b1331
b1330:
  %v2155 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1331
b1331:
  %v2156 = phi i64 [ %v2154, %b1329 ], [ %v2155, %b1330 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2156, i64 0, i64 0)
  %v2158 = load i8, ptr %ctx, align 8
  %c552 = zext i8 %v2158 to i64
  %v2159 = icmp ne i64 %c552, 0
  br i1 %v2159, label %b1333, label %b1332
b1332:
  br label %b1328
b1333:
  %gcvolta1333 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1333, ptr %ctxtopo, align 8
  ret i64 0
b1334:
  %v2163 = add i64 0, 8
  %v2164 = ashr i64 %v2160, %v2163
  store i64 0, ptr %gcs0
  %v2165 = call i64 @df.box_int(i64 %v2164)
  %xcp2166 = load i8, ptr %ctx, align 8
  %xcn2166 = icmp ne i8 %xcp2166, 0
  %xce2166 = call i1 @llvm.expect.i1(i1 %xcn2166, i1 false)
  br i1 %xce2166, label %xc2166.limpar, label %xc2166.fim
xc2166.limpar:
  call void @dartforge_exception_clear()
  br label %xc2166.fim
xc2166.fim:
  %gcvolta1334 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1334, ptr %ctxtopo, align 8
  ret i64 %v2165
b1335:
  %gcvolta1335 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1335, ptr %ctxtopo, align 8
  ret i64 0
b1338:
  %ga2177 = getelementptr i64, ptr %area, i64 7
  %v2177 = load i8, ptr %ga2177
  %c553 = zext i8 %v2177 to i64
  %v2178 = icmp ne i64 %c553, 0
  br i1 %v2178, label %b1340, label %b1341
b1339:
  %v2185 = call i64 @df.unbox_int(i64 %v0)
  %v2186 = load i8, ptr %ctx, align 8
  %c554 = zext i8 %v2186 to i64
  %v2187 = icmp ne i64 %c554, 0
  br i1 %v2187, label %b1346, label %b1345
b1340:
  %ga2179 = getelementptr i64, ptr %area, i64 6
  %v2179 = load i64, ptr %ga2179
  br label %b1342
b1341:
  %v2180 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1342
b1342:
  %v2181 = phi i64 [ %v2179, %b1340 ], [ %v2180, %b1341 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2181, i64 0, i64 0)
  %v2183 = load i8, ptr %ctx, align 8
  %c555 = zext i8 %v2183 to i64
  %v2184 = icmp ne i64 %c555, 0
  br i1 %v2184, label %b1344, label %b1343
b1343:
  br label %b1339
b1344:
  %gcvolta1344 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1344, ptr %ctxtopo, align 8
  ret i64 0
b1345:
  %v2188 = add i64 0, 65538
  %v2189 = mul i64 %v2185, %v2188
  store i64 0, ptr %gcs0
  %v2190 = call i64 @df.box_int(i64 %v2189)
  %xcp2191 = load i8, ptr %ctx, align 8
  %xcn2191 = icmp ne i8 %xcp2191, 0
  %xce2191 = call i1 @llvm.expect.i1(i1 %xcn2191, i1 false)
  br i1 %xce2191, label %xc2191.limpar, label %xc2191.fim
xc2191.limpar:
  call void @dartforge_exception_clear()
  br label %xc2191.fim
xc2191.fim:
  %gcvolta1345 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1345, ptr %ctxtopo, align 8
  ret i64 %v2190
b1346:
  %gcvolta1346 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1346, ptr %ctxtopo, align 8
  ret i64 0
b1348:
  store i64 0, ptr %gcs1
  %v2198 = call double @df.unbox_double(i64 %v2195)
  %v2199 = load i8, ptr %ctx, align 8
  %c556 = zext i8 %v2199 to i64
  %v2200 = icmp ne i64 %c556, 0
  br i1 %v2200, label %b1351, label %b1350
b1349:
  %gcvolta1349 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1349, ptr %ctxtopo, align 8
  ret i64 0
b1350:
  %xcp2202 = load i8, ptr %ctx, align 8
  %xcn2202 = icmp ne i8 %xcp2202, 0
  %xce2202 = call i1 @llvm.expect.i1(i1 %xcn2202, i1 false)
  br i1 %xce2202, label %xc2202.limpar, label %xc2202.fim
xc2202.limpar:
  call void @dartforge_exception_clear()
  br label %xc2202.fim
xc2202.fim:
  %gcvolta1350 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1350, ptr %ctxtopo, align 8
  ret i64 %v2195
b1351:
  %gcvolta1351 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1351, ptr %ctxtopo, align 8
  ret i64 0
b1365:
  %v2528 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2529 = call i64 @df.box_int(i64 %v2528)
  store i64 %v2529, ptr %gcs1
  %sa2530_0 = getelementptr [2 x i64], ptr %sargs2530, i64 0, i64 0
  store i64 %v2529, ptr %sa2530_0
  %sat2530 = getelementptr [2 x i64], ptr %sargs2530, i64 0, i64 1
  store i64 0, ptr %sat2530
  %sic2530 = getelementptr i64, ptr %area, i64 256
  %sf2530 = call ptr @df.selector(ptr %sic2530, i64 %v0, i64 -4020828333918261805, ptr @df.seln.10, i64 4)
  %v2530 = call i64 %sf2530(i64 %v0, ptr %sargs2530, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2530, ptr %gcs0
  %v2531 = load i8, ptr %ctx, align 8
  %c557 = zext i8 %v2531 to i64
  %v2532 = icmp ne i64 %c557, 0
  br i1 %v2532, label %b1560, label %b1559
b1366:
  store i64 0, ptr %gcs1
  %v2208 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v2208, ptr %gcs1
  %v2209 = load i8, ptr %ctx, align 8
  %c558 = zext i8 %v2209 to i64
  %v2210 = icmp ne i64 %c558, 0
  br i1 %v2210, label %b1372, label %b1371
b1367:
  %v2207 = icmp eq i64 %v2204, %v6
  br i1 %v2207, label %b1369, label %b1366
b1368:
  %gcvolta1368 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1368, ptr %ctxtopo, align 8
  ret i64 0
b1369:
  %v2253 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2254 = call i64 @df.box_int(i64 %v2253)
  store i64 %v2254, ptr %gcs1
  %v2255 = icmp eq i64 %v0, 0
  %v2256 = icmp eq i64 %v2254, 0
  %c559 = zext i1 %v2255 to i64
  %c560 = zext i1 %v2256 to i64
  %v2257 = or i64 %c559, %c560
  %v2258 = icmp ne i64 %v2257, 0
  br i1 %v2258, label %b1416, label %b1417
b1370:
  store i64 0, ptr %gcs1
  %v2212 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v2212, ptr %gcs1
  %v2213 = load i8, ptr %ctx, align 8
  %c561 = zext i8 %v2213 to i64
  %v2214 = icmp ne i64 %c561, 0
  br i1 %v2214, label %b1376, label %b1375
b1371:
  %v2211 = icmp eq i64 %v2208, %v6
  br i1 %v2211, label %b1373, label %b1370
b1372:
  %gcvolta1372 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1372, ptr %ctxtopo, align 8
  ret i64 0
b1373:
  %v2285 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2286 = and i64 %v0, 1
  %v2287 = icmp ne i64 %v2286, 0
  br i1 %v2287, label %b1438, label %b1439
b1374:
  store i64 0, ptr %gcs1
  %v2216 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v2216, ptr %gcs1
  %v2217 = load i8, ptr %ctx, align 8
  %c562 = zext i8 %v2217 to i64
  %v2218 = icmp ne i64 %c562, 0
  br i1 %v2218, label %b1380, label %b1379
b1375:
  %v2215 = icmp eq i64 %v2212, %v6
  br i1 %v2215, label %b1377, label %b1374
b1376:
  %gcvolta1376 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1376, ptr %ctxtopo, align 8
  ret i64 0
b1377:
  %v2324 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2325 = and i64 %v0, 1
  %v2326 = icmp ne i64 %v2325, 0
  br i1 %v2326, label %b1458, label %b1459
b1378:
  store i64 0, ptr %gcs1
  %v2220 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v2220, ptr %gcs1
  %v2221 = load i8, ptr %ctx, align 8
  %c563 = zext i8 %v2221 to i64
  %v2222 = icmp ne i64 %c563, 0
  br i1 %v2222, label %b1384, label %b1383
b1379:
  %v2219 = icmp eq i64 %v2216, %v6
  br i1 %v2219, label %b1381, label %b1378
b1380:
  %gcvolta1380 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1380, ptr %ctxtopo, align 8
  ret i64 0
b1381:
  %v2363 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2364 = and i64 %v0, 1
  %v2365 = icmp ne i64 %v2364, 0
  br i1 %v2365, label %b1478, label %b1479
b1382:
  store i64 0, ptr %gcs1
  %v2224 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v2224, ptr %gcs1
  %v2225 = load i8, ptr %ctx, align 8
  %c564 = zext i8 %v2225 to i64
  %v2226 = icmp ne i64 %c564, 0
  br i1 %v2226, label %b1388, label %b1387
b1383:
  %v2223 = icmp eq i64 %v2220, %v6
  br i1 %v2223, label %b1385, label %b1382
b1384:
  %gcvolta1384 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1384, ptr %ctxtopo, align 8
  ret i64 0
b1385:
  %v2402 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2403 = and i64 %v0, 1
  %v2404 = icmp ne i64 %v2403, 0
  br i1 %v2404, label %b1498, label %b1499
b1386:
  store i64 0, ptr %gcs1
  %v2228 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v2228, ptr %gcs1
  %v2229 = load i8, ptr %ctx, align 8
  %c565 = zext i8 %v2229 to i64
  %v2230 = icmp ne i64 %c565, 0
  br i1 %v2230, label %b1392, label %b1391
b1387:
  %v2227 = icmp eq i64 %v2224, %v6
  br i1 %v2227, label %b1389, label %b1386
b1388:
  %gcvolta1388 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1388, ptr %ctxtopo, align 8
  ret i64 0
b1389:
  %v2441 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2442 = and i64 %v0, 1
  %v2443 = icmp ne i64 %v2442, 0
  br i1 %v2443, label %b1518, label %b1519
b1390:
  store i64 0, ptr %gcs1
  %v2232 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v2232, ptr %gcs1
  %v2233 = load i8, ptr %ctx, align 8
  %c566 = zext i8 %v2233 to i64
  %v2234 = icmp ne i64 %c566, 0
  br i1 %v2234, label %b1396, label %b1395
b1391:
  %v2231 = icmp eq i64 %v2228, %v6
  br i1 %v2231, label %b1393, label %b1390
b1392:
  %gcvolta1392 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1392, ptr %ctxtopo, align 8
  ret i64 0
b1393:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2480 = icmp eq i64 %v0, 0
  %v2481 = call i64 @df.class(i64 %v0)
  %v2482 = call i8 @df.subclass(i64 %v2481, i64 674)
  %c567 = zext i8 %v2482 to i64
  %v2483 = icmp ne i64 %c567, 0
  %v2484 = xor i1 %v2480, true
  %c568 = zext i1 %v2483 to i64
  %c569 = zext i1 %v2484 to i64
  %v2485 = and i64 %c568, %c569
  %v2486 = icmp ne i64 %v2485, 0
  br i1 %v2486, label %b1539, label %b1538
b1394:
  store i64 0, ptr %gcs1
  %v2236 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v2236, ptr %gcs1
  %v2237 = load i8, ptr %ctx, align 8
  %c570 = zext i8 %v2237 to i64
  %v2238 = icmp ne i64 %c570, 0
  br i1 %v2238, label %b1400, label %b1399
b1395:
  %v2235 = icmp eq i64 %v2232, %v6
  br i1 %v2235, label %b1397, label %b1394
b1396:
  %gcvolta1396 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1396, ptr %ctxtopo, align 8
  ret i64 0
b1397:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2503 = icmp eq i64 %v0, 0
  %v2504 = call i64 @df.class(i64 %v0)
  %v2505 = call i8 @df.subclass(i64 %v2504, i64 674)
  %c571 = zext i8 %v2505 to i64
  %v2506 = icmp ne i64 %c571, 0
  %v2507 = xor i1 %v2503, true
  %c572 = zext i1 %v2506 to i64
  %c573 = zext i1 %v2507 to i64
  %v2508 = and i64 %c572, %c573
  %v2509 = icmp ne i64 %v2508, 0
  br i1 %v2509, label %b1549, label %b1548
b1398:
  store i64 0, ptr %gcs1
  %v2240 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v2240, ptr %gcs1
  %v2241 = load i8, ptr %ctx, align 8
  %c574 = zext i8 %v2241 to i64
  %v2242 = icmp ne i64 %c574, 0
  br i1 %v2242, label %b1404, label %b1403
b1399:
  %v2239 = icmp eq i64 %v2236, %v6
  br i1 %v2239, label %b1401, label %b1398
b1400:
  %gcvolta1400 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1400, ptr %ctxtopo, align 8
  ret i64 0
b1401:
  %xcp2526 = load i8, ptr %ctx, align 8
  %xcn2526 = icmp ne i8 %xcp2526, 0
  %xce2526 = call i1 @llvm.expect.i1(i1 %xcn2526, i1 false)
  br i1 %xce2526, label %xc2526.limpar, label %xc2526.fim
xc2526.limpar:
  call void @dartforge_exception_clear()
  br label %xc2526.fim
xc2526.fim:
  %gcvolta1401 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1401, ptr %ctxtopo, align 8
  ret i64 %v0
b1402:
  store i64 0, ptr %gcs1
  %v2244 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v2244, ptr %gcs1
  %v2245 = load i8, ptr %ctx, align 8
  %c575 = zext i8 %v2245 to i64
  %v2246 = icmp ne i64 %c575, 0
  br i1 %v2246, label %b1409, label %b1408
b1403:
  %v2243 = icmp eq i64 %v2240, %v6
  br i1 %v2243, label %b1405, label %b1402
b1404:
  %gcvolta1404 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1404, ptr %ctxtopo, align 8
  ret i64 0
b1405:
  br label %b1365
b1407:
  store i64 0, ptr %gcs1
  %v2248 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v2248, ptr %gcs1
  %v2249 = load i8, ptr %ctx, align 8
  %c576 = zext i8 %v2249 to i64
  %v2250 = icmp ne i64 %c576, 0
  br i1 %v2250, label %b1414, label %b1413
b1408:
  %v2247 = icmp eq i64 %v2244, %v6
  br i1 %v2247, label %b1410, label %b1407
b1409:
  %gcvolta1409 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1409, ptr %ctxtopo, align 8
  ret i64 0
b1410:
  br label %b1365
b1412:
  br label %b12
b1413:
  %v2251 = icmp eq i64 %v2248, %v6
  br i1 %v2251, label %b1415, label %b1412
b1414:
  %gcvolta1414 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1414, ptr %ctxtopo, align 8
  ret i64 0
b1415:
  br label %b1365
b1416:
  %v2259 = icmp eq i64 %v0, %v2254
  br label %b1418
b1417:
  %v2260 = call i64 @df.class(i64 %v0)
  switch i64 %v2260, label %b1422 [ i64 675, label %b1419 i64 4, label %b1420 i64 2, label %b1421 i64 3, label %b1421 ]
b1418:
  %v2278 = phi i1 [ %v2259, %b1416 ], [ %v2277, %b1423 ]
  br i1 %v2278, label %b1434, label %b1435
b1419:
  %v2261 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v2254)
  %v2262 = load i8, ptr %ctx, align 8
  %c577 = zext i8 %v2262 to i64
  %v2263 = icmp ne i64 %c577, 0
  br i1 %v2263, label %b1425, label %b1424
b1420:
  %v2265 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v2254)
  %v2266 = load i8, ptr %ctx, align 8
  %c578 = zext i8 %v2266 to i64
  %v2267 = icmp ne i64 %c578, 0
  br i1 %v2267, label %b1427, label %b1426
b1421:
  %v2268 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v2254)
  %v2269 = load i8, ptr %ctx, align 8
  %c579 = zext i8 %v2269 to i64
  %v2270 = icmp ne i64 %c579, 0
  br i1 %v2270, label %b1429, label %b1428
b1422:
  %sa2271_0 = getelementptr [2 x i64], ptr %sargs2271, i64 0, i64 0
  store i64 %v2254, ptr %sa2271_0
  %sat2271 = getelementptr [2 x i64], ptr %sargs2271, i64 0, i64 1
  store i64 0, ptr %sat2271
  %sic2271 = getelementptr i64, ptr %area, i64 258
  %sf2271 = call ptr @df.selector(ptr %sic2271, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v2271 = call i64 %sf2271(i64 %v0, ptr %sargs2271, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2271, ptr %gcs0
  %v2272 = load i8, ptr %ctx, align 8
  %c580 = zext i8 %v2272 to i64
  %v2273 = icmp ne i64 %c580, 0
  br i1 %v2273, label %b1431, label %b1430
b1423:
  %v2277 = phi i1 [ %v2264, %b1424 ], [ %v2265, %b1426 ], [ %v2268, %b1428 ], [ %v2274, %b1432 ]
  br label %b1418
b1424:
  %c581 = zext i8 %v2261 to i64
  %v2264 = icmp ne i64 %c581, 0
  br label %b1423
b1425:
  %gcvolta1425 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1425, ptr %ctxtopo, align 8
  ret i64 0
b1426:
  br label %b1423
b1427:
  %gcvolta1427 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1427, ptr %ctxtopo, align 8
  ret i64 0
b1428:
  br label %b1423
b1429:
  %gcvolta1429 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1429, ptr %ctxtopo, align 8
  ret i64 0
b1430:
  store i64 0, ptr %gcs1
  %v2274 = call i1 @df.unbox_bool(i64 %v2271)
  %v2275 = load i8, ptr %ctx, align 8
  %c582 = zext i8 %v2275 to i64
  %v2276 = icmp ne i64 %c582, 0
  br i1 %v2276, label %b1433, label %b1432
b1431:
  %gcvolta1431 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1431, ptr %ctxtopo, align 8
  ret i64 0
b1432:
  br label %b1423
b1433:
  %gcvolta1433 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1433, ptr %ctxtopo, align 8
  ret i64 0
b1434:
  %v2279 = add i64 0, 0
  br label %b1436
b1435:
  %v2280 = add i64 0, 1
  br label %b1436
b1436:
  %v2281 = phi i64 [ %v2279, %b1434 ], [ %v2280, %b1435 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2282 = call i64 @df.box_int(i64 %v2281)
  %xcp2283 = load i8, ptr %ctx, align 8
  %xcn2283 = icmp ne i8 %xcp2283, 0
  %xce2283 = call i1 @llvm.expect.i1(i1 %xcn2283, i1 false)
  br i1 %xce2283, label %xc2283.limpar, label %xc2283.fim
xc2283.limpar:
  call void @dartforge_exception_clear()
  br label %xc2283.fim
xc2283.fim:
  %gcvolta1436 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1436, ptr %ctxtopo, align 8
  ret i64 %v2282
b1438:
  %v2288 = ashr i64 %v0, 1
  %v2289 = icmp sle i64 %v2288, %v2285
  br label %b1440
b1439:
  %v2290 = call i64 @df.box_int(i64 %v2285)
  store i64 %v2290, ptr %gcs1
  %sa2291_0 = getelementptr [2 x i64], ptr %sargs2291, i64 0, i64 0
  store i64 %v2290, ptr %sa2291_0
  %sat2291 = getelementptr [2 x i64], ptr %sargs2291, i64 0, i64 1
  store i64 0, ptr %sat2291
  %sic2291 = getelementptr i64, ptr %area, i64 260
  %sf2291 = call ptr @df.selector(ptr %sic2291, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2291 = call i64 %sf2291(i64 %v0, ptr %sargs2291, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2291, ptr %gcs1
  %v2292 = load i8, ptr %ctx, align 8
  %c583 = zext i8 %v2292 to i64
  %v2293 = icmp ne i64 %c583, 0
  br i1 %v2293, label %b1442, label %b1441
b1440:
  %v2297 = phi i1 [ %v2289, %b1438 ], [ %v2294, %b1443 ]
  br i1 %v2297, label %b1445, label %b1446
b1441:
  %v2294 = call i1 @df.unbox_bool(i64 %v2291)
  %v2295 = load i8, ptr %ctx, align 8
  %c584 = zext i8 %v2295 to i64
  %v2296 = icmp ne i64 %c584, 0
  br i1 %v2296, label %b1444, label %b1443
b1442:
  %gcvolta1442 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1442, ptr %ctxtopo, align 8
  ret i64 0
b1443:
  br label %b1440
b1444:
  %gcvolta1444 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1444, ptr %ctxtopo, align 8
  ret i64 0
b1445:
  %v2298 = add i64 0, 0
  br label %b1447
b1446:
  store i64 0, ptr %gcs1
  %v2300 = icmp eq i64 %v0, 0
  %v2301 = call i64 @df.class(i64 %v0)
  %v2302 = call i8 @df.subclass(i64 %v2301, i64 674)
  %c585 = zext i8 %v2302 to i64
  %v2303 = icmp ne i64 %c585, 0
  %v2304 = xor i1 %v2300, true
  %c586 = zext i1 %v2303 to i64
  %c587 = zext i1 %v2304 to i64
  %v2305 = and i64 %c586, %c587
  %v2306 = icmp ne i64 %v2305, 0
  br i1 %v2306, label %b1449, label %b1448
b1447:
  %v2320 = phi i64 [ %v2298, %b1445 ], [ %v2319, %b1455 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2321 = call i64 @df.box_int(i64 %v2320)
  %xcp2322 = load i8, ptr %ctx, align 8
  %xcn2322 = icmp ne i8 %xcp2322, 0
  %xce2322 = call i1 @llvm.expect.i1(i1 %xcn2322, i1 false)
  br i1 %xce2322, label %xc2322.limpar, label %xc2322.fim
xc2322.limpar:
  call void @dartforge_exception_clear()
  br label %xc2322.fim
xc2322.fim:
  %gcvolta1447 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1447, ptr %ctxtopo, align 8
  ret i64 %v2321
b1448:
  %ga2307 = getelementptr i64, ptr %area, i64 7
  %v2307 = load i8, ptr %ga2307
  %c588 = zext i8 %v2307 to i64
  %v2308 = icmp ne i64 %c588, 0
  br i1 %v2308, label %b1450, label %b1451
b1449:
  %v2315 = call i64 @df.unbox_int(i64 %v0)
  %v2316 = load i8, ptr %ctx, align 8
  %c589 = zext i8 %v2316 to i64
  %v2317 = icmp ne i64 %c589, 0
  br i1 %v2317, label %b1456, label %b1455
b1450:
  %ga2309 = getelementptr i64, ptr %area, i64 6
  %v2309 = load i64, ptr %ga2309
  br label %b1452
b1451:
  %v2310 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1452
b1452:
  %v2311 = phi i64 [ %v2309, %b1450 ], [ %v2310, %b1451 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2311, i64 0, i64 0)
  %v2313 = load i8, ptr %ctx, align 8
  %c590 = zext i8 %v2313 to i64
  %v2314 = icmp ne i64 %c590, 0
  br i1 %v2314, label %b1454, label %b1453
b1453:
  br label %b1449
b1454:
  %gcvolta1454 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1454, ptr %ctxtopo, align 8
  ret i64 0
b1455:
  %v2318 = add i64 0, 29
  %v2319 = ashr i64 %v2315, %v2318
  br label %b1447
b1456:
  %gcvolta1456 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1456, ptr %ctxtopo, align 8
  ret i64 0
b1458:
  %v2327 = ashr i64 %v0, 1
  %v2328 = icmp sle i64 %v2327, %v2324
  br label %b1460
b1459:
  %v2329 = call i64 @df.box_int(i64 %v2324)
  store i64 %v2329, ptr %gcs1
  %sa2330_0 = getelementptr [2 x i64], ptr %sargs2330, i64 0, i64 0
  store i64 %v2329, ptr %sa2330_0
  %sat2330 = getelementptr [2 x i64], ptr %sargs2330, i64 0, i64 1
  store i64 0, ptr %sat2330
  %sic2330 = getelementptr i64, ptr %area, i64 262
  %sf2330 = call ptr @df.selector(ptr %sic2330, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2330 = call i64 %sf2330(i64 %v0, ptr %sargs2330, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2330, ptr %gcs1
  %v2331 = load i8, ptr %ctx, align 8
  %c591 = zext i8 %v2331 to i64
  %v2332 = icmp ne i64 %c591, 0
  br i1 %v2332, label %b1462, label %b1461
b1460:
  %v2336 = phi i1 [ %v2328, %b1458 ], [ %v2333, %b1463 ]
  br i1 %v2336, label %b1465, label %b1466
b1461:
  %v2333 = call i1 @df.unbox_bool(i64 %v2330)
  %v2334 = load i8, ptr %ctx, align 8
  %c592 = zext i8 %v2334 to i64
  %v2335 = icmp ne i64 %c592, 0
  br i1 %v2335, label %b1464, label %b1463
b1462:
  %gcvolta1462 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1462, ptr %ctxtopo, align 8
  ret i64 0
b1463:
  br label %b1460
b1464:
  %gcvolta1464 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1464, ptr %ctxtopo, align 8
  ret i64 0
b1465:
  %v2337 = add i64 0, 0
  br label %b1467
b1466:
  store i64 0, ptr %gcs1
  %v2339 = icmp eq i64 %v0, 0
  %v2340 = call i64 @df.class(i64 %v0)
  %v2341 = call i8 @df.subclass(i64 %v2340, i64 674)
  %c593 = zext i8 %v2341 to i64
  %v2342 = icmp ne i64 %c593, 0
  %v2343 = xor i1 %v2339, true
  %c594 = zext i1 %v2342 to i64
  %c595 = zext i1 %v2343 to i64
  %v2344 = and i64 %c594, %c595
  %v2345 = icmp ne i64 %v2344, 0
  br i1 %v2345, label %b1469, label %b1468
b1467:
  %v2359 = phi i64 [ %v2337, %b1465 ], [ %v2358, %b1475 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2360 = call i64 @df.box_int(i64 %v2359)
  %xcp2361 = load i8, ptr %ctx, align 8
  %xcn2361 = icmp ne i8 %xcp2361, 0
  %xce2361 = call i1 @llvm.expect.i1(i1 %xcn2361, i1 false)
  br i1 %xce2361, label %xc2361.limpar, label %xc2361.fim
xc2361.limpar:
  call void @dartforge_exception_clear()
  br label %xc2361.fim
xc2361.fim:
  %gcvolta1467 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1467, ptr %ctxtopo, align 8
  ret i64 %v2360
b1468:
  %ga2346 = getelementptr i64, ptr %area, i64 7
  %v2346 = load i8, ptr %ga2346
  %c596 = zext i8 %v2346 to i64
  %v2347 = icmp ne i64 %c596, 0
  br i1 %v2347, label %b1470, label %b1471
b1469:
  %v2354 = call i64 @df.unbox_int(i64 %v0)
  %v2355 = load i8, ptr %ctx, align 8
  %c597 = zext i8 %v2355 to i64
  %v2356 = icmp ne i64 %c597, 0
  br i1 %v2356, label %b1476, label %b1475
b1470:
  %ga2348 = getelementptr i64, ptr %area, i64 6
  %v2348 = load i64, ptr %ga2348
  br label %b1472
b1471:
  %v2349 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1472
b1472:
  %v2350 = phi i64 [ %v2348, %b1470 ], [ %v2349, %b1471 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2350, i64 0, i64 0)
  %v2352 = load i8, ptr %ctx, align 8
  %c598 = zext i8 %v2352 to i64
  %v2353 = icmp ne i64 %c598, 0
  br i1 %v2353, label %b1474, label %b1473
b1473:
  br label %b1469
b1474:
  %gcvolta1474 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1474, ptr %ctxtopo, align 8
  ret i64 0
b1475:
  %v2357 = add i64 0, 27
  %v2358 = ashr i64 %v2354, %v2357
  br label %b1467
b1476:
  %gcvolta1476 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1476, ptr %ctxtopo, align 8
  ret i64 0
b1478:
  %v2366 = ashr i64 %v0, 1
  %v2367 = icmp sle i64 %v2366, %v2363
  br label %b1480
b1479:
  %v2368 = call i64 @df.box_int(i64 %v2363)
  store i64 %v2368, ptr %gcs1
  %sa2369_0 = getelementptr [2 x i64], ptr %sargs2369, i64 0, i64 0
  store i64 %v2368, ptr %sa2369_0
  %sat2369 = getelementptr [2 x i64], ptr %sargs2369, i64 0, i64 1
  store i64 0, ptr %sat2369
  %sic2369 = getelementptr i64, ptr %area, i64 264
  %sf2369 = call ptr @df.selector(ptr %sic2369, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2369 = call i64 %sf2369(i64 %v0, ptr %sargs2369, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2369, ptr %gcs1
  %v2370 = load i8, ptr %ctx, align 8
  %c599 = zext i8 %v2370 to i64
  %v2371 = icmp ne i64 %c599, 0
  br i1 %v2371, label %b1482, label %b1481
b1480:
  %v2375 = phi i1 [ %v2367, %b1478 ], [ %v2372, %b1483 ]
  br i1 %v2375, label %b1485, label %b1486
b1481:
  %v2372 = call i1 @df.unbox_bool(i64 %v2369)
  %v2373 = load i8, ptr %ctx, align 8
  %c600 = zext i8 %v2373 to i64
  %v2374 = icmp ne i64 %c600, 0
  br i1 %v2374, label %b1484, label %b1483
b1482:
  %gcvolta1482 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1482, ptr %ctxtopo, align 8
  ret i64 0
b1483:
  br label %b1480
b1484:
  %gcvolta1484 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1484, ptr %ctxtopo, align 8
  ret i64 0
b1485:
  %v2376 = add i64 0, 0
  br label %b1487
b1486:
  store i64 0, ptr %gcs1
  %v2378 = icmp eq i64 %v0, 0
  %v2379 = call i64 @df.class(i64 %v0)
  %v2380 = call i8 @df.subclass(i64 %v2379, i64 674)
  %c601 = zext i8 %v2380 to i64
  %v2381 = icmp ne i64 %c601, 0
  %v2382 = xor i1 %v2378, true
  %c602 = zext i1 %v2381 to i64
  %c603 = zext i1 %v2382 to i64
  %v2383 = and i64 %c602, %c603
  %v2384 = icmp ne i64 %v2383, 0
  br i1 %v2384, label %b1489, label %b1488
b1487:
  %v2398 = phi i64 [ %v2376, %b1485 ], [ %v2397, %b1495 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2399 = call i64 @df.box_int(i64 %v2398)
  %xcp2400 = load i8, ptr %ctx, align 8
  %xcn2400 = icmp ne i8 %xcp2400, 0
  %xce2400 = call i1 @llvm.expect.i1(i1 %xcn2400, i1 false)
  br i1 %xce2400, label %xc2400.limpar, label %xc2400.fim
xc2400.limpar:
  call void @dartforge_exception_clear()
  br label %xc2400.fim
xc2400.fim:
  %gcvolta1487 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1487, ptr %ctxtopo, align 8
  ret i64 %v2399
b1488:
  %ga2385 = getelementptr i64, ptr %area, i64 7
  %v2385 = load i8, ptr %ga2385
  %c604 = zext i8 %v2385 to i64
  %v2386 = icmp ne i64 %c604, 0
  br i1 %v2386, label %b1490, label %b1491
b1489:
  %v2393 = call i64 @df.unbox_int(i64 %v0)
  %v2394 = load i8, ptr %ctx, align 8
  %c605 = zext i8 %v2394 to i64
  %v2395 = icmp ne i64 %c605, 0
  br i1 %v2395, label %b1496, label %b1495
b1490:
  %ga2387 = getelementptr i64, ptr %area, i64 6
  %v2387 = load i64, ptr %ga2387
  br label %b1492
b1491:
  %v2388 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1492
b1492:
  %v2389 = phi i64 [ %v2387, %b1490 ], [ %v2388, %b1491 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2389, i64 0, i64 0)
  %v2391 = load i8, ptr %ctx, align 8
  %c606 = zext i8 %v2391 to i64
  %v2392 = icmp ne i64 %c606, 0
  br i1 %v2392, label %b1494, label %b1493
b1493:
  br label %b1489
b1494:
  %gcvolta1494 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1494, ptr %ctxtopo, align 8
  ret i64 0
b1495:
  %v2396 = add i64 0, 23
  %v2397 = ashr i64 %v2393, %v2396
  br label %b1487
b1496:
  %gcvolta1496 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1496, ptr %ctxtopo, align 8
  ret i64 0
b1498:
  %v2405 = ashr i64 %v0, 1
  %v2406 = icmp sle i64 %v2405, %v2402
  br label %b1500
b1499:
  %v2407 = call i64 @df.box_int(i64 %v2402)
  store i64 %v2407, ptr %gcs1
  %sa2408_0 = getelementptr [2 x i64], ptr %sargs2408, i64 0, i64 0
  store i64 %v2407, ptr %sa2408_0
  %sat2408 = getelementptr [2 x i64], ptr %sargs2408, i64 0, i64 1
  store i64 0, ptr %sat2408
  %sic2408 = getelementptr i64, ptr %area, i64 266
  %sf2408 = call ptr @df.selector(ptr %sic2408, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2408 = call i64 %sf2408(i64 %v0, ptr %sargs2408, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2408, ptr %gcs1
  %v2409 = load i8, ptr %ctx, align 8
  %c607 = zext i8 %v2409 to i64
  %v2410 = icmp ne i64 %c607, 0
  br i1 %v2410, label %b1502, label %b1501
b1500:
  %v2414 = phi i1 [ %v2406, %b1498 ], [ %v2411, %b1503 ]
  br i1 %v2414, label %b1505, label %b1506
b1501:
  %v2411 = call i1 @df.unbox_bool(i64 %v2408)
  %v2412 = load i8, ptr %ctx, align 8
  %c608 = zext i8 %v2412 to i64
  %v2413 = icmp ne i64 %c608, 0
  br i1 %v2413, label %b1504, label %b1503
b1502:
  %gcvolta1502 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1502, ptr %ctxtopo, align 8
  ret i64 0
b1503:
  br label %b1500
b1504:
  %gcvolta1504 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1504, ptr %ctxtopo, align 8
  ret i64 0
b1505:
  %v2415 = add i64 0, 0
  br label %b1507
b1506:
  store i64 0, ptr %gcs1
  %v2417 = icmp eq i64 %v0, 0
  %v2418 = call i64 @df.class(i64 %v0)
  %v2419 = call i8 @df.subclass(i64 %v2418, i64 674)
  %c609 = zext i8 %v2419 to i64
  %v2420 = icmp ne i64 %c609, 0
  %v2421 = xor i1 %v2417, true
  %c610 = zext i1 %v2420 to i64
  %c611 = zext i1 %v2421 to i64
  %v2422 = and i64 %c610, %c611
  %v2423 = icmp ne i64 %v2422, 0
  br i1 %v2423, label %b1509, label %b1508
b1507:
  %v2437 = phi i64 [ %v2415, %b1505 ], [ %v2436, %b1515 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2438 = call i64 @df.box_int(i64 %v2437)
  %xcp2439 = load i8, ptr %ctx, align 8
  %xcn2439 = icmp ne i8 %xcp2439, 0
  %xce2439 = call i1 @llvm.expect.i1(i1 %xcn2439, i1 false)
  br i1 %xce2439, label %xc2439.limpar, label %xc2439.fim
xc2439.limpar:
  call void @dartforge_exception_clear()
  br label %xc2439.fim
xc2439.fim:
  %gcvolta1507 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1507, ptr %ctxtopo, align 8
  ret i64 %v2438
b1508:
  %ga2424 = getelementptr i64, ptr %area, i64 7
  %v2424 = load i8, ptr %ga2424
  %c612 = zext i8 %v2424 to i64
  %v2425 = icmp ne i64 %c612, 0
  br i1 %v2425, label %b1510, label %b1511
b1509:
  %v2432 = call i64 @df.unbox_int(i64 %v0)
  %v2433 = load i8, ptr %ctx, align 8
  %c613 = zext i8 %v2433 to i64
  %v2434 = icmp ne i64 %c613, 0
  br i1 %v2434, label %b1516, label %b1515
b1510:
  %ga2426 = getelementptr i64, ptr %area, i64 6
  %v2426 = load i64, ptr %ga2426
  br label %b1512
b1511:
  %v2427 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1512
b1512:
  %v2428 = phi i64 [ %v2426, %b1510 ], [ %v2427, %b1511 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2428, i64 0, i64 0)
  %v2430 = load i8, ptr %ctx, align 8
  %c614 = zext i8 %v2430 to i64
  %v2431 = icmp ne i64 %c614, 0
  br i1 %v2431, label %b1514, label %b1513
b1513:
  br label %b1509
b1514:
  %gcvolta1514 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1514, ptr %ctxtopo, align 8
  ret i64 0
b1515:
  %v2435 = add i64 0, 16
  %v2436 = ashr i64 %v2432, %v2435
  br label %b1507
b1516:
  %gcvolta1516 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1516, ptr %ctxtopo, align 8
  ret i64 0
b1518:
  %v2444 = ashr i64 %v0, 1
  %v2445 = icmp sle i64 %v2444, %v2441
  br label %b1520
b1519:
  %v2446 = call i64 @df.box_int(i64 %v2441)
  store i64 %v2446, ptr %gcs1
  %sa2447_0 = getelementptr [2 x i64], ptr %sargs2447, i64 0, i64 0
  store i64 %v2446, ptr %sa2447_0
  %sat2447 = getelementptr [2 x i64], ptr %sargs2447, i64 0, i64 1
  store i64 0, ptr %sat2447
  %sic2447 = getelementptr i64, ptr %area, i64 268
  %sf2447 = call ptr @df.selector(ptr %sic2447, i64 %v0, i64 2959114654109919793, ptr @df.seln.12, i64 5)
  %v2447 = call i64 %sf2447(i64 %v0, ptr %sargs2447, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2447, ptr %gcs1
  %v2448 = load i8, ptr %ctx, align 8
  %c615 = zext i8 %v2448 to i64
  %v2449 = icmp ne i64 %c615, 0
  br i1 %v2449, label %b1522, label %b1521
b1520:
  %v2453 = phi i1 [ %v2445, %b1518 ], [ %v2450, %b1523 ]
  br i1 %v2453, label %b1525, label %b1526
b1521:
  %v2450 = call i1 @df.unbox_bool(i64 %v2447)
  %v2451 = load i8, ptr %ctx, align 8
  %c616 = zext i8 %v2451 to i64
  %v2452 = icmp ne i64 %c616, 0
  br i1 %v2452, label %b1524, label %b1523
b1522:
  %gcvolta1522 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1522, ptr %ctxtopo, align 8
  ret i64 0
b1523:
  br label %b1520
b1524:
  %gcvolta1524 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1524, ptr %ctxtopo, align 8
  ret i64 0
b1525:
  %v2454 = add i64 0, 0
  br label %b1527
b1526:
  store i64 0, ptr %gcs1
  %v2456 = icmp eq i64 %v0, 0
  %v2457 = call i64 @df.class(i64 %v0)
  %v2458 = call i8 @df.subclass(i64 %v2457, i64 674)
  %c617 = zext i8 %v2458 to i64
  %v2459 = icmp ne i64 %c617, 0
  %v2460 = xor i1 %v2456, true
  %c618 = zext i1 %v2459 to i64
  %c619 = zext i1 %v2460 to i64
  %v2461 = and i64 %c618, %c619
  %v2462 = icmp ne i64 %v2461, 0
  br i1 %v2462, label %b1529, label %b1528
b1527:
  %v2476 = phi i64 [ %v2454, %b1525 ], [ %v2475, %b1535 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2477 = call i64 @df.box_int(i64 %v2476)
  %xcp2478 = load i8, ptr %ctx, align 8
  %xcn2478 = icmp ne i8 %xcp2478, 0
  %xce2478 = call i1 @llvm.expect.i1(i1 %xcn2478, i1 false)
  br i1 %xce2478, label %xc2478.limpar, label %xc2478.fim
xc2478.limpar:
  call void @dartforge_exception_clear()
  br label %xc2478.fim
xc2478.fim:
  %gcvolta1527 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1527, ptr %ctxtopo, align 8
  ret i64 %v2477
b1528:
  %ga2463 = getelementptr i64, ptr %area, i64 7
  %v2463 = load i8, ptr %ga2463
  %c620 = zext i8 %v2463 to i64
  %v2464 = icmp ne i64 %c620, 0
  br i1 %v2464, label %b1530, label %b1531
b1529:
  %v2471 = call i64 @df.unbox_int(i64 %v0)
  %v2472 = load i8, ptr %ctx, align 8
  %c621 = zext i8 %v2472 to i64
  %v2473 = icmp ne i64 %c621, 0
  br i1 %v2473, label %b1536, label %b1535
b1530:
  %ga2465 = getelementptr i64, ptr %area, i64 6
  %v2465 = load i64, ptr %ga2465
  br label %b1532
b1531:
  %v2466 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1532
b1532:
  %v2467 = phi i64 [ %v2465, %b1530 ], [ %v2466, %b1531 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2467, i64 0, i64 0)
  %v2469 = load i8, ptr %ctx, align 8
  %c622 = zext i8 %v2469 to i64
  %v2470 = icmp ne i64 %c622, 0
  br i1 %v2470, label %b1534, label %b1533
b1533:
  br label %b1529
b1534:
  %gcvolta1534 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1534, ptr %ctxtopo, align 8
  ret i64 0
b1535:
  %v2474 = add i64 0, 1
  %v2475 = shl i64 %v2471, %v2474
  br label %b1527
b1536:
  %gcvolta1536 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1536, ptr %ctxtopo, align 8
  ret i64 0
b1538:
  %ga2487 = getelementptr i64, ptr %area, i64 7
  %v2487 = load i8, ptr %ga2487
  %c623 = zext i8 %v2487 to i64
  %v2488 = icmp ne i64 %c623, 0
  br i1 %v2488, label %b1540, label %b1541
b1539:
  %v2495 = call i64 @df.unbox_int(i64 %v0)
  %v2496 = load i8, ptr %ctx, align 8
  %c624 = zext i8 %v2496 to i64
  %v2497 = icmp ne i64 %c624, 0
  br i1 %v2497, label %b1546, label %b1545
b1540:
  %ga2489 = getelementptr i64, ptr %area, i64 6
  %v2489 = load i64, ptr %ga2489
  br label %b1542
b1541:
  %v2490 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1542
b1542:
  %v2491 = phi i64 [ %v2489, %b1540 ], [ %v2490, %b1541 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2491, i64 0, i64 0)
  %v2493 = load i8, ptr %ctx, align 8
  %c625 = zext i8 %v2493 to i64
  %v2494 = icmp ne i64 %c625, 0
  br i1 %v2494, label %b1544, label %b1543
b1543:
  br label %b1539
b1544:
  %gcvolta1544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1544, ptr %ctxtopo, align 8
  ret i64 0
b1545:
  %v2498 = add i64 0, 24
  %v2499 = ashr i64 %v2495, %v2498
  store i64 0, ptr %gcs0
  %v2500 = call i64 @df.box_int(i64 %v2499)
  %xcp2501 = load i8, ptr %ctx, align 8
  %xcn2501 = icmp ne i8 %xcp2501, 0
  %xce2501 = call i1 @llvm.expect.i1(i1 %xcn2501, i1 false)
  br i1 %xce2501, label %xc2501.limpar, label %xc2501.fim
xc2501.limpar:
  call void @dartforge_exception_clear()
  br label %xc2501.fim
xc2501.fim:
  %gcvolta1545 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1545, ptr %ctxtopo, align 8
  ret i64 %v2500
b1546:
  %gcvolta1546 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1546, ptr %ctxtopo, align 8
  ret i64 0
b1548:
  %ga2510 = getelementptr i64, ptr %area, i64 7
  %v2510 = load i8, ptr %ga2510
  %c626 = zext i8 %v2510 to i64
  %v2511 = icmp ne i64 %c626, 0
  br i1 %v2511, label %b1550, label %b1551
b1549:
  %v2518 = call i64 @df.unbox_int(i64 %v0)
  %v2519 = load i8, ptr %ctx, align 8
  %c627 = zext i8 %v2519 to i64
  %v2520 = icmp ne i64 %c627, 0
  br i1 %v2520, label %b1556, label %b1555
b1550:
  %ga2512 = getelementptr i64, ptr %area, i64 6
  %v2512 = load i64, ptr %ga2512
  br label %b1552
b1551:
  %v2513 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1552
b1552:
  %v2514 = phi i64 [ %v2512, %b1550 ], [ %v2513, %b1551 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2514, i64 0, i64 0)
  %v2516 = load i8, ptr %ctx, align 8
  %c628 = zext i8 %v2516 to i64
  %v2517 = icmp ne i64 %c628, 0
  br i1 %v2517, label %b1554, label %b1553
b1553:
  br label %b1549
b1554:
  %gcvolta1554 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1554, ptr %ctxtopo, align 8
  ret i64 0
b1555:
  %v2521 = add i64 0, 16
  %v2522 = ashr i64 %v2518, %v2521
  store i64 0, ptr %gcs0
  %v2523 = call i64 @df.box_int(i64 %v2522)
  %xcp2524 = load i8, ptr %ctx, align 8
  %xcn2524 = icmp ne i8 %xcp2524, 0
  %xce2524 = call i1 @llvm.expect.i1(i1 %xcn2524, i1 false)
  br i1 %xce2524, label %xc2524.limpar, label %xc2524.fim
xc2524.limpar:
  call void @dartforge_exception_clear()
  br label %xc2524.fim
xc2524.fim:
  %gcvolta1555 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1555, ptr %ctxtopo, align 8
  ret i64 %v2523
b1556:
  %gcvolta1556 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1556, ptr %ctxtopo, align 8
  ret i64 0
b1559:
  store i64 0, ptr %gcs1
  %v2533 = call double @df.unbox_double(i64 %v2530)
  %v2534 = load i8, ptr %ctx, align 8
  %c629 = zext i8 %v2534 to i64
  %v2535 = icmp ne i64 %c629, 0
  br i1 %v2535, label %b1562, label %b1561
b1560:
  %gcvolta1560 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1560, ptr %ctxtopo, align 8
  ret i64 0
b1561:
  %xcp2537 = load i8, ptr %ctx, align 8
  %xcn2537 = icmp ne i8 %xcp2537, 0
  %xce2537 = call i1 @llvm.expect.i1(i1 %xcn2537, i1 false)
  br i1 %xce2537, label %xc2537.limpar, label %xc2537.fim
xc2537.limpar:
  call void @dartforge_exception_clear()
  br label %xc2537.fim
xc2537.fim:
  %gcvolta1561 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1561, ptr %ctxtopo, align 8
  ret i64 %v2530
b1562:
  %gcvolta1562 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1562, ptr %ctxtopo, align 8
  ret i64 0
b1576:
  %xcp3351 = load i8, ptr %ctx, align 8
  %xcn3351 = icmp ne i8 %xcp3351, 0
  %xce3351 = call i1 @llvm.expect.i1(i1 %xcn3351, i1 false)
  br i1 %xce3351, label %xc3351.limpar, label %xc3351.fim
xc3351.limpar:
  call void @dartforge_exception_clear()
  br label %xc3351.fim
xc3351.fim:
  %gcvolta1576 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1576, ptr %ctxtopo, align 8
  ret i64 %v0
b1577:
  store i64 0, ptr %gcs1
  %v2543 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v2543, ptr %gcs1
  %v2544 = load i8, ptr %ctx, align 8
  %c630 = zext i8 %v2544 to i64
  %v2545 = icmp ne i64 %c630, 0
  br i1 %v2545, label %b1583, label %b1582
b1578:
  %v2542 = icmp eq i64 %v2539, %v6
  br i1 %v2542, label %b1580, label %b1577
b1579:
  %gcvolta1579 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1579, ptr %ctxtopo, align 8
  ret i64 0
b1580:
  %v2588 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2589 = call i64 @df.box_int(i64 %v2588)
  store i64 %v2589, ptr %gcs1
  %v2590 = icmp eq i64 %v0, 0
  %v2591 = icmp eq i64 %v2589, 0
  %c631 = zext i1 %v2590 to i64
  %c632 = zext i1 %v2591 to i64
  %v2592 = or i64 %c631, %c632
  %v2593 = icmp ne i64 %v2592, 0
  br i1 %v2593, label %b1627, label %b1628
b1581:
  store i64 0, ptr %gcs1
  %v2547 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v2547, ptr %gcs1
  %v2548 = load i8, ptr %ctx, align 8
  %c633 = zext i8 %v2548 to i64
  %v2549 = icmp ne i64 %c633, 0
  br i1 %v2549, label %b1587, label %b1586
b1582:
  %v2546 = icmp eq i64 %v2543, %v6
  br i1 %v2546, label %b1584, label %b1581
b1583:
  %gcvolta1583 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1583, ptr %ctxtopo, align 8
  ret i64 0
b1584:
  %v2620 = add i64 0, 0
  %v2621 = add i64 0, 1
  %v2622 = call i64 @df.class(i64 %v0)
  switch i64 %v2622, label %b1651 [ i64 4, label %b1649 i64 2, label %b1650 i64 3, label %b1650 ]
b1585:
  store i64 0, ptr %gcs1
  %v2551 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v2551, ptr %gcs1
  %v2552 = load i8, ptr %ctx, align 8
  %c634 = zext i8 %v2552 to i64
  %v2553 = icmp ne i64 %c634, 0
  br i1 %v2553, label %b1591, label %b1590
b1586:
  %v2550 = icmp eq i64 %v2547, %v6
  br i1 %v2550, label %b1588, label %b1585
b1587:
  %gcvolta1587 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1587, ptr %ctxtopo, align 8
  ret i64 0
b1588:
  %v2687 = add i64 0, 0
  %v2688 = add i64 0, 1
  %v2689 = call i64 @df.class(i64 %v0)
  switch i64 %v2689, label %b1686 [ i64 4, label %b1684 i64 2, label %b1685 i64 3, label %b1685 ]
b1589:
  store i64 0, ptr %gcs1
  %v2555 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v2555, ptr %gcs1
  %v2556 = load i8, ptr %ctx, align 8
  %c635 = zext i8 %v2556 to i64
  %v2557 = icmp ne i64 %c635, 0
  br i1 %v2557, label %b1595, label %b1594
b1590:
  %v2554 = icmp eq i64 %v2551, %v6
  br i1 %v2554, label %b1592, label %b1589
b1591:
  %gcvolta1591 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1591, ptr %ctxtopo, align 8
  ret i64 0
b1592:
  %v2754 = add i64 0, 0
  %v2755 = add i64 0, 1
  %v2756 = call i64 @df.class(i64 %v0)
  switch i64 %v2756, label %b1721 [ i64 4, label %b1719 i64 2, label %b1720 i64 3, label %b1720 ]
b1593:
  store i64 0, ptr %gcs1
  %v2559 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v2559, ptr %gcs1
  %v2560 = load i8, ptr %ctx, align 8
  %c636 = zext i8 %v2560 to i64
  %v2561 = icmp ne i64 %c636, 0
  br i1 %v2561, label %b1599, label %b1598
b1594:
  %v2558 = icmp eq i64 %v2555, %v6
  br i1 %v2558, label %b1596, label %b1593
b1595:
  %gcvolta1595 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1595, ptr %ctxtopo, align 8
  ret i64 0
b1596:
  %v2821 = add i64 0, 0
  %v2822 = add i64 0, 1
  %v2823 = call i64 @df.class(i64 %v0)
  switch i64 %v2823, label %b1756 [ i64 4, label %b1754 i64 2, label %b1755 i64 3, label %b1755 ]
b1597:
  store i64 0, ptr %gcs1
  %v2563 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v2563, ptr %gcs1
  %v2564 = load i8, ptr %ctx, align 8
  %c637 = zext i8 %v2564 to i64
  %v2565 = icmp ne i64 %c637, 0
  br i1 %v2565, label %b1603, label %b1602
b1598:
  %v2562 = icmp eq i64 %v2559, %v6
  br i1 %v2562, label %b1600, label %b1597
b1599:
  %gcvolta1599 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1599, ptr %ctxtopo, align 8
  ret i64 0
b1600:
  %v2888 = add i64 0, 0
  %v2889 = add i64 0, 1
  %v2890 = call i64 @df.class(i64 %v0)
  switch i64 %v2890, label %b1791 [ i64 4, label %b1789 i64 2, label %b1790 i64 3, label %b1790 ]
b1601:
  store i64 0, ptr %gcs1
  %v2567 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v2567, ptr %gcs1
  %v2568 = load i8, ptr %ctx, align 8
  %c638 = zext i8 %v2568 to i64
  %v2569 = icmp ne i64 %c638, 0
  br i1 %v2569, label %b1607, label %b1606
b1602:
  %v2566 = icmp eq i64 %v2563, %v6
  br i1 %v2566, label %b1604, label %b1601
b1603:
  %gcvolta1603 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1603, ptr %ctxtopo, align 8
  ret i64 0
b1604:
  %v2955 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2956 = and i64 %v0, 1
  %v2957 = icmp ne i64 %v2956, 0
  br i1 %v2957, label %b1824, label %b1825
b1605:
  store i64 0, ptr %gcs1
  %v2571 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v2571, ptr %gcs1
  %v2572 = load i8, ptr %ctx, align 8
  %c639 = zext i8 %v2572 to i64
  %v2573 = icmp ne i64 %c639, 0
  br i1 %v2573, label %b1611, label %b1610
b1606:
  %v2570 = icmp eq i64 %v2567, %v6
  br i1 %v2570, label %b1608, label %b1605
b1607:
  %gcvolta1607 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1607, ptr %ctxtopo, align 8
  ret i64 0
b1608:
  %v3087 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3088 = and i64 %v0, 1
  %v3089 = icmp ne i64 %v3088, 0
  br i1 %v3089, label %b1889, label %b1890
b1609:
  store i64 0, ptr %gcs1
  %v2575 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v2575, ptr %gcs1
  %v2576 = load i8, ptr %ctx, align 8
  %c640 = zext i8 %v2576 to i64
  %v2577 = icmp ne i64 %c640, 0
  br i1 %v2577, label %b1615, label %b1614
b1610:
  %v2574 = icmp eq i64 %v2571, %v6
  br i1 %v2574, label %b1612, label %b1609
b1611:
  %gcvolta1611 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1611, ptr %ctxtopo, align 8
  ret i64 0
b1612:
  %v3219 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3220 = and i64 %v0, 1
  %v3221 = icmp ne i64 %v3220, 0
  br i1 %v3221, label %b1954, label %b1955
b1613:
  store i64 0, ptr %gcs1
  %v2579 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v2579, ptr %gcs1
  %v2580 = load i8, ptr %ctx, align 8
  %c641 = zext i8 %v2580 to i64
  %v2581 = icmp ne i64 %c641, 0
  br i1 %v2581, label %b1620, label %b1619
b1614:
  %v2578 = icmp eq i64 %v2575, %v6
  br i1 %v2578, label %b1616, label %b1613
b1615:
  %gcvolta1615 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1615, ptr %ctxtopo, align 8
  ret i64 0
b1616:
  br label %b1576
b1618:
  store i64 0, ptr %gcs1
  %v2583 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v2583, ptr %gcs1
  %v2584 = load i8, ptr %ctx, align 8
  %c642 = zext i8 %v2584 to i64
  %v2585 = icmp ne i64 %c642, 0
  br i1 %v2585, label %b1625, label %b1624
b1619:
  %v2582 = icmp eq i64 %v2579, %v6
  br i1 %v2582, label %b1621, label %b1618
b1620:
  %gcvolta1620 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1620, ptr %ctxtopo, align 8
  ret i64 0
b1621:
  br label %b1576
b1623:
  br label %b12
b1624:
  %v2586 = icmp eq i64 %v2583, %v6
  br i1 %v2586, label %b1626, label %b1623
b1625:
  %gcvolta1625 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1625, ptr %ctxtopo, align 8
  ret i64 0
b1626:
  br label %b1576
b1627:
  %v2594 = icmp eq i64 %v0, %v2589
  br label %b1629
b1628:
  %v2595 = call i64 @df.class(i64 %v0)
  switch i64 %v2595, label %b1633 [ i64 675, label %b1630 i64 4, label %b1631 i64 2, label %b1632 i64 3, label %b1632 ]
b1629:
  %v2613 = phi i1 [ %v2594, %b1627 ], [ %v2612, %b1634 ]
  br i1 %v2613, label %b1645, label %b1646
b1630:
  %v2596 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v2589)
  %v2597 = load i8, ptr %ctx, align 8
  %c643 = zext i8 %v2597 to i64
  %v2598 = icmp ne i64 %c643, 0
  br i1 %v2598, label %b1636, label %b1635
b1631:
  %v2600 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v2589)
  %v2601 = load i8, ptr %ctx, align 8
  %c644 = zext i8 %v2601 to i64
  %v2602 = icmp ne i64 %c644, 0
  br i1 %v2602, label %b1638, label %b1637
b1632:
  %v2603 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v2589)
  %v2604 = load i8, ptr %ctx, align 8
  %c645 = zext i8 %v2604 to i64
  %v2605 = icmp ne i64 %c645, 0
  br i1 %v2605, label %b1640, label %b1639
b1633:
  %sa2606_0 = getelementptr [2 x i64], ptr %sargs2606, i64 0, i64 0
  store i64 %v2589, ptr %sa2606_0
  %sat2606 = getelementptr [2 x i64], ptr %sargs2606, i64 0, i64 1
  store i64 0, ptr %sat2606
  %sic2606 = getelementptr i64, ptr %area, i64 270
  %sf2606 = call ptr @df.selector(ptr %sic2606, i64 %v0, i64 -9163797671095228218, ptr @df.seln.9, i64 4)
  %v2606 = call i64 %sf2606(i64 %v0, ptr %sargs2606, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2606, ptr %gcs0
  %v2607 = load i8, ptr %ctx, align 8
  %c646 = zext i8 %v2607 to i64
  %v2608 = icmp ne i64 %c646, 0
  br i1 %v2608, label %b1642, label %b1641
b1634:
  %v2612 = phi i1 [ %v2599, %b1635 ], [ %v2600, %b1637 ], [ %v2603, %b1639 ], [ %v2609, %b1643 ]
  br label %b1629
b1635:
  %c647 = zext i8 %v2596 to i64
  %v2599 = icmp ne i64 %c647, 0
  br label %b1634
b1636:
  %gcvolta1636 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1636, ptr %ctxtopo, align 8
  ret i64 0
b1637:
  br label %b1634
b1638:
  %gcvolta1638 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1638, ptr %ctxtopo, align 8
  ret i64 0
b1639:
  br label %b1634
b1640:
  %gcvolta1640 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1640, ptr %ctxtopo, align 8
  ret i64 0
b1641:
  store i64 0, ptr %gcs1
  %v2609 = call i1 @df.unbox_bool(i64 %v2606)
  %v2610 = load i8, ptr %ctx, align 8
  %c648 = zext i8 %v2610 to i64
  %v2611 = icmp ne i64 %c648, 0
  br i1 %v2611, label %b1644, label %b1643
b1642:
  %gcvolta1642 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1642, ptr %ctxtopo, align 8
  ret i64 0
b1643:
  br label %b1634
b1644:
  %gcvolta1644 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1644, ptr %ctxtopo, align 8
  ret i64 0
b1645:
  %v2614 = add i64 0, 0
  br label %b1647
b1646:
  %v2615 = add i64 0, 1
  br label %b1647
b1647:
  %v2616 = phi i64 [ %v2614, %b1645 ], [ %v2615, %b1646 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2617 = call i64 @df.box_int(i64 %v2616)
  %xcp2618 = load i8, ptr %ctx, align 8
  %xcn2618 = icmp ne i8 %xcp2618, 0
  %xce2618 = call i1 @llvm.expect.i1(i1 %xcn2618, i1 false)
  br i1 %xce2618, label %xc2618.limpar, label %xc2618.fim
xc2618.limpar:
  call void @dartforge_exception_clear()
  br label %xc2618.fim
xc2618.fim:
  %gcvolta1647 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1647, ptr %ctxtopo, align 8
  ret i64 %v2617
b1649:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2623 = call i64 @df.box_int(i64 %v2620)
  store i64 %v2623, ptr %gcs1
  %v2624 = call i64 @df.box_int(i64 %v2621)
  store i64 %v2624, ptr %gcs2
  %v2625 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2623, i64 %v2624)
  %v2626 = load i8, ptr %ctx, align 8
  %c649 = zext i8 %v2626 to i64
  %v2627 = icmp ne i64 %c649, 0
  br i1 %v2627, label %b1654, label %b1653
b1650:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2628 = call i64 @df.box_int(i64 %v2620)
  store i64 %v2628, ptr %gcs1
  %v2629 = call i64 @df.box_int(i64 %v2621)
  store i64 %v2629, ptr %gcs2
  %v2630 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2628, i64 %v2629)
  %v2631 = load i8, ptr %ctx, align 8
  %c650 = zext i8 %v2631 to i64
  %v2632 = icmp ne i64 %c650, 0
  br i1 %v2632, label %b1656, label %b1655
b1651:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2633 = call i64 @df.box_int(i64 %v2620)
  store i64 %v2633, ptr %gcs1
  %v2634 = call i64 @df.box_int(i64 %v2621)
  store i64 %v2634, ptr %gcs2
  %sa2635_0 = getelementptr [3 x i64], ptr %sargs2635, i64 0, i64 0
  store i64 %v2633, ptr %sa2635_0
  %sa2635_1 = getelementptr [3 x i64], ptr %sargs2635, i64 0, i64 1
  store i64 %v2634, ptr %sa2635_1
  %sat2635 = getelementptr [3 x i64], ptr %sargs2635, i64 0, i64 2
  store i64 0, ptr %sat2635
  %sic2635 = getelementptr i64, ptr %area, i64 272
  %sf2635 = call ptr @df.selector(ptr %sic2635, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2635 = call i64 %sf2635(i64 %v0, ptr %sargs2635, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2636 = load i8, ptr %ctx, align 8
  %c651 = zext i8 %v2636 to i64
  %v2637 = icmp ne i64 %c651, 0
  br i1 %v2637, label %b1658, label %b1657
b1652:
  %v2638 = phi i64 [ %v2625, %b1653 ], [ %v2630, %b1655 ], [ %v2635, %b1657 ]
  store i64 %v2638, ptr %gcs0
  %v2639 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2640 = and i64 %v2638, 1
  %v2641 = icmp ne i64 %v2640, 0
  br i1 %v2641, label %b1659, label %b1660
b1653:
  br label %b1652
b1654:
  %gcvolta1654 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1654, ptr %ctxtopo, align 8
  ret i64 0
b1655:
  br label %b1652
b1656:
  %gcvolta1656 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1656, ptr %ctxtopo, align 8
  ret i64 0
b1657:
  br label %b1652
b1658:
  %gcvolta1658 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1658, ptr %ctxtopo, align 8
  ret i64 0
b1659:
  %v2642 = ashr i64 %v2638, 1
  %v2643 = mul i64 %v2642, %v2639
  store i64 0, ptr %gcs0
  %v2644 = call i64 @df.box_int(i64 %v2643)
  br label %b1661
b1660:
  %v2645 = call i64 @df.box_int(i64 %v2639)
  store i64 %v2645, ptr %gcs1
  %v2646 = call i64 @df.class(i64 %v2638)
  %v2647 = icmp eq i64 %v2646, 2
  %v2648 = icmp eq i64 %v2646, 3
  %c652 = zext i1 %v2647 to i64
  %c653 = zext i1 %v2648 to i64
  %v2649 = or i64 %c652, %c653
  %v2650 = icmp eq i64 %v2646, 4
  %c654 = zext i1 %v2650 to i64
  %v2651 = or i64 %v2649, %c654
  %v2652 = call i64 @df.class(i64 %v2645)
  %v2653 = icmp eq i64 %v2652, 2
  %v2654 = icmp eq i64 %v2652, 3
  %c655 = zext i1 %v2653 to i64
  %c656 = zext i1 %v2654 to i64
  %v2655 = or i64 %c655, %c656
  %v2656 = icmp eq i64 %v2652, 4
  %c657 = zext i1 %v2656 to i64
  %v2657 = or i64 %v2655, %c657
  %v2658 = and i64 %v2651, %v2657
  %c658 = trunc i64 %v2658 to i1
  br i1 %c658, label %b1662, label %b1663
b1661:
  %v2666 = phi i64 [ %v2644, %b1659 ], [ %v2665, %b1664 ]
  store i64 %v2666, ptr %gcs0
  %v2667 = call i64 @df.class(i64 %v2666)
  switch i64 %v2667, label %b1671 [ i64 4, label %b1669 i64 2, label %b1670 i64 3, label %b1670 ]
b1662:
  %v2659 = call i64 @dartforge_dyn_op(i64 2, i64 %v2638, i64 %v2645)
  %v2660 = load i8, ptr %ctx, align 8
  %c659 = zext i8 %v2660 to i64
  %v2661 = icmp ne i64 %c659, 0
  br i1 %v2661, label %b1666, label %b1665
b1663:
  %sa2662_0 = getelementptr [2 x i64], ptr %sargs2662, i64 0, i64 0
  store i64 %v2645, ptr %sa2662_0
  %sat2662 = getelementptr [2 x i64], ptr %sargs2662, i64 0, i64 1
  store i64 0, ptr %sat2662
  %sic2662 = getelementptr i64, ptr %area, i64 274
  %sf2662 = call ptr @df.selector(ptr %sic2662, i64 %v2638, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v2662 = call i64 %sf2662(i64 %v2638, ptr %sargs2662, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2663 = load i8, ptr %ctx, align 8
  %c660 = zext i8 %v2663 to i64
  %v2664 = icmp ne i64 %c660, 0
  br i1 %v2664, label %b1668, label %b1667
b1664:
  %v2665 = phi i64 [ %v2659, %b1665 ], [ %v2662, %b1667 ]
  br label %b1661
b1665:
  br label %b1664
b1666:
  %gcvolta1666 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1666, ptr %ctxtopo, align 8
  ret i64 0
b1667:
  br label %b1664
b1668:
  %gcvolta1668 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1668, ptr %ctxtopo, align 8
  ret i64 0
b1669:
  store i64 0, ptr %gcs1
  %v2668 = call double @df.unbox_double(i64 %v2666)
  %v2669 = load i8, ptr %ctx, align 8
  %c661 = zext i8 %v2669 to i64
  %v2670 = icmp ne i64 %c661, 0
  br i1 %v2670, label %b1674, label %b1673
b1670:
  store i64 0, ptr %gcs1
  %v2674 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2666)
  %v2675 = load i8, ptr %ctx, align 8
  %c662 = zext i8 %v2675 to i64
  %v2676 = icmp ne i64 %c662, 0
  br i1 %v2676, label %b1678, label %b1677
b1671:
  store i64 0, ptr %gcs1
  %sat2677 = getelementptr [1 x i64], ptr %sargs2677, i64 0, i64 0
  store i64 0, ptr %sat2677
  %sic2677 = getelementptr i64, ptr %area, i64 276
  %sf2677 = call ptr @df.selector(ptr %sic2677, i64 %v2666, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2677 = call i64 %sf2677(i64 %v2666, ptr %sargs2677, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2677, ptr %gcs0
  %v2678 = load i8, ptr %ctx, align 8
  %c663 = zext i8 %v2678 to i64
  %v2679 = icmp ne i64 %c663, 0
  br i1 %v2679, label %b1680, label %b1679
b1672:
  %v2683 = phi i64 [ %v2671, %b1675 ], [ %v2674, %b1677 ], [ %v2680, %b1681 ]
  store i64 0, ptr %gcs0
  %v2684 = call i64 @df.box_int(i64 %v2683)
  %xcp2685 = load i8, ptr %ctx, align 8
  %xcn2685 = icmp ne i8 %xcp2685, 0
  %xce2685 = call i1 @llvm.expect.i1(i1 %xcn2685, i1 false)
  br i1 %xce2685, label %xc2685.limpar, label %xc2685.fim
xc2685.limpar:
  call void @dartforge_exception_clear()
  br label %xc2685.fim
xc2685.fim:
  %gcvolta1672 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1672, ptr %ctxtopo, align 8
  ret i64 %v2684
b1673:
  store i64 0, ptr %gcs0
  %v2671 = call i64 @dartforge_native_DartForge_double_toInt(double %v2668)
  %v2672 = load i8, ptr %ctx, align 8
  %c664 = zext i8 %v2672 to i64
  %v2673 = icmp ne i64 %c664, 0
  br i1 %v2673, label %b1676, label %b1675
b1674:
  %gcvolta1674 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1674, ptr %ctxtopo, align 8
  ret i64 0
b1675:
  br label %b1672
b1676:
  %gcvolta1676 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1676, ptr %ctxtopo, align 8
  ret i64 0
b1677:
  br label %b1672
b1678:
  %gcvolta1678 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1678, ptr %ctxtopo, align 8
  ret i64 0
b1679:
  %v2680 = call i64 @df.unbox_int(i64 %v2677)
  %v2681 = load i8, ptr %ctx, align 8
  %c665 = zext i8 %v2681 to i64
  %v2682 = icmp ne i64 %c665, 0
  br i1 %v2682, label %b1682, label %b1681
b1680:
  %gcvolta1680 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1680, ptr %ctxtopo, align 8
  ret i64 0
b1681:
  br label %b1672
b1682:
  %gcvolta1682 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1682, ptr %ctxtopo, align 8
  ret i64 0
b1684:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2690 = call i64 @df.box_int(i64 %v2687)
  store i64 %v2690, ptr %gcs1
  %v2691 = call i64 @df.box_int(i64 %v2688)
  store i64 %v2691, ptr %gcs2
  %v2692 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2690, i64 %v2691)
  %v2693 = load i8, ptr %ctx, align 8
  %c666 = zext i8 %v2693 to i64
  %v2694 = icmp ne i64 %c666, 0
  br i1 %v2694, label %b1689, label %b1688
b1685:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2695 = call i64 @df.box_int(i64 %v2687)
  store i64 %v2695, ptr %gcs1
  %v2696 = call i64 @df.box_int(i64 %v2688)
  store i64 %v2696, ptr %gcs2
  %v2697 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2695, i64 %v2696)
  %v2698 = load i8, ptr %ctx, align 8
  %c667 = zext i8 %v2698 to i64
  %v2699 = icmp ne i64 %c667, 0
  br i1 %v2699, label %b1691, label %b1690
b1686:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2700 = call i64 @df.box_int(i64 %v2687)
  store i64 %v2700, ptr %gcs1
  %v2701 = call i64 @df.box_int(i64 %v2688)
  store i64 %v2701, ptr %gcs2
  %sa2702_0 = getelementptr [3 x i64], ptr %sargs2702, i64 0, i64 0
  store i64 %v2700, ptr %sa2702_0
  %sa2702_1 = getelementptr [3 x i64], ptr %sargs2702, i64 0, i64 1
  store i64 %v2701, ptr %sa2702_1
  %sat2702 = getelementptr [3 x i64], ptr %sargs2702, i64 0, i64 2
  store i64 0, ptr %sat2702
  %sic2702 = getelementptr i64, ptr %area, i64 278
  %sf2702 = call ptr @df.selector(ptr %sic2702, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2702 = call i64 %sf2702(i64 %v0, ptr %sargs2702, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2703 = load i8, ptr %ctx, align 8
  %c668 = zext i8 %v2703 to i64
  %v2704 = icmp ne i64 %c668, 0
  br i1 %v2704, label %b1693, label %b1692
b1687:
  %v2705 = phi i64 [ %v2692, %b1688 ], [ %v2697, %b1690 ], [ %v2702, %b1692 ]
  store i64 %v2705, ptr %gcs0
  %v2706 = add i64 0, 15
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2707 = and i64 %v2705, 1
  %v2708 = icmp ne i64 %v2707, 0
  br i1 %v2708, label %b1694, label %b1695
b1688:
  br label %b1687
b1689:
  %gcvolta1689 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1689, ptr %ctxtopo, align 8
  ret i64 0
b1690:
  br label %b1687
b1691:
  %gcvolta1691 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1691, ptr %ctxtopo, align 8
  ret i64 0
b1692:
  br label %b1687
b1693:
  %gcvolta1693 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1693, ptr %ctxtopo, align 8
  ret i64 0
b1694:
  %v2709 = ashr i64 %v2705, 1
  %v2710 = mul i64 %v2709, %v2706
  store i64 0, ptr %gcs0
  %v2711 = call i64 @df.box_int(i64 %v2710)
  br label %b1696
b1695:
  %v2712 = call i64 @df.box_int(i64 %v2706)
  store i64 %v2712, ptr %gcs1
  %v2713 = call i64 @df.class(i64 %v2705)
  %v2714 = icmp eq i64 %v2713, 2
  %v2715 = icmp eq i64 %v2713, 3
  %c669 = zext i1 %v2714 to i64
  %c670 = zext i1 %v2715 to i64
  %v2716 = or i64 %c669, %c670
  %v2717 = icmp eq i64 %v2713, 4
  %c671 = zext i1 %v2717 to i64
  %v2718 = or i64 %v2716, %c671
  %v2719 = call i64 @df.class(i64 %v2712)
  %v2720 = icmp eq i64 %v2719, 2
  %v2721 = icmp eq i64 %v2719, 3
  %c672 = zext i1 %v2720 to i64
  %c673 = zext i1 %v2721 to i64
  %v2722 = or i64 %c672, %c673
  %v2723 = icmp eq i64 %v2719, 4
  %c674 = zext i1 %v2723 to i64
  %v2724 = or i64 %v2722, %c674
  %v2725 = and i64 %v2718, %v2724
  %c675 = trunc i64 %v2725 to i1
  br i1 %c675, label %b1697, label %b1698
b1696:
  %v2733 = phi i64 [ %v2711, %b1694 ], [ %v2732, %b1699 ]
  store i64 %v2733, ptr %gcs0
  %v2734 = call i64 @df.class(i64 %v2733)
  switch i64 %v2734, label %b1706 [ i64 4, label %b1704 i64 2, label %b1705 i64 3, label %b1705 ]
b1697:
  %v2726 = call i64 @dartforge_dyn_op(i64 2, i64 %v2705, i64 %v2712)
  %v2727 = load i8, ptr %ctx, align 8
  %c676 = zext i8 %v2727 to i64
  %v2728 = icmp ne i64 %c676, 0
  br i1 %v2728, label %b1701, label %b1700
b1698:
  %sa2729_0 = getelementptr [2 x i64], ptr %sargs2729, i64 0, i64 0
  store i64 %v2712, ptr %sa2729_0
  %sat2729 = getelementptr [2 x i64], ptr %sargs2729, i64 0, i64 1
  store i64 0, ptr %sat2729
  %sic2729 = getelementptr i64, ptr %area, i64 280
  %sf2729 = call ptr @df.selector(ptr %sic2729, i64 %v2705, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v2729 = call i64 %sf2729(i64 %v2705, ptr %sargs2729, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2730 = load i8, ptr %ctx, align 8
  %c677 = zext i8 %v2730 to i64
  %v2731 = icmp ne i64 %c677, 0
  br i1 %v2731, label %b1703, label %b1702
b1699:
  %v2732 = phi i64 [ %v2726, %b1700 ], [ %v2729, %b1702 ]
  br label %b1696
b1700:
  br label %b1699
b1701:
  %gcvolta1701 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1701, ptr %ctxtopo, align 8
  ret i64 0
b1702:
  br label %b1699
b1703:
  %gcvolta1703 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1703, ptr %ctxtopo, align 8
  ret i64 0
b1704:
  store i64 0, ptr %gcs1
  %v2735 = call double @df.unbox_double(i64 %v2733)
  %v2736 = load i8, ptr %ctx, align 8
  %c678 = zext i8 %v2736 to i64
  %v2737 = icmp ne i64 %c678, 0
  br i1 %v2737, label %b1709, label %b1708
b1705:
  store i64 0, ptr %gcs1
  %v2741 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2733)
  %v2742 = load i8, ptr %ctx, align 8
  %c679 = zext i8 %v2742 to i64
  %v2743 = icmp ne i64 %c679, 0
  br i1 %v2743, label %b1713, label %b1712
b1706:
  store i64 0, ptr %gcs1
  %sat2744 = getelementptr [1 x i64], ptr %sargs2744, i64 0, i64 0
  store i64 0, ptr %sat2744
  %sic2744 = getelementptr i64, ptr %area, i64 282
  %sf2744 = call ptr @df.selector(ptr %sic2744, i64 %v2733, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2744 = call i64 %sf2744(i64 %v2733, ptr %sargs2744, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2744, ptr %gcs0
  %v2745 = load i8, ptr %ctx, align 8
  %c680 = zext i8 %v2745 to i64
  %v2746 = icmp ne i64 %c680, 0
  br i1 %v2746, label %b1715, label %b1714
b1707:
  %v2750 = phi i64 [ %v2738, %b1710 ], [ %v2741, %b1712 ], [ %v2747, %b1716 ]
  store i64 0, ptr %gcs0
  %v2751 = call i64 @df.box_int(i64 %v2750)
  %xcp2752 = load i8, ptr %ctx, align 8
  %xcn2752 = icmp ne i8 %xcp2752, 0
  %xce2752 = call i1 @llvm.expect.i1(i1 %xcn2752, i1 false)
  br i1 %xce2752, label %xc2752.limpar, label %xc2752.fim
xc2752.limpar:
  call void @dartforge_exception_clear()
  br label %xc2752.fim
xc2752.fim:
  %gcvolta1707 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1707, ptr %ctxtopo, align 8
  ret i64 %v2751
b1708:
  store i64 0, ptr %gcs0
  %v2738 = call i64 @dartforge_native_DartForge_double_toInt(double %v2735)
  %v2739 = load i8, ptr %ctx, align 8
  %c681 = zext i8 %v2739 to i64
  %v2740 = icmp ne i64 %c681, 0
  br i1 %v2740, label %b1711, label %b1710
b1709:
  %gcvolta1709 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1709, ptr %ctxtopo, align 8
  ret i64 0
b1710:
  br label %b1707
b1711:
  %gcvolta1711 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1711, ptr %ctxtopo, align 8
  ret i64 0
b1712:
  br label %b1707
b1713:
  %gcvolta1713 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1713, ptr %ctxtopo, align 8
  ret i64 0
b1714:
  %v2747 = call i64 @df.unbox_int(i64 %v2744)
  %v2748 = load i8, ptr %ctx, align 8
  %c682 = zext i8 %v2748 to i64
  %v2749 = icmp ne i64 %c682, 0
  br i1 %v2749, label %b1717, label %b1716
b1715:
  %gcvolta1715 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1715, ptr %ctxtopo, align 8
  ret i64 0
b1716:
  br label %b1707
b1717:
  %gcvolta1717 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1717, ptr %ctxtopo, align 8
  ret i64 0
b1719:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2757 = call i64 @df.box_int(i64 %v2754)
  store i64 %v2757, ptr %gcs1
  %v2758 = call i64 @df.box_int(i64 %v2755)
  store i64 %v2758, ptr %gcs2
  %v2759 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2757, i64 %v2758)
  %v2760 = load i8, ptr %ctx, align 8
  %c683 = zext i8 %v2760 to i64
  %v2761 = icmp ne i64 %c683, 0
  br i1 %v2761, label %b1724, label %b1723
b1720:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2762 = call i64 @df.box_int(i64 %v2754)
  store i64 %v2762, ptr %gcs1
  %v2763 = call i64 @df.box_int(i64 %v2755)
  store i64 %v2763, ptr %gcs2
  %v2764 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2762, i64 %v2763)
  %v2765 = load i8, ptr %ctx, align 8
  %c684 = zext i8 %v2765 to i64
  %v2766 = icmp ne i64 %c684, 0
  br i1 %v2766, label %b1726, label %b1725
b1721:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2767 = call i64 @df.box_int(i64 %v2754)
  store i64 %v2767, ptr %gcs1
  %v2768 = call i64 @df.box_int(i64 %v2755)
  store i64 %v2768, ptr %gcs2
  %sa2769_0 = getelementptr [3 x i64], ptr %sargs2769, i64 0, i64 0
  store i64 %v2767, ptr %sa2769_0
  %sa2769_1 = getelementptr [3 x i64], ptr %sargs2769, i64 0, i64 1
  store i64 %v2768, ptr %sa2769_1
  %sat2769 = getelementptr [3 x i64], ptr %sargs2769, i64 0, i64 2
  store i64 0, ptr %sat2769
  %sic2769 = getelementptr i64, ptr %area, i64 284
  %sf2769 = call ptr @df.selector(ptr %sic2769, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2769 = call i64 %sf2769(i64 %v0, ptr %sargs2769, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2770 = load i8, ptr %ctx, align 8
  %c685 = zext i8 %v2770 to i64
  %v2771 = icmp ne i64 %c685, 0
  br i1 %v2771, label %b1728, label %b1727
b1722:
  %v2772 = phi i64 [ %v2759, %b1723 ], [ %v2764, %b1725 ], [ %v2769, %b1727 ]
  store i64 %v2772, ptr %gcs0
  %v2773 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2774 = and i64 %v2772, 1
  %v2775 = icmp ne i64 %v2774, 0
  br i1 %v2775, label %b1729, label %b1730
b1723:
  br label %b1722
b1724:
  %gcvolta1724 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1724, ptr %ctxtopo, align 8
  ret i64 0
b1725:
  br label %b1722
b1726:
  %gcvolta1726 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1726, ptr %ctxtopo, align 8
  ret i64 0
b1727:
  br label %b1722
b1728:
  %gcvolta1728 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1728, ptr %ctxtopo, align 8
  ret i64 0
b1729:
  %v2776 = ashr i64 %v2772, 1
  %v2777 = mul i64 %v2776, %v2773
  store i64 0, ptr %gcs0
  %v2778 = call i64 @df.box_int(i64 %v2777)
  br label %b1731
b1730:
  %v2779 = call i64 @df.box_int(i64 %v2773)
  store i64 %v2779, ptr %gcs1
  %v2780 = call i64 @df.class(i64 %v2772)
  %v2781 = icmp eq i64 %v2780, 2
  %v2782 = icmp eq i64 %v2780, 3
  %c686 = zext i1 %v2781 to i64
  %c687 = zext i1 %v2782 to i64
  %v2783 = or i64 %c686, %c687
  %v2784 = icmp eq i64 %v2780, 4
  %c688 = zext i1 %v2784 to i64
  %v2785 = or i64 %v2783, %c688
  %v2786 = call i64 @df.class(i64 %v2779)
  %v2787 = icmp eq i64 %v2786, 2
  %v2788 = icmp eq i64 %v2786, 3
  %c689 = zext i1 %v2787 to i64
  %c690 = zext i1 %v2788 to i64
  %v2789 = or i64 %c689, %c690
  %v2790 = icmp eq i64 %v2786, 4
  %c691 = zext i1 %v2790 to i64
  %v2791 = or i64 %v2789, %c691
  %v2792 = and i64 %v2785, %v2791
  %c692 = trunc i64 %v2792 to i1
  br i1 %c692, label %b1732, label %b1733
b1731:
  %v2800 = phi i64 [ %v2778, %b1729 ], [ %v2799, %b1734 ]
  store i64 %v2800, ptr %gcs0
  %v2801 = call i64 @df.class(i64 %v2800)
  switch i64 %v2801, label %b1741 [ i64 4, label %b1739 i64 2, label %b1740 i64 3, label %b1740 ]
b1732:
  %v2793 = call i64 @dartforge_dyn_op(i64 2, i64 %v2772, i64 %v2779)
  %v2794 = load i8, ptr %ctx, align 8
  %c693 = zext i8 %v2794 to i64
  %v2795 = icmp ne i64 %c693, 0
  br i1 %v2795, label %b1736, label %b1735
b1733:
  %sa2796_0 = getelementptr [2 x i64], ptr %sargs2796, i64 0, i64 0
  store i64 %v2779, ptr %sa2796_0
  %sat2796 = getelementptr [2 x i64], ptr %sargs2796, i64 0, i64 1
  store i64 0, ptr %sat2796
  %sic2796 = getelementptr i64, ptr %area, i64 286
  %sf2796 = call ptr @df.selector(ptr %sic2796, i64 %v2772, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v2796 = call i64 %sf2796(i64 %v2772, ptr %sargs2796, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2797 = load i8, ptr %ctx, align 8
  %c694 = zext i8 %v2797 to i64
  %v2798 = icmp ne i64 %c694, 0
  br i1 %v2798, label %b1738, label %b1737
b1734:
  %v2799 = phi i64 [ %v2793, %b1735 ], [ %v2796, %b1737 ]
  br label %b1731
b1735:
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
  store i64 0, ptr %gcs1
  %v2802 = call double @df.unbox_double(i64 %v2800)
  %v2803 = load i8, ptr %ctx, align 8
  %c695 = zext i8 %v2803 to i64
  %v2804 = icmp ne i64 %c695, 0
  br i1 %v2804, label %b1744, label %b1743
b1740:
  store i64 0, ptr %gcs1
  %v2808 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2800)
  %v2809 = load i8, ptr %ctx, align 8
  %c696 = zext i8 %v2809 to i64
  %v2810 = icmp ne i64 %c696, 0
  br i1 %v2810, label %b1748, label %b1747
b1741:
  store i64 0, ptr %gcs1
  %sat2811 = getelementptr [1 x i64], ptr %sargs2811, i64 0, i64 0
  store i64 0, ptr %sat2811
  %sic2811 = getelementptr i64, ptr %area, i64 288
  %sf2811 = call ptr @df.selector(ptr %sic2811, i64 %v2800, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2811 = call i64 %sf2811(i64 %v2800, ptr %sargs2811, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2811, ptr %gcs0
  %v2812 = load i8, ptr %ctx, align 8
  %c697 = zext i8 %v2812 to i64
  %v2813 = icmp ne i64 %c697, 0
  br i1 %v2813, label %b1750, label %b1749
b1742:
  %v2817 = phi i64 [ %v2805, %b1745 ], [ %v2808, %b1747 ], [ %v2814, %b1751 ]
  store i64 0, ptr %gcs0
  %v2818 = call i64 @df.box_int(i64 %v2817)
  %xcp2819 = load i8, ptr %ctx, align 8
  %xcn2819 = icmp ne i8 %xcp2819, 0
  %xce2819 = call i1 @llvm.expect.i1(i1 %xcn2819, i1 false)
  br i1 %xce2819, label %xc2819.limpar, label %xc2819.fim
xc2819.limpar:
  call void @dartforge_exception_clear()
  br label %xc2819.fim
xc2819.fim:
  %gcvolta1742 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1742, ptr %ctxtopo, align 8
  ret i64 %v2818
b1743:
  store i64 0, ptr %gcs0
  %v2805 = call i64 @dartforge_native_DartForge_double_toInt(double %v2802)
  %v2806 = load i8, ptr %ctx, align 8
  %c698 = zext i8 %v2806 to i64
  %v2807 = icmp ne i64 %c698, 0
  br i1 %v2807, label %b1746, label %b1745
b1744:
  %gcvolta1744 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1744, ptr %ctxtopo, align 8
  ret i64 0
b1745:
  br label %b1742
b1746:
  %gcvolta1746 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1746, ptr %ctxtopo, align 8
  ret i64 0
b1747:
  br label %b1742
b1748:
  %gcvolta1748 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1748, ptr %ctxtopo, align 8
  ret i64 0
b1749:
  %v2814 = call i64 @df.unbox_int(i64 %v2811)
  %v2815 = load i8, ptr %ctx, align 8
  %c699 = zext i8 %v2815 to i64
  %v2816 = icmp ne i64 %c699, 0
  br i1 %v2816, label %b1752, label %b1751
b1750:
  %gcvolta1750 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1750, ptr %ctxtopo, align 8
  ret i64 0
b1751:
  br label %b1742
b1752:
  %gcvolta1752 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1752, ptr %ctxtopo, align 8
  ret i64 0
b1754:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2824 = call i64 @df.box_int(i64 %v2821)
  store i64 %v2824, ptr %gcs1
  %v2825 = call i64 @df.box_int(i64 %v2822)
  store i64 %v2825, ptr %gcs2
  %v2826 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2824, i64 %v2825)
  %v2827 = load i8, ptr %ctx, align 8
  %c700 = zext i8 %v2827 to i64
  %v2828 = icmp ne i64 %c700, 0
  br i1 %v2828, label %b1759, label %b1758
b1755:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2829 = call i64 @df.box_int(i64 %v2821)
  store i64 %v2829, ptr %gcs1
  %v2830 = call i64 @df.box_int(i64 %v2822)
  store i64 %v2830, ptr %gcs2
  %v2831 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2829, i64 %v2830)
  %v2832 = load i8, ptr %ctx, align 8
  %c701 = zext i8 %v2832 to i64
  %v2833 = icmp ne i64 %c701, 0
  br i1 %v2833, label %b1761, label %b1760
b1756:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2834 = call i64 @df.box_int(i64 %v2821)
  store i64 %v2834, ptr %gcs1
  %v2835 = call i64 @df.box_int(i64 %v2822)
  store i64 %v2835, ptr %gcs2
  %sa2836_0 = getelementptr [3 x i64], ptr %sargs2836, i64 0, i64 0
  store i64 %v2834, ptr %sa2836_0
  %sa2836_1 = getelementptr [3 x i64], ptr %sargs2836, i64 0, i64 1
  store i64 %v2835, ptr %sa2836_1
  %sat2836 = getelementptr [3 x i64], ptr %sargs2836, i64 0, i64 2
  store i64 0, ptr %sat2836
  %sic2836 = getelementptr i64, ptr %area, i64 290
  %sf2836 = call ptr @df.selector(ptr %sic2836, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2836 = call i64 %sf2836(i64 %v0, ptr %sargs2836, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2837 = load i8, ptr %ctx, align 8
  %c702 = zext i8 %v2837 to i64
  %v2838 = icmp ne i64 %c702, 0
  br i1 %v2838, label %b1763, label %b1762
b1757:
  %v2839 = phi i64 [ %v2826, %b1758 ], [ %v2831, %b1760 ], [ %v2836, %b1762 ]
  store i64 %v2839, ptr %gcs0
  %v2840 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2841 = and i64 %v2839, 1
  %v2842 = icmp ne i64 %v2841, 0
  br i1 %v2842, label %b1764, label %b1765
b1758:
  br label %b1757
b1759:
  %gcvolta1759 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1759, ptr %ctxtopo, align 8
  ret i64 0
b1760:
  br label %b1757
b1761:
  %gcvolta1761 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1761, ptr %ctxtopo, align 8
  ret i64 0
b1762:
  br label %b1757
b1763:
  %gcvolta1763 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1763, ptr %ctxtopo, align 8
  ret i64 0
b1764:
  %v2843 = ashr i64 %v2839, 1
  %v2844 = mul i64 %v2843, %v2840
  store i64 0, ptr %gcs0
  %v2845 = call i64 @df.box_int(i64 %v2844)
  br label %b1766
b1765:
  %v2846 = call i64 @df.box_int(i64 %v2840)
  store i64 %v2846, ptr %gcs1
  %v2847 = call i64 @df.class(i64 %v2839)
  %v2848 = icmp eq i64 %v2847, 2
  %v2849 = icmp eq i64 %v2847, 3
  %c703 = zext i1 %v2848 to i64
  %c704 = zext i1 %v2849 to i64
  %v2850 = or i64 %c703, %c704
  %v2851 = icmp eq i64 %v2847, 4
  %c705 = zext i1 %v2851 to i64
  %v2852 = or i64 %v2850, %c705
  %v2853 = call i64 @df.class(i64 %v2846)
  %v2854 = icmp eq i64 %v2853, 2
  %v2855 = icmp eq i64 %v2853, 3
  %c706 = zext i1 %v2854 to i64
  %c707 = zext i1 %v2855 to i64
  %v2856 = or i64 %c706, %c707
  %v2857 = icmp eq i64 %v2853, 4
  %c708 = zext i1 %v2857 to i64
  %v2858 = or i64 %v2856, %c708
  %v2859 = and i64 %v2852, %v2858
  %c709 = trunc i64 %v2859 to i1
  br i1 %c709, label %b1767, label %b1768
b1766:
  %v2867 = phi i64 [ %v2845, %b1764 ], [ %v2866, %b1769 ]
  store i64 %v2867, ptr %gcs0
  %v2868 = call i64 @df.class(i64 %v2867)
  switch i64 %v2868, label %b1776 [ i64 4, label %b1774 i64 2, label %b1775 i64 3, label %b1775 ]
b1767:
  %v2860 = call i64 @dartforge_dyn_op(i64 2, i64 %v2839, i64 %v2846)
  %v2861 = load i8, ptr %ctx, align 8
  %c710 = zext i8 %v2861 to i64
  %v2862 = icmp ne i64 %c710, 0
  br i1 %v2862, label %b1771, label %b1770
b1768:
  %sa2863_0 = getelementptr [2 x i64], ptr %sargs2863, i64 0, i64 0
  store i64 %v2846, ptr %sa2863_0
  %sat2863 = getelementptr [2 x i64], ptr %sargs2863, i64 0, i64 1
  store i64 0, ptr %sat2863
  %sic2863 = getelementptr i64, ptr %area, i64 292
  %sf2863 = call ptr @df.selector(ptr %sic2863, i64 %v2839, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v2863 = call i64 %sf2863(i64 %v2839, ptr %sargs2863, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2864 = load i8, ptr %ctx, align 8
  %c711 = zext i8 %v2864 to i64
  %v2865 = icmp ne i64 %c711, 0
  br i1 %v2865, label %b1773, label %b1772
b1769:
  %v2866 = phi i64 [ %v2860, %b1770 ], [ %v2863, %b1772 ]
  br label %b1766
b1770:
  br label %b1769
b1771:
  %gcvolta1771 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1771, ptr %ctxtopo, align 8
  ret i64 0
b1772:
  br label %b1769
b1773:
  %gcvolta1773 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1773, ptr %ctxtopo, align 8
  ret i64 0
b1774:
  store i64 0, ptr %gcs1
  %v2869 = call double @df.unbox_double(i64 %v2867)
  %v2870 = load i8, ptr %ctx, align 8
  %c712 = zext i8 %v2870 to i64
  %v2871 = icmp ne i64 %c712, 0
  br i1 %v2871, label %b1779, label %b1778
b1775:
  store i64 0, ptr %gcs1
  %v2875 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2867)
  %v2876 = load i8, ptr %ctx, align 8
  %c713 = zext i8 %v2876 to i64
  %v2877 = icmp ne i64 %c713, 0
  br i1 %v2877, label %b1783, label %b1782
b1776:
  store i64 0, ptr %gcs1
  %sat2878 = getelementptr [1 x i64], ptr %sargs2878, i64 0, i64 0
  store i64 0, ptr %sat2878
  %sic2878 = getelementptr i64, ptr %area, i64 294
  %sf2878 = call ptr @df.selector(ptr %sic2878, i64 %v2867, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2878 = call i64 %sf2878(i64 %v2867, ptr %sargs2878, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2878, ptr %gcs0
  %v2879 = load i8, ptr %ctx, align 8
  %c714 = zext i8 %v2879 to i64
  %v2880 = icmp ne i64 %c714, 0
  br i1 %v2880, label %b1785, label %b1784
b1777:
  %v2884 = phi i64 [ %v2872, %b1780 ], [ %v2875, %b1782 ], [ %v2881, %b1786 ]
  store i64 0, ptr %gcs0
  %v2885 = call i64 @df.box_int(i64 %v2884)
  %xcp2886 = load i8, ptr %ctx, align 8
  %xcn2886 = icmp ne i8 %xcp2886, 0
  %xce2886 = call i1 @llvm.expect.i1(i1 %xcn2886, i1 false)
  br i1 %xce2886, label %xc2886.limpar, label %xc2886.fim
xc2886.limpar:
  call void @dartforge_exception_clear()
  br label %xc2886.fim
xc2886.fim:
  %gcvolta1777 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1777, ptr %ctxtopo, align 8
  ret i64 %v2885
b1778:
  store i64 0, ptr %gcs0
  %v2872 = call i64 @dartforge_native_DartForge_double_toInt(double %v2869)
  %v2873 = load i8, ptr %ctx, align 8
  %c715 = zext i8 %v2873 to i64
  %v2874 = icmp ne i64 %c715, 0
  br i1 %v2874, label %b1781, label %b1780
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
  %v2881 = call i64 @df.unbox_int(i64 %v2878)
  %v2882 = load i8, ptr %ctx, align 8
  %c716 = zext i8 %v2882 to i64
  %v2883 = icmp ne i64 %c716, 0
  br i1 %v2883, label %b1787, label %b1786
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
  %v2891 = call i64 @df.box_int(i64 %v2888)
  store i64 %v2891, ptr %gcs1
  %v2892 = call i64 @df.box_int(i64 %v2889)
  store i64 %v2892, ptr %gcs2
  %v2893 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2891, i64 %v2892)
  %v2894 = load i8, ptr %ctx, align 8
  %c717 = zext i8 %v2894 to i64
  %v2895 = icmp ne i64 %c717, 0
  br i1 %v2895, label %b1794, label %b1793
b1790:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2896 = call i64 @df.box_int(i64 %v2888)
  store i64 %v2896, ptr %gcs1
  %v2897 = call i64 @df.box_int(i64 %v2889)
  store i64 %v2897, ptr %gcs2
  %v2898 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2896, i64 %v2897)
  %v2899 = load i8, ptr %ctx, align 8
  %c718 = zext i8 %v2899 to i64
  %v2900 = icmp ne i64 %c718, 0
  br i1 %v2900, label %b1796, label %b1795
b1791:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2901 = call i64 @df.box_int(i64 %v2888)
  store i64 %v2901, ptr %gcs1
  %v2902 = call i64 @df.box_int(i64 %v2889)
  store i64 %v2902, ptr %gcs2
  %sa2903_0 = getelementptr [3 x i64], ptr %sargs2903, i64 0, i64 0
  store i64 %v2901, ptr %sa2903_0
  %sa2903_1 = getelementptr [3 x i64], ptr %sargs2903, i64 0, i64 1
  store i64 %v2902, ptr %sa2903_1
  %sat2903 = getelementptr [3 x i64], ptr %sargs2903, i64 0, i64 2
  store i64 0, ptr %sat2903
  %sic2903 = getelementptr i64, ptr %area, i64 296
  %sf2903 = call ptr @df.selector(ptr %sic2903, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2903 = call i64 %sf2903(i64 %v0, ptr %sargs2903, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2904 = load i8, ptr %ctx, align 8
  %c719 = zext i8 %v2904 to i64
  %v2905 = icmp ne i64 %c719, 0
  br i1 %v2905, label %b1798, label %b1797
b1792:
  %v2906 = phi i64 [ %v2893, %b1793 ], [ %v2898, %b1795 ], [ %v2903, %b1797 ]
  store i64 %v2906, ptr %gcs0
  %v2907 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2908 = and i64 %v2906, 1
  %v2909 = icmp ne i64 %v2908, 0
  br i1 %v2909, label %b1799, label %b1800
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
  %v2910 = ashr i64 %v2906, 1
  %v2911 = mul i64 %v2910, %v2907
  store i64 0, ptr %gcs0
  %v2912 = call i64 @df.box_int(i64 %v2911)
  br label %b1801
b1800:
  %v2913 = call i64 @df.box_int(i64 %v2907)
  store i64 %v2913, ptr %gcs1
  %v2914 = call i64 @df.class(i64 %v2906)
  %v2915 = icmp eq i64 %v2914, 2
  %v2916 = icmp eq i64 %v2914, 3
  %c720 = zext i1 %v2915 to i64
  %c721 = zext i1 %v2916 to i64
  %v2917 = or i64 %c720, %c721
  %v2918 = icmp eq i64 %v2914, 4
  %c722 = zext i1 %v2918 to i64
  %v2919 = or i64 %v2917, %c722
  %v2920 = call i64 @df.class(i64 %v2913)
  %v2921 = icmp eq i64 %v2920, 2
  %v2922 = icmp eq i64 %v2920, 3
  %c723 = zext i1 %v2921 to i64
  %c724 = zext i1 %v2922 to i64
  %v2923 = or i64 %c723, %c724
  %v2924 = icmp eq i64 %v2920, 4
  %c725 = zext i1 %v2924 to i64
  %v2925 = or i64 %v2923, %c725
  %v2926 = and i64 %v2919, %v2925
  %c726 = trunc i64 %v2926 to i1
  br i1 %c726, label %b1802, label %b1803
b1801:
  %v2934 = phi i64 [ %v2912, %b1799 ], [ %v2933, %b1804 ]
  store i64 %v2934, ptr %gcs0
  %v2935 = call i64 @df.class(i64 %v2934)
  switch i64 %v2935, label %b1811 [ i64 4, label %b1809 i64 2, label %b1810 i64 3, label %b1810 ]
b1802:
  %v2927 = call i64 @dartforge_dyn_op(i64 2, i64 %v2906, i64 %v2913)
  %v2928 = load i8, ptr %ctx, align 8
  %c727 = zext i8 %v2928 to i64
  %v2929 = icmp ne i64 %c727, 0
  br i1 %v2929, label %b1806, label %b1805
b1803:
  %sa2930_0 = getelementptr [2 x i64], ptr %sargs2930, i64 0, i64 0
  store i64 %v2913, ptr %sa2930_0
  %sat2930 = getelementptr [2 x i64], ptr %sargs2930, i64 0, i64 1
  store i64 0, ptr %sat2930
  %sic2930 = getelementptr i64, ptr %area, i64 298
  %sf2930 = call ptr @df.selector(ptr %sic2930, i64 %v2906, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v2930 = call i64 %sf2930(i64 %v2906, ptr %sargs2930, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2931 = load i8, ptr %ctx, align 8
  %c728 = zext i8 %v2931 to i64
  %v2932 = icmp ne i64 %c728, 0
  br i1 %v2932, label %b1808, label %b1807
b1804:
  %v2933 = phi i64 [ %v2927, %b1805 ], [ %v2930, %b1807 ]
  br label %b1801
b1805:
  br label %b1804
b1806:
  %gcvolta1806 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1806, ptr %ctxtopo, align 8
  ret i64 0
b1807:
  br label %b1804
b1808:
  %gcvolta1808 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1808, ptr %ctxtopo, align 8
  ret i64 0
b1809:
  store i64 0, ptr %gcs1
  %v2936 = call double @df.unbox_double(i64 %v2934)
  %v2937 = load i8, ptr %ctx, align 8
  %c729 = zext i8 %v2937 to i64
  %v2938 = icmp ne i64 %c729, 0
  br i1 %v2938, label %b1814, label %b1813
b1810:
  store i64 0, ptr %gcs1
  %v2942 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2934)
  %v2943 = load i8, ptr %ctx, align 8
  %c730 = zext i8 %v2943 to i64
  %v2944 = icmp ne i64 %c730, 0
  br i1 %v2944, label %b1818, label %b1817
b1811:
  store i64 0, ptr %gcs1
  %sat2945 = getelementptr [1 x i64], ptr %sargs2945, i64 0, i64 0
  store i64 0, ptr %sat2945
  %sic2945 = getelementptr i64, ptr %area, i64 300
  %sf2945 = call ptr @df.selector(ptr %sic2945, i64 %v2934, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v2945 = call i64 %sf2945(i64 %v2934, ptr %sargs2945, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2945, ptr %gcs0
  %v2946 = load i8, ptr %ctx, align 8
  %c731 = zext i8 %v2946 to i64
  %v2947 = icmp ne i64 %c731, 0
  br i1 %v2947, label %b1820, label %b1819
b1812:
  %v2951 = phi i64 [ %v2939, %b1815 ], [ %v2942, %b1817 ], [ %v2948, %b1821 ]
  store i64 0, ptr %gcs0
  %v2952 = call i64 @df.box_int(i64 %v2951)
  %xcp2953 = load i8, ptr %ctx, align 8
  %xcn2953 = icmp ne i8 %xcp2953, 0
  %xce2953 = call i1 @llvm.expect.i1(i1 %xcn2953, i1 false)
  br i1 %xce2953, label %xc2953.limpar, label %xc2953.fim
xc2953.limpar:
  call void @dartforge_exception_clear()
  br label %xc2953.fim
xc2953.fim:
  %gcvolta1812 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1812, ptr %ctxtopo, align 8
  ret i64 %v2952
b1813:
  store i64 0, ptr %gcs0
  %v2939 = call i64 @dartforge_native_DartForge_double_toInt(double %v2936)
  %v2940 = load i8, ptr %ctx, align 8
  %c732 = zext i8 %v2940 to i64
  %v2941 = icmp ne i64 %c732, 0
  br i1 %v2941, label %b1816, label %b1815
b1814:
  %gcvolta1814 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1814, ptr %ctxtopo, align 8
  ret i64 0
b1815:
  br label %b1812
b1816:
  %gcvolta1816 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1816, ptr %ctxtopo, align 8
  ret i64 0
b1817:
  br label %b1812
b1818:
  %gcvolta1818 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1818, ptr %ctxtopo, align 8
  ret i64 0
b1819:
  %v2948 = call i64 @df.unbox_int(i64 %v2945)
  %v2949 = load i8, ptr %ctx, align 8
  %c733 = zext i8 %v2949 to i64
  %v2950 = icmp ne i64 %c733, 0
  br i1 %v2950, label %b1822, label %b1821
b1820:
  %gcvolta1820 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1820, ptr %ctxtopo, align 8
  ret i64 0
b1821:
  br label %b1812
b1822:
  %gcvolta1822 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1822, ptr %ctxtopo, align 8
  ret i64 0
b1824:
  %v2958 = ashr i64 %v0, 1
  %v2959 = icmp slt i64 %v2958, %v2955
  br label %b1826
b1825:
  %v2960 = call i64 @df.box_int(i64 %v2955)
  store i64 %v2960, ptr %gcs1
  %sa2961_0 = getelementptr [2 x i64], ptr %sargs2961, i64 0, i64 0
  store i64 %v2960, ptr %sa2961_0
  %sat2961 = getelementptr [2 x i64], ptr %sargs2961, i64 0, i64 1
  store i64 0, ptr %sat2961
  %sic2961 = getelementptr i64, ptr %area, i64 302
  %sf2961 = call ptr @df.selector(ptr %sic2961, i64 %v0, i64 -4020809642220582218, ptr @df.seln.15, i64 4)
  %v2961 = call i64 %sf2961(i64 %v0, ptr %sargs2961, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2961, ptr %gcs1
  %v2962 = load i8, ptr %ctx, align 8
  %c734 = zext i8 %v2962 to i64
  %v2963 = icmp ne i64 %c734, 0
  br i1 %v2963, label %b1828, label %b1827
b1826:
  %v2967 = phi i1 [ %v2959, %b1824 ], [ %v2964, %b1829 ]
  br i1 %v2967, label %b1831, label %b1832
b1827:
  %v2964 = call i1 @df.unbox_bool(i64 %v2961)
  %v2965 = load i8, ptr %ctx, align 8
  %c735 = zext i8 %v2965 to i64
  %v2966 = icmp ne i64 %c735, 0
  br i1 %v2966, label %b1830, label %b1829
b1828:
  %gcvolta1828 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1828, ptr %ctxtopo, align 8
  ret i64 0
b1829:
  br label %b1826
b1830:
  %gcvolta1830 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1830, ptr %ctxtopo, align 8
  ret i64 0
b1831:
  %v2969 = add i64 0, 1
  %v2970 = sub i64 0, %v2969
  %v2971 = add i64 0, 1
  %v2972 = call i64 @df.class(i64 %v0)
  switch i64 %v2972, label %b1836 [ i64 4, label %b1834 i64 2, label %b1835 i64 3, label %b1835 ]
b1832:
  %v3018 = add i64 0, 1
  %v3019 = sub i64 0, %v3018
  %v3020 = add i64 0, 1
  %v3021 = call i64 @df.class(i64 %v0)
  switch i64 %v3021, label %b1856 [ i64 4, label %b1854 i64 2, label %b1855 i64 3, label %b1855 ]
b1833:
  %v3066 = phi i64 [ %v3016, %b1846 ], [ %v3065, %b1866 ]
  store i64 %v3066, ptr %gcs0
  %v3067 = call i64 @df.class(i64 %v3066)
  switch i64 %v3067, label %b1876 [ i64 4, label %b1874 i64 2, label %b1875 i64 3, label %b1875 ]
b1834:
  store i64 0, ptr %gcs1
  %v2973 = call i64 @df.box_int(i64 %v2970)
  store i64 %v2973, ptr %gcs1
  %v2974 = call i64 @df.box_int(i64 %v2971)
  store i64 %v2974, ptr %gcs2
  %v2975 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2973, i64 %v2974)
  %v2976 = load i8, ptr %ctx, align 8
  %c736 = zext i8 %v2976 to i64
  %v2977 = icmp ne i64 %c736, 0
  br i1 %v2977, label %b1839, label %b1838
b1835:
  store i64 0, ptr %gcs1
  %v2978 = call i64 @df.box_int(i64 %v2970)
  store i64 %v2978, ptr %gcs1
  %v2979 = call i64 @df.box_int(i64 %v2971)
  store i64 %v2979, ptr %gcs2
  %v2980 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2978, i64 %v2979)
  %v2981 = load i8, ptr %ctx, align 8
  %c737 = zext i8 %v2981 to i64
  %v2982 = icmp ne i64 %c737, 0
  br i1 %v2982, label %b1841, label %b1840
b1836:
  store i64 0, ptr %gcs1
  %v2983 = call i64 @df.box_int(i64 %v2970)
  store i64 %v2983, ptr %gcs1
  %v2984 = call i64 @df.box_int(i64 %v2971)
  store i64 %v2984, ptr %gcs2
  %sa2985_0 = getelementptr [3 x i64], ptr %sargs2985, i64 0, i64 0
  store i64 %v2983, ptr %sa2985_0
  %sa2985_1 = getelementptr [3 x i64], ptr %sargs2985, i64 0, i64 1
  store i64 %v2984, ptr %sa2985_1
  %sat2985 = getelementptr [3 x i64], ptr %sargs2985, i64 0, i64 2
  store i64 0, ptr %sat2985
  %sic2985 = getelementptr i64, ptr %area, i64 304
  %sf2985 = call ptr @df.selector(ptr %sic2985, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2985 = call i64 %sf2985(i64 %v0, ptr %sargs2985, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2986 = load i8, ptr %ctx, align 8
  %c738 = zext i8 %v2986 to i64
  %v2987 = icmp ne i64 %c738, 0
  br i1 %v2987, label %b1843, label %b1842
b1837:
  %v2988 = phi i64 [ %v2975, %b1838 ], [ %v2980, %b1840 ], [ %v2985, %b1842 ]
  store i64 %v2988, ptr %gcs0
  %v2989 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2990 = and i64 %v2988, 1
  %v2991 = icmp ne i64 %v2990, 0
  br i1 %v2991, label %b1844, label %b1845
b1838:
  br label %b1837
b1839:
  %gcvolta1839 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1839, ptr %ctxtopo, align 8
  ret i64 0
b1840:
  br label %b1837
b1841:
  %gcvolta1841 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1841, ptr %ctxtopo, align 8
  ret i64 0
b1842:
  br label %b1837
b1843:
  %gcvolta1843 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1843, ptr %ctxtopo, align 8
  ret i64 0
b1844:
  %v2992 = ashr i64 %v2988, 1
  %v2993 = mul i64 %v2992, %v2989
  store i64 0, ptr %gcs0
  %v2994 = call i64 @df.box_int(i64 %v2993)
  br label %b1846
b1845:
  %v2995 = call i64 @df.box_int(i64 %v2989)
  store i64 %v2995, ptr %gcs1
  %v2996 = call i64 @df.class(i64 %v2988)
  %v2997 = icmp eq i64 %v2996, 2
  %v2998 = icmp eq i64 %v2996, 3
  %c739 = zext i1 %v2997 to i64
  %c740 = zext i1 %v2998 to i64
  %v2999 = or i64 %c739, %c740
  %v3000 = icmp eq i64 %v2996, 4
  %c741 = zext i1 %v3000 to i64
  %v3001 = or i64 %v2999, %c741
  %v3002 = call i64 @df.class(i64 %v2995)
  %v3003 = icmp eq i64 %v3002, 2
  %v3004 = icmp eq i64 %v3002, 3
  %c742 = zext i1 %v3003 to i64
  %c743 = zext i1 %v3004 to i64
  %v3005 = or i64 %c742, %c743
  %v3006 = icmp eq i64 %v3002, 4
  %c744 = zext i1 %v3006 to i64
  %v3007 = or i64 %v3005, %c744
  %v3008 = and i64 %v3001, %v3007
  %c745 = trunc i64 %v3008 to i1
  br i1 %c745, label %b1847, label %b1848
b1846:
  %v3016 = phi i64 [ %v2994, %b1844 ], [ %v3015, %b1849 ]
  br label %b1833
b1847:
  %v3009 = call i64 @dartforge_dyn_op(i64 2, i64 %v2988, i64 %v2995)
  %v3010 = load i8, ptr %ctx, align 8
  %c746 = zext i8 %v3010 to i64
  %v3011 = icmp ne i64 %c746, 0
  br i1 %v3011, label %b1851, label %b1850
b1848:
  %sa3012_0 = getelementptr [2 x i64], ptr %sargs3012, i64 0, i64 0
  store i64 %v2995, ptr %sa3012_0
  %sat3012 = getelementptr [2 x i64], ptr %sargs3012, i64 0, i64 1
  store i64 0, ptr %sat3012
  %sic3012 = getelementptr i64, ptr %area, i64 306
  %sf3012 = call ptr @df.selector(ptr %sic3012, i64 %v2988, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v3012 = call i64 %sf3012(i64 %v2988, ptr %sargs3012, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3013 = load i8, ptr %ctx, align 8
  %c747 = zext i8 %v3013 to i64
  %v3014 = icmp ne i64 %c747, 0
  br i1 %v3014, label %b1853, label %b1852
b1849:
  %v3015 = phi i64 [ %v3009, %b1850 ], [ %v3012, %b1852 ]
  br label %b1846
b1850:
  br label %b1849
b1851:
  %gcvolta1851 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1851, ptr %ctxtopo, align 8
  ret i64 0
b1852:
  br label %b1849
b1853:
  %gcvolta1853 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1853, ptr %ctxtopo, align 8
  ret i64 0
b1854:
  store i64 0, ptr %gcs1
  %v3022 = call i64 @df.box_int(i64 %v3019)
  store i64 %v3022, ptr %gcs1
  %v3023 = call i64 @df.box_int(i64 %v3020)
  store i64 %v3023, ptr %gcs2
  %v3024 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3022, i64 %v3023)
  %v3025 = load i8, ptr %ctx, align 8
  %c748 = zext i8 %v3025 to i64
  %v3026 = icmp ne i64 %c748, 0
  br i1 %v3026, label %b1859, label %b1858
b1855:
  store i64 0, ptr %gcs1
  %v3027 = call i64 @df.box_int(i64 %v3019)
  store i64 %v3027, ptr %gcs1
  %v3028 = call i64 @df.box_int(i64 %v3020)
  store i64 %v3028, ptr %gcs2
  %v3029 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3027, i64 %v3028)
  %v3030 = load i8, ptr %ctx, align 8
  %c749 = zext i8 %v3030 to i64
  %v3031 = icmp ne i64 %c749, 0
  br i1 %v3031, label %b1861, label %b1860
b1856:
  store i64 0, ptr %gcs1
  %v3032 = call i64 @df.box_int(i64 %v3019)
  store i64 %v3032, ptr %gcs1
  %v3033 = call i64 @df.box_int(i64 %v3020)
  store i64 %v3033, ptr %gcs2
  %sa3034_0 = getelementptr [3 x i64], ptr %sargs3034, i64 0, i64 0
  store i64 %v3032, ptr %sa3034_0
  %sa3034_1 = getelementptr [3 x i64], ptr %sargs3034, i64 0, i64 1
  store i64 %v3033, ptr %sa3034_1
  %sat3034 = getelementptr [3 x i64], ptr %sargs3034, i64 0, i64 2
  store i64 0, ptr %sat3034
  %sic3034 = getelementptr i64, ptr %area, i64 308
  %sf3034 = call ptr @df.selector(ptr %sic3034, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3034 = call i64 %sf3034(i64 %v0, ptr %sargs3034, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3035 = load i8, ptr %ctx, align 8
  %c750 = zext i8 %v3035 to i64
  %v3036 = icmp ne i64 %c750, 0
  br i1 %v3036, label %b1863, label %b1862
b1857:
  %v3037 = phi i64 [ %v3024, %b1858 ], [ %v3029, %b1860 ], [ %v3034, %b1862 ]
  store i64 %v3037, ptr %gcs0
  %v3038 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3039 = and i64 %v3037, 1
  %v3040 = icmp ne i64 %v3039, 0
  br i1 %v3040, label %b1864, label %b1865
b1858:
  br label %b1857
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
  %v3041 = ashr i64 %v3037, 1
  %v3042 = mul i64 %v3041, %v3038
  store i64 0, ptr %gcs0
  %v3043 = call i64 @df.box_int(i64 %v3042)
  br label %b1866
b1865:
  %v3044 = call i64 @df.box_int(i64 %v3038)
  store i64 %v3044, ptr %gcs1
  %v3045 = call i64 @df.class(i64 %v3037)
  %v3046 = icmp eq i64 %v3045, 2
  %v3047 = icmp eq i64 %v3045, 3
  %c751 = zext i1 %v3046 to i64
  %c752 = zext i1 %v3047 to i64
  %v3048 = or i64 %c751, %c752
  %v3049 = icmp eq i64 %v3045, 4
  %c753 = zext i1 %v3049 to i64
  %v3050 = or i64 %v3048, %c753
  %v3051 = call i64 @df.class(i64 %v3044)
  %v3052 = icmp eq i64 %v3051, 2
  %v3053 = icmp eq i64 %v3051, 3
  %c754 = zext i1 %v3052 to i64
  %c755 = zext i1 %v3053 to i64
  %v3054 = or i64 %c754, %c755
  %v3055 = icmp eq i64 %v3051, 4
  %c756 = zext i1 %v3055 to i64
  %v3056 = or i64 %v3054, %c756
  %v3057 = and i64 %v3050, %v3056
  %c757 = trunc i64 %v3057 to i1
  br i1 %c757, label %b1867, label %b1868
b1866:
  %v3065 = phi i64 [ %v3043, %b1864 ], [ %v3064, %b1869 ]
  br label %b1833
b1867:
  %v3058 = call i64 @dartforge_dyn_op(i64 2, i64 %v3037, i64 %v3044)
  %v3059 = load i8, ptr %ctx, align 8
  %c758 = zext i8 %v3059 to i64
  %v3060 = icmp ne i64 %c758, 0
  br i1 %v3060, label %b1871, label %b1870
b1868:
  %sa3061_0 = getelementptr [2 x i64], ptr %sargs3061, i64 0, i64 0
  store i64 %v3044, ptr %sa3061_0
  %sat3061 = getelementptr [2 x i64], ptr %sargs3061, i64 0, i64 1
  store i64 0, ptr %sat3061
  %sic3061 = getelementptr i64, ptr %area, i64 310
  %sf3061 = call ptr @df.selector(ptr %sic3061, i64 %v3037, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v3061 = call i64 %sf3061(i64 %v3037, ptr %sargs3061, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3062 = load i8, ptr %ctx, align 8
  %c759 = zext i8 %v3062 to i64
  %v3063 = icmp ne i64 %c759, 0
  br i1 %v3063, label %b1873, label %b1872
b1869:
  %v3064 = phi i64 [ %v3058, %b1870 ], [ %v3061, %b1872 ]
  br label %b1866
b1870:
  br label %b1869
b1871:
  %gcvolta1871 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1871, ptr %ctxtopo, align 8
  ret i64 0
b1872:
  br label %b1869
b1873:
  %gcvolta1873 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1873, ptr %ctxtopo, align 8
  ret i64 0
b1874:
  store i64 0, ptr %gcs1
  %v3068 = call double @df.unbox_double(i64 %v3066)
  %v3069 = load i8, ptr %ctx, align 8
  %c760 = zext i8 %v3069 to i64
  %v3070 = icmp ne i64 %c760, 0
  br i1 %v3070, label %b1879, label %b1878
b1875:
  store i64 0, ptr %gcs1
  %v3074 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3066)
  %v3075 = load i8, ptr %ctx, align 8
  %c761 = zext i8 %v3075 to i64
  %v3076 = icmp ne i64 %c761, 0
  br i1 %v3076, label %b1883, label %b1882
b1876:
  store i64 0, ptr %gcs1
  %sat3077 = getelementptr [1 x i64], ptr %sargs3077, i64 0, i64 0
  store i64 0, ptr %sat3077
  %sic3077 = getelementptr i64, ptr %area, i64 312
  %sf3077 = call ptr @df.selector(ptr %sic3077, i64 %v3066, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3077 = call i64 %sf3077(i64 %v3066, ptr %sargs3077, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3077, ptr %gcs0
  %v3078 = load i8, ptr %ctx, align 8
  %c762 = zext i8 %v3078 to i64
  %v3079 = icmp ne i64 %c762, 0
  br i1 %v3079, label %b1885, label %b1884
b1877:
  %v3083 = phi i64 [ %v3071, %b1880 ], [ %v3074, %b1882 ], [ %v3080, %b1886 ]
  store i64 0, ptr %gcs0
  %v3084 = call i64 @df.box_int(i64 %v3083)
  %xcp3085 = load i8, ptr %ctx, align 8
  %xcn3085 = icmp ne i8 %xcp3085, 0
  %xce3085 = call i1 @llvm.expect.i1(i1 %xcn3085, i1 false)
  br i1 %xce3085, label %xc3085.limpar, label %xc3085.fim
xc3085.limpar:
  call void @dartforge_exception_clear()
  br label %xc3085.fim
xc3085.fim:
  %gcvolta1877 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1877, ptr %ctxtopo, align 8
  ret i64 %v3084
b1878:
  store i64 0, ptr %gcs0
  %v3071 = call i64 @dartforge_native_DartForge_double_toInt(double %v3068)
  %v3072 = load i8, ptr %ctx, align 8
  %c763 = zext i8 %v3072 to i64
  %v3073 = icmp ne i64 %c763, 0
  br i1 %v3073, label %b1881, label %b1880
b1879:
  %gcvolta1879 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1879, ptr %ctxtopo, align 8
  ret i64 0
b1880:
  br label %b1877
b1881:
  %gcvolta1881 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1881, ptr %ctxtopo, align 8
  ret i64 0
b1882:
  br label %b1877
b1883:
  %gcvolta1883 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1883, ptr %ctxtopo, align 8
  ret i64 0
b1884:
  %v3080 = call i64 @df.unbox_int(i64 %v3077)
  %v3081 = load i8, ptr %ctx, align 8
  %c764 = zext i8 %v3081 to i64
  %v3082 = icmp ne i64 %c764, 0
  br i1 %v3082, label %b1887, label %b1886
b1885:
  %gcvolta1885 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1885, ptr %ctxtopo, align 8
  ret i64 0
b1886:
  br label %b1877
b1887:
  %gcvolta1887 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1887, ptr %ctxtopo, align 8
  ret i64 0
b1889:
  %v3090 = ashr i64 %v0, 1
  %v3091 = icmp slt i64 %v3090, %v3087
  br label %b1891
b1890:
  %v3092 = call i64 @df.box_int(i64 %v3087)
  store i64 %v3092, ptr %gcs1
  %sa3093_0 = getelementptr [2 x i64], ptr %sargs3093, i64 0, i64 0
  store i64 %v3092, ptr %sa3093_0
  %sat3093 = getelementptr [2 x i64], ptr %sargs3093, i64 0, i64 1
  store i64 0, ptr %sat3093
  %sic3093 = getelementptr i64, ptr %area, i64 314
  %sf3093 = call ptr @df.selector(ptr %sic3093, i64 %v0, i64 -4020809642220582218, ptr @df.seln.15, i64 4)
  %v3093 = call i64 %sf3093(i64 %v0, ptr %sargs3093, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v3093, ptr %gcs1
  %v3094 = load i8, ptr %ctx, align 8
  %c765 = zext i8 %v3094 to i64
  %v3095 = icmp ne i64 %c765, 0
  br i1 %v3095, label %b1893, label %b1892
b1891:
  %v3099 = phi i1 [ %v3091, %b1889 ], [ %v3096, %b1894 ]
  br i1 %v3099, label %b1896, label %b1897
b1892:
  %v3096 = call i1 @df.unbox_bool(i64 %v3093)
  %v3097 = load i8, ptr %ctx, align 8
  %c766 = zext i8 %v3097 to i64
  %v3098 = icmp ne i64 %c766, 0
  br i1 %v3098, label %b1895, label %b1894
b1893:
  %gcvolta1893 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1893, ptr %ctxtopo, align 8
  ret i64 0
b1894:
  br label %b1891
b1895:
  %gcvolta1895 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1895, ptr %ctxtopo, align 8
  ret i64 0
b1896:
  %v3101 = add i64 0, 1
  %v3102 = sub i64 0, %v3101
  %v3103 = add i64 0, 1
  %v3104 = call i64 @df.class(i64 %v0)
  switch i64 %v3104, label %b1901 [ i64 4, label %b1899 i64 2, label %b1900 i64 3, label %b1900 ]
b1897:
  %v3150 = add i64 0, 1
  %v3151 = sub i64 0, %v3150
  %v3152 = add i64 0, 1
  %v3153 = call i64 @df.class(i64 %v0)
  switch i64 %v3153, label %b1921 [ i64 4, label %b1919 i64 2, label %b1920 i64 3, label %b1920 ]
b1898:
  %v3198 = phi i64 [ %v3148, %b1911 ], [ %v3197, %b1931 ]
  store i64 %v3198, ptr %gcs0
  %v3199 = call i64 @df.class(i64 %v3198)
  switch i64 %v3199, label %b1941 [ i64 4, label %b1939 i64 2, label %b1940 i64 3, label %b1940 ]
b1899:
  store i64 0, ptr %gcs1
  %v3105 = call i64 @df.box_int(i64 %v3102)
  store i64 %v3105, ptr %gcs1
  %v3106 = call i64 @df.box_int(i64 %v3103)
  store i64 %v3106, ptr %gcs2
  %v3107 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3105, i64 %v3106)
  %v3108 = load i8, ptr %ctx, align 8
  %c767 = zext i8 %v3108 to i64
  %v3109 = icmp ne i64 %c767, 0
  br i1 %v3109, label %b1904, label %b1903
b1900:
  store i64 0, ptr %gcs1
  %v3110 = call i64 @df.box_int(i64 %v3102)
  store i64 %v3110, ptr %gcs1
  %v3111 = call i64 @df.box_int(i64 %v3103)
  store i64 %v3111, ptr %gcs2
  %v3112 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3110, i64 %v3111)
  %v3113 = load i8, ptr %ctx, align 8
  %c768 = zext i8 %v3113 to i64
  %v3114 = icmp ne i64 %c768, 0
  br i1 %v3114, label %b1906, label %b1905
b1901:
  store i64 0, ptr %gcs1
  %v3115 = call i64 @df.box_int(i64 %v3102)
  store i64 %v3115, ptr %gcs1
  %v3116 = call i64 @df.box_int(i64 %v3103)
  store i64 %v3116, ptr %gcs2
  %sa3117_0 = getelementptr [3 x i64], ptr %sargs3117, i64 0, i64 0
  store i64 %v3115, ptr %sa3117_0
  %sa3117_1 = getelementptr [3 x i64], ptr %sargs3117, i64 0, i64 1
  store i64 %v3116, ptr %sa3117_1
  %sat3117 = getelementptr [3 x i64], ptr %sargs3117, i64 0, i64 2
  store i64 0, ptr %sat3117
  %sic3117 = getelementptr i64, ptr %area, i64 316
  %sf3117 = call ptr @df.selector(ptr %sic3117, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3117 = call i64 %sf3117(i64 %v0, ptr %sargs3117, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3118 = load i8, ptr %ctx, align 8
  %c769 = zext i8 %v3118 to i64
  %v3119 = icmp ne i64 %c769, 0
  br i1 %v3119, label %b1908, label %b1907
b1902:
  %v3120 = phi i64 [ %v3107, %b1903 ], [ %v3112, %b1905 ], [ %v3117, %b1907 ]
  store i64 %v3120, ptr %gcs0
  %v3121 = add i64 0, 32768
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3122 = and i64 %v3120, 1
  %v3123 = icmp ne i64 %v3122, 0
  br i1 %v3123, label %b1909, label %b1910
b1903:
  br label %b1902
b1904:
  %gcvolta1904 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1904, ptr %ctxtopo, align 8
  ret i64 0
b1905:
  br label %b1902
b1906:
  %gcvolta1906 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1906, ptr %ctxtopo, align 8
  ret i64 0
b1907:
  br label %b1902
b1908:
  %gcvolta1908 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1908, ptr %ctxtopo, align 8
  ret i64 0
b1909:
  %v3124 = ashr i64 %v3120, 1
  %v3125 = mul i64 %v3124, %v3121
  store i64 0, ptr %gcs0
  %v3126 = call i64 @df.box_int(i64 %v3125)
  br label %b1911
b1910:
  %v3127 = call i64 @df.box_int(i64 %v3121)
  store i64 %v3127, ptr %gcs1
  %v3128 = call i64 @df.class(i64 %v3120)
  %v3129 = icmp eq i64 %v3128, 2
  %v3130 = icmp eq i64 %v3128, 3
  %c770 = zext i1 %v3129 to i64
  %c771 = zext i1 %v3130 to i64
  %v3131 = or i64 %c770, %c771
  %v3132 = icmp eq i64 %v3128, 4
  %c772 = zext i1 %v3132 to i64
  %v3133 = or i64 %v3131, %c772
  %v3134 = call i64 @df.class(i64 %v3127)
  %v3135 = icmp eq i64 %v3134, 2
  %v3136 = icmp eq i64 %v3134, 3
  %c773 = zext i1 %v3135 to i64
  %c774 = zext i1 %v3136 to i64
  %v3137 = or i64 %c773, %c774
  %v3138 = icmp eq i64 %v3134, 4
  %c775 = zext i1 %v3138 to i64
  %v3139 = or i64 %v3137, %c775
  %v3140 = and i64 %v3133, %v3139
  %c776 = trunc i64 %v3140 to i1
  br i1 %c776, label %b1912, label %b1913
b1911:
  %v3148 = phi i64 [ %v3126, %b1909 ], [ %v3147, %b1914 ]
  br label %b1898
b1912:
  %v3141 = call i64 @dartforge_dyn_op(i64 2, i64 %v3120, i64 %v3127)
  %v3142 = load i8, ptr %ctx, align 8
  %c777 = zext i8 %v3142 to i64
  %v3143 = icmp ne i64 %c777, 0
  br i1 %v3143, label %b1916, label %b1915
b1913:
  %sa3144_0 = getelementptr [2 x i64], ptr %sargs3144, i64 0, i64 0
  store i64 %v3127, ptr %sa3144_0
  %sat3144 = getelementptr [2 x i64], ptr %sargs3144, i64 0, i64 1
  store i64 0, ptr %sat3144
  %sic3144 = getelementptr i64, ptr %area, i64 318
  %sf3144 = call ptr @df.selector(ptr %sic3144, i64 %v3120, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v3144 = call i64 %sf3144(i64 %v3120, ptr %sargs3144, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3145 = load i8, ptr %ctx, align 8
  %c778 = zext i8 %v3145 to i64
  %v3146 = icmp ne i64 %c778, 0
  br i1 %v3146, label %b1918, label %b1917
b1914:
  %v3147 = phi i64 [ %v3141, %b1915 ], [ %v3144, %b1917 ]
  br label %b1911
b1915:
  br label %b1914
b1916:
  %gcvolta1916 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1916, ptr %ctxtopo, align 8
  ret i64 0
b1917:
  br label %b1914
b1918:
  %gcvolta1918 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1918, ptr %ctxtopo, align 8
  ret i64 0
b1919:
  store i64 0, ptr %gcs1
  %v3154 = call i64 @df.box_int(i64 %v3151)
  store i64 %v3154, ptr %gcs1
  %v3155 = call i64 @df.box_int(i64 %v3152)
  store i64 %v3155, ptr %gcs2
  %v3156 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3154, i64 %v3155)
  %v3157 = load i8, ptr %ctx, align 8
  %c779 = zext i8 %v3157 to i64
  %v3158 = icmp ne i64 %c779, 0
  br i1 %v3158, label %b1924, label %b1923
b1920:
  store i64 0, ptr %gcs1
  %v3159 = call i64 @df.box_int(i64 %v3151)
  store i64 %v3159, ptr %gcs1
  %v3160 = call i64 @df.box_int(i64 %v3152)
  store i64 %v3160, ptr %gcs2
  %v3161 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3159, i64 %v3160)
  %v3162 = load i8, ptr %ctx, align 8
  %c780 = zext i8 %v3162 to i64
  %v3163 = icmp ne i64 %c780, 0
  br i1 %v3163, label %b1926, label %b1925
b1921:
  store i64 0, ptr %gcs1
  %v3164 = call i64 @df.box_int(i64 %v3151)
  store i64 %v3164, ptr %gcs1
  %v3165 = call i64 @df.box_int(i64 %v3152)
  store i64 %v3165, ptr %gcs2
  %sa3166_0 = getelementptr [3 x i64], ptr %sargs3166, i64 0, i64 0
  store i64 %v3164, ptr %sa3166_0
  %sa3166_1 = getelementptr [3 x i64], ptr %sargs3166, i64 0, i64 1
  store i64 %v3165, ptr %sa3166_1
  %sat3166 = getelementptr [3 x i64], ptr %sargs3166, i64 0, i64 2
  store i64 0, ptr %sat3166
  %sic3166 = getelementptr i64, ptr %area, i64 320
  %sf3166 = call ptr @df.selector(ptr %sic3166, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3166 = call i64 %sf3166(i64 %v0, ptr %sargs3166, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3167 = load i8, ptr %ctx, align 8
  %c781 = zext i8 %v3167 to i64
  %v3168 = icmp ne i64 %c781, 0
  br i1 %v3168, label %b1928, label %b1927
b1922:
  %v3169 = phi i64 [ %v3156, %b1923 ], [ %v3161, %b1925 ], [ %v3166, %b1927 ]
  store i64 %v3169, ptr %gcs0
  %v3170 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3171 = and i64 %v3169, 1
  %v3172 = icmp ne i64 %v3171, 0
  br i1 %v3172, label %b1929, label %b1930
b1923:
  br label %b1922
b1924:
  %gcvolta1924 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1924, ptr %ctxtopo, align 8
  ret i64 0
b1925:
  br label %b1922
b1926:
  %gcvolta1926 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1926, ptr %ctxtopo, align 8
  ret i64 0
b1927:
  br label %b1922
b1928:
  %gcvolta1928 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1928, ptr %ctxtopo, align 8
  ret i64 0
b1929:
  %v3173 = ashr i64 %v3169, 1
  %v3174 = mul i64 %v3173, %v3170
  store i64 0, ptr %gcs0
  %v3175 = call i64 @df.box_int(i64 %v3174)
  br label %b1931
b1930:
  %v3176 = call i64 @df.box_int(i64 %v3170)
  store i64 %v3176, ptr %gcs1
  %v3177 = call i64 @df.class(i64 %v3169)
  %v3178 = icmp eq i64 %v3177, 2
  %v3179 = icmp eq i64 %v3177, 3
  %c782 = zext i1 %v3178 to i64
  %c783 = zext i1 %v3179 to i64
  %v3180 = or i64 %c782, %c783
  %v3181 = icmp eq i64 %v3177, 4
  %c784 = zext i1 %v3181 to i64
  %v3182 = or i64 %v3180, %c784
  %v3183 = call i64 @df.class(i64 %v3176)
  %v3184 = icmp eq i64 %v3183, 2
  %v3185 = icmp eq i64 %v3183, 3
  %c785 = zext i1 %v3184 to i64
  %c786 = zext i1 %v3185 to i64
  %v3186 = or i64 %c785, %c786
  %v3187 = icmp eq i64 %v3183, 4
  %c787 = zext i1 %v3187 to i64
  %v3188 = or i64 %v3186, %c787
  %v3189 = and i64 %v3182, %v3188
  %c788 = trunc i64 %v3189 to i1
  br i1 %c788, label %b1932, label %b1933
b1931:
  %v3197 = phi i64 [ %v3175, %b1929 ], [ %v3196, %b1934 ]
  br label %b1898
b1932:
  %v3190 = call i64 @dartforge_dyn_op(i64 2, i64 %v3169, i64 %v3176)
  %v3191 = load i8, ptr %ctx, align 8
  %c789 = zext i8 %v3191 to i64
  %v3192 = icmp ne i64 %c789, 0
  br i1 %v3192, label %b1936, label %b1935
b1933:
  %sa3193_0 = getelementptr [2 x i64], ptr %sargs3193, i64 0, i64 0
  store i64 %v3176, ptr %sa3193_0
  %sat3193 = getelementptr [2 x i64], ptr %sargs3193, i64 0, i64 1
  store i64 0, ptr %sat3193
  %sic3193 = getelementptr i64, ptr %area, i64 322
  %sf3193 = call ptr @df.selector(ptr %sic3193, i64 %v3169, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v3193 = call i64 %sf3193(i64 %v3169, ptr %sargs3193, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3194 = load i8, ptr %ctx, align 8
  %c790 = zext i8 %v3194 to i64
  %v3195 = icmp ne i64 %c790, 0
  br i1 %v3195, label %b1938, label %b1937
b1934:
  %v3196 = phi i64 [ %v3190, %b1935 ], [ %v3193, %b1937 ]
  br label %b1931
b1935:
  br label %b1934
b1936:
  %gcvolta1936 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1936, ptr %ctxtopo, align 8
  ret i64 0
b1937:
  br label %b1934
b1938:
  %gcvolta1938 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1938, ptr %ctxtopo, align 8
  ret i64 0
b1939:
  store i64 0, ptr %gcs1
  %v3200 = call double @df.unbox_double(i64 %v3198)
  %v3201 = load i8, ptr %ctx, align 8
  %c791 = zext i8 %v3201 to i64
  %v3202 = icmp ne i64 %c791, 0
  br i1 %v3202, label %b1944, label %b1943
b1940:
  store i64 0, ptr %gcs1
  %v3206 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3198)
  %v3207 = load i8, ptr %ctx, align 8
  %c792 = zext i8 %v3207 to i64
  %v3208 = icmp ne i64 %c792, 0
  br i1 %v3208, label %b1948, label %b1947
b1941:
  store i64 0, ptr %gcs1
  %sat3209 = getelementptr [1 x i64], ptr %sargs3209, i64 0, i64 0
  store i64 0, ptr %sat3209
  %sic3209 = getelementptr i64, ptr %area, i64 324
  %sf3209 = call ptr @df.selector(ptr %sic3209, i64 %v3198, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3209 = call i64 %sf3209(i64 %v3198, ptr %sargs3209, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3209, ptr %gcs0
  %v3210 = load i8, ptr %ctx, align 8
  %c793 = zext i8 %v3210 to i64
  %v3211 = icmp ne i64 %c793, 0
  br i1 %v3211, label %b1950, label %b1949
b1942:
  %v3215 = phi i64 [ %v3203, %b1945 ], [ %v3206, %b1947 ], [ %v3212, %b1951 ]
  store i64 0, ptr %gcs0
  %v3216 = call i64 @df.box_int(i64 %v3215)
  %xcp3217 = load i8, ptr %ctx, align 8
  %xcn3217 = icmp ne i8 %xcp3217, 0
  %xce3217 = call i1 @llvm.expect.i1(i1 %xcn3217, i1 false)
  br i1 %xce3217, label %xc3217.limpar, label %xc3217.fim
xc3217.limpar:
  call void @dartforge_exception_clear()
  br label %xc3217.fim
xc3217.fim:
  %gcvolta1942 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1942, ptr %ctxtopo, align 8
  ret i64 %v3216
b1943:
  store i64 0, ptr %gcs0
  %v3203 = call i64 @dartforge_native_DartForge_double_toInt(double %v3200)
  %v3204 = load i8, ptr %ctx, align 8
  %c794 = zext i8 %v3204 to i64
  %v3205 = icmp ne i64 %c794, 0
  br i1 %v3205, label %b1946, label %b1945
b1944:
  %gcvolta1944 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1944, ptr %ctxtopo, align 8
  ret i64 0
b1945:
  br label %b1942
b1946:
  %gcvolta1946 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1946, ptr %ctxtopo, align 8
  ret i64 0
b1947:
  br label %b1942
b1948:
  %gcvolta1948 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1948, ptr %ctxtopo, align 8
  ret i64 0
b1949:
  %v3212 = call i64 @df.unbox_int(i64 %v3209)
  %v3213 = load i8, ptr %ctx, align 8
  %c795 = zext i8 %v3213 to i64
  %v3214 = icmp ne i64 %c795, 0
  br i1 %v3214, label %b1952, label %b1951
b1950:
  %gcvolta1950 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1950, ptr %ctxtopo, align 8
  ret i64 0
b1951:
  br label %b1942
b1952:
  %gcvolta1952 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1952, ptr %ctxtopo, align 8
  ret i64 0
b1954:
  %v3222 = ashr i64 %v0, 1
  %v3223 = icmp slt i64 %v3222, %v3219
  br label %b1956
b1955:
  %v3224 = call i64 @df.box_int(i64 %v3219)
  store i64 %v3224, ptr %gcs1
  %sa3225_0 = getelementptr [2 x i64], ptr %sargs3225, i64 0, i64 0
  store i64 %v3224, ptr %sa3225_0
  %sat3225 = getelementptr [2 x i64], ptr %sargs3225, i64 0, i64 1
  store i64 0, ptr %sat3225
  %sic3225 = getelementptr i64, ptr %area, i64 326
  %sf3225 = call ptr @df.selector(ptr %sic3225, i64 %v0, i64 -4020809642220582218, ptr @df.seln.15, i64 4)
  %v3225 = call i64 %sf3225(i64 %v0, ptr %sargs3225, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v3225, ptr %gcs1
  %v3226 = load i8, ptr %ctx, align 8
  %c796 = zext i8 %v3226 to i64
  %v3227 = icmp ne i64 %c796, 0
  br i1 %v3227, label %b1958, label %b1957
b1956:
  %v3231 = phi i1 [ %v3223, %b1954 ], [ %v3228, %b1959 ]
  br i1 %v3231, label %b1961, label %b1962
b1957:
  %v3228 = call i1 @df.unbox_bool(i64 %v3225)
  %v3229 = load i8, ptr %ctx, align 8
  %c797 = zext i8 %v3229 to i64
  %v3230 = icmp ne i64 %c797, 0
  br i1 %v3230, label %b1960, label %b1959
b1958:
  %gcvolta1958 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1958, ptr %ctxtopo, align 8
  ret i64 0
b1959:
  br label %b1956
b1960:
  %gcvolta1960 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1960, ptr %ctxtopo, align 8
  ret i64 0
b1961:
  %v3233 = add i64 0, 1
  %v3234 = sub i64 0, %v3233
  %v3235 = add i64 0, 1
  %v3236 = call i64 @df.class(i64 %v0)
  switch i64 %v3236, label %b1966 [ i64 4, label %b1964 i64 2, label %b1965 i64 3, label %b1965 ]
b1962:
  %v3282 = add i64 0, 1
  %v3283 = sub i64 0, %v3282
  %v3284 = add i64 0, 1
  %v3285 = call i64 @df.class(i64 %v0)
  switch i64 %v3285, label %b1986 [ i64 4, label %b1984 i64 2, label %b1985 i64 3, label %b1985 ]
b1963:
  %v3330 = phi i64 [ %v3280, %b1976 ], [ %v3329, %b1996 ]
  store i64 %v3330, ptr %gcs0
  %v3331 = call i64 @df.class(i64 %v3330)
  switch i64 %v3331, label %b2006 [ i64 4, label %b2004 i64 2, label %b2005 i64 3, label %b2005 ]
b1964:
  store i64 0, ptr %gcs1
  %v3237 = call i64 @df.box_int(i64 %v3234)
  store i64 %v3237, ptr %gcs1
  %v3238 = call i64 @df.box_int(i64 %v3235)
  store i64 %v3238, ptr %gcs2
  %v3239 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3237, i64 %v3238)
  %v3240 = load i8, ptr %ctx, align 8
  %c798 = zext i8 %v3240 to i64
  %v3241 = icmp ne i64 %c798, 0
  br i1 %v3241, label %b1969, label %b1968
b1965:
  store i64 0, ptr %gcs1
  %v3242 = call i64 @df.box_int(i64 %v3234)
  store i64 %v3242, ptr %gcs1
  %v3243 = call i64 @df.box_int(i64 %v3235)
  store i64 %v3243, ptr %gcs2
  %v3244 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3242, i64 %v3243)
  %v3245 = load i8, ptr %ctx, align 8
  %c799 = zext i8 %v3245 to i64
  %v3246 = icmp ne i64 %c799, 0
  br i1 %v3246, label %b1971, label %b1970
b1966:
  store i64 0, ptr %gcs1
  %v3247 = call i64 @df.box_int(i64 %v3234)
  store i64 %v3247, ptr %gcs1
  %v3248 = call i64 @df.box_int(i64 %v3235)
  store i64 %v3248, ptr %gcs2
  %sa3249_0 = getelementptr [3 x i64], ptr %sargs3249, i64 0, i64 0
  store i64 %v3247, ptr %sa3249_0
  %sa3249_1 = getelementptr [3 x i64], ptr %sargs3249, i64 0, i64 1
  store i64 %v3248, ptr %sa3249_1
  %sat3249 = getelementptr [3 x i64], ptr %sargs3249, i64 0, i64 2
  store i64 0, ptr %sat3249
  %sic3249 = getelementptr i64, ptr %area, i64 328
  %sf3249 = call ptr @df.selector(ptr %sic3249, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3249 = call i64 %sf3249(i64 %v0, ptr %sargs3249, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3250 = load i8, ptr %ctx, align 8
  %c800 = zext i8 %v3250 to i64
  %v3251 = icmp ne i64 %c800, 0
  br i1 %v3251, label %b1973, label %b1972
b1967:
  %v3252 = phi i64 [ %v3239, %b1968 ], [ %v3244, %b1970 ], [ %v3249, %b1972 ]
  store i64 %v3252, ptr %gcs0
  %v3253 = add i64 0, 2147483648
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3254 = and i64 %v3252, 1
  %v3255 = icmp ne i64 %v3254, 0
  br i1 %v3255, label %b1974, label %b1975
b1968:
  br label %b1967
b1969:
  %gcvolta1969 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1969, ptr %ctxtopo, align 8
  ret i64 0
b1970:
  br label %b1967
b1971:
  %gcvolta1971 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1971, ptr %ctxtopo, align 8
  ret i64 0
b1972:
  br label %b1967
b1973:
  %gcvolta1973 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1973, ptr %ctxtopo, align 8
  ret i64 0
b1974:
  %v3256 = ashr i64 %v3252, 1
  %v3257 = mul i64 %v3256, %v3253
  store i64 0, ptr %gcs0
  %v3258 = call i64 @df.box_int(i64 %v3257)
  br label %b1976
b1975:
  %v3259 = call i64 @df.box_int(i64 %v3253)
  store i64 %v3259, ptr %gcs1
  %v3260 = call i64 @df.class(i64 %v3252)
  %v3261 = icmp eq i64 %v3260, 2
  %v3262 = icmp eq i64 %v3260, 3
  %c801 = zext i1 %v3261 to i64
  %c802 = zext i1 %v3262 to i64
  %v3263 = or i64 %c801, %c802
  %v3264 = icmp eq i64 %v3260, 4
  %c803 = zext i1 %v3264 to i64
  %v3265 = or i64 %v3263, %c803
  %v3266 = call i64 @df.class(i64 %v3259)
  %v3267 = icmp eq i64 %v3266, 2
  %v3268 = icmp eq i64 %v3266, 3
  %c804 = zext i1 %v3267 to i64
  %c805 = zext i1 %v3268 to i64
  %v3269 = or i64 %c804, %c805
  %v3270 = icmp eq i64 %v3266, 4
  %c806 = zext i1 %v3270 to i64
  %v3271 = or i64 %v3269, %c806
  %v3272 = and i64 %v3265, %v3271
  %c807 = trunc i64 %v3272 to i1
  br i1 %c807, label %b1977, label %b1978
b1976:
  %v3280 = phi i64 [ %v3258, %b1974 ], [ %v3279, %b1979 ]
  br label %b1963
b1977:
  %v3273 = call i64 @dartforge_dyn_op(i64 2, i64 %v3252, i64 %v3259)
  %v3274 = load i8, ptr %ctx, align 8
  %c808 = zext i8 %v3274 to i64
  %v3275 = icmp ne i64 %c808, 0
  br i1 %v3275, label %b1981, label %b1980
b1978:
  %sa3276_0 = getelementptr [2 x i64], ptr %sargs3276, i64 0, i64 0
  store i64 %v3259, ptr %sa3276_0
  %sat3276 = getelementptr [2 x i64], ptr %sargs3276, i64 0, i64 1
  store i64 0, ptr %sat3276
  %sic3276 = getelementptr i64, ptr %area, i64 330
  %sf3276 = call ptr @df.selector(ptr %sic3276, i64 %v3252, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v3276 = call i64 %sf3276(i64 %v3252, ptr %sargs3276, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3277 = load i8, ptr %ctx, align 8
  %c809 = zext i8 %v3277 to i64
  %v3278 = icmp ne i64 %c809, 0
  br i1 %v3278, label %b1983, label %b1982
b1979:
  %v3279 = phi i64 [ %v3273, %b1980 ], [ %v3276, %b1982 ]
  br label %b1976
b1980:
  br label %b1979
b1981:
  %gcvolta1981 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1981, ptr %ctxtopo, align 8
  ret i64 0
b1982:
  br label %b1979
b1983:
  %gcvolta1983 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1983, ptr %ctxtopo, align 8
  ret i64 0
b1984:
  store i64 0, ptr %gcs1
  %v3286 = call i64 @df.box_int(i64 %v3283)
  store i64 %v3286, ptr %gcs1
  %v3287 = call i64 @df.box_int(i64 %v3284)
  store i64 %v3287, ptr %gcs2
  %v3288 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v3286, i64 %v3287)
  %v3289 = load i8, ptr %ctx, align 8
  %c810 = zext i8 %v3289 to i64
  %v3290 = icmp ne i64 %c810, 0
  br i1 %v3290, label %b1989, label %b1988
b1985:
  store i64 0, ptr %gcs1
  %v3291 = call i64 @df.box_int(i64 %v3283)
  store i64 %v3291, ptr %gcs1
  %v3292 = call i64 @df.box_int(i64 %v3284)
  store i64 %v3292, ptr %gcs2
  %v3293 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v3291, i64 %v3292)
  %v3294 = load i8, ptr %ctx, align 8
  %c811 = zext i8 %v3294 to i64
  %v3295 = icmp ne i64 %c811, 0
  br i1 %v3295, label %b1991, label %b1990
b1986:
  store i64 0, ptr %gcs1
  %v3296 = call i64 @df.box_int(i64 %v3283)
  store i64 %v3296, ptr %gcs1
  %v3297 = call i64 @df.box_int(i64 %v3284)
  store i64 %v3297, ptr %gcs2
  %sa3298_0 = getelementptr [3 x i64], ptr %sargs3298, i64 0, i64 0
  store i64 %v3296, ptr %sa3298_0
  %sa3298_1 = getelementptr [3 x i64], ptr %sargs3298, i64 0, i64 1
  store i64 %v3297, ptr %sa3298_1
  %sat3298 = getelementptr [3 x i64], ptr %sargs3298, i64 0, i64 2
  store i64 0, ptr %sat3298
  %sic3298 = getelementptr i64, ptr %area, i64 332
  %sf3298 = call ptr @df.selector(ptr %sic3298, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v3298 = call i64 %sf3298(i64 %v0, ptr %sargs3298, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v3299 = load i8, ptr %ctx, align 8
  %c812 = zext i8 %v3299 to i64
  %v3300 = icmp ne i64 %c812, 0
  br i1 %v3300, label %b1993, label %b1992
b1987:
  %v3301 = phi i64 [ %v3288, %b1988 ], [ %v3293, %b1990 ], [ %v3298, %b1992 ]
  store i64 %v3301, ptr %gcs0
  %v3302 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v3303 = and i64 %v3301, 1
  %v3304 = icmp ne i64 %v3303, 0
  br i1 %v3304, label %b1994, label %b1995
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
  %v3305 = ashr i64 %v3301, 1
  %v3306 = mul i64 %v3305, %v3302
  store i64 0, ptr %gcs0
  %v3307 = call i64 @df.box_int(i64 %v3306)
  br label %b1996
b1995:
  %v3308 = call i64 @df.box_int(i64 %v3302)
  store i64 %v3308, ptr %gcs1
  %v3309 = call i64 @df.class(i64 %v3301)
  %v3310 = icmp eq i64 %v3309, 2
  %v3311 = icmp eq i64 %v3309, 3
  %c813 = zext i1 %v3310 to i64
  %c814 = zext i1 %v3311 to i64
  %v3312 = or i64 %c813, %c814
  %v3313 = icmp eq i64 %v3309, 4
  %c815 = zext i1 %v3313 to i64
  %v3314 = or i64 %v3312, %c815
  %v3315 = call i64 @df.class(i64 %v3308)
  %v3316 = icmp eq i64 %v3315, 2
  %v3317 = icmp eq i64 %v3315, 3
  %c816 = zext i1 %v3316 to i64
  %c817 = zext i1 %v3317 to i64
  %v3318 = or i64 %c816, %c817
  %v3319 = icmp eq i64 %v3315, 4
  %c818 = zext i1 %v3319 to i64
  %v3320 = or i64 %v3318, %c818
  %v3321 = and i64 %v3314, %v3320
  %c819 = trunc i64 %v3321 to i1
  br i1 %c819, label %b1997, label %b1998
b1996:
  %v3329 = phi i64 [ %v3307, %b1994 ], [ %v3328, %b1999 ]
  br label %b1963
b1997:
  %v3322 = call i64 @dartforge_dyn_op(i64 2, i64 %v3301, i64 %v3308)
  %v3323 = load i8, ptr %ctx, align 8
  %c820 = zext i8 %v3323 to i64
  %v3324 = icmp ne i64 %c820, 0
  br i1 %v3324, label %b2001, label %b2000
b1998:
  %sa3325_0 = getelementptr [2 x i64], ptr %sargs3325, i64 0, i64 0
  store i64 %v3308, ptr %sa3325_0
  %sat3325 = getelementptr [2 x i64], ptr %sargs3325, i64 0, i64 1
  store i64 0, ptr %sat3325
  %sic3325 = getelementptr i64, ptr %area, i64 334
  %sf3325 = call ptr @df.selector(ptr %sic3325, i64 %v3301, i64 -677990647923348682, ptr @df.seln.11, i64 3)
  %v3325 = call i64 %sf3325(i64 %v3301, ptr %sargs3325, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v3326 = load i8, ptr %ctx, align 8
  %c821 = zext i8 %v3326 to i64
  %v3327 = icmp ne i64 %c821, 0
  br i1 %v3327, label %b2003, label %b2002
b1999:
  %v3328 = phi i64 [ %v3322, %b2000 ], [ %v3325, %b2002 ]
  br label %b1996
b2000:
  br label %b1999
b2001:
  %gcvolta2001 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2001, ptr %ctxtopo, align 8
  ret i64 0
b2002:
  br label %b1999
b2003:
  %gcvolta2003 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2003, ptr %ctxtopo, align 8
  ret i64 0
b2004:
  store i64 0, ptr %gcs1
  %v3332 = call double @df.unbox_double(i64 %v3330)
  %v3333 = load i8, ptr %ctx, align 8
  %c822 = zext i8 %v3333 to i64
  %v3334 = icmp ne i64 %c822, 0
  br i1 %v3334, label %b2009, label %b2008
b2005:
  store i64 0, ptr %gcs1
  %v3338 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v3330)
  %v3339 = load i8, ptr %ctx, align 8
  %c823 = zext i8 %v3339 to i64
  %v3340 = icmp ne i64 %c823, 0
  br i1 %v3340, label %b2013, label %b2012
b2006:
  store i64 0, ptr %gcs1
  %sat3341 = getelementptr [1 x i64], ptr %sargs3341, i64 0, i64 0
  store i64 0, ptr %sat3341
  %sic3341 = getelementptr i64, ptr %area, i64 336
  %sf3341 = call ptr @df.selector(ptr %sic3341, i64 %v3330, i64 4857594363590788828, ptr @df.seln.14, i64 8)
  %v3341 = call i64 %sf3341(i64 %v3330, ptr %sargs3341, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v3341, ptr %gcs0
  %v3342 = load i8, ptr %ctx, align 8
  %c824 = zext i8 %v3342 to i64
  %v3343 = icmp ne i64 %c824, 0
  br i1 %v3343, label %b2015, label %b2014
b2007:
  %v3347 = phi i64 [ %v3335, %b2010 ], [ %v3338, %b2012 ], [ %v3344, %b2016 ]
  store i64 0, ptr %gcs0
  %v3348 = call i64 @df.box_int(i64 %v3347)
  %xcp3349 = load i8, ptr %ctx, align 8
  %xcn3349 = icmp ne i8 %xcp3349, 0
  %xce3349 = call i1 @llvm.expect.i1(i1 %xcn3349, i1 false)
  br i1 %xce3349, label %xc3349.limpar, label %xc3349.fim
xc3349.limpar:
  call void @dartforge_exception_clear()
  br label %xc3349.fim
xc3349.fim:
  %gcvolta2007 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2007, ptr %ctxtopo, align 8
  ret i64 %v3348
b2008:
  store i64 0, ptr %gcs0
  %v3335 = call i64 @dartforge_native_DartForge_double_toInt(double %v3332)
  %v3336 = load i8, ptr %ctx, align 8
  %c825 = zext i8 %v3336 to i64
  %v3337 = icmp ne i64 %c825, 0
  br i1 %v3337, label %b2011, label %b2010
b2009:
  %gcvolta2009 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2009, ptr %ctxtopo, align 8
  ret i64 0
b2010:
  br label %b2007
b2011:
  %gcvolta2011 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2011, ptr %ctxtopo, align 8
  ret i64 0
b2012:
  br label %b2007
b2013:
  %gcvolta2013 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2013, ptr %ctxtopo, align 8
  ret i64 0
b2014:
  %v3344 = call i64 @df.unbox_int(i64 %v3341)
  %v3345 = load i8, ptr %ctx, align 8
  %c826 = zext i8 %v3345 to i64
  %v3346 = icmp ne i64 %c826, 0
  br i1 %v3346, label %b2017, label %b2016
b2015:
  %gcvolta2015 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2015, ptr %ctxtopo, align 8
  ret i64 0
b2016:
  br label %b2007
b2017:
  %gcvolta2017 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2017, ptr %ctxtopo, align 8
  ret i64 0
}
