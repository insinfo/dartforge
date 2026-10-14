define i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..convertFormatValue(i64 %v0, i64 %v3, i64 %v6) nounwind {
b0:
  %sargs16 = alloca [2 x i64]
  %sargs93 = alloca [2 x i64]
  %sargs106 = alloca [2 x i64]
  %sargs295 = alloca [2 x i64]
  %sargs186 = alloca [2 x i64]
  %sargs209 = alloca [2 x i64]
  %sargs222 = alloca [2 x i64]
  %sargs235 = alloca [2 x i64]
  %sargs248 = alloca [2 x i64]
  %sargs261 = alloca [2 x i64]
  %sargs274 = alloca [2 x i64]
  %sargs287 = alloca [2 x i64]
  %sargs490 = alloca [2 x i64]
  %sargs371 = alloca [2 x i64]
  %sargs417 = alloca [2 x i64]
  %sargs430 = alloca [2 x i64]
  %sargs443 = alloca [2 x i64]
  %sargs456 = alloca [2 x i64]
  %sargs469 = alloca [2 x i64]
  %sargs482 = alloca [2 x i64]
  %sargs705 = alloca [2 x i64]
  %sargs566 = alloca [2 x i64]
  %sargs635 = alloca [2 x i64]
  %sargs648 = alloca [2 x i64]
  %sargs684 = alloca [2 x i64]
  %sargs697 = alloca [2 x i64]
  %sargs950 = alloca [2 x i64]
  %sargs781 = alloca [2 x i64]
  %sargs942 = alloca [2 x i64]
  %sargs1205 = alloca [2 x i64]
  %sargs1026 = alloca [2 x i64]
  %sargs1520 = alloca [2 x i64]
  %sargs1281 = alloca [2 x i64]
  %sargs1301 = alloca [2 x i64]
  %sargs1340 = alloca [2 x i64]
  %sargs1379 = alloca [2 x i64]
  %sargs1418 = alloca [2 x i64]
  %sargs1457 = alloca [2 x i64]
  %sargs1499 = alloca [2 x i64]
  %sargs1512 = alloca [2 x i64]
  %sargs1855 = alloca [2 x i64]
  %sargs1596 = alloca [2 x i64]
  %sargs1616 = alloca [2 x i64]
  %sargs1655 = alloca [2 x i64]
  %sargs1694 = alloca [2 x i64]
  %sargs1733 = alloca [2 x i64]
  %sargs1772 = alloca [2 x i64]
  %sargs2190 = alloca [2 x i64]
  %sargs1931 = alloca [2 x i64]
  %sargs1951 = alloca [2 x i64]
  %sargs1990 = alloca [2 x i64]
  %sargs2029 = alloca [2 x i64]
  %sargs2068 = alloca [2 x i64]
  %sargs2107 = alloca [2 x i64]
  %sargs2266 = alloca [2 x i64]
  %sargs2295 = alloca [3 x i64]
  %sargs2306 = alloca [2 x i64]
  %sargs2320 = alloca [1 x i64]
  %sargs2345 = alloca [3 x i64]
  %sargs2356 = alloca [2 x i64]
  %sargs2370 = alloca [1 x i64]
  %sargs2395 = alloca [3 x i64]
  %sargs2406 = alloca [2 x i64]
  %sargs2420 = alloca [1 x i64]
  %sargs2445 = alloca [3 x i64]
  %sargs2456 = alloca [2 x i64]
  %sargs2470 = alloca [1 x i64]
  %sargs2495 = alloca [3 x i64]
  %sargs2506 = alloca [2 x i64]
  %sargs2520 = alloca [1 x i64]
  %sargs2536 = alloca [2 x i64]
  %sargs2560 = alloca [3 x i64]
  %sargs2571 = alloca [2 x i64]
  %sargs2592 = alloca [3 x i64]
  %sargs2603 = alloca [2 x i64]
  %sargs2618 = alloca [1 x i64]
  %sargs2634 = alloca [2 x i64]
  %sargs2658 = alloca [3 x i64]
  %sargs2669 = alloca [2 x i64]
  %sargs2690 = alloca [3 x i64]
  %sargs2701 = alloca [2 x i64]
  %sargs2716 = alloca [1 x i64]
  %sargs2732 = alloca [2 x i64]
  %sargs2756 = alloca [3 x i64]
  %sargs2767 = alloca [2 x i64]
  %sargs2788 = alloca [3 x i64]
  %sargs2799 = alloca [2 x i64]
  %sargs2814 = alloca [1 x i64]
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
  %sic16 = getelementptr i64, ptr %area, i64 212
  %sf16 = call ptr @df.selector(ptr %sic16, i64 %v3, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
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
  %v2199 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v2199, ptr %gcs1
  %v2200 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v2200 to i64
  %v2201 = icmp ne i64 %c5, 0
  br i1 %v2201, label %b1479, label %b1478
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
  %v304 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v304, ptr %gcs1
  %v305 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v305 to i64
  %v306 = icmp ne i64 %c12, 0
  br i1 %v306, label %b254, label %b253
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
  %v499 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v499, ptr %gcs1
  %v500 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v500 to i64
  %v501 = icmp ne i64 %c14, 0
  br i1 %v501, label %b391, label %b390
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
  %v714 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v714, ptr %gcs1
  %v715 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v715 to i64
  %v716 = icmp ne i64 %c16, 0
  br i1 %v716, label %b536, label %b535
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
  %v959 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v959, ptr %gcs1
  %v960 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v960 to i64
  %v961 = icmp ne i64 %c18, 0
  br i1 %v961, label %b693, label %b692
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
  %v1214 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1214, ptr %gcs1
  %v1215 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v1215 to i64
  %v1216 = icmp ne i64 %c20, 0
  br i1 %v1216, label %b854, label %b853
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
  %v1529 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1529, ptr %gcs1
  %v1530 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v1530 to i64
  %v1531 = icmp ne i64 %c22, 0
  br i1 %v1531, label %b1057, label %b1056
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
  %v1864 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1864, ptr %gcs1
  %v1865 = load i8, ptr %ctx, align 8
  %c24 = zext i8 %v1865 to i64
  %v1866 = icmp ne i64 %c24, 0
  br i1 %v1866, label %b1268, label %b1267
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
  %sic93 = getelementptr i64, ptr %area, i64 214
  %sf93 = call ptr @df.selector(ptr %sic93, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
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
  %sic106 = getelementptr i64, ptr %area, i64 216
  %sf106 = call ptr @df.selector(ptr %sic106, i64 %v102, i64 2870274114568068918, ptr @df.seln.13, i64 5)
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
  %v293 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v294 = call i64 @df.box_int(i64 %v293)
  store i64 %v294, ptr %gcs1
  %sa295_0 = getelementptr [2 x i64], ptr %sargs295, i64 0, i64 0
  store i64 %v294, ptr %sa295_0
  %sat295 = getelementptr [2 x i64], ptr %sargs295, i64 0, i64 1
  store i64 0, ptr %sat295
  %sic295 = getelementptr i64, ptr %area, i64 218
  %sf295 = call ptr @df.selector(ptr %sic295, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v295 = call i64 %sf295(i64 %v0, ptr %sargs295, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v295, ptr %gcs0
  %v296 = load i8, ptr %ctx, align 8
  %c36 = zext i8 %v296 to i64
  %v297 = icmp ne i64 %c36, 0
  br i1 %v297, label %b235, label %b234
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
  %v215 = add i64 0, 75
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v216 = and i64 %v0, 1
  %v217 = icmp ne i64 %v216, 0
  br i1 %v217, label %b198, label %b199
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
  %v228 = add i64 0, 21845
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v229 = and i64 %v0, 1
  %v230 = icmp ne i64 %v229, 0
  br i1 %v230, label %b204, label %b205
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
  %v241 = add i64 0, 1431655765
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v242 = and i64 %v0, 1
  %v243 = icmp ne i64 %v242, 0
  br i1 %v243, label %b210, label %b211
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
  %v254 = add i64 0, 42
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v255 = and i64 %v0, 1
  %v256 = icmp ne i64 %v255, 0
  br i1 %v256, label %b216, label %b217
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
  %v267 = add i64 0, 10922
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v268 = and i64 %v0, 1
  %v269 = icmp ne i64 %v268, 0
  br i1 %v269, label %b222, label %b223
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
  %v280 = add i64 0, 715827882
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v281 = and i64 %v0, 1
  %v282 = icmp ne i64 %v281, 0
  br i1 %v282, label %b228, label %b229
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
  %sic186 = getelementptr i64, ptr %area, i64 220
  %sf186 = call ptr @df.selector(ptr %sic186, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
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
  %sa209_0 = getelementptr [2 x i64], ptr %sargs209, i64 0, i64 0
  store i64 %v208, ptr %sa209_0
  %sat209 = getelementptr [2 x i64], ptr %sargs209, i64 0, i64 1
  store i64 0, ptr %sat209
  %sic209 = getelementptr i64, ptr %area, i64 222
  %sf209 = call ptr @df.selector(ptr %sic209, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v209 = call i64 %sf209(i64 %v0, ptr %sargs209, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v210 = load i8, ptr %ctx, align 8
  %c56 = zext i8 %v210 to i64
  %v211 = icmp ne i64 %c56, 0
  br i1 %v211, label %b196, label %b195
b194:
  %v212 = phi i64 [ %v207, %b192 ], [ %v209, %b195 ]
  %xcp213 = load i8, ptr %ctx, align 8
  %xcn213 = icmp ne i8 %xcp213, 0
  %xce213 = call i1 @llvm.expect.i1(i1 %xcn213, i1 false)
  br i1 %xce213, label %xc213.limpar, label %xc213.fim
xc213.limpar:
  call void @dartforge_exception_clear()
  br label %xc213.fim
xc213.fim:
  %gcvolta194 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta194, ptr %ctxtopo, align 8
  ret i64 %v212
b195:
  br label %b194
b196:
  %gcvolta196 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta196, ptr %ctxtopo, align 8
  ret i64 0
b198:
  %v218 = ashr i64 %v0, 1
  %v219 = mul i64 %v218, %v215
  store i64 0, ptr %gcs0
  %v220 = call i64 @df.box_int(i64 %v219)
  br label %b200
b199:
  %v221 = call i64 @df.box_int(i64 %v215)
  store i64 %v221, ptr %gcs1
  %sa222_0 = getelementptr [2 x i64], ptr %sargs222, i64 0, i64 0
  store i64 %v221, ptr %sa222_0
  %sat222 = getelementptr [2 x i64], ptr %sargs222, i64 0, i64 1
  store i64 0, ptr %sat222
  %sic222 = getelementptr i64, ptr %area, i64 224
  %sf222 = call ptr @df.selector(ptr %sic222, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v222 = call i64 %sf222(i64 %v0, ptr %sargs222, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v223 = load i8, ptr %ctx, align 8
  %c57 = zext i8 %v223 to i64
  %v224 = icmp ne i64 %c57, 0
  br i1 %v224, label %b202, label %b201
b200:
  %v225 = phi i64 [ %v220, %b198 ], [ %v222, %b201 ]
  %xcp226 = load i8, ptr %ctx, align 8
  %xcn226 = icmp ne i8 %xcp226, 0
  %xce226 = call i1 @llvm.expect.i1(i1 %xcn226, i1 false)
  br i1 %xce226, label %xc226.limpar, label %xc226.fim
xc226.limpar:
  call void @dartforge_exception_clear()
  br label %xc226.fim
xc226.fim:
  %gcvolta200 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta200, ptr %ctxtopo, align 8
  ret i64 %v225
b201:
  br label %b200
b202:
  %gcvolta202 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta202, ptr %ctxtopo, align 8
  ret i64 0
b204:
  %v231 = ashr i64 %v0, 1
  %v232 = mul i64 %v231, %v228
  store i64 0, ptr %gcs0
  %v233 = call i64 @df.box_int(i64 %v232)
  br label %b206
b205:
  %v234 = call i64 @df.box_int(i64 %v228)
  store i64 %v234, ptr %gcs1
  %sa235_0 = getelementptr [2 x i64], ptr %sargs235, i64 0, i64 0
  store i64 %v234, ptr %sa235_0
  %sat235 = getelementptr [2 x i64], ptr %sargs235, i64 0, i64 1
  store i64 0, ptr %sat235
  %sic235 = getelementptr i64, ptr %area, i64 226
  %sf235 = call ptr @df.selector(ptr %sic235, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v235 = call i64 %sf235(i64 %v0, ptr %sargs235, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v236 = load i8, ptr %ctx, align 8
  %c58 = zext i8 %v236 to i64
  %v237 = icmp ne i64 %c58, 0
  br i1 %v237, label %b208, label %b207
b206:
  %v238 = phi i64 [ %v233, %b204 ], [ %v235, %b207 ]
  %xcp239 = load i8, ptr %ctx, align 8
  %xcn239 = icmp ne i8 %xcp239, 0
  %xce239 = call i1 @llvm.expect.i1(i1 %xcn239, i1 false)
  br i1 %xce239, label %xc239.limpar, label %xc239.fim
xc239.limpar:
  call void @dartforge_exception_clear()
  br label %xc239.fim
xc239.fim:
  %gcvolta206 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta206, ptr %ctxtopo, align 8
  ret i64 %v238
b207:
  br label %b206
b208:
  %gcvolta208 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta208, ptr %ctxtopo, align 8
  ret i64 0
b210:
  %v244 = ashr i64 %v0, 1
  %v245 = mul i64 %v244, %v241
  store i64 0, ptr %gcs0
  %v246 = call i64 @df.box_int(i64 %v245)
  br label %b212
b211:
  %v247 = call i64 @df.box_int(i64 %v241)
  store i64 %v247, ptr %gcs1
  %sa248_0 = getelementptr [2 x i64], ptr %sargs248, i64 0, i64 0
  store i64 %v247, ptr %sa248_0
  %sat248 = getelementptr [2 x i64], ptr %sargs248, i64 0, i64 1
  store i64 0, ptr %sat248
  %sic248 = getelementptr i64, ptr %area, i64 228
  %sf248 = call ptr @df.selector(ptr %sic248, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v248 = call i64 %sf248(i64 %v0, ptr %sargs248, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v249 = load i8, ptr %ctx, align 8
  %c59 = zext i8 %v249 to i64
  %v250 = icmp ne i64 %c59, 0
  br i1 %v250, label %b214, label %b213
b212:
  %v251 = phi i64 [ %v246, %b210 ], [ %v248, %b213 ]
  %xcp252 = load i8, ptr %ctx, align 8
  %xcn252 = icmp ne i8 %xcp252, 0
  %xce252 = call i1 @llvm.expect.i1(i1 %xcn252, i1 false)
  br i1 %xce252, label %xc252.limpar, label %xc252.fim
xc252.limpar:
  call void @dartforge_exception_clear()
  br label %xc252.fim
xc252.fim:
  %gcvolta212 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta212, ptr %ctxtopo, align 8
  ret i64 %v251
b213:
  br label %b212
b214:
  %gcvolta214 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta214, ptr %ctxtopo, align 8
  ret i64 0
b216:
  %v257 = ashr i64 %v0, 1
  %v258 = mul i64 %v257, %v254
  store i64 0, ptr %gcs0
  %v259 = call i64 @df.box_int(i64 %v258)
  br label %b218
b217:
  %v260 = call i64 @df.box_int(i64 %v254)
  store i64 %v260, ptr %gcs1
  %sa261_0 = getelementptr [2 x i64], ptr %sargs261, i64 0, i64 0
  store i64 %v260, ptr %sa261_0
  %sat261 = getelementptr [2 x i64], ptr %sargs261, i64 0, i64 1
  store i64 0, ptr %sat261
  %sic261 = getelementptr i64, ptr %area, i64 230
  %sf261 = call ptr @df.selector(ptr %sic261, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v261 = call i64 %sf261(i64 %v0, ptr %sargs261, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v262 = load i8, ptr %ctx, align 8
  %c60 = zext i8 %v262 to i64
  %v263 = icmp ne i64 %c60, 0
  br i1 %v263, label %b220, label %b219
b218:
  %v264 = phi i64 [ %v259, %b216 ], [ %v261, %b219 ]
  %xcp265 = load i8, ptr %ctx, align 8
  %xcn265 = icmp ne i8 %xcp265, 0
  %xce265 = call i1 @llvm.expect.i1(i1 %xcn265, i1 false)
  br i1 %xce265, label %xc265.limpar, label %xc265.fim
xc265.limpar:
  call void @dartforge_exception_clear()
  br label %xc265.fim
xc265.fim:
  %gcvolta218 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta218, ptr %ctxtopo, align 8
  ret i64 %v264
b219:
  br label %b218
b220:
  %gcvolta220 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta220, ptr %ctxtopo, align 8
  ret i64 0
b222:
  %v270 = ashr i64 %v0, 1
  %v271 = mul i64 %v270, %v267
  store i64 0, ptr %gcs0
  %v272 = call i64 @df.box_int(i64 %v271)
  br label %b224
b223:
  %v273 = call i64 @df.box_int(i64 %v267)
  store i64 %v273, ptr %gcs1
  %sa274_0 = getelementptr [2 x i64], ptr %sargs274, i64 0, i64 0
  store i64 %v273, ptr %sa274_0
  %sat274 = getelementptr [2 x i64], ptr %sargs274, i64 0, i64 1
  store i64 0, ptr %sat274
  %sic274 = getelementptr i64, ptr %area, i64 232
  %sf274 = call ptr @df.selector(ptr %sic274, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v274 = call i64 %sf274(i64 %v0, ptr %sargs274, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v275 = load i8, ptr %ctx, align 8
  %c61 = zext i8 %v275 to i64
  %v276 = icmp ne i64 %c61, 0
  br i1 %v276, label %b226, label %b225
b224:
  %v277 = phi i64 [ %v272, %b222 ], [ %v274, %b225 ]
  %xcp278 = load i8, ptr %ctx, align 8
  %xcn278 = icmp ne i8 %xcp278, 0
  %xce278 = call i1 @llvm.expect.i1(i1 %xcn278, i1 false)
  br i1 %xce278, label %xc278.limpar, label %xc278.fim
xc278.limpar:
  call void @dartforge_exception_clear()
  br label %xc278.fim
xc278.fim:
  %gcvolta224 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta224, ptr %ctxtopo, align 8
  ret i64 %v277
b225:
  br label %b224
b226:
  %gcvolta226 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta226, ptr %ctxtopo, align 8
  ret i64 0
b228:
  %v283 = ashr i64 %v0, 1
  %v284 = mul i64 %v283, %v280
  store i64 0, ptr %gcs0
  %v285 = call i64 @df.box_int(i64 %v284)
  br label %b230
b229:
  %v286 = call i64 @df.box_int(i64 %v280)
  store i64 %v286, ptr %gcs1
  %sa287_0 = getelementptr [2 x i64], ptr %sargs287, i64 0, i64 0
  store i64 %v286, ptr %sa287_0
  %sat287 = getelementptr [2 x i64], ptr %sargs287, i64 0, i64 1
  store i64 0, ptr %sat287
  %sic287 = getelementptr i64, ptr %area, i64 234
  %sf287 = call ptr @df.selector(ptr %sic287, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v287 = call i64 %sf287(i64 %v0, ptr %sargs287, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v288 = load i8, ptr %ctx, align 8
  %c62 = zext i8 %v288 to i64
  %v289 = icmp ne i64 %c62, 0
  br i1 %v289, label %b232, label %b231
b230:
  %v290 = phi i64 [ %v285, %b228 ], [ %v287, %b231 ]
  %xcp291 = load i8, ptr %ctx, align 8
  %xcn291 = icmp ne i8 %xcp291, 0
  %xce291 = call i1 @llvm.expect.i1(i1 %xcn291, i1 false)
  br i1 %xce291, label %xc291.limpar, label %xc291.fim
xc291.limpar:
  call void @dartforge_exception_clear()
  br label %xc291.fim
xc291.fim:
  %gcvolta230 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta230, ptr %ctxtopo, align 8
  ret i64 %v290
b231:
  br label %b230
b232:
  %gcvolta232 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta232, ptr %ctxtopo, align 8
  ret i64 0
b234:
  store i64 0, ptr %gcs1
  %v298 = call double @df.unbox_double(i64 %v295)
  %v299 = load i8, ptr %ctx, align 8
  %c63 = zext i8 %v299 to i64
  %v300 = icmp ne i64 %c63, 0
  br i1 %v300, label %b237, label %b236
b235:
  %gcvolta235 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta235, ptr %ctxtopo, align 8
  ret i64 0
b236:
  store i64 0, ptr %gcs0
  %v301 = call i64 @df.box_double(double %v298)
  %xcp302 = load i8, ptr %ctx, align 8
  %xcn302 = icmp ne i8 %xcp302, 0
  %xce302 = call i1 @llvm.expect.i1(i1 %xcn302, i1 false)
  br i1 %xce302, label %xc302.limpar, label %xc302.fim
xc302.limpar:
  call void @dartforge_exception_clear()
  br label %xc302.fim
xc302.fim:
  %gcvolta236 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta236, ptr %ctxtopo, align 8
  ret i64 %v301
b237:
  %gcvolta237 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta237, ptr %ctxtopo, align 8
  ret i64 0
b251:
  %v488 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v489 = call i64 @df.box_int(i64 %v488)
  store i64 %v489, ptr %gcs1
  %sa490_0 = getelementptr [2 x i64], ptr %sargs490, i64 0, i64 0
  store i64 %v489, ptr %sa490_0
  %sat490 = getelementptr [2 x i64], ptr %sargs490, i64 0, i64 1
  store i64 0, ptr %sat490
  %sic490 = getelementptr i64, ptr %area, i64 236
  %sf490 = call ptr @df.selector(ptr %sic490, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v490 = call i64 %sf490(i64 %v0, ptr %sargs490, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v490, ptr %gcs0
  %v491 = load i8, ptr %ctx, align 8
  %c64 = zext i8 %v491 to i64
  %v492 = icmp ne i64 %c64, 0
  br i1 %v492, label %b372, label %b371
b252:
  store i64 0, ptr %gcs1
  %v308 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v308, ptr %gcs1
  %v309 = load i8, ptr %ctx, align 8
  %c65 = zext i8 %v309 to i64
  %v310 = icmp ne i64 %c65, 0
  br i1 %v310, label %b258, label %b257
b253:
  %v307 = icmp eq i64 %v304, %v6
  br i1 %v307, label %b255, label %b252
b254:
  %gcvolta254 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta254, ptr %ctxtopo, align 8
  ret i64 0
b255:
  %v353 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v354 = call i64 @df.box_int(i64 %v353)
  store i64 %v354, ptr %gcs1
  %v355 = icmp eq i64 %v0, 0
  %v356 = icmp eq i64 %v354, 0
  %c66 = zext i1 %v355 to i64
  %c67 = zext i1 %v356 to i64
  %v357 = or i64 %c66, %c67
  %v358 = icmp ne i64 %v357, 0
  br i1 %v358, label %b302, label %b303
b256:
  store i64 0, ptr %gcs1
  %v312 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v312, ptr %gcs1
  %v313 = load i8, ptr %ctx, align 8
  %c68 = zext i8 %v313 to i64
  %v314 = icmp ne i64 %c68, 0
  br i1 %v314, label %b262, label %b261
b257:
  %v311 = icmp eq i64 %v308, %v6
  br i1 %v311, label %b259, label %b256
b258:
  %gcvolta258 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta258, ptr %ctxtopo, align 8
  ret i64 0
b259:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v385 = icmp eq i64 %v0, 0
  %v386 = call i64 @df.class(i64 %v0)
  %v387 = call i8 @df.subclass(i64 %v386, i64 674)
  %c69 = zext i8 %v387 to i64
  %v388 = icmp ne i64 %c69, 0
  %v389 = xor i1 %v385, true
  %c70 = zext i1 %v388 to i64
  %c71 = zext i1 %v389 to i64
  %v390 = and i64 %c70, %c71
  %v391 = icmp ne i64 %v390, 0
  br i1 %v391, label %b325, label %b324
b260:
  store i64 0, ptr %gcs1
  %v316 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v316, ptr %gcs1
  %v317 = load i8, ptr %ctx, align 8
  %c72 = zext i8 %v317 to i64
  %v318 = icmp ne i64 %c72, 0
  br i1 %v318, label %b266, label %b265
b261:
  %v315 = icmp eq i64 %v312, %v6
  br i1 %v315, label %b263, label %b260
b262:
  %gcvolta262 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta262, ptr %ctxtopo, align 8
  ret i64 0
b263:
  %xcp408 = load i8, ptr %ctx, align 8
  %xcn408 = icmp ne i8 %xcp408, 0
  %xce408 = call i1 @llvm.expect.i1(i1 %xcn408, i1 false)
  br i1 %xce408, label %xc408.limpar, label %xc408.fim
xc408.limpar:
  call void @dartforge_exception_clear()
  br label %xc408.fim
xc408.fim:
  %gcvolta263 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta263, ptr %ctxtopo, align 8
  ret i64 %v0
b264:
  store i64 0, ptr %gcs1
  %v320 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v320, ptr %gcs1
  %v321 = load i8, ptr %ctx, align 8
  %c73 = zext i8 %v321 to i64
  %v322 = icmp ne i64 %c73, 0
  br i1 %v322, label %b270, label %b269
b265:
  %v319 = icmp eq i64 %v316, %v6
  br i1 %v319, label %b267, label %b264
b266:
  %gcvolta266 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta266, ptr %ctxtopo, align 8
  ret i64 0
b267:
  %v410 = add i64 0, 17
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v411 = and i64 %v0, 1
  %v412 = icmp ne i64 %v411, 0
  br i1 %v412, label %b335, label %b336
b268:
  store i64 0, ptr %gcs1
  %v324 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v324, ptr %gcs1
  %v325 = load i8, ptr %ctx, align 8
  %c74 = zext i8 %v325 to i64
  %v326 = icmp ne i64 %c74, 0
  br i1 %v326, label %b274, label %b273
b269:
  %v323 = icmp eq i64 %v320, %v6
  br i1 %v323, label %b271, label %b268
b270:
  %gcvolta270 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta270, ptr %ctxtopo, align 8
  ret i64 0
b271:
  %v423 = add i64 0, 4369
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v424 = and i64 %v0, 1
  %v425 = icmp ne i64 %v424, 0
  br i1 %v425, label %b341, label %b342
b272:
  store i64 0, ptr %gcs1
  %v328 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v328, ptr %gcs1
  %v329 = load i8, ptr %ctx, align 8
  %c75 = zext i8 %v329 to i64
  %v330 = icmp ne i64 %c75, 0
  br i1 %v330, label %b278, label %b277
b273:
  %v327 = icmp eq i64 %v324, %v6
  br i1 %v327, label %b275, label %b272
b274:
  %gcvolta274 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta274, ptr %ctxtopo, align 8
  ret i64 0
b275:
  %v436 = add i64 0, 286331153
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v437 = and i64 %v0, 1
  %v438 = icmp ne i64 %v437, 0
  br i1 %v438, label %b347, label %b348
b276:
  store i64 0, ptr %gcs1
  %v332 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v332, ptr %gcs1
  %v333 = load i8, ptr %ctx, align 8
  %c76 = zext i8 %v333 to i64
  %v334 = icmp ne i64 %c76, 0
  br i1 %v334, label %b282, label %b281
b277:
  %v331 = icmp eq i64 %v328, %v6
  br i1 %v331, label %b279, label %b276
b278:
  %gcvolta278 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta278, ptr %ctxtopo, align 8
  ret i64 0
b279:
  %v449 = add i64 0, 8
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v450 = and i64 %v0, 1
  %v451 = icmp ne i64 %v450, 0
  br i1 %v451, label %b353, label %b354
b280:
  store i64 0, ptr %gcs1
  %v336 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v336, ptr %gcs1
  %v337 = load i8, ptr %ctx, align 8
  %c77 = zext i8 %v337 to i64
  %v338 = icmp ne i64 %c77, 0
  br i1 %v338, label %b286, label %b285
b281:
  %v335 = icmp eq i64 %v332, %v6
  br i1 %v335, label %b283, label %b280
b282:
  %gcvolta282 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta282, ptr %ctxtopo, align 8
  ret i64 0
b283:
  %v462 = add i64 0, 2184
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v463 = and i64 %v0, 1
  %v464 = icmp ne i64 %v463, 0
  br i1 %v464, label %b359, label %b360
b284:
  store i64 0, ptr %gcs1
  %v340 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v340, ptr %gcs1
  %v341 = load i8, ptr %ctx, align 8
  %c78 = zext i8 %v341 to i64
  %v342 = icmp ne i64 %c78, 0
  br i1 %v342, label %b290, label %b289
b285:
  %v339 = icmp eq i64 %v336, %v6
  br i1 %v339, label %b287, label %b284
b286:
  %gcvolta286 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta286, ptr %ctxtopo, align 8
  ret i64 0
b287:
  %v475 = add i64 0, 143165576
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v476 = and i64 %v0, 1
  %v477 = icmp ne i64 %v476, 0
  br i1 %v477, label %b365, label %b366
b288:
  store i64 0, ptr %gcs1
  %v344 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v344, ptr %gcs1
  %v345 = load i8, ptr %ctx, align 8
  %c79 = zext i8 %v345 to i64
  %v346 = icmp ne i64 %c79, 0
  br i1 %v346, label %b295, label %b294
b289:
  %v343 = icmp eq i64 %v340, %v6
  br i1 %v343, label %b291, label %b288
b290:
  %gcvolta290 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta290, ptr %ctxtopo, align 8
  ret i64 0
b291:
  br label %b251
b293:
  store i64 0, ptr %gcs1
  %v348 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v348, ptr %gcs1
  %v349 = load i8, ptr %ctx, align 8
  %c80 = zext i8 %v349 to i64
  %v350 = icmp ne i64 %c80, 0
  br i1 %v350, label %b300, label %b299
b294:
  %v347 = icmp eq i64 %v344, %v6
  br i1 %v347, label %b296, label %b293
b295:
  %gcvolta295 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta295, ptr %ctxtopo, align 8
  ret i64 0
b296:
  br label %b251
b298:
  br label %b12
b299:
  %v351 = icmp eq i64 %v348, %v6
  br i1 %v351, label %b301, label %b298
b300:
  %gcvolta300 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta300, ptr %ctxtopo, align 8
  ret i64 0
b301:
  br label %b251
b302:
  %v359 = icmp eq i64 %v0, %v354
  br label %b304
b303:
  %v360 = call i64 @df.class(i64 %v0)
  switch i64 %v360, label %b308 [ i64 675, label %b305 i64 4, label %b306 i64 2, label %b307 i64 3, label %b307 ]
b304:
  %v378 = phi i1 [ %v359, %b302 ], [ %v377, %b309 ]
  br i1 %v378, label %b320, label %b321
b305:
  %v361 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v354)
  %v362 = load i8, ptr %ctx, align 8
  %c81 = zext i8 %v362 to i64
  %v363 = icmp ne i64 %c81, 0
  br i1 %v363, label %b311, label %b310
b306:
  %v365 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v354)
  %v366 = load i8, ptr %ctx, align 8
  %c82 = zext i8 %v366 to i64
  %v367 = icmp ne i64 %c82, 0
  br i1 %v367, label %b313, label %b312
b307:
  %v368 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v354)
  %v369 = load i8, ptr %ctx, align 8
  %c83 = zext i8 %v369 to i64
  %v370 = icmp ne i64 %c83, 0
  br i1 %v370, label %b315, label %b314
b308:
  %sa371_0 = getelementptr [2 x i64], ptr %sargs371, i64 0, i64 0
  store i64 %v354, ptr %sa371_0
  %sat371 = getelementptr [2 x i64], ptr %sargs371, i64 0, i64 1
  store i64 0, ptr %sat371
  %sic371 = getelementptr i64, ptr %area, i64 238
  %sf371 = call ptr @df.selector(ptr %sic371, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v371 = call i64 %sf371(i64 %v0, ptr %sargs371, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v371, ptr %gcs0
  %v372 = load i8, ptr %ctx, align 8
  %c84 = zext i8 %v372 to i64
  %v373 = icmp ne i64 %c84, 0
  br i1 %v373, label %b317, label %b316
b309:
  %v377 = phi i1 [ %v364, %b310 ], [ %v365, %b312 ], [ %v368, %b314 ], [ %v374, %b318 ]
  br label %b304
b310:
  %c85 = zext i8 %v361 to i64
  %v364 = icmp ne i64 %c85, 0
  br label %b309
b311:
  %gcvolta311 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta311, ptr %ctxtopo, align 8
  ret i64 0
b312:
  br label %b309
b313:
  %gcvolta313 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta313, ptr %ctxtopo, align 8
  ret i64 0
b314:
  br label %b309
b315:
  %gcvolta315 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta315, ptr %ctxtopo, align 8
  ret i64 0
b316:
  store i64 0, ptr %gcs1
  %v374 = call i1 @df.unbox_bool(i64 %v371)
  %v375 = load i8, ptr %ctx, align 8
  %c86 = zext i8 %v375 to i64
  %v376 = icmp ne i64 %c86, 0
  br i1 %v376, label %b319, label %b318
b317:
  %gcvolta317 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta317, ptr %ctxtopo, align 8
  ret i64 0
b318:
  br label %b309
b319:
  %gcvolta319 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta319, ptr %ctxtopo, align 8
  ret i64 0
b320:
  %v379 = add i64 0, 0
  br label %b322
b321:
  %v380 = add i64 0, 1
  br label %b322
b322:
  %v381 = phi i64 [ %v379, %b320 ], [ %v380, %b321 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v382 = call i64 @df.box_int(i64 %v381)
  %xcp383 = load i8, ptr %ctx, align 8
  %xcn383 = icmp ne i8 %xcp383, 0
  %xce383 = call i1 @llvm.expect.i1(i1 %xcn383, i1 false)
  br i1 %xce383, label %xc383.limpar, label %xc383.fim
xc383.limpar:
  call void @dartforge_exception_clear()
  br label %xc383.fim
xc383.fim:
  %gcvolta322 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta322, ptr %ctxtopo, align 8
  ret i64 %v382
b324:
  %ga392 = getelementptr i64, ptr %area, i64 13
  %v392 = load i8, ptr %ga392
  %c87 = zext i8 %v392 to i64
  %v393 = icmp ne i64 %c87, 0
  br i1 %v393, label %b326, label %b327
b325:
  %v400 = call i64 @df.unbox_int(i64 %v0)
  %v401 = load i8, ptr %ctx, align 8
  %c88 = zext i8 %v401 to i64
  %v402 = icmp ne i64 %c88, 0
  br i1 %v402, label %b332, label %b331
b326:
  %ga394 = getelementptr i64, ptr %area, i64 12
  %v394 = load i64, ptr %ga394
  br label %b328
b327:
  %v395 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b328
b328:
  %v396 = phi i64 [ %v394, %b326 ], [ %v395, %b327 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v396, i64 0, i64 0)
  %v398 = load i8, ptr %ctx, align 8
  %c89 = zext i8 %v398 to i64
  %v399 = icmp ne i64 %c89, 0
  br i1 %v399, label %b330, label %b329
b329:
  br label %b325
b330:
  %gcvolta330 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta330, ptr %ctxtopo, align 8
  ret i64 0
b331:
  %v403 = add i64 0, 1
  %v404 = ashr i64 %v400, %v403
  store i64 0, ptr %gcs0
  %v405 = call i64 @df.box_int(i64 %v404)
  %xcp406 = load i8, ptr %ctx, align 8
  %xcn406 = icmp ne i8 %xcp406, 0
  %xce406 = call i1 @llvm.expect.i1(i1 %xcn406, i1 false)
  br i1 %xce406, label %xc406.limpar, label %xc406.fim
xc406.limpar:
  call void @dartforge_exception_clear()
  br label %xc406.fim
xc406.fim:
  %gcvolta331 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta331, ptr %ctxtopo, align 8
  ret i64 %v405
b332:
  %gcvolta332 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta332, ptr %ctxtopo, align 8
  ret i64 0
b335:
  %v413 = ashr i64 %v0, 1
  %v414 = mul i64 %v413, %v410
  store i64 0, ptr %gcs0
  %v415 = call i64 @df.box_int(i64 %v414)
  br label %b337
b336:
  %v416 = call i64 @df.box_int(i64 %v410)
  store i64 %v416, ptr %gcs1
  %sa417_0 = getelementptr [2 x i64], ptr %sargs417, i64 0, i64 0
  store i64 %v416, ptr %sa417_0
  %sat417 = getelementptr [2 x i64], ptr %sargs417, i64 0, i64 1
  store i64 0, ptr %sat417
  %sic417 = getelementptr i64, ptr %area, i64 240
  %sf417 = call ptr @df.selector(ptr %sic417, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v417 = call i64 %sf417(i64 %v0, ptr %sargs417, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v418 = load i8, ptr %ctx, align 8
  %c90 = zext i8 %v418 to i64
  %v419 = icmp ne i64 %c90, 0
  br i1 %v419, label %b339, label %b338
b337:
  %v420 = phi i64 [ %v415, %b335 ], [ %v417, %b338 ]
  %xcp421 = load i8, ptr %ctx, align 8
  %xcn421 = icmp ne i8 %xcp421, 0
  %xce421 = call i1 @llvm.expect.i1(i1 %xcn421, i1 false)
  br i1 %xce421, label %xc421.limpar, label %xc421.fim
xc421.limpar:
  call void @dartforge_exception_clear()
  br label %xc421.fim
xc421.fim:
  %gcvolta337 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta337, ptr %ctxtopo, align 8
  ret i64 %v420
b338:
  br label %b337
b339:
  %gcvolta339 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta339, ptr %ctxtopo, align 8
  ret i64 0
b341:
  %v426 = ashr i64 %v0, 1
  %v427 = mul i64 %v426, %v423
  store i64 0, ptr %gcs0
  %v428 = call i64 @df.box_int(i64 %v427)
  br label %b343
b342:
  %v429 = call i64 @df.box_int(i64 %v423)
  store i64 %v429, ptr %gcs1
  %sa430_0 = getelementptr [2 x i64], ptr %sargs430, i64 0, i64 0
  store i64 %v429, ptr %sa430_0
  %sat430 = getelementptr [2 x i64], ptr %sargs430, i64 0, i64 1
  store i64 0, ptr %sat430
  %sic430 = getelementptr i64, ptr %area, i64 242
  %sf430 = call ptr @df.selector(ptr %sic430, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v430 = call i64 %sf430(i64 %v0, ptr %sargs430, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v431 = load i8, ptr %ctx, align 8
  %c91 = zext i8 %v431 to i64
  %v432 = icmp ne i64 %c91, 0
  br i1 %v432, label %b345, label %b344
b343:
  %v433 = phi i64 [ %v428, %b341 ], [ %v430, %b344 ]
  %xcp434 = load i8, ptr %ctx, align 8
  %xcn434 = icmp ne i8 %xcp434, 0
  %xce434 = call i1 @llvm.expect.i1(i1 %xcn434, i1 false)
  br i1 %xce434, label %xc434.limpar, label %xc434.fim
xc434.limpar:
  call void @dartforge_exception_clear()
  br label %xc434.fim
xc434.fim:
  %gcvolta343 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta343, ptr %ctxtopo, align 8
  ret i64 %v433
b344:
  br label %b343
b345:
  %gcvolta345 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta345, ptr %ctxtopo, align 8
  ret i64 0
b347:
  %v439 = ashr i64 %v0, 1
  %v440 = mul i64 %v439, %v436
  store i64 0, ptr %gcs0
  %v441 = call i64 @df.box_int(i64 %v440)
  br label %b349
b348:
  %v442 = call i64 @df.box_int(i64 %v436)
  store i64 %v442, ptr %gcs1
  %sa443_0 = getelementptr [2 x i64], ptr %sargs443, i64 0, i64 0
  store i64 %v442, ptr %sa443_0
  %sat443 = getelementptr [2 x i64], ptr %sargs443, i64 0, i64 1
  store i64 0, ptr %sat443
  %sic443 = getelementptr i64, ptr %area, i64 244
  %sf443 = call ptr @df.selector(ptr %sic443, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v443 = call i64 %sf443(i64 %v0, ptr %sargs443, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v444 = load i8, ptr %ctx, align 8
  %c92 = zext i8 %v444 to i64
  %v445 = icmp ne i64 %c92, 0
  br i1 %v445, label %b351, label %b350
b349:
  %v446 = phi i64 [ %v441, %b347 ], [ %v443, %b350 ]
  %xcp447 = load i8, ptr %ctx, align 8
  %xcn447 = icmp ne i8 %xcp447, 0
  %xce447 = call i1 @llvm.expect.i1(i1 %xcn447, i1 false)
  br i1 %xce447, label %xc447.limpar, label %xc447.fim
xc447.limpar:
  call void @dartforge_exception_clear()
  br label %xc447.fim
xc447.fim:
  %gcvolta349 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta349, ptr %ctxtopo, align 8
  ret i64 %v446
b350:
  br label %b349
b351:
  %gcvolta351 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta351, ptr %ctxtopo, align 8
  ret i64 0
b353:
  %v452 = ashr i64 %v0, 1
  %v453 = mul i64 %v452, %v449
  store i64 0, ptr %gcs0
  %v454 = call i64 @df.box_int(i64 %v453)
  br label %b355
b354:
  %v455 = call i64 @df.box_int(i64 %v449)
  store i64 %v455, ptr %gcs1
  %sa456_0 = getelementptr [2 x i64], ptr %sargs456, i64 0, i64 0
  store i64 %v455, ptr %sa456_0
  %sat456 = getelementptr [2 x i64], ptr %sargs456, i64 0, i64 1
  store i64 0, ptr %sat456
  %sic456 = getelementptr i64, ptr %area, i64 246
  %sf456 = call ptr @df.selector(ptr %sic456, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v456 = call i64 %sf456(i64 %v0, ptr %sargs456, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v457 = load i8, ptr %ctx, align 8
  %c93 = zext i8 %v457 to i64
  %v458 = icmp ne i64 %c93, 0
  br i1 %v458, label %b357, label %b356
b355:
  %v459 = phi i64 [ %v454, %b353 ], [ %v456, %b356 ]
  %xcp460 = load i8, ptr %ctx, align 8
  %xcn460 = icmp ne i8 %xcp460, 0
  %xce460 = call i1 @llvm.expect.i1(i1 %xcn460, i1 false)
  br i1 %xce460, label %xc460.limpar, label %xc460.fim
xc460.limpar:
  call void @dartforge_exception_clear()
  br label %xc460.fim
xc460.fim:
  %gcvolta355 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta355, ptr %ctxtopo, align 8
  ret i64 %v459
b356:
  br label %b355
b357:
  %gcvolta357 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta357, ptr %ctxtopo, align 8
  ret i64 0
b359:
  %v465 = ashr i64 %v0, 1
  %v466 = mul i64 %v465, %v462
  store i64 0, ptr %gcs0
  %v467 = call i64 @df.box_int(i64 %v466)
  br label %b361
b360:
  %v468 = call i64 @df.box_int(i64 %v462)
  store i64 %v468, ptr %gcs1
  %sa469_0 = getelementptr [2 x i64], ptr %sargs469, i64 0, i64 0
  store i64 %v468, ptr %sa469_0
  %sat469 = getelementptr [2 x i64], ptr %sargs469, i64 0, i64 1
  store i64 0, ptr %sat469
  %sic469 = getelementptr i64, ptr %area, i64 248
  %sf469 = call ptr @df.selector(ptr %sic469, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v469 = call i64 %sf469(i64 %v0, ptr %sargs469, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v470 = load i8, ptr %ctx, align 8
  %c94 = zext i8 %v470 to i64
  %v471 = icmp ne i64 %c94, 0
  br i1 %v471, label %b363, label %b362
b361:
  %v472 = phi i64 [ %v467, %b359 ], [ %v469, %b362 ]
  %xcp473 = load i8, ptr %ctx, align 8
  %xcn473 = icmp ne i8 %xcp473, 0
  %xce473 = call i1 @llvm.expect.i1(i1 %xcn473, i1 false)
  br i1 %xce473, label %xc473.limpar, label %xc473.fim
xc473.limpar:
  call void @dartforge_exception_clear()
  br label %xc473.fim
xc473.fim:
  %gcvolta361 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta361, ptr %ctxtopo, align 8
  ret i64 %v472
b362:
  br label %b361
b363:
  %gcvolta363 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta363, ptr %ctxtopo, align 8
  ret i64 0
b365:
  %v478 = ashr i64 %v0, 1
  %v479 = mul i64 %v478, %v475
  store i64 0, ptr %gcs0
  %v480 = call i64 @df.box_int(i64 %v479)
  br label %b367
b366:
  %v481 = call i64 @df.box_int(i64 %v475)
  store i64 %v481, ptr %gcs1
  %sa482_0 = getelementptr [2 x i64], ptr %sargs482, i64 0, i64 0
  store i64 %v481, ptr %sa482_0
  %sat482 = getelementptr [2 x i64], ptr %sargs482, i64 0, i64 1
  store i64 0, ptr %sat482
  %sic482 = getelementptr i64, ptr %area, i64 250
  %sf482 = call ptr @df.selector(ptr %sic482, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v482 = call i64 %sf482(i64 %v0, ptr %sargs482, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v483 = load i8, ptr %ctx, align 8
  %c95 = zext i8 %v483 to i64
  %v484 = icmp ne i64 %c95, 0
  br i1 %v484, label %b369, label %b368
b367:
  %v485 = phi i64 [ %v480, %b365 ], [ %v482, %b368 ]
  %xcp486 = load i8, ptr %ctx, align 8
  %xcn486 = icmp ne i8 %xcp486, 0
  %xce486 = call i1 @llvm.expect.i1(i1 %xcn486, i1 false)
  br i1 %xce486, label %xc486.limpar, label %xc486.fim
xc486.limpar:
  call void @dartforge_exception_clear()
  br label %xc486.fim
xc486.fim:
  %gcvolta367 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta367, ptr %ctxtopo, align 8
  ret i64 %v485
b368:
  br label %b367
b369:
  %gcvolta369 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta369, ptr %ctxtopo, align 8
  ret i64 0
b371:
  store i64 0, ptr %gcs1
  %v493 = call double @df.unbox_double(i64 %v490)
  %v494 = load i8, ptr %ctx, align 8
  %c96 = zext i8 %v494 to i64
  %v495 = icmp ne i64 %c96, 0
  br i1 %v495, label %b374, label %b373
b372:
  %gcvolta372 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta372, ptr %ctxtopo, align 8
  ret i64 0
b373:
  store i64 0, ptr %gcs0
  %v496 = call i64 @df.box_double(double %v493)
  %xcp497 = load i8, ptr %ctx, align 8
  %xcn497 = icmp ne i8 %xcp497, 0
  %xce497 = call i1 @llvm.expect.i1(i1 %xcn497, i1 false)
  br i1 %xce497, label %xc497.limpar, label %xc497.fim
xc497.limpar:
  call void @dartforge_exception_clear()
  br label %xc497.fim
xc497.fim:
  %gcvolta373 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta373, ptr %ctxtopo, align 8
  ret i64 %v496
b374:
  %gcvolta374 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta374, ptr %ctxtopo, align 8
  ret i64 0
b388:
  %v703 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v704 = call i64 @df.box_int(i64 %v703)
  store i64 %v704, ptr %gcs1
  %sa705_0 = getelementptr [2 x i64], ptr %sargs705, i64 0, i64 0
  store i64 %v704, ptr %sa705_0
  %sat705 = getelementptr [2 x i64], ptr %sargs705, i64 0, i64 1
  store i64 0, ptr %sat705
  %sic705 = getelementptr i64, ptr %area, i64 252
  %sf705 = call ptr @df.selector(ptr %sic705, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v705 = call i64 %sf705(i64 %v0, ptr %sargs705, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v705, ptr %gcs0
  %v706 = load i8, ptr %ctx, align 8
  %c97 = zext i8 %v706 to i64
  %v707 = icmp ne i64 %c97, 0
  br i1 %v707, label %b517, label %b516
b389:
  store i64 0, ptr %gcs1
  %v503 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v503, ptr %gcs1
  %v504 = load i8, ptr %ctx, align 8
  %c98 = zext i8 %v504 to i64
  %v505 = icmp ne i64 %c98, 0
  br i1 %v505, label %b395, label %b394
b390:
  %v502 = icmp eq i64 %v499, %v6
  br i1 %v502, label %b392, label %b389
b391:
  %gcvolta391 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta391, ptr %ctxtopo, align 8
  ret i64 0
b392:
  %v548 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v549 = call i64 @df.box_int(i64 %v548)
  store i64 %v549, ptr %gcs1
  %v550 = icmp eq i64 %v0, 0
  %v551 = icmp eq i64 %v549, 0
  %c99 = zext i1 %v550 to i64
  %c100 = zext i1 %v551 to i64
  %v552 = or i64 %c99, %c100
  %v553 = icmp ne i64 %v552, 0
  br i1 %v553, label %b439, label %b440
b393:
  store i64 0, ptr %gcs1
  %v507 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v507, ptr %gcs1
  %v508 = load i8, ptr %ctx, align 8
  %c101 = zext i8 %v508 to i64
  %v509 = icmp ne i64 %c101, 0
  br i1 %v509, label %b399, label %b398
b394:
  %v506 = icmp eq i64 %v503, %v6
  br i1 %v506, label %b396, label %b393
b395:
  %gcvolta395 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta395, ptr %ctxtopo, align 8
  ret i64 0
b396:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v580 = icmp eq i64 %v0, 0
  %v581 = call i64 @df.class(i64 %v0)
  %v582 = call i8 @df.subclass(i64 %v581, i64 674)
  %c102 = zext i8 %v582 to i64
  %v583 = icmp ne i64 %c102, 0
  %v584 = xor i1 %v580, true
  %c103 = zext i1 %v583 to i64
  %c104 = zext i1 %v584 to i64
  %v585 = and i64 %c103, %c104
  %v586 = icmp ne i64 %v585, 0
  br i1 %v586, label %b462, label %b461
b397:
  store i64 0, ptr %gcs1
  %v511 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v511, ptr %gcs1
  %v512 = load i8, ptr %ctx, align 8
  %c105 = zext i8 %v512 to i64
  %v513 = icmp ne i64 %c105, 0
  br i1 %v513, label %b403, label %b402
b398:
  %v510 = icmp eq i64 %v507, %v6
  br i1 %v510, label %b400, label %b397
b399:
  %gcvolta399 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta399, ptr %ctxtopo, align 8
  ret i64 0
b400:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v603 = icmp eq i64 %v0, 0
  %v604 = call i64 @df.class(i64 %v0)
  %v605 = call i8 @df.subclass(i64 %v604, i64 674)
  %c106 = zext i8 %v605 to i64
  %v606 = icmp ne i64 %c106, 0
  %v607 = xor i1 %v603, true
  %c107 = zext i1 %v606 to i64
  %c108 = zext i1 %v607 to i64
  %v608 = and i64 %c107, %c108
  %v609 = icmp ne i64 %v608, 0
  br i1 %v609, label %b472, label %b471
b401:
  store i64 0, ptr %gcs1
  %v515 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v515, ptr %gcs1
  %v516 = load i8, ptr %ctx, align 8
  %c109 = zext i8 %v516 to i64
  %v517 = icmp ne i64 %c109, 0
  br i1 %v517, label %b407, label %b406
b402:
  %v514 = icmp eq i64 %v511, %v6
  br i1 %v514, label %b404, label %b401
b403:
  %gcvolta403 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta403, ptr %ctxtopo, align 8
  ret i64 0
b404:
  %xcp626 = load i8, ptr %ctx, align 8
  %xcn626 = icmp ne i8 %xcp626, 0
  %xce626 = call i1 @llvm.expect.i1(i1 %xcn626, i1 false)
  br i1 %xce626, label %xc626.limpar, label %xc626.fim
xc626.limpar:
  call void @dartforge_exception_clear()
  br label %xc626.fim
xc626.fim:
  %gcvolta404 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta404, ptr %ctxtopo, align 8
  ret i64 %v0
b405:
  store i64 0, ptr %gcs1
  %v519 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v519, ptr %gcs1
  %v520 = load i8, ptr %ctx, align 8
  %c110 = zext i8 %v520 to i64
  %v521 = icmp ne i64 %c110, 0
  br i1 %v521, label %b411, label %b410
b406:
  %v518 = icmp eq i64 %v515, %v6
  br i1 %v518, label %b408, label %b405
b407:
  %gcvolta407 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta407, ptr %ctxtopo, align 8
  ret i64 0
b408:
  %v628 = add i64 0, 257
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v629 = and i64 %v0, 1
  %v630 = icmp ne i64 %v629, 0
  br i1 %v630, label %b482, label %b483
b409:
  store i64 0, ptr %gcs1
  %v523 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v523, ptr %gcs1
  %v524 = load i8, ptr %ctx, align 8
  %c111 = zext i8 %v524 to i64
  %v525 = icmp ne i64 %c111, 0
  br i1 %v525, label %b415, label %b414
b410:
  %v522 = icmp eq i64 %v519, %v6
  br i1 %v522, label %b412, label %b409
b411:
  %gcvolta411 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta411, ptr %ctxtopo, align 8
  ret i64 0
b412:
  %v641 = add i64 0, 16843009
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v642 = and i64 %v0, 1
  %v643 = icmp ne i64 %v642, 0
  br i1 %v643, label %b488, label %b489
b413:
  store i64 0, ptr %gcs1
  %v527 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v527, ptr %gcs1
  %v528 = load i8, ptr %ctx, align 8
  %c112 = zext i8 %v528 to i64
  %v529 = icmp ne i64 %c112, 0
  br i1 %v529, label %b419, label %b418
b414:
  %v526 = icmp eq i64 %v523, %v6
  br i1 %v526, label %b416, label %b413
b415:
  %gcvolta415 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta415, ptr %ctxtopo, align 8
  ret i64 0
b416:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v654 = icmp eq i64 %v0, 0
  %v655 = call i64 @df.class(i64 %v0)
  %v656 = call i8 @df.subclass(i64 %v655, i64 674)
  %c113 = zext i8 %v656 to i64
  %v657 = icmp ne i64 %c113, 0
  %v658 = xor i1 %v654, true
  %c114 = zext i1 %v657 to i64
  %c115 = zext i1 %v658 to i64
  %v659 = and i64 %c114, %c115
  %v660 = icmp ne i64 %v659, 0
  br i1 %v660, label %b495, label %b494
b417:
  store i64 0, ptr %gcs1
  %v531 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v531, ptr %gcs1
  %v532 = load i8, ptr %ctx, align 8
  %c116 = zext i8 %v532 to i64
  %v533 = icmp ne i64 %c116, 0
  br i1 %v533, label %b423, label %b422
b418:
  %v530 = icmp eq i64 %v527, %v6
  br i1 %v530, label %b420, label %b417
b419:
  %gcvolta419 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta419, ptr %ctxtopo, align 8
  ret i64 0
b420:
  %v677 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v678 = and i64 %v0, 1
  %v679 = icmp ne i64 %v678, 0
  br i1 %v679, label %b504, label %b505
b421:
  store i64 0, ptr %gcs1
  %v535 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v535, ptr %gcs1
  %v536 = load i8, ptr %ctx, align 8
  %c117 = zext i8 %v536 to i64
  %v537 = icmp ne i64 %c117, 0
  br i1 %v537, label %b427, label %b426
b422:
  %v534 = icmp eq i64 %v531, %v6
  br i1 %v534, label %b424, label %b421
b423:
  %gcvolta423 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta423, ptr %ctxtopo, align 8
  ret i64 0
b424:
  %v690 = add i64 0, 8421504
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v691 = and i64 %v0, 1
  %v692 = icmp ne i64 %v691, 0
  br i1 %v692, label %b510, label %b511
b425:
  store i64 0, ptr %gcs1
  %v539 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v539, ptr %gcs1
  %v540 = load i8, ptr %ctx, align 8
  %c118 = zext i8 %v540 to i64
  %v541 = icmp ne i64 %c118, 0
  br i1 %v541, label %b432, label %b431
b426:
  %v538 = icmp eq i64 %v535, %v6
  br i1 %v538, label %b428, label %b425
b427:
  %gcvolta427 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta427, ptr %ctxtopo, align 8
  ret i64 0
b428:
  br label %b388
b430:
  store i64 0, ptr %gcs1
  %v543 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v543, ptr %gcs1
  %v544 = load i8, ptr %ctx, align 8
  %c119 = zext i8 %v544 to i64
  %v545 = icmp ne i64 %c119, 0
  br i1 %v545, label %b437, label %b436
b431:
  %v542 = icmp eq i64 %v539, %v6
  br i1 %v542, label %b433, label %b430
b432:
  %gcvolta432 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta432, ptr %ctxtopo, align 8
  ret i64 0
b433:
  br label %b388
b435:
  br label %b12
b436:
  %v546 = icmp eq i64 %v543, %v6
  br i1 %v546, label %b438, label %b435
b437:
  %gcvolta437 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta437, ptr %ctxtopo, align 8
  ret i64 0
b438:
  br label %b388
b439:
  %v554 = icmp eq i64 %v0, %v549
  br label %b441
b440:
  %v555 = call i64 @df.class(i64 %v0)
  switch i64 %v555, label %b445 [ i64 675, label %b442 i64 4, label %b443 i64 2, label %b444 i64 3, label %b444 ]
b441:
  %v573 = phi i1 [ %v554, %b439 ], [ %v572, %b446 ]
  br i1 %v573, label %b457, label %b458
b442:
  %v556 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v549)
  %v557 = load i8, ptr %ctx, align 8
  %c120 = zext i8 %v557 to i64
  %v558 = icmp ne i64 %c120, 0
  br i1 %v558, label %b448, label %b447
b443:
  %v560 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v549)
  %v561 = load i8, ptr %ctx, align 8
  %c121 = zext i8 %v561 to i64
  %v562 = icmp ne i64 %c121, 0
  br i1 %v562, label %b450, label %b449
b444:
  %v563 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v549)
  %v564 = load i8, ptr %ctx, align 8
  %c122 = zext i8 %v564 to i64
  %v565 = icmp ne i64 %c122, 0
  br i1 %v565, label %b452, label %b451
b445:
  %sa566_0 = getelementptr [2 x i64], ptr %sargs566, i64 0, i64 0
  store i64 %v549, ptr %sa566_0
  %sat566 = getelementptr [2 x i64], ptr %sargs566, i64 0, i64 1
  store i64 0, ptr %sat566
  %sic566 = getelementptr i64, ptr %area, i64 254
  %sf566 = call ptr @df.selector(ptr %sic566, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v566 = call i64 %sf566(i64 %v0, ptr %sargs566, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v566, ptr %gcs0
  %v567 = load i8, ptr %ctx, align 8
  %c123 = zext i8 %v567 to i64
  %v568 = icmp ne i64 %c123, 0
  br i1 %v568, label %b454, label %b453
b446:
  %v572 = phi i1 [ %v559, %b447 ], [ %v560, %b449 ], [ %v563, %b451 ], [ %v569, %b455 ]
  br label %b441
b447:
  %c124 = zext i8 %v556 to i64
  %v559 = icmp ne i64 %c124, 0
  br label %b446
b448:
  %gcvolta448 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta448, ptr %ctxtopo, align 8
  ret i64 0
b449:
  br label %b446
b450:
  %gcvolta450 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta450, ptr %ctxtopo, align 8
  ret i64 0
b451:
  br label %b446
b452:
  %gcvolta452 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta452, ptr %ctxtopo, align 8
  ret i64 0
b453:
  store i64 0, ptr %gcs1
  %v569 = call i1 @df.unbox_bool(i64 %v566)
  %v570 = load i8, ptr %ctx, align 8
  %c125 = zext i8 %v570 to i64
  %v571 = icmp ne i64 %c125, 0
  br i1 %v571, label %b456, label %b455
b454:
  %gcvolta454 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta454, ptr %ctxtopo, align 8
  ret i64 0
b455:
  br label %b446
b456:
  %gcvolta456 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta456, ptr %ctxtopo, align 8
  ret i64 0
b457:
  %v574 = add i64 0, 0
  br label %b459
b458:
  %v575 = add i64 0, 1
  br label %b459
b459:
  %v576 = phi i64 [ %v574, %b457 ], [ %v575, %b458 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v577 = call i64 @df.box_int(i64 %v576)
  %xcp578 = load i8, ptr %ctx, align 8
  %xcn578 = icmp ne i8 %xcp578, 0
  %xce578 = call i1 @llvm.expect.i1(i1 %xcn578, i1 false)
  br i1 %xce578, label %xc578.limpar, label %xc578.fim
xc578.limpar:
  call void @dartforge_exception_clear()
  br label %xc578.fim
xc578.fim:
  %gcvolta459 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta459, ptr %ctxtopo, align 8
  ret i64 %v577
b461:
  %ga587 = getelementptr i64, ptr %area, i64 13
  %v587 = load i8, ptr %ga587
  %c126 = zext i8 %v587 to i64
  %v588 = icmp ne i64 %c126, 0
  br i1 %v588, label %b463, label %b464
b462:
  %v595 = call i64 @df.unbox_int(i64 %v0)
  %v596 = load i8, ptr %ctx, align 8
  %c127 = zext i8 %v596 to i64
  %v597 = icmp ne i64 %c127, 0
  br i1 %v597, label %b469, label %b468
b463:
  %ga589 = getelementptr i64, ptr %area, i64 12
  %v589 = load i64, ptr %ga589
  br label %b465
b464:
  %v590 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b465
b465:
  %v591 = phi i64 [ %v589, %b463 ], [ %v590, %b464 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v591, i64 0, i64 0)
  %v593 = load i8, ptr %ctx, align 8
  %c128 = zext i8 %v593 to i64
  %v594 = icmp ne i64 %c128, 0
  br i1 %v594, label %b467, label %b466
b466:
  br label %b462
b467:
  %gcvolta467 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta467, ptr %ctxtopo, align 8
  ret i64 0
b468:
  %v598 = add i64 0, 6
  %v599 = ashr i64 %v595, %v598
  store i64 0, ptr %gcs0
  %v600 = call i64 @df.box_int(i64 %v599)
  %xcp601 = load i8, ptr %ctx, align 8
  %xcn601 = icmp ne i8 %xcp601, 0
  %xce601 = call i1 @llvm.expect.i1(i1 %xcn601, i1 false)
  br i1 %xce601, label %xc601.limpar, label %xc601.fim
xc601.limpar:
  call void @dartforge_exception_clear()
  br label %xc601.fim
xc601.fim:
  %gcvolta468 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta468, ptr %ctxtopo, align 8
  ret i64 %v600
b469:
  %gcvolta469 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta469, ptr %ctxtopo, align 8
  ret i64 0
b471:
  %ga610 = getelementptr i64, ptr %area, i64 13
  %v610 = load i8, ptr %ga610
  %c129 = zext i8 %v610 to i64
  %v611 = icmp ne i64 %c129, 0
  br i1 %v611, label %b473, label %b474
b472:
  %v618 = call i64 @df.unbox_int(i64 %v0)
  %v619 = load i8, ptr %ctx, align 8
  %c130 = zext i8 %v619 to i64
  %v620 = icmp ne i64 %c130, 0
  br i1 %v620, label %b479, label %b478
b473:
  %ga612 = getelementptr i64, ptr %area, i64 12
  %v612 = load i64, ptr %ga612
  br label %b475
b474:
  %v613 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b475
b475:
  %v614 = phi i64 [ %v612, %b473 ], [ %v613, %b474 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v614, i64 0, i64 0)
  %v616 = load i8, ptr %ctx, align 8
  %c131 = zext i8 %v616 to i64
  %v617 = icmp ne i64 %c131, 0
  br i1 %v617, label %b477, label %b476
b476:
  br label %b472
b477:
  %gcvolta477 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta477, ptr %ctxtopo, align 8
  ret i64 0
b478:
  %v621 = add i64 0, 4
  %v622 = ashr i64 %v618, %v621
  store i64 0, ptr %gcs0
  %v623 = call i64 @df.box_int(i64 %v622)
  %xcp624 = load i8, ptr %ctx, align 8
  %xcn624 = icmp ne i8 %xcp624, 0
  %xce624 = call i1 @llvm.expect.i1(i1 %xcn624, i1 false)
  br i1 %xce624, label %xc624.limpar, label %xc624.fim
xc624.limpar:
  call void @dartforge_exception_clear()
  br label %xc624.fim
xc624.fim:
  %gcvolta478 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta478, ptr %ctxtopo, align 8
  ret i64 %v623
b479:
  %gcvolta479 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta479, ptr %ctxtopo, align 8
  ret i64 0
b482:
  %v631 = ashr i64 %v0, 1
  %v632 = mul i64 %v631, %v628
  store i64 0, ptr %gcs0
  %v633 = call i64 @df.box_int(i64 %v632)
  br label %b484
b483:
  %v634 = call i64 @df.box_int(i64 %v628)
  store i64 %v634, ptr %gcs1
  %sa635_0 = getelementptr [2 x i64], ptr %sargs635, i64 0, i64 0
  store i64 %v634, ptr %sa635_0
  %sat635 = getelementptr [2 x i64], ptr %sargs635, i64 0, i64 1
  store i64 0, ptr %sat635
  %sic635 = getelementptr i64, ptr %area, i64 256
  %sf635 = call ptr @df.selector(ptr %sic635, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v635 = call i64 %sf635(i64 %v0, ptr %sargs635, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v636 = load i8, ptr %ctx, align 8
  %c132 = zext i8 %v636 to i64
  %v637 = icmp ne i64 %c132, 0
  br i1 %v637, label %b486, label %b485
b484:
  %v638 = phi i64 [ %v633, %b482 ], [ %v635, %b485 ]
  %xcp639 = load i8, ptr %ctx, align 8
  %xcn639 = icmp ne i8 %xcp639, 0
  %xce639 = call i1 @llvm.expect.i1(i1 %xcn639, i1 false)
  br i1 %xce639, label %xc639.limpar, label %xc639.fim
xc639.limpar:
  call void @dartforge_exception_clear()
  br label %xc639.fim
xc639.fim:
  %gcvolta484 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta484, ptr %ctxtopo, align 8
  ret i64 %v638
b485:
  br label %b484
b486:
  %gcvolta486 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta486, ptr %ctxtopo, align 8
  ret i64 0
b488:
  %v644 = ashr i64 %v0, 1
  %v645 = mul i64 %v644, %v641
  store i64 0, ptr %gcs0
  %v646 = call i64 @df.box_int(i64 %v645)
  br label %b490
b489:
  %v647 = call i64 @df.box_int(i64 %v641)
  store i64 %v647, ptr %gcs1
  %sa648_0 = getelementptr [2 x i64], ptr %sargs648, i64 0, i64 0
  store i64 %v647, ptr %sa648_0
  %sat648 = getelementptr [2 x i64], ptr %sargs648, i64 0, i64 1
  store i64 0, ptr %sat648
  %sic648 = getelementptr i64, ptr %area, i64 258
  %sf648 = call ptr @df.selector(ptr %sic648, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v648 = call i64 %sf648(i64 %v0, ptr %sargs648, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v649 = load i8, ptr %ctx, align 8
  %c133 = zext i8 %v649 to i64
  %v650 = icmp ne i64 %c133, 0
  br i1 %v650, label %b492, label %b491
b490:
  %v651 = phi i64 [ %v646, %b488 ], [ %v648, %b491 ]
  %xcp652 = load i8, ptr %ctx, align 8
  %xcn652 = icmp ne i8 %xcp652, 0
  %xce652 = call i1 @llvm.expect.i1(i1 %xcn652, i1 false)
  br i1 %xce652, label %xc652.limpar, label %xc652.fim
xc652.limpar:
  call void @dartforge_exception_clear()
  br label %xc652.fim
xc652.fim:
  %gcvolta490 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta490, ptr %ctxtopo, align 8
  ret i64 %v651
b491:
  br label %b490
b492:
  %gcvolta492 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta492, ptr %ctxtopo, align 8
  ret i64 0
b494:
  %ga661 = getelementptr i64, ptr %area, i64 13
  %v661 = load i8, ptr %ga661
  %c134 = zext i8 %v661 to i64
  %v662 = icmp ne i64 %c134, 0
  br i1 %v662, label %b496, label %b497
b495:
  %v669 = call i64 @df.unbox_int(i64 %v0)
  %v670 = load i8, ptr %ctx, align 8
  %c135 = zext i8 %v670 to i64
  %v671 = icmp ne i64 %c135, 0
  br i1 %v671, label %b502, label %b501
b496:
  %ga663 = getelementptr i64, ptr %area, i64 12
  %v663 = load i64, ptr %ga663
  br label %b498
b497:
  %v664 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b498
b498:
  %v665 = phi i64 [ %v663, %b496 ], [ %v664, %b497 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v665, i64 0, i64 0)
  %v667 = load i8, ptr %ctx, align 8
  %c136 = zext i8 %v667 to i64
  %v668 = icmp ne i64 %c136, 0
  br i1 %v668, label %b500, label %b499
b499:
  br label %b495
b500:
  %gcvolta500 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta500, ptr %ctxtopo, align 8
  ret i64 0
b501:
  %v672 = add i64 0, 1
  %v673 = ashr i64 %v669, %v672
  store i64 0, ptr %gcs0
  %v674 = call i64 @df.box_int(i64 %v673)
  %xcp675 = load i8, ptr %ctx, align 8
  %xcn675 = icmp ne i8 %xcp675, 0
  %xce675 = call i1 @llvm.expect.i1(i1 %xcn675, i1 false)
  br i1 %xce675, label %xc675.limpar, label %xc675.fim
xc675.limpar:
  call void @dartforge_exception_clear()
  br label %xc675.fim
xc675.fim:
  %gcvolta501 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta501, ptr %ctxtopo, align 8
  ret i64 %v674
b502:
  %gcvolta502 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta502, ptr %ctxtopo, align 8
  ret i64 0
b504:
  %v680 = ashr i64 %v0, 1
  %v681 = mul i64 %v680, %v677
  store i64 0, ptr %gcs0
  %v682 = call i64 @df.box_int(i64 %v681)
  br label %b506
b505:
  %v683 = call i64 @df.box_int(i64 %v677)
  store i64 %v683, ptr %gcs1
  %sa684_0 = getelementptr [2 x i64], ptr %sargs684, i64 0, i64 0
  store i64 %v683, ptr %sa684_0
  %sat684 = getelementptr [2 x i64], ptr %sargs684, i64 0, i64 1
  store i64 0, ptr %sat684
  %sic684 = getelementptr i64, ptr %area, i64 260
  %sf684 = call ptr @df.selector(ptr %sic684, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v684 = call i64 %sf684(i64 %v0, ptr %sargs684, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v685 = load i8, ptr %ctx, align 8
  %c137 = zext i8 %v685 to i64
  %v686 = icmp ne i64 %c137, 0
  br i1 %v686, label %b508, label %b507
b506:
  %v687 = phi i64 [ %v682, %b504 ], [ %v684, %b507 ]
  %xcp688 = load i8, ptr %ctx, align 8
  %xcn688 = icmp ne i8 %xcp688, 0
  %xce688 = call i1 @llvm.expect.i1(i1 %xcn688, i1 false)
  br i1 %xce688, label %xc688.limpar, label %xc688.fim
xc688.limpar:
  call void @dartforge_exception_clear()
  br label %xc688.fim
xc688.fim:
  %gcvolta506 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta506, ptr %ctxtopo, align 8
  ret i64 %v687
b507:
  br label %b506
b508:
  %gcvolta508 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta508, ptr %ctxtopo, align 8
  ret i64 0
b510:
  %v693 = ashr i64 %v0, 1
  %v694 = mul i64 %v693, %v690
  store i64 0, ptr %gcs0
  %v695 = call i64 @df.box_int(i64 %v694)
  br label %b512
b511:
  %v696 = call i64 @df.box_int(i64 %v690)
  store i64 %v696, ptr %gcs1
  %sa697_0 = getelementptr [2 x i64], ptr %sargs697, i64 0, i64 0
  store i64 %v696, ptr %sa697_0
  %sat697 = getelementptr [2 x i64], ptr %sargs697, i64 0, i64 1
  store i64 0, ptr %sat697
  %sic697 = getelementptr i64, ptr %area, i64 262
  %sf697 = call ptr @df.selector(ptr %sic697, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v697 = call i64 %sf697(i64 %v0, ptr %sargs697, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v698 = load i8, ptr %ctx, align 8
  %c138 = zext i8 %v698 to i64
  %v699 = icmp ne i64 %c138, 0
  br i1 %v699, label %b514, label %b513
b512:
  %v700 = phi i64 [ %v695, %b510 ], [ %v697, %b513 ]
  %xcp701 = load i8, ptr %ctx, align 8
  %xcn701 = icmp ne i8 %xcp701, 0
  %xce701 = call i1 @llvm.expect.i1(i1 %xcn701, i1 false)
  br i1 %xce701, label %xc701.limpar, label %xc701.fim
xc701.limpar:
  call void @dartforge_exception_clear()
  br label %xc701.fim
xc701.fim:
  %gcvolta512 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta512, ptr %ctxtopo, align 8
  ret i64 %v700
b513:
  br label %b512
b514:
  %gcvolta514 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta514, ptr %ctxtopo, align 8
  ret i64 0
b516:
  store i64 0, ptr %gcs1
  %v708 = call double @df.unbox_double(i64 %v705)
  %v709 = load i8, ptr %ctx, align 8
  %c139 = zext i8 %v709 to i64
  %v710 = icmp ne i64 %c139, 0
  br i1 %v710, label %b519, label %b518
b517:
  %gcvolta517 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta517, ptr %ctxtopo, align 8
  ret i64 0
b518:
  store i64 0, ptr %gcs0
  %v711 = call i64 @df.box_double(double %v708)
  %xcp712 = load i8, ptr %ctx, align 8
  %xcn712 = icmp ne i8 %xcp712, 0
  %xce712 = call i1 @llvm.expect.i1(i1 %xcn712, i1 false)
  br i1 %xce712, label %xc712.limpar, label %xc712.fim
xc712.limpar:
  call void @dartforge_exception_clear()
  br label %xc712.fim
xc712.fim:
  %gcvolta518 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta518, ptr %ctxtopo, align 8
  ret i64 %v711
b519:
  %gcvolta519 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta519, ptr %ctxtopo, align 8
  ret i64 0
b533:
  %v948 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v949 = call i64 @df.box_int(i64 %v948)
  store i64 %v949, ptr %gcs1
  %sa950_0 = getelementptr [2 x i64], ptr %sargs950, i64 0, i64 0
  store i64 %v949, ptr %sa950_0
  %sat950 = getelementptr [2 x i64], ptr %sargs950, i64 0, i64 1
  store i64 0, ptr %sat950
  %sic950 = getelementptr i64, ptr %area, i64 264
  %sf950 = call ptr @df.selector(ptr %sic950, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v950 = call i64 %sf950(i64 %v0, ptr %sargs950, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v950, ptr %gcs0
  %v951 = load i8, ptr %ctx, align 8
  %c140 = zext i8 %v951 to i64
  %v952 = icmp ne i64 %c140, 0
  br i1 %v952, label %b674, label %b673
b534:
  store i64 0, ptr %gcs1
  %v718 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v718, ptr %gcs1
  %v719 = load i8, ptr %ctx, align 8
  %c141 = zext i8 %v719 to i64
  %v720 = icmp ne i64 %c141, 0
  br i1 %v720, label %b540, label %b539
b535:
  %v717 = icmp eq i64 %v714, %v6
  br i1 %v717, label %b537, label %b534
b536:
  %gcvolta536 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta536, ptr %ctxtopo, align 8
  ret i64 0
b537:
  %v763 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v764 = call i64 @df.box_int(i64 %v763)
  store i64 %v764, ptr %gcs1
  %v765 = icmp eq i64 %v0, 0
  %v766 = icmp eq i64 %v764, 0
  %c142 = zext i1 %v765 to i64
  %c143 = zext i1 %v766 to i64
  %v767 = or i64 %c142, %c143
  %v768 = icmp ne i64 %v767, 0
  br i1 %v768, label %b584, label %b585
b538:
  store i64 0, ptr %gcs1
  %v722 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v722, ptr %gcs1
  %v723 = load i8, ptr %ctx, align 8
  %c144 = zext i8 %v723 to i64
  %v724 = icmp ne i64 %c144, 0
  br i1 %v724, label %b544, label %b543
b539:
  %v721 = icmp eq i64 %v718, %v6
  br i1 %v721, label %b541, label %b538
b540:
  %gcvolta540 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta540, ptr %ctxtopo, align 8
  ret i64 0
b541:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v795 = icmp eq i64 %v0, 0
  %v796 = call i64 @df.class(i64 %v0)
  %v797 = call i8 @df.subclass(i64 %v796, i64 674)
  %c145 = zext i8 %v797 to i64
  %v798 = icmp ne i64 %c145, 0
  %v799 = xor i1 %v795, true
  %c146 = zext i1 %v798 to i64
  %c147 = zext i1 %v799 to i64
  %v800 = and i64 %c146, %c147
  %v801 = icmp ne i64 %v800, 0
  br i1 %v801, label %b607, label %b606
b542:
  store i64 0, ptr %gcs1
  %v726 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v726, ptr %gcs1
  %v727 = load i8, ptr %ctx, align 8
  %c148 = zext i8 %v727 to i64
  %v728 = icmp ne i64 %c148, 0
  br i1 %v728, label %b548, label %b547
b543:
  %v725 = icmp eq i64 %v722, %v6
  br i1 %v725, label %b545, label %b542
b544:
  %gcvolta544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta544, ptr %ctxtopo, align 8
  ret i64 0
b545:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v818 = icmp eq i64 %v0, 0
  %v819 = call i64 @df.class(i64 %v0)
  %v820 = call i8 @df.subclass(i64 %v819, i64 674)
  %c149 = zext i8 %v820 to i64
  %v821 = icmp ne i64 %c149, 0
  %v822 = xor i1 %v818, true
  %c150 = zext i1 %v821 to i64
  %c151 = zext i1 %v822 to i64
  %v823 = and i64 %c150, %c151
  %v824 = icmp ne i64 %v823, 0
  br i1 %v824, label %b617, label %b616
b546:
  store i64 0, ptr %gcs1
  %v730 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v730, ptr %gcs1
  %v731 = load i8, ptr %ctx, align 8
  %c152 = zext i8 %v731 to i64
  %v732 = icmp ne i64 %c152, 0
  br i1 %v732, label %b552, label %b551
b547:
  %v729 = icmp eq i64 %v726, %v6
  br i1 %v729, label %b549, label %b546
b548:
  %gcvolta548 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta548, ptr %ctxtopo, align 8
  ret i64 0
b549:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v841 = icmp eq i64 %v0, 0
  %v842 = call i64 @df.class(i64 %v0)
  %v843 = call i8 @df.subclass(i64 %v842, i64 674)
  %c153 = zext i8 %v843 to i64
  %v844 = icmp ne i64 %c153, 0
  %v845 = xor i1 %v841, true
  %c154 = zext i1 %v844 to i64
  %c155 = zext i1 %v845 to i64
  %v846 = and i64 %c154, %c155
  %v847 = icmp ne i64 %v846, 0
  br i1 %v847, label %b627, label %b626
b550:
  store i64 0, ptr %gcs1
  %v734 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v734, ptr %gcs1
  %v735 = load i8, ptr %ctx, align 8
  %c156 = zext i8 %v735 to i64
  %v736 = icmp ne i64 %c156, 0
  br i1 %v736, label %b556, label %b555
b551:
  %v733 = icmp eq i64 %v730, %v6
  br i1 %v733, label %b553, label %b550
b552:
  %gcvolta552 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta552, ptr %ctxtopo, align 8
  ret i64 0
b553:
  %xcp864 = load i8, ptr %ctx, align 8
  %xcn864 = icmp ne i8 %xcp864, 0
  %xce864 = call i1 @llvm.expect.i1(i1 %xcn864, i1 false)
  br i1 %xce864, label %xc864.limpar, label %xc864.fim
xc864.limpar:
  call void @dartforge_exception_clear()
  br label %xc864.fim
xc864.fim:
  %gcvolta553 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta553, ptr %ctxtopo, align 8
  ret i64 %v0
b554:
  store i64 0, ptr %gcs1
  %v738 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v738, ptr %gcs1
  %v739 = load i8, ptr %ctx, align 8
  %c157 = zext i8 %v739 to i64
  %v740 = icmp ne i64 %c157, 0
  br i1 %v740, label %b560, label %b559
b555:
  %v737 = icmp eq i64 %v734, %v6
  br i1 %v737, label %b557, label %b554
b556:
  %gcvolta556 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta556, ptr %ctxtopo, align 8
  ret i64 0
b557:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v866 = icmp eq i64 %v0, 0
  %v867 = call i64 @df.class(i64 %v0)
  %v868 = call i8 @df.subclass(i64 %v867, i64 674)
  %c158 = zext i8 %v868 to i64
  %v869 = icmp ne i64 %c158, 0
  %v870 = xor i1 %v866, true
  %c159 = zext i1 %v869 to i64
  %c160 = zext i1 %v870 to i64
  %v871 = and i64 %c159, %c160
  %v872 = icmp ne i64 %v871, 0
  br i1 %v872, label %b638, label %b637
b558:
  store i64 0, ptr %gcs1
  %v742 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v742, ptr %gcs1
  %v743 = load i8, ptr %ctx, align 8
  %c161 = zext i8 %v743 to i64
  %v744 = icmp ne i64 %c161, 0
  br i1 %v744, label %b564, label %b563
b559:
  %v741 = icmp eq i64 %v738, %v6
  br i1 %v741, label %b561, label %b558
b560:
  %gcvolta560 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta560, ptr %ctxtopo, align 8
  ret i64 0
b561:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v889 = icmp eq i64 %v0, 0
  %v890 = call i64 @df.class(i64 %v0)
  %v891 = call i8 @df.subclass(i64 %v890, i64 674)
  %c162 = zext i8 %v891 to i64
  %v892 = icmp ne i64 %c162, 0
  %v893 = xor i1 %v889, true
  %c163 = zext i1 %v892 to i64
  %c164 = zext i1 %v893 to i64
  %v894 = and i64 %c163, %c164
  %v895 = icmp ne i64 %v894, 0
  br i1 %v895, label %b648, label %b647
b562:
  store i64 0, ptr %gcs1
  %v746 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v746, ptr %gcs1
  %v747 = load i8, ptr %ctx, align 8
  %c165 = zext i8 %v747 to i64
  %v748 = icmp ne i64 %c165, 0
  br i1 %v748, label %b568, label %b567
b563:
  %v745 = icmp eq i64 %v742, %v6
  br i1 %v745, label %b565, label %b562
b564:
  %gcvolta564 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta564, ptr %ctxtopo, align 8
  ret i64 0
b565:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v912 = icmp eq i64 %v0, 0
  %v913 = call i64 @df.class(i64 %v0)
  %v914 = call i8 @df.subclass(i64 %v913, i64 674)
  %c166 = zext i8 %v914 to i64
  %v915 = icmp ne i64 %c166, 0
  %v916 = xor i1 %v912, true
  %c167 = zext i1 %v915 to i64
  %c168 = zext i1 %v916 to i64
  %v917 = and i64 %c167, %c168
  %v918 = icmp ne i64 %v917, 0
  br i1 %v918, label %b658, label %b657
b566:
  store i64 0, ptr %gcs1
  %v750 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v750, ptr %gcs1
  %v751 = load i8, ptr %ctx, align 8
  %c169 = zext i8 %v751 to i64
  %v752 = icmp ne i64 %c169, 0
  br i1 %v752, label %b572, label %b571
b567:
  %v749 = icmp eq i64 %v746, %v6
  br i1 %v749, label %b569, label %b566
b568:
  %gcvolta568 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta568, ptr %ctxtopo, align 8
  ret i64 0
b569:
  %v935 = add i64 0, 524296
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v936 = and i64 %v0, 1
  %v937 = icmp ne i64 %v936, 0
  br i1 %v937, label %b667, label %b668
b570:
  store i64 0, ptr %gcs1
  %v754 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v754, ptr %gcs1
  %v755 = load i8, ptr %ctx, align 8
  %c170 = zext i8 %v755 to i64
  %v756 = icmp ne i64 %c170, 0
  br i1 %v756, label %b577, label %b576
b571:
  %v753 = icmp eq i64 %v750, %v6
  br i1 %v753, label %b573, label %b570
b572:
  %gcvolta572 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta572, ptr %ctxtopo, align 8
  ret i64 0
b573:
  br label %b533
b575:
  store i64 0, ptr %gcs1
  %v758 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v758, ptr %gcs1
  %v759 = load i8, ptr %ctx, align 8
  %c171 = zext i8 %v759 to i64
  %v760 = icmp ne i64 %c171, 0
  br i1 %v760, label %b582, label %b581
b576:
  %v757 = icmp eq i64 %v754, %v6
  br i1 %v757, label %b578, label %b575
b577:
  %gcvolta577 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta577, ptr %ctxtopo, align 8
  ret i64 0
b578:
  br label %b533
b580:
  br label %b12
b581:
  %v761 = icmp eq i64 %v758, %v6
  br i1 %v761, label %b583, label %b580
b582:
  %gcvolta582 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta582, ptr %ctxtopo, align 8
  ret i64 0
b583:
  br label %b533
b584:
  %v769 = icmp eq i64 %v0, %v764
  br label %b586
b585:
  %v770 = call i64 @df.class(i64 %v0)
  switch i64 %v770, label %b590 [ i64 675, label %b587 i64 4, label %b588 i64 2, label %b589 i64 3, label %b589 ]
b586:
  %v788 = phi i1 [ %v769, %b584 ], [ %v787, %b591 ]
  br i1 %v788, label %b602, label %b603
b587:
  %v771 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v764)
  %v772 = load i8, ptr %ctx, align 8
  %c172 = zext i8 %v772 to i64
  %v773 = icmp ne i64 %c172, 0
  br i1 %v773, label %b593, label %b592
b588:
  %v775 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v764)
  %v776 = load i8, ptr %ctx, align 8
  %c173 = zext i8 %v776 to i64
  %v777 = icmp ne i64 %c173, 0
  br i1 %v777, label %b595, label %b594
b589:
  %v778 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v764)
  %v779 = load i8, ptr %ctx, align 8
  %c174 = zext i8 %v779 to i64
  %v780 = icmp ne i64 %c174, 0
  br i1 %v780, label %b597, label %b596
b590:
  %sa781_0 = getelementptr [2 x i64], ptr %sargs781, i64 0, i64 0
  store i64 %v764, ptr %sa781_0
  %sat781 = getelementptr [2 x i64], ptr %sargs781, i64 0, i64 1
  store i64 0, ptr %sat781
  %sic781 = getelementptr i64, ptr %area, i64 266
  %sf781 = call ptr @df.selector(ptr %sic781, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v781 = call i64 %sf781(i64 %v0, ptr %sargs781, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v781, ptr %gcs0
  %v782 = load i8, ptr %ctx, align 8
  %c175 = zext i8 %v782 to i64
  %v783 = icmp ne i64 %c175, 0
  br i1 %v783, label %b599, label %b598
b591:
  %v787 = phi i1 [ %v774, %b592 ], [ %v775, %b594 ], [ %v778, %b596 ], [ %v784, %b600 ]
  br label %b586
b592:
  %c176 = zext i8 %v771 to i64
  %v774 = icmp ne i64 %c176, 0
  br label %b591
b593:
  %gcvolta593 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta593, ptr %ctxtopo, align 8
  ret i64 0
b594:
  br label %b591
b595:
  %gcvolta595 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta595, ptr %ctxtopo, align 8
  ret i64 0
b596:
  br label %b591
b597:
  %gcvolta597 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta597, ptr %ctxtopo, align 8
  ret i64 0
b598:
  store i64 0, ptr %gcs1
  %v784 = call i1 @df.unbox_bool(i64 %v781)
  %v785 = load i8, ptr %ctx, align 8
  %c177 = zext i8 %v785 to i64
  %v786 = icmp ne i64 %c177, 0
  br i1 %v786, label %b601, label %b600
b599:
  %gcvolta599 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta599, ptr %ctxtopo, align 8
  ret i64 0
b600:
  br label %b591
b601:
  %gcvolta601 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta601, ptr %ctxtopo, align 8
  ret i64 0
b602:
  %v789 = add i64 0, 0
  br label %b604
b603:
  %v790 = add i64 0, 1
  br label %b604
b604:
  %v791 = phi i64 [ %v789, %b602 ], [ %v790, %b603 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v792 = call i64 @df.box_int(i64 %v791)
  %xcp793 = load i8, ptr %ctx, align 8
  %xcn793 = icmp ne i8 %xcp793, 0
  %xce793 = call i1 @llvm.expect.i1(i1 %xcn793, i1 false)
  br i1 %xce793, label %xc793.limpar, label %xc793.fim
xc793.limpar:
  call void @dartforge_exception_clear()
  br label %xc793.fim
xc793.fim:
  %gcvolta604 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta604, ptr %ctxtopo, align 8
  ret i64 %v792
b606:
  %ga802 = getelementptr i64, ptr %area, i64 13
  %v802 = load i8, ptr %ga802
  %c178 = zext i8 %v802 to i64
  %v803 = icmp ne i64 %c178, 0
  br i1 %v803, label %b608, label %b609
b607:
  %v810 = call i64 @df.unbox_int(i64 %v0)
  %v811 = load i8, ptr %ctx, align 8
  %c179 = zext i8 %v811 to i64
  %v812 = icmp ne i64 %c179, 0
  br i1 %v812, label %b614, label %b613
b608:
  %ga804 = getelementptr i64, ptr %area, i64 12
  %v804 = load i64, ptr %ga804
  br label %b610
b609:
  %v805 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b610
b610:
  %v806 = phi i64 [ %v804, %b608 ], [ %v805, %b609 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v806, i64 0, i64 0)
  %v808 = load i8, ptr %ctx, align 8
  %c180 = zext i8 %v808 to i64
  %v809 = icmp ne i64 %c180, 0
  br i1 %v809, label %b612, label %b611
b611:
  br label %b607
b612:
  %gcvolta612 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta612, ptr %ctxtopo, align 8
  ret i64 0
b613:
  %v813 = add i64 0, 14
  %v814 = ashr i64 %v810, %v813
  store i64 0, ptr %gcs0
  %v815 = call i64 @df.box_int(i64 %v814)
  %xcp816 = load i8, ptr %ctx, align 8
  %xcn816 = icmp ne i8 %xcp816, 0
  %xce816 = call i1 @llvm.expect.i1(i1 %xcn816, i1 false)
  br i1 %xce816, label %xc816.limpar, label %xc816.fim
xc816.limpar:
  call void @dartforge_exception_clear()
  br label %xc816.fim
xc816.fim:
  %gcvolta613 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta613, ptr %ctxtopo, align 8
  ret i64 %v815
b614:
  %gcvolta614 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta614, ptr %ctxtopo, align 8
  ret i64 0
b616:
  %ga825 = getelementptr i64, ptr %area, i64 13
  %v825 = load i8, ptr %ga825
  %c181 = zext i8 %v825 to i64
  %v826 = icmp ne i64 %c181, 0
  br i1 %v826, label %b618, label %b619
b617:
  %v833 = call i64 @df.unbox_int(i64 %v0)
  %v834 = load i8, ptr %ctx, align 8
  %c182 = zext i8 %v834 to i64
  %v835 = icmp ne i64 %c182, 0
  br i1 %v835, label %b624, label %b623
b618:
  %ga827 = getelementptr i64, ptr %area, i64 12
  %v827 = load i64, ptr %ga827
  br label %b620
b619:
  %v828 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b620
b620:
  %v829 = phi i64 [ %v827, %b618 ], [ %v828, %b619 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v829, i64 0, i64 0)
  %v831 = load i8, ptr %ctx, align 8
  %c183 = zext i8 %v831 to i64
  %v832 = icmp ne i64 %c183, 0
  br i1 %v832, label %b622, label %b621
b621:
  br label %b617
b622:
  %gcvolta622 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta622, ptr %ctxtopo, align 8
  ret i64 0
b623:
  %v836 = add i64 0, 12
  %v837 = ashr i64 %v833, %v836
  store i64 0, ptr %gcs0
  %v838 = call i64 @df.box_int(i64 %v837)
  %xcp839 = load i8, ptr %ctx, align 8
  %xcn839 = icmp ne i8 %xcp839, 0
  %xce839 = call i1 @llvm.expect.i1(i1 %xcn839, i1 false)
  br i1 %xce839, label %xc839.limpar, label %xc839.fim
xc839.limpar:
  call void @dartforge_exception_clear()
  br label %xc839.fim
xc839.fim:
  %gcvolta623 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta623, ptr %ctxtopo, align 8
  ret i64 %v838
b624:
  %gcvolta624 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta624, ptr %ctxtopo, align 8
  ret i64 0
b626:
  %ga848 = getelementptr i64, ptr %area, i64 13
  %v848 = load i8, ptr %ga848
  %c184 = zext i8 %v848 to i64
  %v849 = icmp ne i64 %c184, 0
  br i1 %v849, label %b628, label %b629
b627:
  %v856 = call i64 @df.unbox_int(i64 %v0)
  %v857 = load i8, ptr %ctx, align 8
  %c185 = zext i8 %v857 to i64
  %v858 = icmp ne i64 %c185, 0
  br i1 %v858, label %b634, label %b633
b628:
  %ga850 = getelementptr i64, ptr %area, i64 12
  %v850 = load i64, ptr %ga850
  br label %b630
b629:
  %v851 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b630
b630:
  %v852 = phi i64 [ %v850, %b628 ], [ %v851, %b629 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v852, i64 0, i64 0)
  %v854 = load i8, ptr %ctx, align 8
  %c186 = zext i8 %v854 to i64
  %v855 = icmp ne i64 %c186, 0
  br i1 %v855, label %b632, label %b631
b631:
  br label %b627
b632:
  %gcvolta632 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta632, ptr %ctxtopo, align 8
  ret i64 0
b633:
  %v859 = add i64 0, 8
  %v860 = ashr i64 %v856, %v859
  store i64 0, ptr %gcs0
  %v861 = call i64 @df.box_int(i64 %v860)
  %xcp862 = load i8, ptr %ctx, align 8
  %xcn862 = icmp ne i8 %xcp862, 0
  %xce862 = call i1 @llvm.expect.i1(i1 %xcn862, i1 false)
  br i1 %xce862, label %xc862.limpar, label %xc862.fim
xc862.limpar:
  call void @dartforge_exception_clear()
  br label %xc862.fim
xc862.fim:
  %gcvolta633 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta633, ptr %ctxtopo, align 8
  ret i64 %v861
b634:
  %gcvolta634 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta634, ptr %ctxtopo, align 8
  ret i64 0
b637:
  %ga873 = getelementptr i64, ptr %area, i64 13
  %v873 = load i8, ptr %ga873
  %c187 = zext i8 %v873 to i64
  %v874 = icmp ne i64 %c187, 0
  br i1 %v874, label %b639, label %b640
b638:
  %v881 = call i64 @df.unbox_int(i64 %v0)
  %v882 = load i8, ptr %ctx, align 8
  %c188 = zext i8 %v882 to i64
  %v883 = icmp ne i64 %c188, 0
  br i1 %v883, label %b645, label %b644
b639:
  %ga875 = getelementptr i64, ptr %area, i64 12
  %v875 = load i64, ptr %ga875
  br label %b641
b640:
  %v876 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b641
b641:
  %v877 = phi i64 [ %v875, %b639 ], [ %v876, %b640 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v877, i64 0, i64 0)
  %v879 = load i8, ptr %ctx, align 8
  %c189 = zext i8 %v879 to i64
  %v880 = icmp ne i64 %c189, 0
  br i1 %v880, label %b643, label %b642
b642:
  br label %b638
b643:
  %gcvolta643 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta643, ptr %ctxtopo, align 8
  ret i64 0
b644:
  %v884 = add i64 0, 8
  %v885 = shl i64 %v881, %v884
  store i64 0, ptr %gcs0
  %v886 = call i64 @df.box_int(i64 %v885)
  %xcp887 = load i8, ptr %ctx, align 8
  %xcn887 = icmp ne i8 %xcp887, 0
  %xce887 = call i1 @llvm.expect.i1(i1 %xcn887, i1 false)
  br i1 %xce887, label %xc887.limpar, label %xc887.fim
xc887.limpar:
  call void @dartforge_exception_clear()
  br label %xc887.fim
xc887.fim:
  %gcvolta644 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta644, ptr %ctxtopo, align 8
  ret i64 %v886
b645:
  %gcvolta645 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta645, ptr %ctxtopo, align 8
  ret i64 0
b647:
  %ga896 = getelementptr i64, ptr %area, i64 13
  %v896 = load i8, ptr %ga896
  %c190 = zext i8 %v896 to i64
  %v897 = icmp ne i64 %c190, 0
  br i1 %v897, label %b649, label %b650
b648:
  %v904 = call i64 @df.unbox_int(i64 %v0)
  %v905 = load i8, ptr %ctx, align 8
  %c191 = zext i8 %v905 to i64
  %v906 = icmp ne i64 %c191, 0
  br i1 %v906, label %b655, label %b654
b649:
  %ga898 = getelementptr i64, ptr %area, i64 12
  %v898 = load i64, ptr %ga898
  br label %b651
b650:
  %v899 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b651
b651:
  %v900 = phi i64 [ %v898, %b649 ], [ %v899, %b650 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v900, i64 0, i64 0)
  %v902 = load i8, ptr %ctx, align 8
  %c192 = zext i8 %v902 to i64
  %v903 = icmp ne i64 %c192, 0
  br i1 %v903, label %b653, label %b652
b652:
  br label %b648
b653:
  %gcvolta653 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta653, ptr %ctxtopo, align 8
  ret i64 0
b654:
  %v907 = add i64 0, 9
  %v908 = ashr i64 %v904, %v907
  store i64 0, ptr %gcs0
  %v909 = call i64 @df.box_int(i64 %v908)
  %xcp910 = load i8, ptr %ctx, align 8
  %xcn910 = icmp ne i8 %xcp910, 0
  %xce910 = call i1 @llvm.expect.i1(i1 %xcn910, i1 false)
  br i1 %xce910, label %xc910.limpar, label %xc910.fim
xc910.limpar:
  call void @dartforge_exception_clear()
  br label %xc910.fim
xc910.fim:
  %gcvolta654 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta654, ptr %ctxtopo, align 8
  ret i64 %v909
b655:
  %gcvolta655 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta655, ptr %ctxtopo, align 8
  ret i64 0
b657:
  %ga919 = getelementptr i64, ptr %area, i64 13
  %v919 = load i8, ptr %ga919
  %c193 = zext i8 %v919 to i64
  %v920 = icmp ne i64 %c193, 0
  br i1 %v920, label %b659, label %b660
b658:
  %v927 = call i64 @df.unbox_int(i64 %v0)
  %v928 = load i8, ptr %ctx, align 8
  %c194 = zext i8 %v928 to i64
  %v929 = icmp ne i64 %c194, 0
  br i1 %v929, label %b665, label %b664
b659:
  %ga921 = getelementptr i64, ptr %area, i64 12
  %v921 = load i64, ptr %ga921
  br label %b661
b660:
  %v922 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b661
b661:
  %v923 = phi i64 [ %v921, %b659 ], [ %v922, %b660 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v923, i64 0, i64 0)
  %v925 = load i8, ptr %ctx, align 8
  %c195 = zext i8 %v925 to i64
  %v926 = icmp ne i64 %c195, 0
  br i1 %v926, label %b663, label %b662
b662:
  br label %b658
b663:
  %gcvolta663 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta663, ptr %ctxtopo, align 8
  ret i64 0
b664:
  %v930 = add i64 0, 1
  %v931 = ashr i64 %v927, %v930
  store i64 0, ptr %gcs0
  %v932 = call i64 @df.box_int(i64 %v931)
  %xcp933 = load i8, ptr %ctx, align 8
  %xcn933 = icmp ne i8 %xcp933, 0
  %xce933 = call i1 @llvm.expect.i1(i1 %xcn933, i1 false)
  br i1 %xce933, label %xc933.limpar, label %xc933.fim
xc933.limpar:
  call void @dartforge_exception_clear()
  br label %xc933.fim
xc933.fim:
  %gcvolta664 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta664, ptr %ctxtopo, align 8
  ret i64 %v932
b665:
  %gcvolta665 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta665, ptr %ctxtopo, align 8
  ret i64 0
b667:
  %v938 = ashr i64 %v0, 1
  %v939 = mul i64 %v938, %v935
  store i64 0, ptr %gcs0
  %v940 = call i64 @df.box_int(i64 %v939)
  br label %b669
b668:
  %v941 = call i64 @df.box_int(i64 %v935)
  store i64 %v941, ptr %gcs1
  %sa942_0 = getelementptr [2 x i64], ptr %sargs942, i64 0, i64 0
  store i64 %v941, ptr %sa942_0
  %sat942 = getelementptr [2 x i64], ptr %sargs942, i64 0, i64 1
  store i64 0, ptr %sat942
  %sic942 = getelementptr i64, ptr %area, i64 268
  %sf942 = call ptr @df.selector(ptr %sic942, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v942 = call i64 %sf942(i64 %v0, ptr %sargs942, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v943 = load i8, ptr %ctx, align 8
  %c196 = zext i8 %v943 to i64
  %v944 = icmp ne i64 %c196, 0
  br i1 %v944, label %b671, label %b670
b669:
  %v945 = phi i64 [ %v940, %b667 ], [ %v942, %b670 ]
  %xcp946 = load i8, ptr %ctx, align 8
  %xcn946 = icmp ne i8 %xcp946, 0
  %xce946 = call i1 @llvm.expect.i1(i1 %xcn946, i1 false)
  br i1 %xce946, label %xc946.limpar, label %xc946.fim
xc946.limpar:
  call void @dartforge_exception_clear()
  br label %xc946.fim
xc946.fim:
  %gcvolta669 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta669, ptr %ctxtopo, align 8
  ret i64 %v945
b670:
  br label %b669
b671:
  %gcvolta671 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta671, ptr %ctxtopo, align 8
  ret i64 0
b673:
  store i64 0, ptr %gcs1
  %v953 = call double @df.unbox_double(i64 %v950)
  %v954 = load i8, ptr %ctx, align 8
  %c197 = zext i8 %v954 to i64
  %v955 = icmp ne i64 %c197, 0
  br i1 %v955, label %b676, label %b675
b674:
  %gcvolta674 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta674, ptr %ctxtopo, align 8
  ret i64 0
b675:
  store i64 0, ptr %gcs0
  %v956 = call i64 @df.box_double(double %v953)
  %xcp957 = load i8, ptr %ctx, align 8
  %xcn957 = icmp ne i8 %xcp957, 0
  %xce957 = call i1 @llvm.expect.i1(i1 %xcn957, i1 false)
  br i1 %xce957, label %xc957.limpar, label %xc957.fim
xc957.limpar:
  call void @dartforge_exception_clear()
  br label %xc957.fim
xc957.fim:
  %gcvolta675 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta675, ptr %ctxtopo, align 8
  ret i64 %v956
b676:
  %gcvolta676 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta676, ptr %ctxtopo, align 8
  ret i64 0
b690:
  %v1203 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1204 = call i64 @df.box_int(i64 %v1203)
  store i64 %v1204, ptr %gcs1
  %sa1205_0 = getelementptr [2 x i64], ptr %sargs1205, i64 0, i64 0
  store i64 %v1204, ptr %sa1205_0
  %sat1205 = getelementptr [2 x i64], ptr %sargs1205, i64 0, i64 1
  store i64 0, ptr %sat1205
  %sic1205 = getelementptr i64, ptr %area, i64 270
  %sf1205 = call ptr @df.selector(ptr %sic1205, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v1205 = call i64 %sf1205(i64 %v0, ptr %sargs1205, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1205, ptr %gcs0
  %v1206 = load i8, ptr %ctx, align 8
  %c198 = zext i8 %v1206 to i64
  %v1207 = icmp ne i64 %c198, 0
  br i1 %v1207, label %b835, label %b834
b691:
  store i64 0, ptr %gcs1
  %v963 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v963, ptr %gcs1
  %v964 = load i8, ptr %ctx, align 8
  %c199 = zext i8 %v964 to i64
  %v965 = icmp ne i64 %c199, 0
  br i1 %v965, label %b697, label %b696
b692:
  %v962 = icmp eq i64 %v959, %v6
  br i1 %v962, label %b694, label %b691
b693:
  %gcvolta693 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta693, ptr %ctxtopo, align 8
  ret i64 0
b694:
  %v1008 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1009 = call i64 @df.box_int(i64 %v1008)
  store i64 %v1009, ptr %gcs1
  %v1010 = icmp eq i64 %v0, 0
  %v1011 = icmp eq i64 %v1009, 0
  %c200 = zext i1 %v1010 to i64
  %c201 = zext i1 %v1011 to i64
  %v1012 = or i64 %c200, %c201
  %v1013 = icmp ne i64 %v1012, 0
  br i1 %v1013, label %b741, label %b742
b695:
  store i64 0, ptr %gcs1
  %v967 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v967, ptr %gcs1
  %v968 = load i8, ptr %ctx, align 8
  %c202 = zext i8 %v968 to i64
  %v969 = icmp ne i64 %c202, 0
  br i1 %v969, label %b701, label %b700
b696:
  %v966 = icmp eq i64 %v963, %v6
  br i1 %v966, label %b698, label %b695
b697:
  %gcvolta697 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta697, ptr %ctxtopo, align 8
  ret i64 0
b698:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1040 = icmp eq i64 %v0, 0
  %v1041 = call i64 @df.class(i64 %v0)
  %v1042 = call i8 @df.subclass(i64 %v1041, i64 674)
  %c203 = zext i8 %v1042 to i64
  %v1043 = icmp ne i64 %c203, 0
  %v1044 = xor i1 %v1040, true
  %c204 = zext i1 %v1043 to i64
  %c205 = zext i1 %v1044 to i64
  %v1045 = and i64 %c204, %c205
  %v1046 = icmp ne i64 %v1045, 0
  br i1 %v1046, label %b764, label %b763
b699:
  store i64 0, ptr %gcs1
  %v971 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v971, ptr %gcs1
  %v972 = load i8, ptr %ctx, align 8
  %c206 = zext i8 %v972 to i64
  %v973 = icmp ne i64 %c206, 0
  br i1 %v973, label %b705, label %b704
b700:
  %v970 = icmp eq i64 %v967, %v6
  br i1 %v970, label %b702, label %b699
b701:
  %gcvolta701 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta701, ptr %ctxtopo, align 8
  ret i64 0
b702:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1063 = icmp eq i64 %v0, 0
  %v1064 = call i64 @df.class(i64 %v0)
  %v1065 = call i8 @df.subclass(i64 %v1064, i64 674)
  %c207 = zext i8 %v1065 to i64
  %v1066 = icmp ne i64 %c207, 0
  %v1067 = xor i1 %v1063, true
  %c208 = zext i1 %v1066 to i64
  %c209 = zext i1 %v1067 to i64
  %v1068 = and i64 %c208, %c209
  %v1069 = icmp ne i64 %v1068, 0
  br i1 %v1069, label %b774, label %b773
b703:
  store i64 0, ptr %gcs1
  %v975 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v975, ptr %gcs1
  %v976 = load i8, ptr %ctx, align 8
  %c210 = zext i8 %v976 to i64
  %v977 = icmp ne i64 %c210, 0
  br i1 %v977, label %b709, label %b708
b704:
  %v974 = icmp eq i64 %v971, %v6
  br i1 %v974, label %b706, label %b703
b705:
  %gcvolta705 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta705, ptr %ctxtopo, align 8
  ret i64 0
b706:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1086 = icmp eq i64 %v0, 0
  %v1087 = call i64 @df.class(i64 %v0)
  %v1088 = call i8 @df.subclass(i64 %v1087, i64 674)
  %c211 = zext i8 %v1088 to i64
  %v1089 = icmp ne i64 %c211, 0
  %v1090 = xor i1 %v1086, true
  %c212 = zext i1 %v1089 to i64
  %c213 = zext i1 %v1090 to i64
  %v1091 = and i64 %c212, %c213
  %v1092 = icmp ne i64 %v1091, 0
  br i1 %v1092, label %b784, label %b783
b707:
  store i64 0, ptr %gcs1
  %v979 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v979, ptr %gcs1
  %v980 = load i8, ptr %ctx, align 8
  %c214 = zext i8 %v980 to i64
  %v981 = icmp ne i64 %c214, 0
  br i1 %v981, label %b713, label %b712
b708:
  %v978 = icmp eq i64 %v975, %v6
  br i1 %v978, label %b710, label %b707
b709:
  %gcvolta709 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta709, ptr %ctxtopo, align 8
  ret i64 0
b710:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1109 = icmp eq i64 %v0, 0
  %v1110 = call i64 @df.class(i64 %v0)
  %v1111 = call i8 @df.subclass(i64 %v1110, i64 674)
  %c215 = zext i8 %v1111 to i64
  %v1112 = icmp ne i64 %c215, 0
  %v1113 = xor i1 %v1109, true
  %c216 = zext i1 %v1112 to i64
  %c217 = zext i1 %v1113 to i64
  %v1114 = and i64 %c216, %c217
  %v1115 = icmp ne i64 %v1114, 0
  br i1 %v1115, label %b794, label %b793
b711:
  store i64 0, ptr %gcs1
  %v983 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v983, ptr %gcs1
  %v984 = load i8, ptr %ctx, align 8
  %c218 = zext i8 %v984 to i64
  %v985 = icmp ne i64 %c218, 0
  br i1 %v985, label %b717, label %b716
b712:
  %v982 = icmp eq i64 %v979, %v6
  br i1 %v982, label %b714, label %b711
b713:
  %gcvolta713 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta713, ptr %ctxtopo, align 8
  ret i64 0
b714:
  %xcp1132 = load i8, ptr %ctx, align 8
  %xcn1132 = icmp ne i8 %xcp1132, 0
  %xce1132 = call i1 @llvm.expect.i1(i1 %xcn1132, i1 false)
  br i1 %xce1132, label %xc1132.limpar, label %xc1132.fim
xc1132.limpar:
  call void @dartforge_exception_clear()
  br label %xc1132.fim
xc1132.fim:
  %gcvolta714 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta714, ptr %ctxtopo, align 8
  ret i64 %v0
b715:
  store i64 0, ptr %gcs1
  %v987 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v987, ptr %gcs1
  %v988 = load i8, ptr %ctx, align 8
  %c219 = zext i8 %v988 to i64
  %v989 = icmp ne i64 %c219, 0
  br i1 %v989, label %b721, label %b720
b716:
  %v986 = icmp eq i64 %v983, %v6
  br i1 %v986, label %b718, label %b715
b717:
  %gcvolta717 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta717, ptr %ctxtopo, align 8
  ret i64 0
b718:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1134 = icmp eq i64 %v0, 0
  %v1135 = call i64 @df.class(i64 %v0)
  %v1136 = call i8 @df.subclass(i64 %v1135, i64 674)
  %c220 = zext i8 %v1136 to i64
  %v1137 = icmp ne i64 %c220, 0
  %v1138 = xor i1 %v1134, true
  %c221 = zext i1 %v1137 to i64
  %c222 = zext i1 %v1138 to i64
  %v1139 = and i64 %c221, %c222
  %v1140 = icmp ne i64 %v1139, 0
  br i1 %v1140, label %b805, label %b804
b719:
  store i64 0, ptr %gcs1
  %v991 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v991, ptr %gcs1
  %v992 = load i8, ptr %ctx, align 8
  %c223 = zext i8 %v992 to i64
  %v993 = icmp ne i64 %c223, 0
  br i1 %v993, label %b725, label %b724
b720:
  %v990 = icmp eq i64 %v987, %v6
  br i1 %v990, label %b722, label %b719
b721:
  %gcvolta721 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta721, ptr %ctxtopo, align 8
  ret i64 0
b722:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1157 = icmp eq i64 %v0, 0
  %v1158 = call i64 @df.class(i64 %v0)
  %v1159 = call i8 @df.subclass(i64 %v1158, i64 674)
  %c224 = zext i8 %v1159 to i64
  %v1160 = icmp ne i64 %c224, 0
  %v1161 = xor i1 %v1157, true
  %c225 = zext i1 %v1160 to i64
  %c226 = zext i1 %v1161 to i64
  %v1162 = and i64 %c225, %c226
  %v1163 = icmp ne i64 %v1162, 0
  br i1 %v1163, label %b815, label %b814
b723:
  store i64 0, ptr %gcs1
  %v995 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v995, ptr %gcs1
  %v996 = load i8, ptr %ctx, align 8
  %c227 = zext i8 %v996 to i64
  %v997 = icmp ne i64 %c227, 0
  br i1 %v997, label %b729, label %b728
b724:
  %v994 = icmp eq i64 %v991, %v6
  br i1 %v994, label %b726, label %b723
b725:
  %gcvolta725 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta725, ptr %ctxtopo, align 8
  ret i64 0
b726:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1180 = icmp eq i64 %v0, 0
  %v1181 = call i64 @df.class(i64 %v0)
  %v1182 = call i8 @df.subclass(i64 %v1181, i64 674)
  %c228 = zext i8 %v1182 to i64
  %v1183 = icmp ne i64 %c228, 0
  %v1184 = xor i1 %v1180, true
  %c229 = zext i1 %v1183 to i64
  %c230 = zext i1 %v1184 to i64
  %v1185 = and i64 %c229, %c230
  %v1186 = icmp ne i64 %v1185, 0
  br i1 %v1186, label %b825, label %b824
b727:
  store i64 0, ptr %gcs1
  %v999 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v999, ptr %gcs1
  %v1000 = load i8, ptr %ctx, align 8
  %c231 = zext i8 %v1000 to i64
  %v1001 = icmp ne i64 %c231, 0
  br i1 %v1001, label %b734, label %b733
b728:
  %v998 = icmp eq i64 %v995, %v6
  br i1 %v998, label %b730, label %b727
b729:
  %gcvolta729 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta729, ptr %ctxtopo, align 8
  ret i64 0
b730:
  br label %b690
b732:
  store i64 0, ptr %gcs1
  %v1003 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1003, ptr %gcs1
  %v1004 = load i8, ptr %ctx, align 8
  %c232 = zext i8 %v1004 to i64
  %v1005 = icmp ne i64 %c232, 0
  br i1 %v1005, label %b739, label %b738
b733:
  %v1002 = icmp eq i64 %v999, %v6
  br i1 %v1002, label %b735, label %b732
b734:
  %gcvolta734 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta734, ptr %ctxtopo, align 8
  ret i64 0
b735:
  br label %b690
b737:
  br label %b12
b738:
  %v1006 = icmp eq i64 %v1003, %v6
  br i1 %v1006, label %b740, label %b737
b739:
  %gcvolta739 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta739, ptr %ctxtopo, align 8
  ret i64 0
b740:
  br label %b690
b741:
  %v1014 = icmp eq i64 %v0, %v1009
  br label %b743
b742:
  %v1015 = call i64 @df.class(i64 %v0)
  switch i64 %v1015, label %b747 [ i64 675, label %b744 i64 4, label %b745 i64 2, label %b746 i64 3, label %b746 ]
b743:
  %v1033 = phi i1 [ %v1014, %b741 ], [ %v1032, %b748 ]
  br i1 %v1033, label %b759, label %b760
b744:
  %v1016 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1009)
  %v1017 = load i8, ptr %ctx, align 8
  %c233 = zext i8 %v1017 to i64
  %v1018 = icmp ne i64 %c233, 0
  br i1 %v1018, label %b750, label %b749
b745:
  %v1020 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1009)
  %v1021 = load i8, ptr %ctx, align 8
  %c234 = zext i8 %v1021 to i64
  %v1022 = icmp ne i64 %c234, 0
  br i1 %v1022, label %b752, label %b751
b746:
  %v1023 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1009)
  %v1024 = load i8, ptr %ctx, align 8
  %c235 = zext i8 %v1024 to i64
  %v1025 = icmp ne i64 %c235, 0
  br i1 %v1025, label %b754, label %b753
b747:
  %sa1026_0 = getelementptr [2 x i64], ptr %sargs1026, i64 0, i64 0
  store i64 %v1009, ptr %sa1026_0
  %sat1026 = getelementptr [2 x i64], ptr %sargs1026, i64 0, i64 1
  store i64 0, ptr %sat1026
  %sic1026 = getelementptr i64, ptr %area, i64 272
  %sf1026 = call ptr @df.selector(ptr %sic1026, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v1026 = call i64 %sf1026(i64 %v0, ptr %sargs1026, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1026, ptr %gcs0
  %v1027 = load i8, ptr %ctx, align 8
  %c236 = zext i8 %v1027 to i64
  %v1028 = icmp ne i64 %c236, 0
  br i1 %v1028, label %b756, label %b755
b748:
  %v1032 = phi i1 [ %v1019, %b749 ], [ %v1020, %b751 ], [ %v1023, %b753 ], [ %v1029, %b757 ]
  br label %b743
b749:
  %c237 = zext i8 %v1016 to i64
  %v1019 = icmp ne i64 %c237, 0
  br label %b748
b750:
  %gcvolta750 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta750, ptr %ctxtopo, align 8
  ret i64 0
b751:
  br label %b748
b752:
  %gcvolta752 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta752, ptr %ctxtopo, align 8
  ret i64 0
b753:
  br label %b748
b754:
  %gcvolta754 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta754, ptr %ctxtopo, align 8
  ret i64 0
b755:
  store i64 0, ptr %gcs1
  %v1029 = call i1 @df.unbox_bool(i64 %v1026)
  %v1030 = load i8, ptr %ctx, align 8
  %c238 = zext i8 %v1030 to i64
  %v1031 = icmp ne i64 %c238, 0
  br i1 %v1031, label %b758, label %b757
b756:
  %gcvolta756 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta756, ptr %ctxtopo, align 8
  ret i64 0
b757:
  br label %b748
b758:
  %gcvolta758 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta758, ptr %ctxtopo, align 8
  ret i64 0
b759:
  %v1034 = add i64 0, 0
  br label %b761
b760:
  %v1035 = add i64 0, 1
  br label %b761
b761:
  %v1036 = phi i64 [ %v1034, %b759 ], [ %v1035, %b760 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1037 = call i64 @df.box_int(i64 %v1036)
  %xcp1038 = load i8, ptr %ctx, align 8
  %xcn1038 = icmp ne i8 %xcp1038, 0
  %xce1038 = call i1 @llvm.expect.i1(i1 %xcn1038, i1 false)
  br i1 %xce1038, label %xc1038.limpar, label %xc1038.fim
xc1038.limpar:
  call void @dartforge_exception_clear()
  br label %xc1038.fim
xc1038.fim:
  %gcvolta761 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta761, ptr %ctxtopo, align 8
  ret i64 %v1037
b763:
  %ga1047 = getelementptr i64, ptr %area, i64 13
  %v1047 = load i8, ptr %ga1047
  %c239 = zext i8 %v1047 to i64
  %v1048 = icmp ne i64 %c239, 0
  br i1 %v1048, label %b765, label %b766
b764:
  %v1055 = call i64 @df.unbox_int(i64 %v0)
  %v1056 = load i8, ptr %ctx, align 8
  %c240 = zext i8 %v1056 to i64
  %v1057 = icmp ne i64 %c240, 0
  br i1 %v1057, label %b771, label %b770
b765:
  %ga1049 = getelementptr i64, ptr %area, i64 12
  %v1049 = load i64, ptr %ga1049
  br label %b767
b766:
  %v1050 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b767
b767:
  %v1051 = phi i64 [ %v1049, %b765 ], [ %v1050, %b766 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1051, i64 0, i64 0)
  %v1053 = load i8, ptr %ctx, align 8
  %c241 = zext i8 %v1053 to i64
  %v1054 = icmp ne i64 %c241, 0
  br i1 %v1054, label %b769, label %b768
b768:
  br label %b764
b769:
  %gcvolta769 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta769, ptr %ctxtopo, align 8
  ret i64 0
b770:
  %v1058 = add i64 0, 30
  %v1059 = ashr i64 %v1055, %v1058
  store i64 0, ptr %gcs0
  %v1060 = call i64 @df.box_int(i64 %v1059)
  %xcp1061 = load i8, ptr %ctx, align 8
  %xcn1061 = icmp ne i8 %xcp1061, 0
  %xce1061 = call i1 @llvm.expect.i1(i1 %xcn1061, i1 false)
  br i1 %xce1061, label %xc1061.limpar, label %xc1061.fim
xc1061.limpar:
  call void @dartforge_exception_clear()
  br label %xc1061.fim
xc1061.fim:
  %gcvolta770 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta770, ptr %ctxtopo, align 8
  ret i64 %v1060
b771:
  %gcvolta771 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta771, ptr %ctxtopo, align 8
  ret i64 0
b773:
  %ga1070 = getelementptr i64, ptr %area, i64 13
  %v1070 = load i8, ptr %ga1070
  %c242 = zext i8 %v1070 to i64
  %v1071 = icmp ne i64 %c242, 0
  br i1 %v1071, label %b775, label %b776
b774:
  %v1078 = call i64 @df.unbox_int(i64 %v0)
  %v1079 = load i8, ptr %ctx, align 8
  %c243 = zext i8 %v1079 to i64
  %v1080 = icmp ne i64 %c243, 0
  br i1 %v1080, label %b781, label %b780
b775:
  %ga1072 = getelementptr i64, ptr %area, i64 12
  %v1072 = load i64, ptr %ga1072
  br label %b777
b776:
  %v1073 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b777
b777:
  %v1074 = phi i64 [ %v1072, %b775 ], [ %v1073, %b776 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1074, i64 0, i64 0)
  %v1076 = load i8, ptr %ctx, align 8
  %c244 = zext i8 %v1076 to i64
  %v1077 = icmp ne i64 %c244, 0
  br i1 %v1077, label %b779, label %b778
b778:
  br label %b774
b779:
  %gcvolta779 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta779, ptr %ctxtopo, align 8
  ret i64 0
b780:
  %v1081 = add i64 0, 28
  %v1082 = ashr i64 %v1078, %v1081
  store i64 0, ptr %gcs0
  %v1083 = call i64 @df.box_int(i64 %v1082)
  %xcp1084 = load i8, ptr %ctx, align 8
  %xcn1084 = icmp ne i8 %xcp1084, 0
  %xce1084 = call i1 @llvm.expect.i1(i1 %xcn1084, i1 false)
  br i1 %xce1084, label %xc1084.limpar, label %xc1084.fim
xc1084.limpar:
  call void @dartforge_exception_clear()
  br label %xc1084.fim
xc1084.fim:
  %gcvolta780 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta780, ptr %ctxtopo, align 8
  ret i64 %v1083
b781:
  %gcvolta781 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta781, ptr %ctxtopo, align 8
  ret i64 0
b783:
  %ga1093 = getelementptr i64, ptr %area, i64 13
  %v1093 = load i8, ptr %ga1093
  %c245 = zext i8 %v1093 to i64
  %v1094 = icmp ne i64 %c245, 0
  br i1 %v1094, label %b785, label %b786
b784:
  %v1101 = call i64 @df.unbox_int(i64 %v0)
  %v1102 = load i8, ptr %ctx, align 8
  %c246 = zext i8 %v1102 to i64
  %v1103 = icmp ne i64 %c246, 0
  br i1 %v1103, label %b791, label %b790
b785:
  %ga1095 = getelementptr i64, ptr %area, i64 12
  %v1095 = load i64, ptr %ga1095
  br label %b787
b786:
  %v1096 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b787
b787:
  %v1097 = phi i64 [ %v1095, %b785 ], [ %v1096, %b786 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1097, i64 0, i64 0)
  %v1099 = load i8, ptr %ctx, align 8
  %c247 = zext i8 %v1099 to i64
  %v1100 = icmp ne i64 %c247, 0
  br i1 %v1100, label %b789, label %b788
b788:
  br label %b784
b789:
  %gcvolta789 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta789, ptr %ctxtopo, align 8
  ret i64 0
b790:
  %v1104 = add i64 0, 24
  %v1105 = ashr i64 %v1101, %v1104
  store i64 0, ptr %gcs0
  %v1106 = call i64 @df.box_int(i64 %v1105)
  %xcp1107 = load i8, ptr %ctx, align 8
  %xcn1107 = icmp ne i8 %xcp1107, 0
  %xce1107 = call i1 @llvm.expect.i1(i1 %xcn1107, i1 false)
  br i1 %xce1107, label %xc1107.limpar, label %xc1107.fim
xc1107.limpar:
  call void @dartforge_exception_clear()
  br label %xc1107.fim
xc1107.fim:
  %gcvolta790 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta790, ptr %ctxtopo, align 8
  ret i64 %v1106
b791:
  %gcvolta791 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta791, ptr %ctxtopo, align 8
  ret i64 0
b793:
  %ga1116 = getelementptr i64, ptr %area, i64 13
  %v1116 = load i8, ptr %ga1116
  %c248 = zext i8 %v1116 to i64
  %v1117 = icmp ne i64 %c248, 0
  br i1 %v1117, label %b795, label %b796
b794:
  %v1124 = call i64 @df.unbox_int(i64 %v0)
  %v1125 = load i8, ptr %ctx, align 8
  %c249 = zext i8 %v1125 to i64
  %v1126 = icmp ne i64 %c249, 0
  br i1 %v1126, label %b801, label %b800
b795:
  %ga1118 = getelementptr i64, ptr %area, i64 12
  %v1118 = load i64, ptr %ga1118
  br label %b797
b796:
  %v1119 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b797
b797:
  %v1120 = phi i64 [ %v1118, %b795 ], [ %v1119, %b796 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1120, i64 0, i64 0)
  %v1122 = load i8, ptr %ctx, align 8
  %c250 = zext i8 %v1122 to i64
  %v1123 = icmp ne i64 %c250, 0
  br i1 %v1123, label %b799, label %b798
b798:
  br label %b794
b799:
  %gcvolta799 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta799, ptr %ctxtopo, align 8
  ret i64 0
b800:
  %v1127 = add i64 0, 16
  %v1128 = ashr i64 %v1124, %v1127
  store i64 0, ptr %gcs0
  %v1129 = call i64 @df.box_int(i64 %v1128)
  %xcp1130 = load i8, ptr %ctx, align 8
  %xcn1130 = icmp ne i8 %xcp1130, 0
  %xce1130 = call i1 @llvm.expect.i1(i1 %xcn1130, i1 false)
  br i1 %xce1130, label %xc1130.limpar, label %xc1130.fim
xc1130.limpar:
  call void @dartforge_exception_clear()
  br label %xc1130.fim
xc1130.fim:
  %gcvolta800 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta800, ptr %ctxtopo, align 8
  ret i64 %v1129
b801:
  %gcvolta801 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta801, ptr %ctxtopo, align 8
  ret i64 0
b804:
  %ga1141 = getelementptr i64, ptr %area, i64 13
  %v1141 = load i8, ptr %ga1141
  %c251 = zext i8 %v1141 to i64
  %v1142 = icmp ne i64 %c251, 0
  br i1 %v1142, label %b806, label %b807
b805:
  %v1149 = call i64 @df.unbox_int(i64 %v0)
  %v1150 = load i8, ptr %ctx, align 8
  %c252 = zext i8 %v1150 to i64
  %v1151 = icmp ne i64 %c252, 0
  br i1 %v1151, label %b812, label %b811
b806:
  %ga1143 = getelementptr i64, ptr %area, i64 12
  %v1143 = load i64, ptr %ga1143
  br label %b808
b807:
  %v1144 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b808
b808:
  %v1145 = phi i64 [ %v1143, %b806 ], [ %v1144, %b807 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1145, i64 0, i64 0)
  %v1147 = load i8, ptr %ctx, align 8
  %c253 = zext i8 %v1147 to i64
  %v1148 = icmp ne i64 %c253, 0
  br i1 %v1148, label %b810, label %b809
b809:
  br label %b805
b810:
  %gcvolta810 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta810, ptr %ctxtopo, align 8
  ret i64 0
b811:
  %v1152 = add i64 0, 25
  %v1153 = ashr i64 %v1149, %v1152
  store i64 0, ptr %gcs0
  %v1154 = call i64 @df.box_int(i64 %v1153)
  %xcp1155 = load i8, ptr %ctx, align 8
  %xcn1155 = icmp ne i8 %xcp1155, 0
  %xce1155 = call i1 @llvm.expect.i1(i1 %xcn1155, i1 false)
  br i1 %xce1155, label %xc1155.limpar, label %xc1155.fim
xc1155.limpar:
  call void @dartforge_exception_clear()
  br label %xc1155.fim
xc1155.fim:
  %gcvolta811 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta811, ptr %ctxtopo, align 8
  ret i64 %v1154
b812:
  %gcvolta812 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta812, ptr %ctxtopo, align 8
  ret i64 0
b814:
  %ga1164 = getelementptr i64, ptr %area, i64 13
  %v1164 = load i8, ptr %ga1164
  %c254 = zext i8 %v1164 to i64
  %v1165 = icmp ne i64 %c254, 0
  br i1 %v1165, label %b816, label %b817
b815:
  %v1172 = call i64 @df.unbox_int(i64 %v0)
  %v1173 = load i8, ptr %ctx, align 8
  %c255 = zext i8 %v1173 to i64
  %v1174 = icmp ne i64 %c255, 0
  br i1 %v1174, label %b822, label %b821
b816:
  %ga1166 = getelementptr i64, ptr %area, i64 12
  %v1166 = load i64, ptr %ga1166
  br label %b818
b817:
  %v1167 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b818
b818:
  %v1168 = phi i64 [ %v1166, %b816 ], [ %v1167, %b817 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1168, i64 0, i64 0)
  %v1170 = load i8, ptr %ctx, align 8
  %c256 = zext i8 %v1170 to i64
  %v1171 = icmp ne i64 %c256, 0
  br i1 %v1171, label %b820, label %b819
b819:
  br label %b815
b820:
  %gcvolta820 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta820, ptr %ctxtopo, align 8
  ret i64 0
b821:
  %v1175 = add i64 0, 17
  %v1176 = ashr i64 %v1172, %v1175
  store i64 0, ptr %gcs0
  %v1177 = call i64 @df.box_int(i64 %v1176)
  %xcp1178 = load i8, ptr %ctx, align 8
  %xcn1178 = icmp ne i8 %xcp1178, 0
  %xce1178 = call i1 @llvm.expect.i1(i1 %xcn1178, i1 false)
  br i1 %xce1178, label %xc1178.limpar, label %xc1178.fim
xc1178.limpar:
  call void @dartforge_exception_clear()
  br label %xc1178.fim
xc1178.fim:
  %gcvolta821 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta821, ptr %ctxtopo, align 8
  ret i64 %v1177
b822:
  %gcvolta822 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta822, ptr %ctxtopo, align 8
  ret i64 0
b824:
  %ga1187 = getelementptr i64, ptr %area, i64 13
  %v1187 = load i8, ptr %ga1187
  %c257 = zext i8 %v1187 to i64
  %v1188 = icmp ne i64 %c257, 0
  br i1 %v1188, label %b826, label %b827
b825:
  %v1195 = call i64 @df.unbox_int(i64 %v0)
  %v1196 = load i8, ptr %ctx, align 8
  %c258 = zext i8 %v1196 to i64
  %v1197 = icmp ne i64 %c258, 0
  br i1 %v1197, label %b832, label %b831
b826:
  %ga1189 = getelementptr i64, ptr %area, i64 12
  %v1189 = load i64, ptr %ga1189
  br label %b828
b827:
  %v1190 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b828
b828:
  %v1191 = phi i64 [ %v1189, %b826 ], [ %v1190, %b827 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1191, i64 0, i64 0)
  %v1193 = load i8, ptr %ctx, align 8
  %c259 = zext i8 %v1193 to i64
  %v1194 = icmp ne i64 %c259, 0
  br i1 %v1194, label %b830, label %b829
b829:
  br label %b825
b830:
  %gcvolta830 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta830, ptr %ctxtopo, align 8
  ret i64 0
b831:
  %v1198 = add i64 0, 1
  %v1199 = ashr i64 %v1195, %v1198
  store i64 0, ptr %gcs0
  %v1200 = call i64 @df.box_int(i64 %v1199)
  %xcp1201 = load i8, ptr %ctx, align 8
  %xcn1201 = icmp ne i8 %xcp1201, 0
  %xce1201 = call i1 @llvm.expect.i1(i1 %xcn1201, i1 false)
  br i1 %xce1201, label %xc1201.limpar, label %xc1201.fim
xc1201.limpar:
  call void @dartforge_exception_clear()
  br label %xc1201.fim
xc1201.fim:
  %gcvolta831 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta831, ptr %ctxtopo, align 8
  ret i64 %v1200
b832:
  %gcvolta832 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta832, ptr %ctxtopo, align 8
  ret i64 0
b834:
  store i64 0, ptr %gcs1
  %v1208 = call double @df.unbox_double(i64 %v1205)
  %v1209 = load i8, ptr %ctx, align 8
  %c260 = zext i8 %v1209 to i64
  %v1210 = icmp ne i64 %c260, 0
  br i1 %v1210, label %b837, label %b836
b835:
  %gcvolta835 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta835, ptr %ctxtopo, align 8
  ret i64 0
b836:
  store i64 0, ptr %gcs0
  %v1211 = call i64 @df.box_double(double %v1208)
  %xcp1212 = load i8, ptr %ctx, align 8
  %xcn1212 = icmp ne i8 %xcp1212, 0
  %xce1212 = call i1 @llvm.expect.i1(i1 %xcn1212, i1 false)
  br i1 %xce1212, label %xc1212.limpar, label %xc1212.fim
xc1212.limpar:
  call void @dartforge_exception_clear()
  br label %xc1212.fim
xc1212.fim:
  %gcvolta836 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta836, ptr %ctxtopo, align 8
  ret i64 %v1211
b837:
  %gcvolta837 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta837, ptr %ctxtopo, align 8
  ret i64 0
b851:
  %v1518 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1519 = call i64 @df.box_int(i64 %v1518)
  store i64 %v1519, ptr %gcs1
  %sa1520_0 = getelementptr [2 x i64], ptr %sargs1520, i64 0, i64 0
  store i64 %v1519, ptr %sa1520_0
  %sat1520 = getelementptr [2 x i64], ptr %sargs1520, i64 0, i64 1
  store i64 0, ptr %sat1520
  %sic1520 = getelementptr i64, ptr %area, i64 274
  %sf1520 = call ptr @df.selector(ptr %sic1520, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v1520 = call i64 %sf1520(i64 %v0, ptr %sargs1520, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1520, ptr %gcs0
  %v1521 = load i8, ptr %ctx, align 8
  %c261 = zext i8 %v1521 to i64
  %v1522 = icmp ne i64 %c261, 0
  br i1 %v1522, label %b1038, label %b1037
b852:
  store i64 0, ptr %gcs1
  %v1218 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1218, ptr %gcs1
  %v1219 = load i8, ptr %ctx, align 8
  %c262 = zext i8 %v1219 to i64
  %v1220 = icmp ne i64 %c262, 0
  br i1 %v1220, label %b858, label %b857
b853:
  %v1217 = icmp eq i64 %v1214, %v6
  br i1 %v1217, label %b855, label %b852
b854:
  %gcvolta854 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta854, ptr %ctxtopo, align 8
  ret i64 0
b855:
  %v1263 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1264 = call i64 @df.box_int(i64 %v1263)
  store i64 %v1264, ptr %gcs1
  %v1265 = icmp eq i64 %v0, 0
  %v1266 = icmp eq i64 %v1264, 0
  %c263 = zext i1 %v1265 to i64
  %c264 = zext i1 %v1266 to i64
  %v1267 = or i64 %c263, %c264
  %v1268 = icmp ne i64 %v1267, 0
  br i1 %v1268, label %b902, label %b903
b856:
  store i64 0, ptr %gcs1
  %v1222 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1222, ptr %gcs1
  %v1223 = load i8, ptr %ctx, align 8
  %c265 = zext i8 %v1223 to i64
  %v1224 = icmp ne i64 %c265, 0
  br i1 %v1224, label %b862, label %b861
b857:
  %v1221 = icmp eq i64 %v1218, %v6
  br i1 %v1221, label %b859, label %b856
b858:
  %gcvolta858 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta858, ptr %ctxtopo, align 8
  ret i64 0
b859:
  %v1295 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1296 = and i64 %v0, 1
  %v1297 = icmp ne i64 %v1296, 0
  br i1 %v1297, label %b924, label %b925
b860:
  store i64 0, ptr %gcs1
  %v1226 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1226, ptr %gcs1
  %v1227 = load i8, ptr %ctx, align 8
  %c266 = zext i8 %v1227 to i64
  %v1228 = icmp ne i64 %c266, 0
  br i1 %v1228, label %b866, label %b865
b861:
  %v1225 = icmp eq i64 %v1222, %v6
  br i1 %v1225, label %b863, label %b860
b862:
  %gcvolta862 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta862, ptr %ctxtopo, align 8
  ret i64 0
b863:
  %v1334 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1335 = and i64 %v0, 1
  %v1336 = icmp ne i64 %v1335, 0
  br i1 %v1336, label %b944, label %b945
b864:
  store i64 0, ptr %gcs1
  %v1230 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1230, ptr %gcs1
  %v1231 = load i8, ptr %ctx, align 8
  %c267 = zext i8 %v1231 to i64
  %v1232 = icmp ne i64 %c267, 0
  br i1 %v1232, label %b870, label %b869
b865:
  %v1229 = icmp eq i64 %v1226, %v6
  br i1 %v1229, label %b867, label %b864
b866:
  %gcvolta866 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta866, ptr %ctxtopo, align 8
  ret i64 0
b867:
  %v1373 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1374 = and i64 %v0, 1
  %v1375 = icmp ne i64 %v1374, 0
  br i1 %v1375, label %b964, label %b965
b868:
  store i64 0, ptr %gcs1
  %v1234 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1234, ptr %gcs1
  %v1235 = load i8, ptr %ctx, align 8
  %c268 = zext i8 %v1235 to i64
  %v1236 = icmp ne i64 %c268, 0
  br i1 %v1236, label %b874, label %b873
b869:
  %v1233 = icmp eq i64 %v1230, %v6
  br i1 %v1233, label %b871, label %b868
b870:
  %gcvolta870 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta870, ptr %ctxtopo, align 8
  ret i64 0
b871:
  %v1412 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1413 = and i64 %v0, 1
  %v1414 = icmp ne i64 %v1413, 0
  br i1 %v1414, label %b984, label %b985
b872:
  store i64 0, ptr %gcs1
  %v1238 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1238, ptr %gcs1
  %v1239 = load i8, ptr %ctx, align 8
  %c269 = zext i8 %v1239 to i64
  %v1240 = icmp ne i64 %c269, 0
  br i1 %v1240, label %b878, label %b877
b873:
  %v1237 = icmp eq i64 %v1234, %v6
  br i1 %v1237, label %b875, label %b872
b874:
  %gcvolta874 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta874, ptr %ctxtopo, align 8
  ret i64 0
b875:
  %v1451 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1452 = and i64 %v0, 1
  %v1453 = icmp ne i64 %v1452, 0
  br i1 %v1453, label %b1004, label %b1005
b876:
  store i64 0, ptr %gcs1
  %v1242 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1242, ptr %gcs1
  %v1243 = load i8, ptr %ctx, align 8
  %c270 = zext i8 %v1243 to i64
  %v1244 = icmp ne i64 %c270, 0
  br i1 %v1244, label %b882, label %b881
b877:
  %v1241 = icmp eq i64 %v1238, %v6
  br i1 %v1241, label %b879, label %b876
b878:
  %gcvolta878 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta878, ptr %ctxtopo, align 8
  ret i64 0
b879:
  %xcp1490 = load i8, ptr %ctx, align 8
  %xcn1490 = icmp ne i8 %xcp1490, 0
  %xce1490 = call i1 @llvm.expect.i1(i1 %xcn1490, i1 false)
  br i1 %xce1490, label %xc1490.limpar, label %xc1490.fim
xc1490.limpar:
  call void @dartforge_exception_clear()
  br label %xc1490.fim
xc1490.fim:
  %gcvolta879 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta879, ptr %ctxtopo, align 8
  ret i64 %v0
b880:
  store i64 0, ptr %gcs1
  %v1246 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1246, ptr %gcs1
  %v1247 = load i8, ptr %ctx, align 8
  %c271 = zext i8 %v1247 to i64
  %v1248 = icmp ne i64 %c271, 0
  br i1 %v1248, label %b886, label %b885
b881:
  %v1245 = icmp eq i64 %v1242, %v6
  br i1 %v1245, label %b883, label %b880
b882:
  %gcvolta882 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta882, ptr %ctxtopo, align 8
  ret i64 0
b883:
  %v1492 = add i64 0, 258
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1493 = and i64 %v0, 1
  %v1494 = icmp ne i64 %v1493, 0
  br i1 %v1494, label %b1025, label %b1026
b884:
  store i64 0, ptr %gcs1
  %v1250 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1250, ptr %gcs1
  %v1251 = load i8, ptr %ctx, align 8
  %c272 = zext i8 %v1251 to i64
  %v1252 = icmp ne i64 %c272, 0
  br i1 %v1252, label %b890, label %b889
b885:
  %v1249 = icmp eq i64 %v1246, %v6
  br i1 %v1249, label %b887, label %b884
b886:
  %gcvolta886 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta886, ptr %ctxtopo, align 8
  ret i64 0
b887:
  %v1505 = add i64 0, 16909320
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1506 = and i64 %v0, 1
  %v1507 = icmp ne i64 %v1506, 0
  br i1 %v1507, label %b1031, label %b1032
b888:
  store i64 0, ptr %gcs1
  %v1254 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1254, ptr %gcs1
  %v1255 = load i8, ptr %ctx, align 8
  %c273 = zext i8 %v1255 to i64
  %v1256 = icmp ne i64 %c273, 0
  br i1 %v1256, label %b895, label %b894
b889:
  %v1253 = icmp eq i64 %v1250, %v6
  br i1 %v1253, label %b891, label %b888
b890:
  %gcvolta890 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta890, ptr %ctxtopo, align 8
  ret i64 0
b891:
  br label %b851
b893:
  store i64 0, ptr %gcs1
  %v1258 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1258, ptr %gcs1
  %v1259 = load i8, ptr %ctx, align 8
  %c274 = zext i8 %v1259 to i64
  %v1260 = icmp ne i64 %c274, 0
  br i1 %v1260, label %b900, label %b899
b894:
  %v1257 = icmp eq i64 %v1254, %v6
  br i1 %v1257, label %b896, label %b893
b895:
  %gcvolta895 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta895, ptr %ctxtopo, align 8
  ret i64 0
b896:
  br label %b851
b898:
  br label %b12
b899:
  %v1261 = icmp eq i64 %v1258, %v6
  br i1 %v1261, label %b901, label %b898
b900:
  %gcvolta900 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta900, ptr %ctxtopo, align 8
  ret i64 0
b901:
  br label %b851
b902:
  %v1269 = icmp eq i64 %v0, %v1264
  br label %b904
b903:
  %v1270 = call i64 @df.class(i64 %v0)
  switch i64 %v1270, label %b908 [ i64 675, label %b905 i64 4, label %b906 i64 2, label %b907 i64 3, label %b907 ]
b904:
  %v1288 = phi i1 [ %v1269, %b902 ], [ %v1287, %b909 ]
  br i1 %v1288, label %b920, label %b921
b905:
  %v1271 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1264)
  %v1272 = load i8, ptr %ctx, align 8
  %c275 = zext i8 %v1272 to i64
  %v1273 = icmp ne i64 %c275, 0
  br i1 %v1273, label %b911, label %b910
b906:
  %v1275 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1264)
  %v1276 = load i8, ptr %ctx, align 8
  %c276 = zext i8 %v1276 to i64
  %v1277 = icmp ne i64 %c276, 0
  br i1 %v1277, label %b913, label %b912
b907:
  %v1278 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1264)
  %v1279 = load i8, ptr %ctx, align 8
  %c277 = zext i8 %v1279 to i64
  %v1280 = icmp ne i64 %c277, 0
  br i1 %v1280, label %b915, label %b914
b908:
  %sa1281_0 = getelementptr [2 x i64], ptr %sargs1281, i64 0, i64 0
  store i64 %v1264, ptr %sa1281_0
  %sat1281 = getelementptr [2 x i64], ptr %sargs1281, i64 0, i64 1
  store i64 0, ptr %sat1281
  %sic1281 = getelementptr i64, ptr %area, i64 276
  %sf1281 = call ptr @df.selector(ptr %sic1281, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v1281 = call i64 %sf1281(i64 %v0, ptr %sargs1281, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1281, ptr %gcs0
  %v1282 = load i8, ptr %ctx, align 8
  %c278 = zext i8 %v1282 to i64
  %v1283 = icmp ne i64 %c278, 0
  br i1 %v1283, label %b917, label %b916
b909:
  %v1287 = phi i1 [ %v1274, %b910 ], [ %v1275, %b912 ], [ %v1278, %b914 ], [ %v1284, %b918 ]
  br label %b904
b910:
  %c279 = zext i8 %v1271 to i64
  %v1274 = icmp ne i64 %c279, 0
  br label %b909
b911:
  %gcvolta911 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta911, ptr %ctxtopo, align 8
  ret i64 0
b912:
  br label %b909
b913:
  %gcvolta913 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta913, ptr %ctxtopo, align 8
  ret i64 0
b914:
  br label %b909
b915:
  %gcvolta915 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta915, ptr %ctxtopo, align 8
  ret i64 0
b916:
  store i64 0, ptr %gcs1
  %v1284 = call i1 @df.unbox_bool(i64 %v1281)
  %v1285 = load i8, ptr %ctx, align 8
  %c280 = zext i8 %v1285 to i64
  %v1286 = icmp ne i64 %c280, 0
  br i1 %v1286, label %b919, label %b918
b917:
  %gcvolta917 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta917, ptr %ctxtopo, align 8
  ret i64 0
b918:
  br label %b909
b919:
  %gcvolta919 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta919, ptr %ctxtopo, align 8
  ret i64 0
b920:
  %v1289 = add i64 0, 0
  br label %b922
b921:
  %v1290 = add i64 0, 1
  br label %b922
b922:
  %v1291 = phi i64 [ %v1289, %b920 ], [ %v1290, %b921 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1292 = call i64 @df.box_int(i64 %v1291)
  %xcp1293 = load i8, ptr %ctx, align 8
  %xcn1293 = icmp ne i8 %xcp1293, 0
  %xce1293 = call i1 @llvm.expect.i1(i1 %xcn1293, i1 false)
  br i1 %xce1293, label %xc1293.limpar, label %xc1293.fim
xc1293.limpar:
  call void @dartforge_exception_clear()
  br label %xc1293.fim
xc1293.fim:
  %gcvolta922 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta922, ptr %ctxtopo, align 8
  ret i64 %v1292
b924:
  %v1298 = ashr i64 %v0, 1
  %v1299 = icmp sle i64 %v1298, %v1295
  br label %b926
b925:
  %v1300 = call i64 @df.box_int(i64 %v1295)
  store i64 %v1300, ptr %gcs1
  %sa1301_0 = getelementptr [2 x i64], ptr %sargs1301, i64 0, i64 0
  store i64 %v1300, ptr %sa1301_0
  %sat1301 = getelementptr [2 x i64], ptr %sargs1301, i64 0, i64 1
  store i64 0, ptr %sat1301
  %sic1301 = getelementptr i64, ptr %area, i64 278
  %sf1301 = call ptr @df.selector(ptr %sic1301, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1301 = call i64 %sf1301(i64 %v0, ptr %sargs1301, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1301, ptr %gcs1
  %v1302 = load i8, ptr %ctx, align 8
  %c281 = zext i8 %v1302 to i64
  %v1303 = icmp ne i64 %c281, 0
  br i1 %v1303, label %b928, label %b927
b926:
  %v1307 = phi i1 [ %v1299, %b924 ], [ %v1304, %b929 ]
  br i1 %v1307, label %b931, label %b932
b927:
  %v1304 = call i1 @df.unbox_bool(i64 %v1301)
  %v1305 = load i8, ptr %ctx, align 8
  %c282 = zext i8 %v1305 to i64
  %v1306 = icmp ne i64 %c282, 0
  br i1 %v1306, label %b930, label %b929
b928:
  %gcvolta928 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta928, ptr %ctxtopo, align 8
  ret i64 0
b929:
  br label %b926
b930:
  %gcvolta930 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta930, ptr %ctxtopo, align 8
  ret i64 0
b931:
  %v1308 = add i64 0, 0
  br label %b933
b932:
  store i64 0, ptr %gcs1
  %v1310 = icmp eq i64 %v0, 0
  %v1311 = call i64 @df.class(i64 %v0)
  %v1312 = call i8 @df.subclass(i64 %v1311, i64 674)
  %c283 = zext i8 %v1312 to i64
  %v1313 = icmp ne i64 %c283, 0
  %v1314 = xor i1 %v1310, true
  %c284 = zext i1 %v1313 to i64
  %c285 = zext i1 %v1314 to i64
  %v1315 = and i64 %c284, %c285
  %v1316 = icmp ne i64 %v1315, 0
  br i1 %v1316, label %b935, label %b934
b933:
  %v1330 = phi i64 [ %v1308, %b931 ], [ %v1329, %b941 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1331 = call i64 @df.box_int(i64 %v1330)
  %xcp1332 = load i8, ptr %ctx, align 8
  %xcn1332 = icmp ne i8 %xcp1332, 0
  %xce1332 = call i1 @llvm.expect.i1(i1 %xcn1332, i1 false)
  br i1 %xce1332, label %xc1332.limpar, label %xc1332.fim
xc1332.limpar:
  call void @dartforge_exception_clear()
  br label %xc1332.fim
xc1332.fim:
  %gcvolta933 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta933, ptr %ctxtopo, align 8
  ret i64 %v1331
b934:
  %ga1317 = getelementptr i64, ptr %area, i64 13
  %v1317 = load i8, ptr %ga1317
  %c286 = zext i8 %v1317 to i64
  %v1318 = icmp ne i64 %c286, 0
  br i1 %v1318, label %b936, label %b937
b935:
  %v1325 = call i64 @df.unbox_int(i64 %v0)
  %v1326 = load i8, ptr %ctx, align 8
  %c287 = zext i8 %v1326 to i64
  %v1327 = icmp ne i64 %c287, 0
  br i1 %v1327, label %b942, label %b941
b936:
  %ga1319 = getelementptr i64, ptr %area, i64 12
  %v1319 = load i64, ptr %ga1319
  br label %b938
b937:
  %v1320 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b938
b938:
  %v1321 = phi i64 [ %v1319, %b936 ], [ %v1320, %b937 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1321, i64 0, i64 0)
  %v1323 = load i8, ptr %ctx, align 8
  %c288 = zext i8 %v1323 to i64
  %v1324 = icmp ne i64 %c288, 0
  br i1 %v1324, label %b940, label %b939
b939:
  br label %b935
b940:
  %gcvolta940 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta940, ptr %ctxtopo, align 8
  ret i64 0
b941:
  %v1328 = add i64 0, 5
  %v1329 = ashr i64 %v1325, %v1328
  br label %b933
b942:
  %gcvolta942 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta942, ptr %ctxtopo, align 8
  ret i64 0
b944:
  %v1337 = ashr i64 %v0, 1
  %v1338 = icmp sle i64 %v1337, %v1334
  br label %b946
b945:
  %v1339 = call i64 @df.box_int(i64 %v1334)
  store i64 %v1339, ptr %gcs1
  %sa1340_0 = getelementptr [2 x i64], ptr %sargs1340, i64 0, i64 0
  store i64 %v1339, ptr %sa1340_0
  %sat1340 = getelementptr [2 x i64], ptr %sargs1340, i64 0, i64 1
  store i64 0, ptr %sat1340
  %sic1340 = getelementptr i64, ptr %area, i64 280
  %sf1340 = call ptr @df.selector(ptr %sic1340, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1340 = call i64 %sf1340(i64 %v0, ptr %sargs1340, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1340, ptr %gcs1
  %v1341 = load i8, ptr %ctx, align 8
  %c289 = zext i8 %v1341 to i64
  %v1342 = icmp ne i64 %c289, 0
  br i1 %v1342, label %b948, label %b947
b946:
  %v1346 = phi i1 [ %v1338, %b944 ], [ %v1343, %b949 ]
  br i1 %v1346, label %b951, label %b952
b947:
  %v1343 = call i1 @df.unbox_bool(i64 %v1340)
  %v1344 = load i8, ptr %ctx, align 8
  %c290 = zext i8 %v1344 to i64
  %v1345 = icmp ne i64 %c290, 0
  br i1 %v1345, label %b950, label %b949
b948:
  %gcvolta948 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta948, ptr %ctxtopo, align 8
  ret i64 0
b949:
  br label %b946
b950:
  %gcvolta950 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta950, ptr %ctxtopo, align 8
  ret i64 0
b951:
  %v1347 = add i64 0, 0
  br label %b953
b952:
  store i64 0, ptr %gcs1
  %v1349 = icmp eq i64 %v0, 0
  %v1350 = call i64 @df.class(i64 %v0)
  %v1351 = call i8 @df.subclass(i64 %v1350, i64 674)
  %c291 = zext i8 %v1351 to i64
  %v1352 = icmp ne i64 %c291, 0
  %v1353 = xor i1 %v1349, true
  %c292 = zext i1 %v1352 to i64
  %c293 = zext i1 %v1353 to i64
  %v1354 = and i64 %c292, %c293
  %v1355 = icmp ne i64 %v1354, 0
  br i1 %v1355, label %b955, label %b954
b953:
  %v1369 = phi i64 [ %v1347, %b951 ], [ %v1368, %b961 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1370 = call i64 @df.box_int(i64 %v1369)
  %xcp1371 = load i8, ptr %ctx, align 8
  %xcn1371 = icmp ne i8 %xcp1371, 0
  %xce1371 = call i1 @llvm.expect.i1(i1 %xcn1371, i1 false)
  br i1 %xce1371, label %xc1371.limpar, label %xc1371.fim
xc1371.limpar:
  call void @dartforge_exception_clear()
  br label %xc1371.fim
xc1371.fim:
  %gcvolta953 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta953, ptr %ctxtopo, align 8
  ret i64 %v1370
b954:
  %ga1356 = getelementptr i64, ptr %area, i64 13
  %v1356 = load i8, ptr %ga1356
  %c294 = zext i8 %v1356 to i64
  %v1357 = icmp ne i64 %c294, 0
  br i1 %v1357, label %b956, label %b957
b955:
  %v1364 = call i64 @df.unbox_int(i64 %v0)
  %v1365 = load i8, ptr %ctx, align 8
  %c295 = zext i8 %v1365 to i64
  %v1366 = icmp ne i64 %c295, 0
  br i1 %v1366, label %b962, label %b961
b956:
  %ga1358 = getelementptr i64, ptr %area, i64 12
  %v1358 = load i64, ptr %ga1358
  br label %b958
b957:
  %v1359 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b958
b958:
  %v1360 = phi i64 [ %v1358, %b956 ], [ %v1359, %b957 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1360, i64 0, i64 0)
  %v1362 = load i8, ptr %ctx, align 8
  %c296 = zext i8 %v1362 to i64
  %v1363 = icmp ne i64 %c296, 0
  br i1 %v1363, label %b960, label %b959
b959:
  br label %b955
b960:
  %gcvolta960 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta960, ptr %ctxtopo, align 8
  ret i64 0
b961:
  %v1367 = add i64 0, 3
  %v1368 = ashr i64 %v1364, %v1367
  br label %b953
b962:
  %gcvolta962 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta962, ptr %ctxtopo, align 8
  ret i64 0
b964:
  %v1376 = ashr i64 %v0, 1
  %v1377 = icmp sle i64 %v1376, %v1373
  br label %b966
b965:
  %v1378 = call i64 @df.box_int(i64 %v1373)
  store i64 %v1378, ptr %gcs1
  %sa1379_0 = getelementptr [2 x i64], ptr %sargs1379, i64 0, i64 0
  store i64 %v1378, ptr %sa1379_0
  %sat1379 = getelementptr [2 x i64], ptr %sargs1379, i64 0, i64 1
  store i64 0, ptr %sat1379
  %sic1379 = getelementptr i64, ptr %area, i64 282
  %sf1379 = call ptr @df.selector(ptr %sic1379, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1379 = call i64 %sf1379(i64 %v0, ptr %sargs1379, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1379, ptr %gcs1
  %v1380 = load i8, ptr %ctx, align 8
  %c297 = zext i8 %v1380 to i64
  %v1381 = icmp ne i64 %c297, 0
  br i1 %v1381, label %b968, label %b967
b966:
  %v1385 = phi i1 [ %v1377, %b964 ], [ %v1382, %b969 ]
  br i1 %v1385, label %b971, label %b972
b967:
  %v1382 = call i1 @df.unbox_bool(i64 %v1379)
  %v1383 = load i8, ptr %ctx, align 8
  %c298 = zext i8 %v1383 to i64
  %v1384 = icmp ne i64 %c298, 0
  br i1 %v1384, label %b970, label %b969
b968:
  %gcvolta968 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta968, ptr %ctxtopo, align 8
  ret i64 0
b969:
  br label %b966
b970:
  %gcvolta970 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta970, ptr %ctxtopo, align 8
  ret i64 0
b971:
  %v1386 = add i64 0, 0
  br label %b973
b972:
  store i64 0, ptr %gcs1
  %v1388 = icmp eq i64 %v0, 0
  %v1389 = call i64 @df.class(i64 %v0)
  %v1390 = call i8 @df.subclass(i64 %v1389, i64 674)
  %c299 = zext i8 %v1390 to i64
  %v1391 = icmp ne i64 %c299, 0
  %v1392 = xor i1 %v1388, true
  %c300 = zext i1 %v1391 to i64
  %c301 = zext i1 %v1392 to i64
  %v1393 = and i64 %c300, %c301
  %v1394 = icmp ne i64 %v1393, 0
  br i1 %v1394, label %b975, label %b974
b973:
  %v1408 = phi i64 [ %v1386, %b971 ], [ %v1407, %b981 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1409 = call i64 @df.box_int(i64 %v1408)
  %xcp1410 = load i8, ptr %ctx, align 8
  %xcn1410 = icmp ne i8 %xcp1410, 0
  %xce1410 = call i1 @llvm.expect.i1(i1 %xcn1410, i1 false)
  br i1 %xce1410, label %xc1410.limpar, label %xc1410.fim
xc1410.limpar:
  call void @dartforge_exception_clear()
  br label %xc1410.fim
xc1410.fim:
  %gcvolta973 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta973, ptr %ctxtopo, align 8
  ret i64 %v1409
b974:
  %ga1395 = getelementptr i64, ptr %area, i64 13
  %v1395 = load i8, ptr %ga1395
  %c302 = zext i8 %v1395 to i64
  %v1396 = icmp ne i64 %c302, 0
  br i1 %v1396, label %b976, label %b977
b975:
  %v1403 = call i64 @df.unbox_int(i64 %v0)
  %v1404 = load i8, ptr %ctx, align 8
  %c303 = zext i8 %v1404 to i64
  %v1405 = icmp ne i64 %c303, 0
  br i1 %v1405, label %b982, label %b981
b976:
  %ga1397 = getelementptr i64, ptr %area, i64 12
  %v1397 = load i64, ptr %ga1397
  br label %b978
b977:
  %v1398 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b978
b978:
  %v1399 = phi i64 [ %v1397, %b976 ], [ %v1398, %b977 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1399, i64 0, i64 0)
  %v1401 = load i8, ptr %ctx, align 8
  %c304 = zext i8 %v1401 to i64
  %v1402 = icmp ne i64 %c304, 0
  br i1 %v1402, label %b980, label %b979
b979:
  br label %b975
b980:
  %gcvolta980 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta980, ptr %ctxtopo, align 8
  ret i64 0
b981:
  %v1406 = add i64 0, 1
  %v1407 = shl i64 %v1403, %v1406
  br label %b973
b982:
  %gcvolta982 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta982, ptr %ctxtopo, align 8
  ret i64 0
b984:
  %v1415 = ashr i64 %v0, 1
  %v1416 = icmp sle i64 %v1415, %v1412
  br label %b986
b985:
  %v1417 = call i64 @df.box_int(i64 %v1412)
  store i64 %v1417, ptr %gcs1
  %sa1418_0 = getelementptr [2 x i64], ptr %sargs1418, i64 0, i64 0
  store i64 %v1417, ptr %sa1418_0
  %sat1418 = getelementptr [2 x i64], ptr %sargs1418, i64 0, i64 1
  store i64 0, ptr %sat1418
  %sic1418 = getelementptr i64, ptr %area, i64 284
  %sf1418 = call ptr @df.selector(ptr %sic1418, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1418 = call i64 %sf1418(i64 %v0, ptr %sargs1418, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1418, ptr %gcs1
  %v1419 = load i8, ptr %ctx, align 8
  %c305 = zext i8 %v1419 to i64
  %v1420 = icmp ne i64 %c305, 0
  br i1 %v1420, label %b988, label %b987
b986:
  %v1424 = phi i1 [ %v1416, %b984 ], [ %v1421, %b989 ]
  br i1 %v1424, label %b991, label %b992
b987:
  %v1421 = call i1 @df.unbox_bool(i64 %v1418)
  %v1422 = load i8, ptr %ctx, align 8
  %c306 = zext i8 %v1422 to i64
  %v1423 = icmp ne i64 %c306, 0
  br i1 %v1423, label %b990, label %b989
b988:
  %gcvolta988 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta988, ptr %ctxtopo, align 8
  ret i64 0
b989:
  br label %b986
b990:
  %gcvolta990 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta990, ptr %ctxtopo, align 8
  ret i64 0
b991:
  %v1425 = add i64 0, 0
  br label %b993
b992:
  store i64 0, ptr %gcs1
  %v1427 = icmp eq i64 %v0, 0
  %v1428 = call i64 @df.class(i64 %v0)
  %v1429 = call i8 @df.subclass(i64 %v1428, i64 674)
  %c307 = zext i8 %v1429 to i64
  %v1430 = icmp ne i64 %c307, 0
  %v1431 = xor i1 %v1427, true
  %c308 = zext i1 %v1430 to i64
  %c309 = zext i1 %v1431 to i64
  %v1432 = and i64 %c308, %c309
  %v1433 = icmp ne i64 %v1432, 0
  br i1 %v1433, label %b995, label %b994
b993:
  %v1447 = phi i64 [ %v1425, %b991 ], [ %v1446, %b1001 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1448 = call i64 @df.box_int(i64 %v1447)
  %xcp1449 = load i8, ptr %ctx, align 8
  %xcn1449 = icmp ne i8 %xcp1449, 0
  %xce1449 = call i1 @llvm.expect.i1(i1 %xcn1449, i1 false)
  br i1 %xce1449, label %xc1449.limpar, label %xc1449.fim
xc1449.limpar:
  call void @dartforge_exception_clear()
  br label %xc1449.fim
xc1449.fim:
  %gcvolta993 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta993, ptr %ctxtopo, align 8
  ret i64 %v1448
b994:
  %ga1434 = getelementptr i64, ptr %area, i64 13
  %v1434 = load i8, ptr %ga1434
  %c310 = zext i8 %v1434 to i64
  %v1435 = icmp ne i64 %c310, 0
  br i1 %v1435, label %b996, label %b997
b995:
  %v1442 = call i64 @df.unbox_int(i64 %v0)
  %v1443 = load i8, ptr %ctx, align 8
  %c311 = zext i8 %v1443 to i64
  %v1444 = icmp ne i64 %c311, 0
  br i1 %v1444, label %b1002, label %b1001
b996:
  %ga1436 = getelementptr i64, ptr %area, i64 12
  %v1436 = load i64, ptr %ga1436
  br label %b998
b997:
  %v1437 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b998
b998:
  %v1438 = phi i64 [ %v1436, %b996 ], [ %v1437, %b997 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1438, i64 0, i64 0)
  %v1440 = load i8, ptr %ctx, align 8
  %c312 = zext i8 %v1440 to i64
  %v1441 = icmp ne i64 %c312, 0
  br i1 %v1441, label %b1000, label %b999
b999:
  br label %b995
b1000:
  %gcvolta1000 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1000, ptr %ctxtopo, align 8
  ret i64 0
b1001:
  %v1445 = add i64 0, 516
  %v1446 = mul i64 %v1442, %v1445
  br label %b993
b1002:
  %gcvolta1002 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1002, ptr %ctxtopo, align 8
  ret i64 0
b1004:
  %v1454 = ashr i64 %v0, 1
  %v1455 = icmp sle i64 %v1454, %v1451
  br label %b1006
b1005:
  %v1456 = call i64 @df.box_int(i64 %v1451)
  store i64 %v1456, ptr %gcs1
  %sa1457_0 = getelementptr [2 x i64], ptr %sargs1457, i64 0, i64 0
  store i64 %v1456, ptr %sa1457_0
  %sat1457 = getelementptr [2 x i64], ptr %sargs1457, i64 0, i64 1
  store i64 0, ptr %sat1457
  %sic1457 = getelementptr i64, ptr %area, i64 286
  %sf1457 = call ptr @df.selector(ptr %sic1457, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1457 = call i64 %sf1457(i64 %v0, ptr %sargs1457, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1457, ptr %gcs1
  %v1458 = load i8, ptr %ctx, align 8
  %c313 = zext i8 %v1458 to i64
  %v1459 = icmp ne i64 %c313, 0
  br i1 %v1459, label %b1008, label %b1007
b1006:
  %v1463 = phi i1 [ %v1455, %b1004 ], [ %v1460, %b1009 ]
  br i1 %v1463, label %b1011, label %b1012
b1007:
  %v1460 = call i1 @df.unbox_bool(i64 %v1457)
  %v1461 = load i8, ptr %ctx, align 8
  %c314 = zext i8 %v1461 to i64
  %v1462 = icmp ne i64 %c314, 0
  br i1 %v1462, label %b1010, label %b1009
b1008:
  %gcvolta1008 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1008, ptr %ctxtopo, align 8
  ret i64 0
b1009:
  br label %b1006
b1010:
  %gcvolta1010 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1010, ptr %ctxtopo, align 8
  ret i64 0
b1011:
  %v1464 = add i64 0, 0
  br label %b1013
b1012:
  store i64 0, ptr %gcs1
  %v1466 = icmp eq i64 %v0, 0
  %v1467 = call i64 @df.class(i64 %v0)
  %v1468 = call i8 @df.subclass(i64 %v1467, i64 674)
  %c315 = zext i8 %v1468 to i64
  %v1469 = icmp ne i64 %c315, 0
  %v1470 = xor i1 %v1466, true
  %c316 = zext i1 %v1469 to i64
  %c317 = zext i1 %v1470 to i64
  %v1471 = and i64 %c316, %c317
  %v1472 = icmp ne i64 %v1471, 0
  br i1 %v1472, label %b1015, label %b1014
b1013:
  %v1486 = phi i64 [ %v1464, %b1011 ], [ %v1485, %b1021 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1487 = call i64 @df.box_int(i64 %v1486)
  %xcp1488 = load i8, ptr %ctx, align 8
  %xcn1488 = icmp ne i8 %xcp1488, 0
  %xce1488 = call i1 @llvm.expect.i1(i1 %xcn1488, i1 false)
  br i1 %xce1488, label %xc1488.limpar, label %xc1488.fim
xc1488.limpar:
  call void @dartforge_exception_clear()
  br label %xc1488.fim
xc1488.fim:
  %gcvolta1013 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1013, ptr %ctxtopo, align 8
  ret i64 %v1487
b1014:
  %ga1473 = getelementptr i64, ptr %area, i64 13
  %v1473 = load i8, ptr %ga1473
  %c318 = zext i8 %v1473 to i64
  %v1474 = icmp ne i64 %c318, 0
  br i1 %v1474, label %b1016, label %b1017
b1015:
  %v1481 = call i64 @df.unbox_int(i64 %v0)
  %v1482 = load i8, ptr %ctx, align 8
  %c319 = zext i8 %v1482 to i64
  %v1483 = icmp ne i64 %c319, 0
  br i1 %v1483, label %b1022, label %b1021
b1016:
  %ga1475 = getelementptr i64, ptr %area, i64 12
  %v1475 = load i64, ptr %ga1475
  br label %b1018
b1017:
  %v1476 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1018
b1018:
  %v1477 = phi i64 [ %v1475, %b1016 ], [ %v1476, %b1017 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1477, i64 0, i64 0)
  %v1479 = load i8, ptr %ctx, align 8
  %c320 = zext i8 %v1479 to i64
  %v1480 = icmp ne i64 %c320, 0
  br i1 %v1480, label %b1020, label %b1019
b1019:
  br label %b1015
b1020:
  %gcvolta1020 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1020, ptr %ctxtopo, align 8
  ret i64 0
b1021:
  %v1484 = add i64 0, 33818640
  %v1485 = mul i64 %v1481, %v1484
  br label %b1013
b1022:
  %gcvolta1022 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1022, ptr %ctxtopo, align 8
  ret i64 0
b1025:
  %v1495 = ashr i64 %v0, 1
  %v1496 = mul i64 %v1495, %v1492
  store i64 0, ptr %gcs0
  %v1497 = call i64 @df.box_int(i64 %v1496)
  br label %b1027
b1026:
  %v1498 = call i64 @df.box_int(i64 %v1492)
  store i64 %v1498, ptr %gcs1
  %sa1499_0 = getelementptr [2 x i64], ptr %sargs1499, i64 0, i64 0
  store i64 %v1498, ptr %sa1499_0
  %sat1499 = getelementptr [2 x i64], ptr %sargs1499, i64 0, i64 1
  store i64 0, ptr %sat1499
  %sic1499 = getelementptr i64, ptr %area, i64 288
  %sf1499 = call ptr @df.selector(ptr %sic1499, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v1499 = call i64 %sf1499(i64 %v0, ptr %sargs1499, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1500 = load i8, ptr %ctx, align 8
  %c321 = zext i8 %v1500 to i64
  %v1501 = icmp ne i64 %c321, 0
  br i1 %v1501, label %b1029, label %b1028
b1027:
  %v1502 = phi i64 [ %v1497, %b1025 ], [ %v1499, %b1028 ]
  %xcp1503 = load i8, ptr %ctx, align 8
  %xcn1503 = icmp ne i8 %xcp1503, 0
  %xce1503 = call i1 @llvm.expect.i1(i1 %xcn1503, i1 false)
  br i1 %xce1503, label %xc1503.limpar, label %xc1503.fim
xc1503.limpar:
  call void @dartforge_exception_clear()
  br label %xc1503.fim
xc1503.fim:
  %gcvolta1027 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1027, ptr %ctxtopo, align 8
  ret i64 %v1502
b1028:
  br label %b1027
b1029:
  %gcvolta1029 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1029, ptr %ctxtopo, align 8
  ret i64 0
b1031:
  %v1508 = ashr i64 %v0, 1
  %v1509 = mul i64 %v1508, %v1505
  store i64 0, ptr %gcs0
  %v1510 = call i64 @df.box_int(i64 %v1509)
  br label %b1033
b1032:
  %v1511 = call i64 @df.box_int(i64 %v1505)
  store i64 %v1511, ptr %gcs1
  %sa1512_0 = getelementptr [2 x i64], ptr %sargs1512, i64 0, i64 0
  store i64 %v1511, ptr %sa1512_0
  %sat1512 = getelementptr [2 x i64], ptr %sargs1512, i64 0, i64 1
  store i64 0, ptr %sat1512
  %sic1512 = getelementptr i64, ptr %area, i64 290
  %sf1512 = call ptr @df.selector(ptr %sic1512, i64 %v0, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v1512 = call i64 %sf1512(i64 %v0, ptr %sargs1512, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1513 = load i8, ptr %ctx, align 8
  %c322 = zext i8 %v1513 to i64
  %v1514 = icmp ne i64 %c322, 0
  br i1 %v1514, label %b1035, label %b1034
b1033:
  %v1515 = phi i64 [ %v1510, %b1031 ], [ %v1512, %b1034 ]
  %xcp1516 = load i8, ptr %ctx, align 8
  %xcn1516 = icmp ne i8 %xcp1516, 0
  %xce1516 = call i1 @llvm.expect.i1(i1 %xcn1516, i1 false)
  br i1 %xce1516, label %xc1516.limpar, label %xc1516.fim
xc1516.limpar:
  call void @dartforge_exception_clear()
  br label %xc1516.fim
xc1516.fim:
  %gcvolta1033 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1033, ptr %ctxtopo, align 8
  ret i64 %v1515
b1034:
  br label %b1033
b1035:
  %gcvolta1035 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1035, ptr %ctxtopo, align 8
  ret i64 0
b1037:
  store i64 0, ptr %gcs1
  %v1523 = call double @df.unbox_double(i64 %v1520)
  %v1524 = load i8, ptr %ctx, align 8
  %c323 = zext i8 %v1524 to i64
  %v1525 = icmp ne i64 %c323, 0
  br i1 %v1525, label %b1040, label %b1039
b1038:
  %gcvolta1038 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1038, ptr %ctxtopo, align 8
  ret i64 0
b1039:
  store i64 0, ptr %gcs0
  %v1526 = call i64 @df.box_double(double %v1523)
  %xcp1527 = load i8, ptr %ctx, align 8
  %xcn1527 = icmp ne i8 %xcp1527, 0
  %xce1527 = call i1 @llvm.expect.i1(i1 %xcn1527, i1 false)
  br i1 %xce1527, label %xc1527.limpar, label %xc1527.fim
xc1527.limpar:
  call void @dartforge_exception_clear()
  br label %xc1527.fim
xc1527.fim:
  %gcvolta1039 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1039, ptr %ctxtopo, align 8
  ret i64 %v1526
b1040:
  %gcvolta1040 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1040, ptr %ctxtopo, align 8
  ret i64 0
b1054:
  %v1853 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1854 = call i64 @df.box_int(i64 %v1853)
  store i64 %v1854, ptr %gcs1
  %sa1855_0 = getelementptr [2 x i64], ptr %sargs1855, i64 0, i64 0
  store i64 %v1854, ptr %sa1855_0
  %sat1855 = getelementptr [2 x i64], ptr %sargs1855, i64 0, i64 1
  store i64 0, ptr %sat1855
  %sic1855 = getelementptr i64, ptr %area, i64 292
  %sf1855 = call ptr @df.selector(ptr %sic1855, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v1855 = call i64 %sf1855(i64 %v0, ptr %sargs1855, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1855, ptr %gcs0
  %v1856 = load i8, ptr %ctx, align 8
  %c324 = zext i8 %v1856 to i64
  %v1857 = icmp ne i64 %c324, 0
  br i1 %v1857, label %b1249, label %b1248
b1055:
  store i64 0, ptr %gcs1
  %v1533 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1533, ptr %gcs1
  %v1534 = load i8, ptr %ctx, align 8
  %c325 = zext i8 %v1534 to i64
  %v1535 = icmp ne i64 %c325, 0
  br i1 %v1535, label %b1061, label %b1060
b1056:
  %v1532 = icmp eq i64 %v1529, %v6
  br i1 %v1532, label %b1058, label %b1055
b1057:
  %gcvolta1057 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1057, ptr %ctxtopo, align 8
  ret i64 0
b1058:
  %v1578 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1579 = call i64 @df.box_int(i64 %v1578)
  store i64 %v1579, ptr %gcs1
  %v1580 = icmp eq i64 %v0, 0
  %v1581 = icmp eq i64 %v1579, 0
  %c326 = zext i1 %v1580 to i64
  %c327 = zext i1 %v1581 to i64
  %v1582 = or i64 %c326, %c327
  %v1583 = icmp ne i64 %v1582, 0
  br i1 %v1583, label %b1105, label %b1106
b1059:
  store i64 0, ptr %gcs1
  %v1537 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1537, ptr %gcs1
  %v1538 = load i8, ptr %ctx, align 8
  %c328 = zext i8 %v1538 to i64
  %v1539 = icmp ne i64 %c328, 0
  br i1 %v1539, label %b1065, label %b1064
b1060:
  %v1536 = icmp eq i64 %v1533, %v6
  br i1 %v1536, label %b1062, label %b1059
b1061:
  %gcvolta1061 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1061, ptr %ctxtopo, align 8
  ret i64 0
b1062:
  %v1610 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1611 = and i64 %v0, 1
  %v1612 = icmp ne i64 %v1611, 0
  br i1 %v1612, label %b1127, label %b1128
b1063:
  store i64 0, ptr %gcs1
  %v1541 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1541, ptr %gcs1
  %v1542 = load i8, ptr %ctx, align 8
  %c329 = zext i8 %v1542 to i64
  %v1543 = icmp ne i64 %c329, 0
  br i1 %v1543, label %b1069, label %b1068
b1064:
  %v1540 = icmp eq i64 %v1537, %v6
  br i1 %v1540, label %b1066, label %b1063
b1065:
  %gcvolta1065 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1065, ptr %ctxtopo, align 8
  ret i64 0
b1066:
  %v1649 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1650 = and i64 %v0, 1
  %v1651 = icmp ne i64 %v1650, 0
  br i1 %v1651, label %b1147, label %b1148
b1067:
  store i64 0, ptr %gcs1
  %v1545 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1545, ptr %gcs1
  %v1546 = load i8, ptr %ctx, align 8
  %c330 = zext i8 %v1546 to i64
  %v1547 = icmp ne i64 %c330, 0
  br i1 %v1547, label %b1073, label %b1072
b1068:
  %v1544 = icmp eq i64 %v1541, %v6
  br i1 %v1544, label %b1070, label %b1067
b1069:
  %gcvolta1069 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1069, ptr %ctxtopo, align 8
  ret i64 0
b1070:
  %v1688 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1689 = and i64 %v0, 1
  %v1690 = icmp ne i64 %v1689, 0
  br i1 %v1690, label %b1167, label %b1168
b1071:
  store i64 0, ptr %gcs1
  %v1549 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1549, ptr %gcs1
  %v1550 = load i8, ptr %ctx, align 8
  %c331 = zext i8 %v1550 to i64
  %v1551 = icmp ne i64 %c331, 0
  br i1 %v1551, label %b1077, label %b1076
b1072:
  %v1548 = icmp eq i64 %v1545, %v6
  br i1 %v1548, label %b1074, label %b1071
b1073:
  %gcvolta1073 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1073, ptr %ctxtopo, align 8
  ret i64 0
b1074:
  %v1727 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1728 = and i64 %v0, 1
  %v1729 = icmp ne i64 %v1728, 0
  br i1 %v1729, label %b1187, label %b1188
b1075:
  store i64 0, ptr %gcs1
  %v1553 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1553, ptr %gcs1
  %v1554 = load i8, ptr %ctx, align 8
  %c332 = zext i8 %v1554 to i64
  %v1555 = icmp ne i64 %c332, 0
  br i1 %v1555, label %b1081, label %b1080
b1076:
  %v1552 = icmp eq i64 %v1549, %v6
  br i1 %v1552, label %b1078, label %b1075
b1077:
  %gcvolta1077 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1077, ptr %ctxtopo, align 8
  ret i64 0
b1078:
  %v1766 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1767 = and i64 %v0, 1
  %v1768 = icmp ne i64 %v1767, 0
  br i1 %v1768, label %b1207, label %b1208
b1079:
  store i64 0, ptr %gcs1
  %v1557 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1557, ptr %gcs1
  %v1558 = load i8, ptr %ctx, align 8
  %c333 = zext i8 %v1558 to i64
  %v1559 = icmp ne i64 %c333, 0
  br i1 %v1559, label %b1085, label %b1084
b1080:
  %v1556 = icmp eq i64 %v1553, %v6
  br i1 %v1556, label %b1082, label %b1079
b1081:
  %gcvolta1081 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1081, ptr %ctxtopo, align 8
  ret i64 0
b1082:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1805 = icmp eq i64 %v0, 0
  %v1806 = call i64 @df.class(i64 %v0)
  %v1807 = call i8 @df.subclass(i64 %v1806, i64 674)
  %c334 = zext i8 %v1807 to i64
  %v1808 = icmp ne i64 %c334, 0
  %v1809 = xor i1 %v1805, true
  %c335 = zext i1 %v1808 to i64
  %c336 = zext i1 %v1809 to i64
  %v1810 = and i64 %c335, %c336
  %v1811 = icmp ne i64 %v1810, 0
  br i1 %v1811, label %b1228, label %b1227
b1083:
  store i64 0, ptr %gcs1
  %v1561 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1561, ptr %gcs1
  %v1562 = load i8, ptr %ctx, align 8
  %c337 = zext i8 %v1562 to i64
  %v1563 = icmp ne i64 %c337, 0
  br i1 %v1563, label %b1089, label %b1088
b1084:
  %v1560 = icmp eq i64 %v1557, %v6
  br i1 %v1560, label %b1086, label %b1083
b1085:
  %gcvolta1085 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1085, ptr %ctxtopo, align 8
  ret i64 0
b1086:
  %xcp1828 = load i8, ptr %ctx, align 8
  %xcn1828 = icmp ne i8 %xcp1828, 0
  %xce1828 = call i1 @llvm.expect.i1(i1 %xcn1828, i1 false)
  br i1 %xce1828, label %xc1828.limpar, label %xc1828.fim
xc1828.limpar:
  call void @dartforge_exception_clear()
  br label %xc1828.fim
xc1828.fim:
  %gcvolta1086 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1086, ptr %ctxtopo, align 8
  ret i64 %v0
b1087:
  store i64 0, ptr %gcs1
  %v1565 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1565, ptr %gcs1
  %v1566 = load i8, ptr %ctx, align 8
  %c338 = zext i8 %v1566 to i64
  %v1567 = icmp ne i64 %c338, 0
  br i1 %v1567, label %b1093, label %b1092
b1088:
  %v1564 = icmp eq i64 %v1561, %v6
  br i1 %v1564, label %b1090, label %b1087
b1089:
  %gcvolta1089 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1089, ptr %ctxtopo, align 8
  ret i64 0
b1090:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1830 = icmp eq i64 %v0, 0
  %v1831 = call i64 @df.class(i64 %v0)
  %v1832 = call i8 @df.subclass(i64 %v1831, i64 674)
  %c339 = zext i8 %v1832 to i64
  %v1833 = icmp ne i64 %c339, 0
  %v1834 = xor i1 %v1830, true
  %c340 = zext i1 %v1833 to i64
  %c341 = zext i1 %v1834 to i64
  %v1835 = and i64 %c340, %c341
  %v1836 = icmp ne i64 %v1835, 0
  br i1 %v1836, label %b1239, label %b1238
b1091:
  store i64 0, ptr %gcs1
  %v1569 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1569, ptr %gcs1
  %v1570 = load i8, ptr %ctx, align 8
  %c342 = zext i8 %v1570 to i64
  %v1571 = icmp ne i64 %c342, 0
  br i1 %v1571, label %b1098, label %b1097
b1092:
  %v1568 = icmp eq i64 %v1565, %v6
  br i1 %v1568, label %b1094, label %b1091
b1093:
  %gcvolta1093 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1093, ptr %ctxtopo, align 8
  ret i64 0
b1094:
  br label %b1054
b1096:
  store i64 0, ptr %gcs1
  %v1573 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1573, ptr %gcs1
  %v1574 = load i8, ptr %ctx, align 8
  %c343 = zext i8 %v1574 to i64
  %v1575 = icmp ne i64 %c343, 0
  br i1 %v1575, label %b1103, label %b1102
b1097:
  %v1572 = icmp eq i64 %v1569, %v6
  br i1 %v1572, label %b1099, label %b1096
b1098:
  %gcvolta1098 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1098, ptr %ctxtopo, align 8
  ret i64 0
b1099:
  br label %b1054
b1101:
  br label %b12
b1102:
  %v1576 = icmp eq i64 %v1573, %v6
  br i1 %v1576, label %b1104, label %b1101
b1103:
  %gcvolta1103 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1103, ptr %ctxtopo, align 8
  ret i64 0
b1104:
  br label %b1054
b1105:
  %v1584 = icmp eq i64 %v0, %v1579
  br label %b1107
b1106:
  %v1585 = call i64 @df.class(i64 %v0)
  switch i64 %v1585, label %b1111 [ i64 675, label %b1108 i64 4, label %b1109 i64 2, label %b1110 i64 3, label %b1110 ]
b1107:
  %v1603 = phi i1 [ %v1584, %b1105 ], [ %v1602, %b1112 ]
  br i1 %v1603, label %b1123, label %b1124
b1108:
  %v1586 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1579)
  %v1587 = load i8, ptr %ctx, align 8
  %c344 = zext i8 %v1587 to i64
  %v1588 = icmp ne i64 %c344, 0
  br i1 %v1588, label %b1114, label %b1113
b1109:
  %v1590 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1579)
  %v1591 = load i8, ptr %ctx, align 8
  %c345 = zext i8 %v1591 to i64
  %v1592 = icmp ne i64 %c345, 0
  br i1 %v1592, label %b1116, label %b1115
b1110:
  %v1593 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1579)
  %v1594 = load i8, ptr %ctx, align 8
  %c346 = zext i8 %v1594 to i64
  %v1595 = icmp ne i64 %c346, 0
  br i1 %v1595, label %b1118, label %b1117
b1111:
  %sa1596_0 = getelementptr [2 x i64], ptr %sargs1596, i64 0, i64 0
  store i64 %v1579, ptr %sa1596_0
  %sat1596 = getelementptr [2 x i64], ptr %sargs1596, i64 0, i64 1
  store i64 0, ptr %sat1596
  %sic1596 = getelementptr i64, ptr %area, i64 294
  %sf1596 = call ptr @df.selector(ptr %sic1596, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v1596 = call i64 %sf1596(i64 %v0, ptr %sargs1596, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1596, ptr %gcs0
  %v1597 = load i8, ptr %ctx, align 8
  %c347 = zext i8 %v1597 to i64
  %v1598 = icmp ne i64 %c347, 0
  br i1 %v1598, label %b1120, label %b1119
b1112:
  %v1602 = phi i1 [ %v1589, %b1113 ], [ %v1590, %b1115 ], [ %v1593, %b1117 ], [ %v1599, %b1121 ]
  br label %b1107
b1113:
  %c348 = zext i8 %v1586 to i64
  %v1589 = icmp ne i64 %c348, 0
  br label %b1112
b1114:
  %gcvolta1114 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1114, ptr %ctxtopo, align 8
  ret i64 0
b1115:
  br label %b1112
b1116:
  %gcvolta1116 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1116, ptr %ctxtopo, align 8
  ret i64 0
b1117:
  br label %b1112
b1118:
  %gcvolta1118 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1118, ptr %ctxtopo, align 8
  ret i64 0
b1119:
  store i64 0, ptr %gcs1
  %v1599 = call i1 @df.unbox_bool(i64 %v1596)
  %v1600 = load i8, ptr %ctx, align 8
  %c349 = zext i8 %v1600 to i64
  %v1601 = icmp ne i64 %c349, 0
  br i1 %v1601, label %b1122, label %b1121
b1120:
  %gcvolta1120 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1120, ptr %ctxtopo, align 8
  ret i64 0
b1121:
  br label %b1112
b1122:
  %gcvolta1122 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1122, ptr %ctxtopo, align 8
  ret i64 0
b1123:
  %v1604 = add i64 0, 0
  br label %b1125
b1124:
  %v1605 = add i64 0, 1
  br label %b1125
b1125:
  %v1606 = phi i64 [ %v1604, %b1123 ], [ %v1605, %b1124 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1607 = call i64 @df.box_int(i64 %v1606)
  %xcp1608 = load i8, ptr %ctx, align 8
  %xcn1608 = icmp ne i8 %xcp1608, 0
  %xce1608 = call i1 @llvm.expect.i1(i1 %xcn1608, i1 false)
  br i1 %xce1608, label %xc1608.limpar, label %xc1608.fim
xc1608.limpar:
  call void @dartforge_exception_clear()
  br label %xc1608.fim
xc1608.fim:
  %gcvolta1125 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1125, ptr %ctxtopo, align 8
  ret i64 %v1607
b1127:
  %v1613 = ashr i64 %v0, 1
  %v1614 = icmp sle i64 %v1613, %v1610
  br label %b1129
b1128:
  %v1615 = call i64 @df.box_int(i64 %v1610)
  store i64 %v1615, ptr %gcs1
  %sa1616_0 = getelementptr [2 x i64], ptr %sargs1616, i64 0, i64 0
  store i64 %v1615, ptr %sa1616_0
  %sat1616 = getelementptr [2 x i64], ptr %sargs1616, i64 0, i64 1
  store i64 0, ptr %sat1616
  %sic1616 = getelementptr i64, ptr %area, i64 296
  %sf1616 = call ptr @df.selector(ptr %sic1616, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1616 = call i64 %sf1616(i64 %v0, ptr %sargs1616, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1616, ptr %gcs1
  %v1617 = load i8, ptr %ctx, align 8
  %c350 = zext i8 %v1617 to i64
  %v1618 = icmp ne i64 %c350, 0
  br i1 %v1618, label %b1131, label %b1130
b1129:
  %v1622 = phi i1 [ %v1614, %b1127 ], [ %v1619, %b1132 ]
  br i1 %v1622, label %b1134, label %b1135
b1130:
  %v1619 = call i1 @df.unbox_bool(i64 %v1616)
  %v1620 = load i8, ptr %ctx, align 8
  %c351 = zext i8 %v1620 to i64
  %v1621 = icmp ne i64 %c351, 0
  br i1 %v1621, label %b1133, label %b1132
b1131:
  %gcvolta1131 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1131, ptr %ctxtopo, align 8
  ret i64 0
b1132:
  br label %b1129
b1133:
  %gcvolta1133 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1133, ptr %ctxtopo, align 8
  ret i64 0
b1134:
  %v1623 = add i64 0, 0
  br label %b1136
b1135:
  store i64 0, ptr %gcs1
  %v1625 = icmp eq i64 %v0, 0
  %v1626 = call i64 @df.class(i64 %v0)
  %v1627 = call i8 @df.subclass(i64 %v1626, i64 674)
  %c352 = zext i8 %v1627 to i64
  %v1628 = icmp ne i64 %c352, 0
  %v1629 = xor i1 %v1625, true
  %c353 = zext i1 %v1628 to i64
  %c354 = zext i1 %v1629 to i64
  %v1630 = and i64 %c353, %c354
  %v1631 = icmp ne i64 %v1630, 0
  br i1 %v1631, label %b1138, label %b1137
b1136:
  %v1645 = phi i64 [ %v1623, %b1134 ], [ %v1644, %b1144 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1646 = call i64 @df.box_int(i64 %v1645)
  %xcp1647 = load i8, ptr %ctx, align 8
  %xcn1647 = icmp ne i8 %xcp1647, 0
  %xce1647 = call i1 @llvm.expect.i1(i1 %xcn1647, i1 false)
  br i1 %xce1647, label %xc1647.limpar, label %xc1647.fim
xc1647.limpar:
  call void @dartforge_exception_clear()
  br label %xc1647.fim
xc1647.fim:
  %gcvolta1136 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1136, ptr %ctxtopo, align 8
  ret i64 %v1646
b1137:
  %ga1632 = getelementptr i64, ptr %area, i64 13
  %v1632 = load i8, ptr %ga1632
  %c355 = zext i8 %v1632 to i64
  %v1633 = icmp ne i64 %c355, 0
  br i1 %v1633, label %b1139, label %b1140
b1138:
  %v1640 = call i64 @df.unbox_int(i64 %v0)
  %v1641 = load i8, ptr %ctx, align 8
  %c356 = zext i8 %v1641 to i64
  %v1642 = icmp ne i64 %c356, 0
  br i1 %v1642, label %b1145, label %b1144
b1139:
  %ga1634 = getelementptr i64, ptr %area, i64 12
  %v1634 = load i64, ptr %ga1634
  br label %b1141
b1140:
  %v1635 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1141
b1141:
  %v1636 = phi i64 [ %v1634, %b1139 ], [ %v1635, %b1140 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1636, i64 0, i64 0)
  %v1638 = load i8, ptr %ctx, align 8
  %c357 = zext i8 %v1638 to i64
  %v1639 = icmp ne i64 %c357, 0
  br i1 %v1639, label %b1143, label %b1142
b1142:
  br label %b1138
b1143:
  %gcvolta1143 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1143, ptr %ctxtopo, align 8
  ret i64 0
b1144:
  %v1643 = add i64 0, 15
  %v1644 = ashr i64 %v1640, %v1643
  br label %b1136
b1145:
  %gcvolta1145 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1145, ptr %ctxtopo, align 8
  ret i64 0
b1147:
  %v1652 = ashr i64 %v0, 1
  %v1653 = icmp sle i64 %v1652, %v1649
  br label %b1149
b1148:
  %v1654 = call i64 @df.box_int(i64 %v1649)
  store i64 %v1654, ptr %gcs1
  %sa1655_0 = getelementptr [2 x i64], ptr %sargs1655, i64 0, i64 0
  store i64 %v1654, ptr %sa1655_0
  %sat1655 = getelementptr [2 x i64], ptr %sargs1655, i64 0, i64 1
  store i64 0, ptr %sat1655
  %sic1655 = getelementptr i64, ptr %area, i64 298
  %sf1655 = call ptr @df.selector(ptr %sic1655, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1655 = call i64 %sf1655(i64 %v0, ptr %sargs1655, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1655, ptr %gcs1
  %v1656 = load i8, ptr %ctx, align 8
  %c358 = zext i8 %v1656 to i64
  %v1657 = icmp ne i64 %c358, 0
  br i1 %v1657, label %b1151, label %b1150
b1149:
  %v1661 = phi i1 [ %v1653, %b1147 ], [ %v1658, %b1152 ]
  br i1 %v1661, label %b1154, label %b1155
b1150:
  %v1658 = call i1 @df.unbox_bool(i64 %v1655)
  %v1659 = load i8, ptr %ctx, align 8
  %c359 = zext i8 %v1659 to i64
  %v1660 = icmp ne i64 %c359, 0
  br i1 %v1660, label %b1153, label %b1152
b1151:
  %gcvolta1151 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1151, ptr %ctxtopo, align 8
  ret i64 0
b1152:
  br label %b1149
b1153:
  %gcvolta1153 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1153, ptr %ctxtopo, align 8
  ret i64 0
b1154:
  %v1662 = add i64 0, 0
  br label %b1156
b1155:
  store i64 0, ptr %gcs1
  %v1664 = icmp eq i64 %v0, 0
  %v1665 = call i64 @df.class(i64 %v0)
  %v1666 = call i8 @df.subclass(i64 %v1665, i64 674)
  %c360 = zext i8 %v1666 to i64
  %v1667 = icmp ne i64 %c360, 0
  %v1668 = xor i1 %v1664, true
  %c361 = zext i1 %v1667 to i64
  %c362 = zext i1 %v1668 to i64
  %v1669 = and i64 %c361, %c362
  %v1670 = icmp ne i64 %v1669, 0
  br i1 %v1670, label %b1158, label %b1157
b1156:
  %v1684 = phi i64 [ %v1662, %b1154 ], [ %v1683, %b1164 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1685 = call i64 @df.box_int(i64 %v1684)
  %xcp1686 = load i8, ptr %ctx, align 8
  %xcn1686 = icmp ne i8 %xcp1686, 0
  %xce1686 = call i1 @llvm.expect.i1(i1 %xcn1686, i1 false)
  br i1 %xce1686, label %xc1686.limpar, label %xc1686.fim
xc1686.limpar:
  call void @dartforge_exception_clear()
  br label %xc1686.fim
xc1686.fim:
  %gcvolta1156 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1156, ptr %ctxtopo, align 8
  ret i64 %v1685
b1157:
  %ga1671 = getelementptr i64, ptr %area, i64 13
  %v1671 = load i8, ptr %ga1671
  %c363 = zext i8 %v1671 to i64
  %v1672 = icmp ne i64 %c363, 0
  br i1 %v1672, label %b1159, label %b1160
b1158:
  %v1679 = call i64 @df.unbox_int(i64 %v0)
  %v1680 = load i8, ptr %ctx, align 8
  %c364 = zext i8 %v1680 to i64
  %v1681 = icmp ne i64 %c364, 0
  br i1 %v1681, label %b1165, label %b1164
b1159:
  %ga1673 = getelementptr i64, ptr %area, i64 12
  %v1673 = load i64, ptr %ga1673
  br label %b1161
b1160:
  %v1674 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1161
b1161:
  %v1675 = phi i64 [ %v1673, %b1159 ], [ %v1674, %b1160 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1675, i64 0, i64 0)
  %v1677 = load i8, ptr %ctx, align 8
  %c365 = zext i8 %v1677 to i64
  %v1678 = icmp ne i64 %c365, 0
  br i1 %v1678, label %b1163, label %b1162
b1162:
  br label %b1158
b1163:
  %gcvolta1163 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1163, ptr %ctxtopo, align 8
  ret i64 0
b1164:
  %v1682 = add i64 0, 11
  %v1683 = ashr i64 %v1679, %v1682
  br label %b1156
b1165:
  %gcvolta1165 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1165, ptr %ctxtopo, align 8
  ret i64 0
b1167:
  %v1691 = ashr i64 %v0, 1
  %v1692 = icmp sle i64 %v1691, %v1688
  br label %b1169
b1168:
  %v1693 = call i64 @df.box_int(i64 %v1688)
  store i64 %v1693, ptr %gcs1
  %sa1694_0 = getelementptr [2 x i64], ptr %sargs1694, i64 0, i64 0
  store i64 %v1693, ptr %sa1694_0
  %sat1694 = getelementptr [2 x i64], ptr %sargs1694, i64 0, i64 1
  store i64 0, ptr %sat1694
  %sic1694 = getelementptr i64, ptr %area, i64 300
  %sf1694 = call ptr @df.selector(ptr %sic1694, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1694 = call i64 %sf1694(i64 %v0, ptr %sargs1694, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1694, ptr %gcs1
  %v1695 = load i8, ptr %ctx, align 8
  %c366 = zext i8 %v1695 to i64
  %v1696 = icmp ne i64 %c366, 0
  br i1 %v1696, label %b1171, label %b1170
b1169:
  %v1700 = phi i1 [ %v1692, %b1167 ], [ %v1697, %b1172 ]
  br i1 %v1700, label %b1174, label %b1175
b1170:
  %v1697 = call i1 @df.unbox_bool(i64 %v1694)
  %v1698 = load i8, ptr %ctx, align 8
  %c367 = zext i8 %v1698 to i64
  %v1699 = icmp ne i64 %c367, 0
  br i1 %v1699, label %b1173, label %b1172
b1171:
  %gcvolta1171 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1171, ptr %ctxtopo, align 8
  ret i64 0
b1172:
  br label %b1169
b1173:
  %gcvolta1173 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1173, ptr %ctxtopo, align 8
  ret i64 0
b1174:
  %v1701 = add i64 0, 0
  br label %b1176
b1175:
  store i64 0, ptr %gcs1
  %v1703 = icmp eq i64 %v0, 0
  %v1704 = call i64 @df.class(i64 %v0)
  %v1705 = call i8 @df.subclass(i64 %v1704, i64 674)
  %c368 = zext i8 %v1705 to i64
  %v1706 = icmp ne i64 %c368, 0
  %v1707 = xor i1 %v1703, true
  %c369 = zext i1 %v1706 to i64
  %c370 = zext i1 %v1707 to i64
  %v1708 = and i64 %c369, %c370
  %v1709 = icmp ne i64 %v1708, 0
  br i1 %v1709, label %b1178, label %b1177
b1176:
  %v1723 = phi i64 [ %v1701, %b1174 ], [ %v1722, %b1184 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1724 = call i64 @df.box_int(i64 %v1723)
  %xcp1725 = load i8, ptr %ctx, align 8
  %xcn1725 = icmp ne i8 %xcp1725, 0
  %xce1725 = call i1 @llvm.expect.i1(i1 %xcn1725, i1 false)
  br i1 %xce1725, label %xc1725.limpar, label %xc1725.fim
xc1725.limpar:
  call void @dartforge_exception_clear()
  br label %xc1725.fim
xc1725.fim:
  %gcvolta1176 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1176, ptr %ctxtopo, align 8
  ret i64 %v1724
b1177:
  %ga1710 = getelementptr i64, ptr %area, i64 13
  %v1710 = load i8, ptr %ga1710
  %c371 = zext i8 %v1710 to i64
  %v1711 = icmp ne i64 %c371, 0
  br i1 %v1711, label %b1179, label %b1180
b1178:
  %v1718 = call i64 @df.unbox_int(i64 %v0)
  %v1719 = load i8, ptr %ctx, align 8
  %c372 = zext i8 %v1719 to i64
  %v1720 = icmp ne i64 %c372, 0
  br i1 %v1720, label %b1185, label %b1184
b1179:
  %ga1712 = getelementptr i64, ptr %area, i64 12
  %v1712 = load i64, ptr %ga1712
  br label %b1181
b1180:
  %v1713 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1181
b1181:
  %v1714 = phi i64 [ %v1712, %b1179 ], [ %v1713, %b1180 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1714, i64 0, i64 0)
  %v1716 = load i8, ptr %ctx, align 8
  %c373 = zext i8 %v1716 to i64
  %v1717 = icmp ne i64 %c373, 0
  br i1 %v1717, label %b1183, label %b1182
b1182:
  br label %b1178
b1183:
  %gcvolta1183 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1183, ptr %ctxtopo, align 8
  ret i64 0
b1184:
  %v1721 = add i64 0, 7
  %v1722 = ashr i64 %v1718, %v1721
  br label %b1176
b1185:
  %gcvolta1185 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1185, ptr %ctxtopo, align 8
  ret i64 0
b1187:
  %v1730 = ashr i64 %v0, 1
  %v1731 = icmp sle i64 %v1730, %v1727
  br label %b1189
b1188:
  %v1732 = call i64 @df.box_int(i64 %v1727)
  store i64 %v1732, ptr %gcs1
  %sa1733_0 = getelementptr [2 x i64], ptr %sargs1733, i64 0, i64 0
  store i64 %v1732, ptr %sa1733_0
  %sat1733 = getelementptr [2 x i64], ptr %sargs1733, i64 0, i64 1
  store i64 0, ptr %sat1733
  %sic1733 = getelementptr i64, ptr %area, i64 302
  %sf1733 = call ptr @df.selector(ptr %sic1733, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1733 = call i64 %sf1733(i64 %v0, ptr %sargs1733, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1733, ptr %gcs1
  %v1734 = load i8, ptr %ctx, align 8
  %c374 = zext i8 %v1734 to i64
  %v1735 = icmp ne i64 %c374, 0
  br i1 %v1735, label %b1191, label %b1190
b1189:
  %v1739 = phi i1 [ %v1731, %b1187 ], [ %v1736, %b1192 ]
  br i1 %v1739, label %b1194, label %b1195
b1190:
  %v1736 = call i1 @df.unbox_bool(i64 %v1733)
  %v1737 = load i8, ptr %ctx, align 8
  %c375 = zext i8 %v1737 to i64
  %v1738 = icmp ne i64 %c375, 0
  br i1 %v1738, label %b1193, label %b1192
b1191:
  %gcvolta1191 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1191, ptr %ctxtopo, align 8
  ret i64 0
b1192:
  br label %b1189
b1193:
  %gcvolta1193 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1193, ptr %ctxtopo, align 8
  ret i64 0
b1194:
  %v1740 = add i64 0, 0
  br label %b1196
b1195:
  store i64 0, ptr %gcs1
  %v1742 = icmp eq i64 %v0, 0
  %v1743 = call i64 @df.class(i64 %v0)
  %v1744 = call i8 @df.subclass(i64 %v1743, i64 674)
  %c376 = zext i8 %v1744 to i64
  %v1745 = icmp ne i64 %c376, 0
  %v1746 = xor i1 %v1742, true
  %c377 = zext i1 %v1745 to i64
  %c378 = zext i1 %v1746 to i64
  %v1747 = and i64 %c377, %c378
  %v1748 = icmp ne i64 %v1747, 0
  br i1 %v1748, label %b1198, label %b1197
b1196:
  %v1762 = phi i64 [ %v1740, %b1194 ], [ %v1761, %b1204 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1763 = call i64 @df.box_int(i64 %v1762)
  %xcp1764 = load i8, ptr %ctx, align 8
  %xcn1764 = icmp ne i8 %xcp1764, 0
  %xce1764 = call i1 @llvm.expect.i1(i1 %xcn1764, i1 false)
  br i1 %xce1764, label %xc1764.limpar, label %xc1764.fim
xc1764.limpar:
  call void @dartforge_exception_clear()
  br label %xc1764.fim
xc1764.fim:
  %gcvolta1196 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1196, ptr %ctxtopo, align 8
  ret i64 %v1763
b1197:
  %ga1749 = getelementptr i64, ptr %area, i64 13
  %v1749 = load i8, ptr %ga1749
  %c379 = zext i8 %v1749 to i64
  %v1750 = icmp ne i64 %c379, 0
  br i1 %v1750, label %b1199, label %b1200
b1198:
  %v1757 = call i64 @df.unbox_int(i64 %v0)
  %v1758 = load i8, ptr %ctx, align 8
  %c380 = zext i8 %v1758 to i64
  %v1759 = icmp ne i64 %c380, 0
  br i1 %v1759, label %b1205, label %b1204
b1199:
  %ga1751 = getelementptr i64, ptr %area, i64 12
  %v1751 = load i64, ptr %ga1751
  br label %b1201
b1200:
  %v1752 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1201
b1201:
  %v1753 = phi i64 [ %v1751, %b1199 ], [ %v1752, %b1200 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1753, i64 0, i64 0)
  %v1755 = load i8, ptr %ctx, align 8
  %c381 = zext i8 %v1755 to i64
  %v1756 = icmp ne i64 %c381, 0
  br i1 %v1756, label %b1203, label %b1202
b1202:
  br label %b1198
b1203:
  %gcvolta1203 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1203, ptr %ctxtopo, align 8
  ret i64 0
b1204:
  %v1760 = add i64 0, 1
  %v1761 = shl i64 %v1757, %v1760
  br label %b1196
b1205:
  %gcvolta1205 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1205, ptr %ctxtopo, align 8
  ret i64 0
b1207:
  %v1769 = ashr i64 %v0, 1
  %v1770 = icmp sle i64 %v1769, %v1766
  br label %b1209
b1208:
  %v1771 = call i64 @df.box_int(i64 %v1766)
  store i64 %v1771, ptr %gcs1
  %sa1772_0 = getelementptr [2 x i64], ptr %sargs1772, i64 0, i64 0
  store i64 %v1771, ptr %sa1772_0
  %sat1772 = getelementptr [2 x i64], ptr %sargs1772, i64 0, i64 1
  store i64 0, ptr %sat1772
  %sic1772 = getelementptr i64, ptr %area, i64 304
  %sf1772 = call ptr @df.selector(ptr %sic1772, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1772 = call i64 %sf1772(i64 %v0, ptr %sargs1772, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1772, ptr %gcs1
  %v1773 = load i8, ptr %ctx, align 8
  %c382 = zext i8 %v1773 to i64
  %v1774 = icmp ne i64 %c382, 0
  br i1 %v1774, label %b1211, label %b1210
b1209:
  %v1778 = phi i1 [ %v1770, %b1207 ], [ %v1775, %b1212 ]
  br i1 %v1778, label %b1214, label %b1215
b1210:
  %v1775 = call i1 @df.unbox_bool(i64 %v1772)
  %v1776 = load i8, ptr %ctx, align 8
  %c383 = zext i8 %v1776 to i64
  %v1777 = icmp ne i64 %c383, 0
  br i1 %v1777, label %b1213, label %b1212
b1211:
  %gcvolta1211 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1211, ptr %ctxtopo, align 8
  ret i64 0
b1212:
  br label %b1209
b1213:
  %gcvolta1213 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1213, ptr %ctxtopo, align 8
  ret i64 0
b1214:
  %v1779 = add i64 0, 0
  br label %b1216
b1215:
  store i64 0, ptr %gcs1
  %v1781 = icmp eq i64 %v0, 0
  %v1782 = call i64 @df.class(i64 %v0)
  %v1783 = call i8 @df.subclass(i64 %v1782, i64 674)
  %c384 = zext i8 %v1783 to i64
  %v1784 = icmp ne i64 %c384, 0
  %v1785 = xor i1 %v1781, true
  %c385 = zext i1 %v1784 to i64
  %c386 = zext i1 %v1785 to i64
  %v1786 = and i64 %c385, %c386
  %v1787 = icmp ne i64 %v1786, 0
  br i1 %v1787, label %b1218, label %b1217
b1216:
  %v1801 = phi i64 [ %v1779, %b1214 ], [ %v1800, %b1224 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1802 = call i64 @df.box_int(i64 %v1801)
  %xcp1803 = load i8, ptr %ctx, align 8
  %xcn1803 = icmp ne i8 %xcp1803, 0
  %xce1803 = call i1 @llvm.expect.i1(i1 %xcn1803, i1 false)
  br i1 %xce1803, label %xc1803.limpar, label %xc1803.fim
xc1803.limpar:
  call void @dartforge_exception_clear()
  br label %xc1803.fim
xc1803.fim:
  %gcvolta1216 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1216, ptr %ctxtopo, align 8
  ret i64 %v1802
b1217:
  %ga1788 = getelementptr i64, ptr %area, i64 13
  %v1788 = load i8, ptr %ga1788
  %c387 = zext i8 %v1788 to i64
  %v1789 = icmp ne i64 %c387, 0
  br i1 %v1789, label %b1219, label %b1220
b1218:
  %v1796 = call i64 @df.unbox_int(i64 %v0)
  %v1797 = load i8, ptr %ctx, align 8
  %c388 = zext i8 %v1797 to i64
  %v1798 = icmp ne i64 %c388, 0
  br i1 %v1798, label %b1225, label %b1224
b1219:
  %ga1790 = getelementptr i64, ptr %area, i64 12
  %v1790 = load i64, ptr %ga1790
  br label %b1221
b1220:
  %v1791 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1221
b1221:
  %v1792 = phi i64 [ %v1790, %b1219 ], [ %v1791, %b1220 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1792, i64 0, i64 0)
  %v1794 = load i8, ptr %ctx, align 8
  %c389 = zext i8 %v1794 to i64
  %v1795 = icmp ne i64 %c389, 0
  br i1 %v1795, label %b1223, label %b1222
b1222:
  br label %b1218
b1223:
  %gcvolta1223 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1223, ptr %ctxtopo, align 8
  ret i64 0
b1224:
  %v1799 = add i64 0, 131076
  %v1800 = mul i64 %v1796, %v1799
  br label %b1216
b1225:
  %gcvolta1225 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1225, ptr %ctxtopo, align 8
  ret i64 0
b1227:
  %ga1812 = getelementptr i64, ptr %area, i64 13
  %v1812 = load i8, ptr %ga1812
  %c390 = zext i8 %v1812 to i64
  %v1813 = icmp ne i64 %c390, 0
  br i1 %v1813, label %b1229, label %b1230
b1228:
  %v1820 = call i64 @df.unbox_int(i64 %v0)
  %v1821 = load i8, ptr %ctx, align 8
  %c391 = zext i8 %v1821 to i64
  %v1822 = icmp ne i64 %c391, 0
  br i1 %v1822, label %b1235, label %b1234
b1229:
  %ga1814 = getelementptr i64, ptr %area, i64 12
  %v1814 = load i64, ptr %ga1814
  br label %b1231
b1230:
  %v1815 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1231
b1231:
  %v1816 = phi i64 [ %v1814, %b1229 ], [ %v1815, %b1230 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1816, i64 0, i64 0)
  %v1818 = load i8, ptr %ctx, align 8
  %c392 = zext i8 %v1818 to i64
  %v1819 = icmp ne i64 %c392, 0
  br i1 %v1819, label %b1233, label %b1232
b1232:
  br label %b1228
b1233:
  %gcvolta1233 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1233, ptr %ctxtopo, align 8
  ret i64 0
b1234:
  %v1823 = add i64 0, 8
  %v1824 = ashr i64 %v1820, %v1823
  store i64 0, ptr %gcs0
  %v1825 = call i64 @df.box_int(i64 %v1824)
  %xcp1826 = load i8, ptr %ctx, align 8
  %xcn1826 = icmp ne i8 %xcp1826, 0
  %xce1826 = call i1 @llvm.expect.i1(i1 %xcn1826, i1 false)
  br i1 %xce1826, label %xc1826.limpar, label %xc1826.fim
xc1826.limpar:
  call void @dartforge_exception_clear()
  br label %xc1826.fim
xc1826.fim:
  %gcvolta1234 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1234, ptr %ctxtopo, align 8
  ret i64 %v1825
b1235:
  %gcvolta1235 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1235, ptr %ctxtopo, align 8
  ret i64 0
b1238:
  %ga1837 = getelementptr i64, ptr %area, i64 13
  %v1837 = load i8, ptr %ga1837
  %c393 = zext i8 %v1837 to i64
  %v1838 = icmp ne i64 %c393, 0
  br i1 %v1838, label %b1240, label %b1241
b1239:
  %v1845 = call i64 @df.unbox_int(i64 %v0)
  %v1846 = load i8, ptr %ctx, align 8
  %c394 = zext i8 %v1846 to i64
  %v1847 = icmp ne i64 %c394, 0
  br i1 %v1847, label %b1246, label %b1245
b1240:
  %ga1839 = getelementptr i64, ptr %area, i64 12
  %v1839 = load i64, ptr %ga1839
  br label %b1242
b1241:
  %v1840 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1242
b1242:
  %v1841 = phi i64 [ %v1839, %b1240 ], [ %v1840, %b1241 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1841, i64 0, i64 0)
  %v1843 = load i8, ptr %ctx, align 8
  %c395 = zext i8 %v1843 to i64
  %v1844 = icmp ne i64 %c395, 0
  br i1 %v1844, label %b1244, label %b1243
b1243:
  br label %b1239
b1244:
  %gcvolta1244 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1244, ptr %ctxtopo, align 8
  ret i64 0
b1245:
  %v1848 = add i64 0, 65538
  %v1849 = mul i64 %v1845, %v1848
  store i64 0, ptr %gcs0
  %v1850 = call i64 @df.box_int(i64 %v1849)
  %xcp1851 = load i8, ptr %ctx, align 8
  %xcn1851 = icmp ne i8 %xcp1851, 0
  %xce1851 = call i1 @llvm.expect.i1(i1 %xcn1851, i1 false)
  br i1 %xce1851, label %xc1851.limpar, label %xc1851.fim
xc1851.limpar:
  call void @dartforge_exception_clear()
  br label %xc1851.fim
xc1851.fim:
  %gcvolta1245 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1245, ptr %ctxtopo, align 8
  ret i64 %v1850
b1246:
  %gcvolta1246 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1246, ptr %ctxtopo, align 8
  ret i64 0
b1248:
  store i64 0, ptr %gcs1
  %v1858 = call double @df.unbox_double(i64 %v1855)
  %v1859 = load i8, ptr %ctx, align 8
  %c396 = zext i8 %v1859 to i64
  %v1860 = icmp ne i64 %c396, 0
  br i1 %v1860, label %b1251, label %b1250
b1249:
  %gcvolta1249 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1249, ptr %ctxtopo, align 8
  ret i64 0
b1250:
  store i64 0, ptr %gcs0
  %v1861 = call i64 @df.box_double(double %v1858)
  %xcp1862 = load i8, ptr %ctx, align 8
  %xcn1862 = icmp ne i8 %xcp1862, 0
  %xce1862 = call i1 @llvm.expect.i1(i1 %xcn1862, i1 false)
  br i1 %xce1862, label %xc1862.limpar, label %xc1862.fim
xc1862.limpar:
  call void @dartforge_exception_clear()
  br label %xc1862.fim
xc1862.fim:
  %gcvolta1250 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1250, ptr %ctxtopo, align 8
  ret i64 %v1861
b1251:
  %gcvolta1251 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1251, ptr %ctxtopo, align 8
  ret i64 0
b1265:
  %v2188 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2189 = call i64 @df.box_int(i64 %v2188)
  store i64 %v2189, ptr %gcs1
  %sa2190_0 = getelementptr [2 x i64], ptr %sargs2190, i64 0, i64 0
  store i64 %v2189, ptr %sa2190_0
  %sat2190 = getelementptr [2 x i64], ptr %sargs2190, i64 0, i64 1
  store i64 0, ptr %sat2190
  %sic2190 = getelementptr i64, ptr %area, i64 306
  %sf2190 = call ptr @df.selector(ptr %sic2190, i64 %v0, i64 -4020828333918261805, ptr @df.seln.18, i64 4)
  %v2190 = call i64 %sf2190(i64 %v0, ptr %sargs2190, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2190, ptr %gcs0
  %v2191 = load i8, ptr %ctx, align 8
  %c397 = zext i8 %v2191 to i64
  %v2192 = icmp ne i64 %c397, 0
  br i1 %v2192, label %b1460, label %b1459
b1266:
  store i64 0, ptr %gcs1
  %v1868 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1868, ptr %gcs1
  %v1869 = load i8, ptr %ctx, align 8
  %c398 = zext i8 %v1869 to i64
  %v1870 = icmp ne i64 %c398, 0
  br i1 %v1870, label %b1272, label %b1271
b1267:
  %v1867 = icmp eq i64 %v1864, %v6
  br i1 %v1867, label %b1269, label %b1266
b1268:
  %gcvolta1268 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1268, ptr %ctxtopo, align 8
  ret i64 0
b1269:
  %v1913 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1914 = call i64 @df.box_int(i64 %v1913)
  store i64 %v1914, ptr %gcs1
  %v1915 = icmp eq i64 %v0, 0
  %v1916 = icmp eq i64 %v1914, 0
  %c399 = zext i1 %v1915 to i64
  %c400 = zext i1 %v1916 to i64
  %v1917 = or i64 %c399, %c400
  %v1918 = icmp ne i64 %v1917, 0
  br i1 %v1918, label %b1316, label %b1317
b1270:
  store i64 0, ptr %gcs1
  %v1872 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1872, ptr %gcs1
  %v1873 = load i8, ptr %ctx, align 8
  %c401 = zext i8 %v1873 to i64
  %v1874 = icmp ne i64 %c401, 0
  br i1 %v1874, label %b1276, label %b1275
b1271:
  %v1871 = icmp eq i64 %v1868, %v6
  br i1 %v1871, label %b1273, label %b1270
b1272:
  %gcvolta1272 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1272, ptr %ctxtopo, align 8
  ret i64 0
b1273:
  %v1945 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1946 = and i64 %v0, 1
  %v1947 = icmp ne i64 %v1946, 0
  br i1 %v1947, label %b1338, label %b1339
b1274:
  store i64 0, ptr %gcs1
  %v1876 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1876, ptr %gcs1
  %v1877 = load i8, ptr %ctx, align 8
  %c402 = zext i8 %v1877 to i64
  %v1878 = icmp ne i64 %c402, 0
  br i1 %v1878, label %b1280, label %b1279
b1275:
  %v1875 = icmp eq i64 %v1872, %v6
  br i1 %v1875, label %b1277, label %b1274
b1276:
  %gcvolta1276 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1276, ptr %ctxtopo, align 8
  ret i64 0
b1277:
  %v1984 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1985 = and i64 %v0, 1
  %v1986 = icmp ne i64 %v1985, 0
  br i1 %v1986, label %b1358, label %b1359
b1278:
  store i64 0, ptr %gcs1
  %v1880 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1880, ptr %gcs1
  %v1881 = load i8, ptr %ctx, align 8
  %c403 = zext i8 %v1881 to i64
  %v1882 = icmp ne i64 %c403, 0
  br i1 %v1882, label %b1284, label %b1283
b1279:
  %v1879 = icmp eq i64 %v1876, %v6
  br i1 %v1879, label %b1281, label %b1278
b1280:
  %gcvolta1280 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1280, ptr %ctxtopo, align 8
  ret i64 0
b1281:
  %v2023 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2024 = and i64 %v0, 1
  %v2025 = icmp ne i64 %v2024, 0
  br i1 %v2025, label %b1378, label %b1379
b1282:
  store i64 0, ptr %gcs1
  %v1884 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1884, ptr %gcs1
  %v1885 = load i8, ptr %ctx, align 8
  %c404 = zext i8 %v1885 to i64
  %v1886 = icmp ne i64 %c404, 0
  br i1 %v1886, label %b1288, label %b1287
b1283:
  %v1883 = icmp eq i64 %v1880, %v6
  br i1 %v1883, label %b1285, label %b1282
b1284:
  %gcvolta1284 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1284, ptr %ctxtopo, align 8
  ret i64 0
b1285:
  %v2062 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2063 = and i64 %v0, 1
  %v2064 = icmp ne i64 %v2063, 0
  br i1 %v2064, label %b1398, label %b1399
b1286:
  store i64 0, ptr %gcs1
  %v1888 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1888, ptr %gcs1
  %v1889 = load i8, ptr %ctx, align 8
  %c405 = zext i8 %v1889 to i64
  %v1890 = icmp ne i64 %c405, 0
  br i1 %v1890, label %b1292, label %b1291
b1287:
  %v1887 = icmp eq i64 %v1884, %v6
  br i1 %v1887, label %b1289, label %b1286
b1288:
  %gcvolta1288 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1288, ptr %ctxtopo, align 8
  ret i64 0
b1289:
  %v2101 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2102 = and i64 %v0, 1
  %v2103 = icmp ne i64 %v2102, 0
  br i1 %v2103, label %b1418, label %b1419
b1290:
  store i64 0, ptr %gcs1
  %v1892 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1892, ptr %gcs1
  %v1893 = load i8, ptr %ctx, align 8
  %c406 = zext i8 %v1893 to i64
  %v1894 = icmp ne i64 %c406, 0
  br i1 %v1894, label %b1296, label %b1295
b1291:
  %v1891 = icmp eq i64 %v1888, %v6
  br i1 %v1891, label %b1293, label %b1290
b1292:
  %gcvolta1292 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1292, ptr %ctxtopo, align 8
  ret i64 0
b1293:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2140 = icmp eq i64 %v0, 0
  %v2141 = call i64 @df.class(i64 %v0)
  %v2142 = call i8 @df.subclass(i64 %v2141, i64 674)
  %c407 = zext i8 %v2142 to i64
  %v2143 = icmp ne i64 %c407, 0
  %v2144 = xor i1 %v2140, true
  %c408 = zext i1 %v2143 to i64
  %c409 = zext i1 %v2144 to i64
  %v2145 = and i64 %c408, %c409
  %v2146 = icmp ne i64 %v2145, 0
  br i1 %v2146, label %b1439, label %b1438
b1294:
  store i64 0, ptr %gcs1
  %v1896 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1896, ptr %gcs1
  %v1897 = load i8, ptr %ctx, align 8
  %c410 = zext i8 %v1897 to i64
  %v1898 = icmp ne i64 %c410, 0
  br i1 %v1898, label %b1300, label %b1299
b1295:
  %v1895 = icmp eq i64 %v1892, %v6
  br i1 %v1895, label %b1297, label %b1294
b1296:
  %gcvolta1296 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1296, ptr %ctxtopo, align 8
  ret i64 0
b1297:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2163 = icmp eq i64 %v0, 0
  %v2164 = call i64 @df.class(i64 %v0)
  %v2165 = call i8 @df.subclass(i64 %v2164, i64 674)
  %c411 = zext i8 %v2165 to i64
  %v2166 = icmp ne i64 %c411, 0
  %v2167 = xor i1 %v2163, true
  %c412 = zext i1 %v2166 to i64
  %c413 = zext i1 %v2167 to i64
  %v2168 = and i64 %c412, %c413
  %v2169 = icmp ne i64 %v2168, 0
  br i1 %v2169, label %b1449, label %b1448
b1298:
  store i64 0, ptr %gcs1
  %v1900 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1900, ptr %gcs1
  %v1901 = load i8, ptr %ctx, align 8
  %c414 = zext i8 %v1901 to i64
  %v1902 = icmp ne i64 %c414, 0
  br i1 %v1902, label %b1304, label %b1303
b1299:
  %v1899 = icmp eq i64 %v1896, %v6
  br i1 %v1899, label %b1301, label %b1298
b1300:
  %gcvolta1300 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1300, ptr %ctxtopo, align 8
  ret i64 0
b1301:
  %xcp2186 = load i8, ptr %ctx, align 8
  %xcn2186 = icmp ne i8 %xcp2186, 0
  %xce2186 = call i1 @llvm.expect.i1(i1 %xcn2186, i1 false)
  br i1 %xce2186, label %xc2186.limpar, label %xc2186.fim
xc2186.limpar:
  call void @dartforge_exception_clear()
  br label %xc2186.fim
xc2186.fim:
  %gcvolta1301 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1301, ptr %ctxtopo, align 8
  ret i64 %v0
b1302:
  store i64 0, ptr %gcs1
  %v1904 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1904, ptr %gcs1
  %v1905 = load i8, ptr %ctx, align 8
  %c415 = zext i8 %v1905 to i64
  %v1906 = icmp ne i64 %c415, 0
  br i1 %v1906, label %b1309, label %b1308
b1303:
  %v1903 = icmp eq i64 %v1900, %v6
  br i1 %v1903, label %b1305, label %b1302
b1304:
  %gcvolta1304 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1304, ptr %ctxtopo, align 8
  ret i64 0
b1305:
  br label %b1265
b1307:
  store i64 0, ptr %gcs1
  %v1908 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1908, ptr %gcs1
  %v1909 = load i8, ptr %ctx, align 8
  %c416 = zext i8 %v1909 to i64
  %v1910 = icmp ne i64 %c416, 0
  br i1 %v1910, label %b1314, label %b1313
b1308:
  %v1907 = icmp eq i64 %v1904, %v6
  br i1 %v1907, label %b1310, label %b1307
b1309:
  %gcvolta1309 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1309, ptr %ctxtopo, align 8
  ret i64 0
b1310:
  br label %b1265
b1312:
  br label %b12
b1313:
  %v1911 = icmp eq i64 %v1908, %v6
  br i1 %v1911, label %b1315, label %b1312
b1314:
  %gcvolta1314 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1314, ptr %ctxtopo, align 8
  ret i64 0
b1315:
  br label %b1265
b1316:
  %v1919 = icmp eq i64 %v0, %v1914
  br label %b1318
b1317:
  %v1920 = call i64 @df.class(i64 %v0)
  switch i64 %v1920, label %b1322 [ i64 675, label %b1319 i64 4, label %b1320 i64 2, label %b1321 i64 3, label %b1321 ]
b1318:
  %v1938 = phi i1 [ %v1919, %b1316 ], [ %v1937, %b1323 ]
  br i1 %v1938, label %b1334, label %b1335
b1319:
  %v1921 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v1914)
  %v1922 = load i8, ptr %ctx, align 8
  %c417 = zext i8 %v1922 to i64
  %v1923 = icmp ne i64 %c417, 0
  br i1 %v1923, label %b1325, label %b1324
b1320:
  %v1925 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v1914)
  %v1926 = load i8, ptr %ctx, align 8
  %c418 = zext i8 %v1926 to i64
  %v1927 = icmp ne i64 %c418, 0
  br i1 %v1927, label %b1327, label %b1326
b1321:
  %v1928 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v1914)
  %v1929 = load i8, ptr %ctx, align 8
  %c419 = zext i8 %v1929 to i64
  %v1930 = icmp ne i64 %c419, 0
  br i1 %v1930, label %b1329, label %b1328
b1322:
  %sa1931_0 = getelementptr [2 x i64], ptr %sargs1931, i64 0, i64 0
  store i64 %v1914, ptr %sa1931_0
  %sat1931 = getelementptr [2 x i64], ptr %sargs1931, i64 0, i64 1
  store i64 0, ptr %sat1931
  %sic1931 = getelementptr i64, ptr %area, i64 308
  %sf1931 = call ptr @df.selector(ptr %sic1931, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v1931 = call i64 %sf1931(i64 %v0, ptr %sargs1931, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1931, ptr %gcs0
  %v1932 = load i8, ptr %ctx, align 8
  %c420 = zext i8 %v1932 to i64
  %v1933 = icmp ne i64 %c420, 0
  br i1 %v1933, label %b1331, label %b1330
b1323:
  %v1937 = phi i1 [ %v1924, %b1324 ], [ %v1925, %b1326 ], [ %v1928, %b1328 ], [ %v1934, %b1332 ]
  br label %b1318
b1324:
  %c421 = zext i8 %v1921 to i64
  %v1924 = icmp ne i64 %c421, 0
  br label %b1323
b1325:
  %gcvolta1325 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1325, ptr %ctxtopo, align 8
  ret i64 0
b1326:
  br label %b1323
b1327:
  %gcvolta1327 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1327, ptr %ctxtopo, align 8
  ret i64 0
b1328:
  br label %b1323
b1329:
  %gcvolta1329 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1329, ptr %ctxtopo, align 8
  ret i64 0
b1330:
  store i64 0, ptr %gcs1
  %v1934 = call i1 @df.unbox_bool(i64 %v1931)
  %v1935 = load i8, ptr %ctx, align 8
  %c422 = zext i8 %v1935 to i64
  %v1936 = icmp ne i64 %c422, 0
  br i1 %v1936, label %b1333, label %b1332
b1331:
  %gcvolta1331 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1331, ptr %ctxtopo, align 8
  ret i64 0
b1332:
  br label %b1323
b1333:
  %gcvolta1333 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1333, ptr %ctxtopo, align 8
  ret i64 0
b1334:
  %v1939 = add i64 0, 0
  br label %b1336
b1335:
  %v1940 = add i64 0, 1
  br label %b1336
b1336:
  %v1941 = phi i64 [ %v1939, %b1334 ], [ %v1940, %b1335 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1942 = call i64 @df.box_int(i64 %v1941)
  %xcp1943 = load i8, ptr %ctx, align 8
  %xcn1943 = icmp ne i8 %xcp1943, 0
  %xce1943 = call i1 @llvm.expect.i1(i1 %xcn1943, i1 false)
  br i1 %xce1943, label %xc1943.limpar, label %xc1943.fim
xc1943.limpar:
  call void @dartforge_exception_clear()
  br label %xc1943.fim
xc1943.fim:
  %gcvolta1336 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1336, ptr %ctxtopo, align 8
  ret i64 %v1942
b1338:
  %v1948 = ashr i64 %v0, 1
  %v1949 = icmp sle i64 %v1948, %v1945
  br label %b1340
b1339:
  %v1950 = call i64 @df.box_int(i64 %v1945)
  store i64 %v1950, ptr %gcs1
  %sa1951_0 = getelementptr [2 x i64], ptr %sargs1951, i64 0, i64 0
  store i64 %v1950, ptr %sa1951_0
  %sat1951 = getelementptr [2 x i64], ptr %sargs1951, i64 0, i64 1
  store i64 0, ptr %sat1951
  %sic1951 = getelementptr i64, ptr %area, i64 310
  %sf1951 = call ptr @df.selector(ptr %sic1951, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1951 = call i64 %sf1951(i64 %v0, ptr %sargs1951, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1951, ptr %gcs1
  %v1952 = load i8, ptr %ctx, align 8
  %c423 = zext i8 %v1952 to i64
  %v1953 = icmp ne i64 %c423, 0
  br i1 %v1953, label %b1342, label %b1341
b1340:
  %v1957 = phi i1 [ %v1949, %b1338 ], [ %v1954, %b1343 ]
  br i1 %v1957, label %b1345, label %b1346
b1341:
  %v1954 = call i1 @df.unbox_bool(i64 %v1951)
  %v1955 = load i8, ptr %ctx, align 8
  %c424 = zext i8 %v1955 to i64
  %v1956 = icmp ne i64 %c424, 0
  br i1 %v1956, label %b1344, label %b1343
b1342:
  %gcvolta1342 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1342, ptr %ctxtopo, align 8
  ret i64 0
b1343:
  br label %b1340
b1344:
  %gcvolta1344 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1344, ptr %ctxtopo, align 8
  ret i64 0
b1345:
  %v1958 = add i64 0, 0
  br label %b1347
b1346:
  store i64 0, ptr %gcs1
  %v1960 = icmp eq i64 %v0, 0
  %v1961 = call i64 @df.class(i64 %v0)
  %v1962 = call i8 @df.subclass(i64 %v1961, i64 674)
  %c425 = zext i8 %v1962 to i64
  %v1963 = icmp ne i64 %c425, 0
  %v1964 = xor i1 %v1960, true
  %c426 = zext i1 %v1963 to i64
  %c427 = zext i1 %v1964 to i64
  %v1965 = and i64 %c426, %c427
  %v1966 = icmp ne i64 %v1965, 0
  br i1 %v1966, label %b1349, label %b1348
b1347:
  %v1980 = phi i64 [ %v1958, %b1345 ], [ %v1979, %b1355 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1981 = call i64 @df.box_int(i64 %v1980)
  %xcp1982 = load i8, ptr %ctx, align 8
  %xcn1982 = icmp ne i8 %xcp1982, 0
  %xce1982 = call i1 @llvm.expect.i1(i1 %xcn1982, i1 false)
  br i1 %xce1982, label %xc1982.limpar, label %xc1982.fim
xc1982.limpar:
  call void @dartforge_exception_clear()
  br label %xc1982.fim
xc1982.fim:
  %gcvolta1347 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1347, ptr %ctxtopo, align 8
  ret i64 %v1981
b1348:
  %ga1967 = getelementptr i64, ptr %area, i64 13
  %v1967 = load i8, ptr %ga1967
  %c428 = zext i8 %v1967 to i64
  %v1968 = icmp ne i64 %c428, 0
  br i1 %v1968, label %b1350, label %b1351
b1349:
  %v1975 = call i64 @df.unbox_int(i64 %v0)
  %v1976 = load i8, ptr %ctx, align 8
  %c429 = zext i8 %v1976 to i64
  %v1977 = icmp ne i64 %c429, 0
  br i1 %v1977, label %b1356, label %b1355
b1350:
  %ga1969 = getelementptr i64, ptr %area, i64 12
  %v1969 = load i64, ptr %ga1969
  br label %b1352
b1351:
  %v1970 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1352
b1352:
  %v1971 = phi i64 [ %v1969, %b1350 ], [ %v1970, %b1351 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1971, i64 0, i64 0)
  %v1973 = load i8, ptr %ctx, align 8
  %c430 = zext i8 %v1973 to i64
  %v1974 = icmp ne i64 %c430, 0
  br i1 %v1974, label %b1354, label %b1353
b1353:
  br label %b1349
b1354:
  %gcvolta1354 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1354, ptr %ctxtopo, align 8
  ret i64 0
b1355:
  %v1978 = add i64 0, 29
  %v1979 = ashr i64 %v1975, %v1978
  br label %b1347
b1356:
  %gcvolta1356 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1356, ptr %ctxtopo, align 8
  ret i64 0
b1358:
  %v1987 = ashr i64 %v0, 1
  %v1988 = icmp sle i64 %v1987, %v1984
  br label %b1360
b1359:
  %v1989 = call i64 @df.box_int(i64 %v1984)
  store i64 %v1989, ptr %gcs1
  %sa1990_0 = getelementptr [2 x i64], ptr %sargs1990, i64 0, i64 0
  store i64 %v1989, ptr %sa1990_0
  %sat1990 = getelementptr [2 x i64], ptr %sargs1990, i64 0, i64 1
  store i64 0, ptr %sat1990
  %sic1990 = getelementptr i64, ptr %area, i64 312
  %sf1990 = call ptr @df.selector(ptr %sic1990, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v1990 = call i64 %sf1990(i64 %v0, ptr %sargs1990, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1990, ptr %gcs1
  %v1991 = load i8, ptr %ctx, align 8
  %c431 = zext i8 %v1991 to i64
  %v1992 = icmp ne i64 %c431, 0
  br i1 %v1992, label %b1362, label %b1361
b1360:
  %v1996 = phi i1 [ %v1988, %b1358 ], [ %v1993, %b1363 ]
  br i1 %v1996, label %b1365, label %b1366
b1361:
  %v1993 = call i1 @df.unbox_bool(i64 %v1990)
  %v1994 = load i8, ptr %ctx, align 8
  %c432 = zext i8 %v1994 to i64
  %v1995 = icmp ne i64 %c432, 0
  br i1 %v1995, label %b1364, label %b1363
b1362:
  %gcvolta1362 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1362, ptr %ctxtopo, align 8
  ret i64 0
b1363:
  br label %b1360
b1364:
  %gcvolta1364 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1364, ptr %ctxtopo, align 8
  ret i64 0
b1365:
  %v1997 = add i64 0, 0
  br label %b1367
b1366:
  store i64 0, ptr %gcs1
  %v1999 = icmp eq i64 %v0, 0
  %v2000 = call i64 @df.class(i64 %v0)
  %v2001 = call i8 @df.subclass(i64 %v2000, i64 674)
  %c433 = zext i8 %v2001 to i64
  %v2002 = icmp ne i64 %c433, 0
  %v2003 = xor i1 %v1999, true
  %c434 = zext i1 %v2002 to i64
  %c435 = zext i1 %v2003 to i64
  %v2004 = and i64 %c434, %c435
  %v2005 = icmp ne i64 %v2004, 0
  br i1 %v2005, label %b1369, label %b1368
b1367:
  %v2019 = phi i64 [ %v1997, %b1365 ], [ %v2018, %b1375 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2020 = call i64 @df.box_int(i64 %v2019)
  %xcp2021 = load i8, ptr %ctx, align 8
  %xcn2021 = icmp ne i8 %xcp2021, 0
  %xce2021 = call i1 @llvm.expect.i1(i1 %xcn2021, i1 false)
  br i1 %xce2021, label %xc2021.limpar, label %xc2021.fim
xc2021.limpar:
  call void @dartforge_exception_clear()
  br label %xc2021.fim
xc2021.fim:
  %gcvolta1367 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1367, ptr %ctxtopo, align 8
  ret i64 %v2020
b1368:
  %ga2006 = getelementptr i64, ptr %area, i64 13
  %v2006 = load i8, ptr %ga2006
  %c436 = zext i8 %v2006 to i64
  %v2007 = icmp ne i64 %c436, 0
  br i1 %v2007, label %b1370, label %b1371
b1369:
  %v2014 = call i64 @df.unbox_int(i64 %v0)
  %v2015 = load i8, ptr %ctx, align 8
  %c437 = zext i8 %v2015 to i64
  %v2016 = icmp ne i64 %c437, 0
  br i1 %v2016, label %b1376, label %b1375
b1370:
  %ga2008 = getelementptr i64, ptr %area, i64 12
  %v2008 = load i64, ptr %ga2008
  br label %b1372
b1371:
  %v2009 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1372
b1372:
  %v2010 = phi i64 [ %v2008, %b1370 ], [ %v2009, %b1371 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2010, i64 0, i64 0)
  %v2012 = load i8, ptr %ctx, align 8
  %c438 = zext i8 %v2012 to i64
  %v2013 = icmp ne i64 %c438, 0
  br i1 %v2013, label %b1374, label %b1373
b1373:
  br label %b1369
b1374:
  %gcvolta1374 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1374, ptr %ctxtopo, align 8
  ret i64 0
b1375:
  %v2017 = add i64 0, 27
  %v2018 = ashr i64 %v2014, %v2017
  br label %b1367
b1376:
  %gcvolta1376 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1376, ptr %ctxtopo, align 8
  ret i64 0
b1378:
  %v2026 = ashr i64 %v0, 1
  %v2027 = icmp sle i64 %v2026, %v2023
  br label %b1380
b1379:
  %v2028 = call i64 @df.box_int(i64 %v2023)
  store i64 %v2028, ptr %gcs1
  %sa2029_0 = getelementptr [2 x i64], ptr %sargs2029, i64 0, i64 0
  store i64 %v2028, ptr %sa2029_0
  %sat2029 = getelementptr [2 x i64], ptr %sargs2029, i64 0, i64 1
  store i64 0, ptr %sat2029
  %sic2029 = getelementptr i64, ptr %area, i64 314
  %sf2029 = call ptr @df.selector(ptr %sic2029, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v2029 = call i64 %sf2029(i64 %v0, ptr %sargs2029, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2029, ptr %gcs1
  %v2030 = load i8, ptr %ctx, align 8
  %c439 = zext i8 %v2030 to i64
  %v2031 = icmp ne i64 %c439, 0
  br i1 %v2031, label %b1382, label %b1381
b1380:
  %v2035 = phi i1 [ %v2027, %b1378 ], [ %v2032, %b1383 ]
  br i1 %v2035, label %b1385, label %b1386
b1381:
  %v2032 = call i1 @df.unbox_bool(i64 %v2029)
  %v2033 = load i8, ptr %ctx, align 8
  %c440 = zext i8 %v2033 to i64
  %v2034 = icmp ne i64 %c440, 0
  br i1 %v2034, label %b1384, label %b1383
b1382:
  %gcvolta1382 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1382, ptr %ctxtopo, align 8
  ret i64 0
b1383:
  br label %b1380
b1384:
  %gcvolta1384 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1384, ptr %ctxtopo, align 8
  ret i64 0
b1385:
  %v2036 = add i64 0, 0
  br label %b1387
b1386:
  store i64 0, ptr %gcs1
  %v2038 = icmp eq i64 %v0, 0
  %v2039 = call i64 @df.class(i64 %v0)
  %v2040 = call i8 @df.subclass(i64 %v2039, i64 674)
  %c441 = zext i8 %v2040 to i64
  %v2041 = icmp ne i64 %c441, 0
  %v2042 = xor i1 %v2038, true
  %c442 = zext i1 %v2041 to i64
  %c443 = zext i1 %v2042 to i64
  %v2043 = and i64 %c442, %c443
  %v2044 = icmp ne i64 %v2043, 0
  br i1 %v2044, label %b1389, label %b1388
b1387:
  %v2058 = phi i64 [ %v2036, %b1385 ], [ %v2057, %b1395 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2059 = call i64 @df.box_int(i64 %v2058)
  %xcp2060 = load i8, ptr %ctx, align 8
  %xcn2060 = icmp ne i8 %xcp2060, 0
  %xce2060 = call i1 @llvm.expect.i1(i1 %xcn2060, i1 false)
  br i1 %xce2060, label %xc2060.limpar, label %xc2060.fim
xc2060.limpar:
  call void @dartforge_exception_clear()
  br label %xc2060.fim
xc2060.fim:
  %gcvolta1387 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1387, ptr %ctxtopo, align 8
  ret i64 %v2059
b1388:
  %ga2045 = getelementptr i64, ptr %area, i64 13
  %v2045 = load i8, ptr %ga2045
  %c444 = zext i8 %v2045 to i64
  %v2046 = icmp ne i64 %c444, 0
  br i1 %v2046, label %b1390, label %b1391
b1389:
  %v2053 = call i64 @df.unbox_int(i64 %v0)
  %v2054 = load i8, ptr %ctx, align 8
  %c445 = zext i8 %v2054 to i64
  %v2055 = icmp ne i64 %c445, 0
  br i1 %v2055, label %b1396, label %b1395
b1390:
  %ga2047 = getelementptr i64, ptr %area, i64 12
  %v2047 = load i64, ptr %ga2047
  br label %b1392
b1391:
  %v2048 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1392
b1392:
  %v2049 = phi i64 [ %v2047, %b1390 ], [ %v2048, %b1391 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2049, i64 0, i64 0)
  %v2051 = load i8, ptr %ctx, align 8
  %c446 = zext i8 %v2051 to i64
  %v2052 = icmp ne i64 %c446, 0
  br i1 %v2052, label %b1394, label %b1393
b1393:
  br label %b1389
b1394:
  %gcvolta1394 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1394, ptr %ctxtopo, align 8
  ret i64 0
b1395:
  %v2056 = add i64 0, 23
  %v2057 = ashr i64 %v2053, %v2056
  br label %b1387
b1396:
  %gcvolta1396 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1396, ptr %ctxtopo, align 8
  ret i64 0
b1398:
  %v2065 = ashr i64 %v0, 1
  %v2066 = icmp sle i64 %v2065, %v2062
  br label %b1400
b1399:
  %v2067 = call i64 @df.box_int(i64 %v2062)
  store i64 %v2067, ptr %gcs1
  %sa2068_0 = getelementptr [2 x i64], ptr %sargs2068, i64 0, i64 0
  store i64 %v2067, ptr %sa2068_0
  %sat2068 = getelementptr [2 x i64], ptr %sargs2068, i64 0, i64 1
  store i64 0, ptr %sat2068
  %sic2068 = getelementptr i64, ptr %area, i64 316
  %sf2068 = call ptr @df.selector(ptr %sic2068, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v2068 = call i64 %sf2068(i64 %v0, ptr %sargs2068, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2068, ptr %gcs1
  %v2069 = load i8, ptr %ctx, align 8
  %c447 = zext i8 %v2069 to i64
  %v2070 = icmp ne i64 %c447, 0
  br i1 %v2070, label %b1402, label %b1401
b1400:
  %v2074 = phi i1 [ %v2066, %b1398 ], [ %v2071, %b1403 ]
  br i1 %v2074, label %b1405, label %b1406
b1401:
  %v2071 = call i1 @df.unbox_bool(i64 %v2068)
  %v2072 = load i8, ptr %ctx, align 8
  %c448 = zext i8 %v2072 to i64
  %v2073 = icmp ne i64 %c448, 0
  br i1 %v2073, label %b1404, label %b1403
b1402:
  %gcvolta1402 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1402, ptr %ctxtopo, align 8
  ret i64 0
b1403:
  br label %b1400
b1404:
  %gcvolta1404 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1404, ptr %ctxtopo, align 8
  ret i64 0
b1405:
  %v2075 = add i64 0, 0
  br label %b1407
b1406:
  store i64 0, ptr %gcs1
  %v2077 = icmp eq i64 %v0, 0
  %v2078 = call i64 @df.class(i64 %v0)
  %v2079 = call i8 @df.subclass(i64 %v2078, i64 674)
  %c449 = zext i8 %v2079 to i64
  %v2080 = icmp ne i64 %c449, 0
  %v2081 = xor i1 %v2077, true
  %c450 = zext i1 %v2080 to i64
  %c451 = zext i1 %v2081 to i64
  %v2082 = and i64 %c450, %c451
  %v2083 = icmp ne i64 %v2082, 0
  br i1 %v2083, label %b1409, label %b1408
b1407:
  %v2097 = phi i64 [ %v2075, %b1405 ], [ %v2096, %b1415 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2098 = call i64 @df.box_int(i64 %v2097)
  %xcp2099 = load i8, ptr %ctx, align 8
  %xcn2099 = icmp ne i8 %xcp2099, 0
  %xce2099 = call i1 @llvm.expect.i1(i1 %xcn2099, i1 false)
  br i1 %xce2099, label %xc2099.limpar, label %xc2099.fim
xc2099.limpar:
  call void @dartforge_exception_clear()
  br label %xc2099.fim
xc2099.fim:
  %gcvolta1407 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1407, ptr %ctxtopo, align 8
  ret i64 %v2098
b1408:
  %ga2084 = getelementptr i64, ptr %area, i64 13
  %v2084 = load i8, ptr %ga2084
  %c452 = zext i8 %v2084 to i64
  %v2085 = icmp ne i64 %c452, 0
  br i1 %v2085, label %b1410, label %b1411
b1409:
  %v2092 = call i64 @df.unbox_int(i64 %v0)
  %v2093 = load i8, ptr %ctx, align 8
  %c453 = zext i8 %v2093 to i64
  %v2094 = icmp ne i64 %c453, 0
  br i1 %v2094, label %b1416, label %b1415
b1410:
  %ga2086 = getelementptr i64, ptr %area, i64 12
  %v2086 = load i64, ptr %ga2086
  br label %b1412
b1411:
  %v2087 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1412
b1412:
  %v2088 = phi i64 [ %v2086, %b1410 ], [ %v2087, %b1411 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2088, i64 0, i64 0)
  %v2090 = load i8, ptr %ctx, align 8
  %c454 = zext i8 %v2090 to i64
  %v2091 = icmp ne i64 %c454, 0
  br i1 %v2091, label %b1414, label %b1413
b1413:
  br label %b1409
b1414:
  %gcvolta1414 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1414, ptr %ctxtopo, align 8
  ret i64 0
b1415:
  %v2095 = add i64 0, 16
  %v2096 = ashr i64 %v2092, %v2095
  br label %b1407
b1416:
  %gcvolta1416 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1416, ptr %ctxtopo, align 8
  ret i64 0
b1418:
  %v2104 = ashr i64 %v0, 1
  %v2105 = icmp sle i64 %v2104, %v2101
  br label %b1420
b1419:
  %v2106 = call i64 @df.box_int(i64 %v2101)
  store i64 %v2106, ptr %gcs1
  %sa2107_0 = getelementptr [2 x i64], ptr %sargs2107, i64 0, i64 0
  store i64 %v2106, ptr %sa2107_0
  %sat2107 = getelementptr [2 x i64], ptr %sargs2107, i64 0, i64 1
  store i64 0, ptr %sat2107
  %sic2107 = getelementptr i64, ptr %area, i64 318
  %sf2107 = call ptr @df.selector(ptr %sic2107, i64 %v0, i64 2959114654109919793, ptr @df.seln.20, i64 5)
  %v2107 = call i64 %sf2107(i64 %v0, ptr %sargs2107, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2107, ptr %gcs1
  %v2108 = load i8, ptr %ctx, align 8
  %c455 = zext i8 %v2108 to i64
  %v2109 = icmp ne i64 %c455, 0
  br i1 %v2109, label %b1422, label %b1421
b1420:
  %v2113 = phi i1 [ %v2105, %b1418 ], [ %v2110, %b1423 ]
  br i1 %v2113, label %b1425, label %b1426
b1421:
  %v2110 = call i1 @df.unbox_bool(i64 %v2107)
  %v2111 = load i8, ptr %ctx, align 8
  %c456 = zext i8 %v2111 to i64
  %v2112 = icmp ne i64 %c456, 0
  br i1 %v2112, label %b1424, label %b1423
b1422:
  %gcvolta1422 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1422, ptr %ctxtopo, align 8
  ret i64 0
b1423:
  br label %b1420
b1424:
  %gcvolta1424 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1424, ptr %ctxtopo, align 8
  ret i64 0
b1425:
  %v2114 = add i64 0, 0
  br label %b1427
b1426:
  store i64 0, ptr %gcs1
  %v2116 = icmp eq i64 %v0, 0
  %v2117 = call i64 @df.class(i64 %v0)
  %v2118 = call i8 @df.subclass(i64 %v2117, i64 674)
  %c457 = zext i8 %v2118 to i64
  %v2119 = icmp ne i64 %c457, 0
  %v2120 = xor i1 %v2116, true
  %c458 = zext i1 %v2119 to i64
  %c459 = zext i1 %v2120 to i64
  %v2121 = and i64 %c458, %c459
  %v2122 = icmp ne i64 %v2121, 0
  br i1 %v2122, label %b1429, label %b1428
b1427:
  %v2136 = phi i64 [ %v2114, %b1425 ], [ %v2135, %b1435 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2137 = call i64 @df.box_int(i64 %v2136)
  %xcp2138 = load i8, ptr %ctx, align 8
  %xcn2138 = icmp ne i8 %xcp2138, 0
  %xce2138 = call i1 @llvm.expect.i1(i1 %xcn2138, i1 false)
  br i1 %xce2138, label %xc2138.limpar, label %xc2138.fim
xc2138.limpar:
  call void @dartforge_exception_clear()
  br label %xc2138.fim
xc2138.fim:
  %gcvolta1427 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1427, ptr %ctxtopo, align 8
  ret i64 %v2137
b1428:
  %ga2123 = getelementptr i64, ptr %area, i64 13
  %v2123 = load i8, ptr %ga2123
  %c460 = zext i8 %v2123 to i64
  %v2124 = icmp ne i64 %c460, 0
  br i1 %v2124, label %b1430, label %b1431
b1429:
  %v2131 = call i64 @df.unbox_int(i64 %v0)
  %v2132 = load i8, ptr %ctx, align 8
  %c461 = zext i8 %v2132 to i64
  %v2133 = icmp ne i64 %c461, 0
  br i1 %v2133, label %b1436, label %b1435
b1430:
  %ga2125 = getelementptr i64, ptr %area, i64 12
  %v2125 = load i64, ptr %ga2125
  br label %b1432
b1431:
  %v2126 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1432
b1432:
  %v2127 = phi i64 [ %v2125, %b1430 ], [ %v2126, %b1431 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2127, i64 0, i64 0)
  %v2129 = load i8, ptr %ctx, align 8
  %c462 = zext i8 %v2129 to i64
  %v2130 = icmp ne i64 %c462, 0
  br i1 %v2130, label %b1434, label %b1433
b1433:
  br label %b1429
b1434:
  %gcvolta1434 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1434, ptr %ctxtopo, align 8
  ret i64 0
b1435:
  %v2134 = add i64 0, 1
  %v2135 = shl i64 %v2131, %v2134
  br label %b1427
b1436:
  %gcvolta1436 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1436, ptr %ctxtopo, align 8
  ret i64 0
b1438:
  %ga2147 = getelementptr i64, ptr %area, i64 13
  %v2147 = load i8, ptr %ga2147
  %c463 = zext i8 %v2147 to i64
  %v2148 = icmp ne i64 %c463, 0
  br i1 %v2148, label %b1440, label %b1441
b1439:
  %v2155 = call i64 @df.unbox_int(i64 %v0)
  %v2156 = load i8, ptr %ctx, align 8
  %c464 = zext i8 %v2156 to i64
  %v2157 = icmp ne i64 %c464, 0
  br i1 %v2157, label %b1446, label %b1445
b1440:
  %ga2149 = getelementptr i64, ptr %area, i64 12
  %v2149 = load i64, ptr %ga2149
  br label %b1442
b1441:
  %v2150 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1442
b1442:
  %v2151 = phi i64 [ %v2149, %b1440 ], [ %v2150, %b1441 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2151, i64 0, i64 0)
  %v2153 = load i8, ptr %ctx, align 8
  %c465 = zext i8 %v2153 to i64
  %v2154 = icmp ne i64 %c465, 0
  br i1 %v2154, label %b1444, label %b1443
b1443:
  br label %b1439
b1444:
  %gcvolta1444 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1444, ptr %ctxtopo, align 8
  ret i64 0
b1445:
  %v2158 = add i64 0, 24
  %v2159 = ashr i64 %v2155, %v2158
  store i64 0, ptr %gcs0
  %v2160 = call i64 @df.box_int(i64 %v2159)
  %xcp2161 = load i8, ptr %ctx, align 8
  %xcn2161 = icmp ne i8 %xcp2161, 0
  %xce2161 = call i1 @llvm.expect.i1(i1 %xcn2161, i1 false)
  br i1 %xce2161, label %xc2161.limpar, label %xc2161.fim
xc2161.limpar:
  call void @dartforge_exception_clear()
  br label %xc2161.fim
xc2161.fim:
  %gcvolta1445 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1445, ptr %ctxtopo, align 8
  ret i64 %v2160
b1446:
  %gcvolta1446 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1446, ptr %ctxtopo, align 8
  ret i64 0
b1448:
  %ga2170 = getelementptr i64, ptr %area, i64 13
  %v2170 = load i8, ptr %ga2170
  %c466 = zext i8 %v2170 to i64
  %v2171 = icmp ne i64 %c466, 0
  br i1 %v2171, label %b1450, label %b1451
b1449:
  %v2178 = call i64 @df.unbox_int(i64 %v0)
  %v2179 = load i8, ptr %ctx, align 8
  %c467 = zext i8 %v2179 to i64
  %v2180 = icmp ne i64 %c467, 0
  br i1 %v2180, label %b1456, label %b1455
b1450:
  %ga2172 = getelementptr i64, ptr %area, i64 12
  %v2172 = load i64, ptr %ga2172
  br label %b1452
b1451:
  %v2173 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1452
b1452:
  %v2174 = phi i64 [ %v2172, %b1450 ], [ %v2173, %b1451 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2174, i64 0, i64 0)
  %v2176 = load i8, ptr %ctx, align 8
  %c468 = zext i8 %v2176 to i64
  %v2177 = icmp ne i64 %c468, 0
  br i1 %v2177, label %b1454, label %b1453
b1453:
  br label %b1449
b1454:
  %gcvolta1454 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1454, ptr %ctxtopo, align 8
  ret i64 0
b1455:
  %v2181 = add i64 0, 16
  %v2182 = ashr i64 %v2178, %v2181
  store i64 0, ptr %gcs0
  %v2183 = call i64 @df.box_int(i64 %v2182)
  %xcp2184 = load i8, ptr %ctx, align 8
  %xcn2184 = icmp ne i8 %xcp2184, 0
  %xce2184 = call i1 @llvm.expect.i1(i1 %xcn2184, i1 false)
  br i1 %xce2184, label %xc2184.limpar, label %xc2184.fim
xc2184.limpar:
  call void @dartforge_exception_clear()
  br label %xc2184.fim
xc2184.fim:
  %gcvolta1455 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1455, ptr %ctxtopo, align 8
  ret i64 %v2183
b1456:
  %gcvolta1456 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1456, ptr %ctxtopo, align 8
  ret i64 0
b1459:
  store i64 0, ptr %gcs1
  %v2193 = call double @df.unbox_double(i64 %v2190)
  %v2194 = load i8, ptr %ctx, align 8
  %c469 = zext i8 %v2194 to i64
  %v2195 = icmp ne i64 %c469, 0
  br i1 %v2195, label %b1462, label %b1461
b1460:
  %gcvolta1460 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1460, ptr %ctxtopo, align 8
  ret i64 0
b1461:
  store i64 0, ptr %gcs0
  %v2196 = call i64 @df.box_double(double %v2193)
  %xcp2197 = load i8, ptr %ctx, align 8
  %xcn2197 = icmp ne i8 %xcp2197, 0
  %xce2197 = call i1 @llvm.expect.i1(i1 %xcn2197, i1 false)
  br i1 %xce2197, label %xc2197.limpar, label %xc2197.fim
xc2197.limpar:
  call void @dartforge_exception_clear()
  br label %xc2197.fim
xc2197.fim:
  %gcvolta1461 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1461, ptr %ctxtopo, align 8
  ret i64 %v2196
b1462:
  %gcvolta1462 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1462, ptr %ctxtopo, align 8
  ret i64 0
b1476:
  %xcp2824 = load i8, ptr %ctx, align 8
  %xcn2824 = icmp ne i8 %xcp2824, 0
  %xce2824 = call i1 @llvm.expect.i1(i1 %xcn2824, i1 false)
  br i1 %xce2824, label %xc2824.limpar, label %xc2824.fim
xc2824.limpar:
  call void @dartforge_exception_clear()
  br label %xc2824.fim
xc2824.fim:
  %gcvolta1476 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1476, ptr %ctxtopo, align 8
  ret i64 %v0
b1477:
  store i64 0, ptr %gcs1
  %v2203 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v2203, ptr %gcs1
  %v2204 = load i8, ptr %ctx, align 8
  %c470 = zext i8 %v2204 to i64
  %v2205 = icmp ne i64 %c470, 0
  br i1 %v2205, label %b1483, label %b1482
b1478:
  %v2202 = icmp eq i64 %v2199, %v6
  br i1 %v2202, label %b1480, label %b1477
b1479:
  %gcvolta1479 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1479, ptr %ctxtopo, align 8
  ret i64 0
b1480:
  %v2248 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2249 = call i64 @df.box_int(i64 %v2248)
  store i64 %v2249, ptr %gcs1
  %v2250 = icmp eq i64 %v0, 0
  %v2251 = icmp eq i64 %v2249, 0
  %c471 = zext i1 %v2250 to i64
  %c472 = zext i1 %v2251 to i64
  %v2252 = or i64 %c471, %c472
  %v2253 = icmp ne i64 %v2252, 0
  br i1 %v2253, label %b1527, label %b1528
b1481:
  store i64 0, ptr %gcs1
  %v2207 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v2207, ptr %gcs1
  %v2208 = load i8, ptr %ctx, align 8
  %c473 = zext i8 %v2208 to i64
  %v2209 = icmp ne i64 %c473, 0
  br i1 %v2209, label %b1487, label %b1486
b1482:
  %v2206 = icmp eq i64 %v2203, %v6
  br i1 %v2206, label %b1484, label %b1481
b1483:
  %gcvolta1483 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1483, ptr %ctxtopo, align 8
  ret i64 0
b1484:
  %v2280 = add i64 0, 0
  %v2281 = add i64 0, 1
  %v2282 = call i64 @df.class(i64 %v0)
  switch i64 %v2282, label %b1551 [ i64 4, label %b1549 i64 2, label %b1550 i64 3, label %b1550 ]
b1485:
  store i64 0, ptr %gcs1
  %v2211 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v2211, ptr %gcs1
  %v2212 = load i8, ptr %ctx, align 8
  %c474 = zext i8 %v2212 to i64
  %v2213 = icmp ne i64 %c474, 0
  br i1 %v2213, label %b1491, label %b1490
b1486:
  %v2210 = icmp eq i64 %v2207, %v6
  br i1 %v2210, label %b1488, label %b1485
b1487:
  %gcvolta1487 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1487, ptr %ctxtopo, align 8
  ret i64 0
b1488:
  %v2330 = add i64 0, 0
  %v2331 = add i64 0, 1
  %v2332 = call i64 @df.class(i64 %v0)
  switch i64 %v2332, label %b1581 [ i64 4, label %b1579 i64 2, label %b1580 i64 3, label %b1580 ]
b1489:
  store i64 0, ptr %gcs1
  %v2215 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v2215, ptr %gcs1
  %v2216 = load i8, ptr %ctx, align 8
  %c475 = zext i8 %v2216 to i64
  %v2217 = icmp ne i64 %c475, 0
  br i1 %v2217, label %b1495, label %b1494
b1490:
  %v2214 = icmp eq i64 %v2211, %v6
  br i1 %v2214, label %b1492, label %b1489
b1491:
  %gcvolta1491 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1491, ptr %ctxtopo, align 8
  ret i64 0
b1492:
  %v2380 = add i64 0, 0
  %v2381 = add i64 0, 1
  %v2382 = call i64 @df.class(i64 %v0)
  switch i64 %v2382, label %b1611 [ i64 4, label %b1609 i64 2, label %b1610 i64 3, label %b1610 ]
b1493:
  store i64 0, ptr %gcs1
  %v2219 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v2219, ptr %gcs1
  %v2220 = load i8, ptr %ctx, align 8
  %c476 = zext i8 %v2220 to i64
  %v2221 = icmp ne i64 %c476, 0
  br i1 %v2221, label %b1499, label %b1498
b1494:
  %v2218 = icmp eq i64 %v2215, %v6
  br i1 %v2218, label %b1496, label %b1493
b1495:
  %gcvolta1495 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1495, ptr %ctxtopo, align 8
  ret i64 0
b1496:
  %v2430 = add i64 0, 0
  %v2431 = add i64 0, 1
  %v2432 = call i64 @df.class(i64 %v0)
  switch i64 %v2432, label %b1641 [ i64 4, label %b1639 i64 2, label %b1640 i64 3, label %b1640 ]
b1497:
  store i64 0, ptr %gcs1
  %v2223 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v2223, ptr %gcs1
  %v2224 = load i8, ptr %ctx, align 8
  %c477 = zext i8 %v2224 to i64
  %v2225 = icmp ne i64 %c477, 0
  br i1 %v2225, label %b1503, label %b1502
b1498:
  %v2222 = icmp eq i64 %v2219, %v6
  br i1 %v2222, label %b1500, label %b1497
b1499:
  %gcvolta1499 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1499, ptr %ctxtopo, align 8
  ret i64 0
b1500:
  %v2480 = add i64 0, 0
  %v2481 = add i64 0, 1
  %v2482 = call i64 @df.class(i64 %v0)
  switch i64 %v2482, label %b1671 [ i64 4, label %b1669 i64 2, label %b1670 i64 3, label %b1670 ]
b1501:
  store i64 0, ptr %gcs1
  %v2227 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v2227, ptr %gcs1
  %v2228 = load i8, ptr %ctx, align 8
  %c478 = zext i8 %v2228 to i64
  %v2229 = icmp ne i64 %c478, 0
  br i1 %v2229, label %b1507, label %b1506
b1502:
  %v2226 = icmp eq i64 %v2223, %v6
  br i1 %v2226, label %b1504, label %b1501
b1503:
  %gcvolta1503 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1503, ptr %ctxtopo, align 8
  ret i64 0
b1504:
  %v2530 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2531 = and i64 %v0, 1
  %v2532 = icmp ne i64 %v2531, 0
  br i1 %v2532, label %b1699, label %b1700
b1505:
  store i64 0, ptr %gcs1
  %v2231 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v2231, ptr %gcs1
  %v2232 = load i8, ptr %ctx, align 8
  %c479 = zext i8 %v2232 to i64
  %v2233 = icmp ne i64 %c479, 0
  br i1 %v2233, label %b1511, label %b1510
b1506:
  %v2230 = icmp eq i64 %v2227, %v6
  br i1 %v2230, label %b1508, label %b1505
b1507:
  %gcvolta1507 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1507, ptr %ctxtopo, align 8
  ret i64 0
b1508:
  %v2628 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2629 = and i64 %v0, 1
  %v2630 = icmp ne i64 %v2629, 0
  br i1 %v2630, label %b1754, label %b1755
b1509:
  store i64 0, ptr %gcs1
  %v2235 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v2235, ptr %gcs1
  %v2236 = load i8, ptr %ctx, align 8
  %c480 = zext i8 %v2236 to i64
  %v2237 = icmp ne i64 %c480, 0
  br i1 %v2237, label %b1515, label %b1514
b1510:
  %v2234 = icmp eq i64 %v2231, %v6
  br i1 %v2234, label %b1512, label %b1509
b1511:
  %gcvolta1511 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1511, ptr %ctxtopo, align 8
  ret i64 0
b1512:
  %v2726 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2727 = and i64 %v0, 1
  %v2728 = icmp ne i64 %v2727, 0
  br i1 %v2728, label %b1809, label %b1810
b1513:
  store i64 0, ptr %gcs1
  %v2239 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v2239, ptr %gcs1
  %v2240 = load i8, ptr %ctx, align 8
  %c481 = zext i8 %v2240 to i64
  %v2241 = icmp ne i64 %c481, 0
  br i1 %v2241, label %b1520, label %b1519
b1514:
  %v2238 = icmp eq i64 %v2235, %v6
  br i1 %v2238, label %b1516, label %b1513
b1515:
  %gcvolta1515 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1515, ptr %ctxtopo, align 8
  ret i64 0
b1516:
  br label %b1476
b1518:
  store i64 0, ptr %gcs1
  %v2243 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v2243, ptr %gcs1
  %v2244 = load i8, ptr %ctx, align 8
  %c482 = zext i8 %v2244 to i64
  %v2245 = icmp ne i64 %c482, 0
  br i1 %v2245, label %b1525, label %b1524
b1519:
  %v2242 = icmp eq i64 %v2239, %v6
  br i1 %v2242, label %b1521, label %b1518
b1520:
  %gcvolta1520 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1520, ptr %ctxtopo, align 8
  ret i64 0
b1521:
  br label %b1476
b1523:
  br label %b12
b1524:
  %v2246 = icmp eq i64 %v2243, %v6
  br i1 %v2246, label %b1526, label %b1523
b1525:
  %gcvolta1525 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1525, ptr %ctxtopo, align 8
  ret i64 0
b1526:
  br label %b1476
b1527:
  %v2254 = icmp eq i64 %v0, %v2249
  br label %b1529
b1528:
  %v2255 = call i64 @df.class(i64 %v0)
  switch i64 %v2255, label %b1533 [ i64 675, label %b1530 i64 4, label %b1531 i64 2, label %b1532 i64 3, label %b1532 ]
b1529:
  %v2273 = phi i1 [ %v2254, %b1527 ], [ %v2272, %b1534 ]
  br i1 %v2273, label %b1545, label %b1546
b1530:
  %v2256 = call i8 @dartforge_native_Object_equals(i64 %v0, i64 %v2249)
  %v2257 = load i8, ptr %ctx, align 8
  %c483 = zext i8 %v2257 to i64
  %v2258 = icmp ne i64 %c483, 0
  br i1 %v2258, label %b1536, label %b1535
b1531:
  %v2260 = call i1 @df.dart$3acore._Double.$3d$3d(i64 %v0, i64 %v2249)
  %v2261 = load i8, ptr %ctx, align 8
  %c484 = zext i8 %v2261 to i64
  %v2262 = icmp ne i64 %c484, 0
  br i1 %v2262, label %b1538, label %b1537
b1532:
  %v2263 = call i1 @df.dart$3acore._IntegerImplementation.$3d$3d(i64 %v0, i64 %v2249)
  %v2264 = load i8, ptr %ctx, align 8
  %c485 = zext i8 %v2264 to i64
  %v2265 = icmp ne i64 %c485, 0
  br i1 %v2265, label %b1540, label %b1539
b1533:
  %sa2266_0 = getelementptr [2 x i64], ptr %sargs2266, i64 0, i64 0
  store i64 %v2249, ptr %sa2266_0
  %sat2266 = getelementptr [2 x i64], ptr %sargs2266, i64 0, i64 1
  store i64 0, ptr %sat2266
  %sic2266 = getelementptr i64, ptr %area, i64 320
  %sf2266 = call ptr @df.selector(ptr %sic2266, i64 %v0, i64 -9163797671095228218, ptr @df.seln.17, i64 4)
  %v2266 = call i64 %sf2266(i64 %v0, ptr %sargs2266, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2266, ptr %gcs0
  %v2267 = load i8, ptr %ctx, align 8
  %c486 = zext i8 %v2267 to i64
  %v2268 = icmp ne i64 %c486, 0
  br i1 %v2268, label %b1542, label %b1541
b1534:
  %v2272 = phi i1 [ %v2259, %b1535 ], [ %v2260, %b1537 ], [ %v2263, %b1539 ], [ %v2269, %b1543 ]
  br label %b1529
b1535:
  %c487 = zext i8 %v2256 to i64
  %v2259 = icmp ne i64 %c487, 0
  br label %b1534
b1536:
  %gcvolta1536 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1536, ptr %ctxtopo, align 8
  ret i64 0
b1537:
  br label %b1534
b1538:
  %gcvolta1538 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1538, ptr %ctxtopo, align 8
  ret i64 0
b1539:
  br label %b1534
b1540:
  %gcvolta1540 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1540, ptr %ctxtopo, align 8
  ret i64 0
b1541:
  store i64 0, ptr %gcs1
  %v2269 = call i1 @df.unbox_bool(i64 %v2266)
  %v2270 = load i8, ptr %ctx, align 8
  %c488 = zext i8 %v2270 to i64
  %v2271 = icmp ne i64 %c488, 0
  br i1 %v2271, label %b1544, label %b1543
b1542:
  %gcvolta1542 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1542, ptr %ctxtopo, align 8
  ret i64 0
b1543:
  br label %b1534
b1544:
  %gcvolta1544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1544, ptr %ctxtopo, align 8
  ret i64 0
b1545:
  %v2274 = add i64 0, 0
  br label %b1547
b1546:
  %v2275 = add i64 0, 1
  br label %b1547
b1547:
  %v2276 = phi i64 [ %v2274, %b1545 ], [ %v2275, %b1546 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2277 = call i64 @df.box_int(i64 %v2276)
  %xcp2278 = load i8, ptr %ctx, align 8
  %xcn2278 = icmp ne i8 %xcp2278, 0
  %xce2278 = call i1 @llvm.expect.i1(i1 %xcn2278, i1 false)
  br i1 %xce2278, label %xc2278.limpar, label %xc2278.fim
xc2278.limpar:
  call void @dartforge_exception_clear()
  br label %xc2278.fim
xc2278.fim:
  %gcvolta1547 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1547, ptr %ctxtopo, align 8
  ret i64 %v2277
b1549:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2283 = call i64 @df.box_int(i64 %v2280)
  store i64 %v2283, ptr %gcs1
  %v2284 = call i64 @df.box_int(i64 %v2281)
  store i64 %v2284, ptr %gcs2
  %v2285 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2283, i64 %v2284)
  %v2286 = load i8, ptr %ctx, align 8
  %c489 = zext i8 %v2286 to i64
  %v2287 = icmp ne i64 %c489, 0
  br i1 %v2287, label %b1554, label %b1553
b1550:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2288 = call i64 @df.box_int(i64 %v2280)
  store i64 %v2288, ptr %gcs1
  %v2289 = call i64 @df.box_int(i64 %v2281)
  store i64 %v2289, ptr %gcs2
  %v2290 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2288, i64 %v2289)
  %v2291 = load i8, ptr %ctx, align 8
  %c490 = zext i8 %v2291 to i64
  %v2292 = icmp ne i64 %c490, 0
  br i1 %v2292, label %b1556, label %b1555
b1551:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2293 = call i64 @df.box_int(i64 %v2280)
  store i64 %v2293, ptr %gcs1
  %v2294 = call i64 @df.box_int(i64 %v2281)
  store i64 %v2294, ptr %gcs2
  %sa2295_0 = getelementptr [3 x i64], ptr %sargs2295, i64 0, i64 0
  store i64 %v2293, ptr %sa2295_0
  %sa2295_1 = getelementptr [3 x i64], ptr %sargs2295, i64 0, i64 1
  store i64 %v2294, ptr %sa2295_1
  %sat2295 = getelementptr [3 x i64], ptr %sargs2295, i64 0, i64 2
  store i64 0, ptr %sat2295
  %sic2295 = getelementptr i64, ptr %area, i64 322
  %sf2295 = call ptr @df.selector(ptr %sic2295, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2295 = call i64 %sf2295(i64 %v0, ptr %sargs2295, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2296 = load i8, ptr %ctx, align 8
  %c491 = zext i8 %v2296 to i64
  %v2297 = icmp ne i64 %c491, 0
  br i1 %v2297, label %b1558, label %b1557
b1552:
  %v2298 = phi i64 [ %v2285, %b1553 ], [ %v2290, %b1555 ], [ %v2295, %b1557 ]
  store i64 %v2298, ptr %gcs0
  %v2299 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2300 = and i64 %v2298, 1
  %v2301 = icmp ne i64 %v2300, 0
  br i1 %v2301, label %b1559, label %b1560
b1553:
  br label %b1552
b1554:
  %gcvolta1554 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1554, ptr %ctxtopo, align 8
  ret i64 0
b1555:
  br label %b1552
b1556:
  %gcvolta1556 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1556, ptr %ctxtopo, align 8
  ret i64 0
b1557:
  br label %b1552
b1558:
  %gcvolta1558 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1558, ptr %ctxtopo, align 8
  ret i64 0
b1559:
  %v2302 = ashr i64 %v2298, 1
  %v2303 = mul i64 %v2302, %v2299
  store i64 0, ptr %gcs0
  %v2304 = call i64 @df.box_int(i64 %v2303)
  br label %b1561
b1560:
  %v2305 = call i64 @df.box_int(i64 %v2299)
  store i64 %v2305, ptr %gcs1
  %sa2306_0 = getelementptr [2 x i64], ptr %sargs2306, i64 0, i64 0
  store i64 %v2305, ptr %sa2306_0
  %sat2306 = getelementptr [2 x i64], ptr %sargs2306, i64 0, i64 1
  store i64 0, ptr %sat2306
  %sic2306 = getelementptr i64, ptr %area, i64 324
  %sf2306 = call ptr @df.selector(ptr %sic2306, i64 %v2298, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2306 = call i64 %sf2306(i64 %v2298, ptr %sargs2306, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2307 = load i8, ptr %ctx, align 8
  %c492 = zext i8 %v2307 to i64
  %v2308 = icmp ne i64 %c492, 0
  br i1 %v2308, label %b1563, label %b1562
b1561:
  %v2309 = phi i64 [ %v2304, %b1559 ], [ %v2306, %b1562 ]
  store i64 %v2309, ptr %gcs0
  %v2310 = call i64 @df.class(i64 %v2309)
  switch i64 %v2310, label %b1566 [ i64 4, label %b1564 i64 2, label %b1565 i64 3, label %b1565 ]
b1562:
  br label %b1561
b1563:
  %gcvolta1563 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1563, ptr %ctxtopo, align 8
  ret i64 0
b1564:
  store i64 0, ptr %gcs1
  %v2311 = call double @df.unbox_double(i64 %v2309)
  %v2312 = load i8, ptr %ctx, align 8
  %c493 = zext i8 %v2312 to i64
  %v2313 = icmp ne i64 %c493, 0
  br i1 %v2313, label %b1569, label %b1568
b1565:
  store i64 0, ptr %gcs1
  %v2317 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2309)
  %v2318 = load i8, ptr %ctx, align 8
  %c494 = zext i8 %v2318 to i64
  %v2319 = icmp ne i64 %c494, 0
  br i1 %v2319, label %b1573, label %b1572
b1566:
  store i64 0, ptr %gcs1
  %sat2320 = getelementptr [1 x i64], ptr %sargs2320, i64 0, i64 0
  store i64 0, ptr %sat2320
  %sic2320 = getelementptr i64, ptr %area, i64 326
  %sf2320 = call ptr @df.selector(ptr %sic2320, i64 %v2309, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2320 = call i64 %sf2320(i64 %v2309, ptr %sargs2320, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2320, ptr %gcs0
  %v2321 = load i8, ptr %ctx, align 8
  %c495 = zext i8 %v2321 to i64
  %v2322 = icmp ne i64 %c495, 0
  br i1 %v2322, label %b1575, label %b1574
b1567:
  %v2326 = phi i64 [ %v2314, %b1570 ], [ %v2317, %b1572 ], [ %v2323, %b1576 ]
  store i64 0, ptr %gcs0
  %v2327 = call i64 @df.box_int(i64 %v2326)
  %xcp2328 = load i8, ptr %ctx, align 8
  %xcn2328 = icmp ne i8 %xcp2328, 0
  %xce2328 = call i1 @llvm.expect.i1(i1 %xcn2328, i1 false)
  br i1 %xce2328, label %xc2328.limpar, label %xc2328.fim
xc2328.limpar:
  call void @dartforge_exception_clear()
  br label %xc2328.fim
xc2328.fim:
  %gcvolta1567 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1567, ptr %ctxtopo, align 8
  ret i64 %v2327
b1568:
  store i64 0, ptr %gcs0
  %v2314 = call i64 @dartforge_native_DartForge_double_toInt(double %v2311)
  %v2315 = load i8, ptr %ctx, align 8
  %c496 = zext i8 %v2315 to i64
  %v2316 = icmp ne i64 %c496, 0
  br i1 %v2316, label %b1571, label %b1570
b1569:
  %gcvolta1569 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1569, ptr %ctxtopo, align 8
  ret i64 0
b1570:
  br label %b1567
b1571:
  %gcvolta1571 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1571, ptr %ctxtopo, align 8
  ret i64 0
b1572:
  br label %b1567
b1573:
  %gcvolta1573 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1573, ptr %ctxtopo, align 8
  ret i64 0
b1574:
  %v2323 = call i64 @df.unbox_int(i64 %v2320)
  %v2324 = load i8, ptr %ctx, align 8
  %c497 = zext i8 %v2324 to i64
  %v2325 = icmp ne i64 %c497, 0
  br i1 %v2325, label %b1577, label %b1576
b1575:
  %gcvolta1575 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1575, ptr %ctxtopo, align 8
  ret i64 0
b1576:
  br label %b1567
b1577:
  %gcvolta1577 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1577, ptr %ctxtopo, align 8
  ret i64 0
b1579:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2333 = call i64 @df.box_int(i64 %v2330)
  store i64 %v2333, ptr %gcs1
  %v2334 = call i64 @df.box_int(i64 %v2331)
  store i64 %v2334, ptr %gcs2
  %v2335 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2333, i64 %v2334)
  %v2336 = load i8, ptr %ctx, align 8
  %c498 = zext i8 %v2336 to i64
  %v2337 = icmp ne i64 %c498, 0
  br i1 %v2337, label %b1584, label %b1583
b1580:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2338 = call i64 @df.box_int(i64 %v2330)
  store i64 %v2338, ptr %gcs1
  %v2339 = call i64 @df.box_int(i64 %v2331)
  store i64 %v2339, ptr %gcs2
  %v2340 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2338, i64 %v2339)
  %v2341 = load i8, ptr %ctx, align 8
  %c499 = zext i8 %v2341 to i64
  %v2342 = icmp ne i64 %c499, 0
  br i1 %v2342, label %b1586, label %b1585
b1581:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2343 = call i64 @df.box_int(i64 %v2330)
  store i64 %v2343, ptr %gcs1
  %v2344 = call i64 @df.box_int(i64 %v2331)
  store i64 %v2344, ptr %gcs2
  %sa2345_0 = getelementptr [3 x i64], ptr %sargs2345, i64 0, i64 0
  store i64 %v2343, ptr %sa2345_0
  %sa2345_1 = getelementptr [3 x i64], ptr %sargs2345, i64 0, i64 1
  store i64 %v2344, ptr %sa2345_1
  %sat2345 = getelementptr [3 x i64], ptr %sargs2345, i64 0, i64 2
  store i64 0, ptr %sat2345
  %sic2345 = getelementptr i64, ptr %area, i64 328
  %sf2345 = call ptr @df.selector(ptr %sic2345, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2345 = call i64 %sf2345(i64 %v0, ptr %sargs2345, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2346 = load i8, ptr %ctx, align 8
  %c500 = zext i8 %v2346 to i64
  %v2347 = icmp ne i64 %c500, 0
  br i1 %v2347, label %b1588, label %b1587
b1582:
  %v2348 = phi i64 [ %v2335, %b1583 ], [ %v2340, %b1585 ], [ %v2345, %b1587 ]
  store i64 %v2348, ptr %gcs0
  %v2349 = add i64 0, 15
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2350 = and i64 %v2348, 1
  %v2351 = icmp ne i64 %v2350, 0
  br i1 %v2351, label %b1589, label %b1590
b1583:
  br label %b1582
b1584:
  %gcvolta1584 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1584, ptr %ctxtopo, align 8
  ret i64 0
b1585:
  br label %b1582
b1586:
  %gcvolta1586 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1586, ptr %ctxtopo, align 8
  ret i64 0
b1587:
  br label %b1582
b1588:
  %gcvolta1588 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1588, ptr %ctxtopo, align 8
  ret i64 0
b1589:
  %v2352 = ashr i64 %v2348, 1
  %v2353 = mul i64 %v2352, %v2349
  store i64 0, ptr %gcs0
  %v2354 = call i64 @df.box_int(i64 %v2353)
  br label %b1591
b1590:
  %v2355 = call i64 @df.box_int(i64 %v2349)
  store i64 %v2355, ptr %gcs1
  %sa2356_0 = getelementptr [2 x i64], ptr %sargs2356, i64 0, i64 0
  store i64 %v2355, ptr %sa2356_0
  %sat2356 = getelementptr [2 x i64], ptr %sargs2356, i64 0, i64 1
  store i64 0, ptr %sat2356
  %sic2356 = getelementptr i64, ptr %area, i64 330
  %sf2356 = call ptr @df.selector(ptr %sic2356, i64 %v2348, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2356 = call i64 %sf2356(i64 %v2348, ptr %sargs2356, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2357 = load i8, ptr %ctx, align 8
  %c501 = zext i8 %v2357 to i64
  %v2358 = icmp ne i64 %c501, 0
  br i1 %v2358, label %b1593, label %b1592
b1591:
  %v2359 = phi i64 [ %v2354, %b1589 ], [ %v2356, %b1592 ]
  store i64 %v2359, ptr %gcs0
  %v2360 = call i64 @df.class(i64 %v2359)
  switch i64 %v2360, label %b1596 [ i64 4, label %b1594 i64 2, label %b1595 i64 3, label %b1595 ]
b1592:
  br label %b1591
b1593:
  %gcvolta1593 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1593, ptr %ctxtopo, align 8
  ret i64 0
b1594:
  store i64 0, ptr %gcs1
  %v2361 = call double @df.unbox_double(i64 %v2359)
  %v2362 = load i8, ptr %ctx, align 8
  %c502 = zext i8 %v2362 to i64
  %v2363 = icmp ne i64 %c502, 0
  br i1 %v2363, label %b1599, label %b1598
b1595:
  store i64 0, ptr %gcs1
  %v2367 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2359)
  %v2368 = load i8, ptr %ctx, align 8
  %c503 = zext i8 %v2368 to i64
  %v2369 = icmp ne i64 %c503, 0
  br i1 %v2369, label %b1603, label %b1602
b1596:
  store i64 0, ptr %gcs1
  %sat2370 = getelementptr [1 x i64], ptr %sargs2370, i64 0, i64 0
  store i64 0, ptr %sat2370
  %sic2370 = getelementptr i64, ptr %area, i64 332
  %sf2370 = call ptr @df.selector(ptr %sic2370, i64 %v2359, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2370 = call i64 %sf2370(i64 %v2359, ptr %sargs2370, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2370, ptr %gcs0
  %v2371 = load i8, ptr %ctx, align 8
  %c504 = zext i8 %v2371 to i64
  %v2372 = icmp ne i64 %c504, 0
  br i1 %v2372, label %b1605, label %b1604
b1597:
  %v2376 = phi i64 [ %v2364, %b1600 ], [ %v2367, %b1602 ], [ %v2373, %b1606 ]
  store i64 0, ptr %gcs0
  %v2377 = call i64 @df.box_int(i64 %v2376)
  %xcp2378 = load i8, ptr %ctx, align 8
  %xcn2378 = icmp ne i8 %xcp2378, 0
  %xce2378 = call i1 @llvm.expect.i1(i1 %xcn2378, i1 false)
  br i1 %xce2378, label %xc2378.limpar, label %xc2378.fim
xc2378.limpar:
  call void @dartforge_exception_clear()
  br label %xc2378.fim
xc2378.fim:
  %gcvolta1597 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1597, ptr %ctxtopo, align 8
  ret i64 %v2377
b1598:
  store i64 0, ptr %gcs0
  %v2364 = call i64 @dartforge_native_DartForge_double_toInt(double %v2361)
  %v2365 = load i8, ptr %ctx, align 8
  %c505 = zext i8 %v2365 to i64
  %v2366 = icmp ne i64 %c505, 0
  br i1 %v2366, label %b1601, label %b1600
b1599:
  %gcvolta1599 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1599, ptr %ctxtopo, align 8
  ret i64 0
b1600:
  br label %b1597
b1601:
  %gcvolta1601 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1601, ptr %ctxtopo, align 8
  ret i64 0
b1602:
  br label %b1597
b1603:
  %gcvolta1603 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1603, ptr %ctxtopo, align 8
  ret i64 0
b1604:
  %v2373 = call i64 @df.unbox_int(i64 %v2370)
  %v2374 = load i8, ptr %ctx, align 8
  %c506 = zext i8 %v2374 to i64
  %v2375 = icmp ne i64 %c506, 0
  br i1 %v2375, label %b1607, label %b1606
b1605:
  %gcvolta1605 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1605, ptr %ctxtopo, align 8
  ret i64 0
b1606:
  br label %b1597
b1607:
  %gcvolta1607 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1607, ptr %ctxtopo, align 8
  ret i64 0
b1609:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2383 = call i64 @df.box_int(i64 %v2380)
  store i64 %v2383, ptr %gcs1
  %v2384 = call i64 @df.box_int(i64 %v2381)
  store i64 %v2384, ptr %gcs2
  %v2385 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2383, i64 %v2384)
  %v2386 = load i8, ptr %ctx, align 8
  %c507 = zext i8 %v2386 to i64
  %v2387 = icmp ne i64 %c507, 0
  br i1 %v2387, label %b1614, label %b1613
b1610:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2388 = call i64 @df.box_int(i64 %v2380)
  store i64 %v2388, ptr %gcs1
  %v2389 = call i64 @df.box_int(i64 %v2381)
  store i64 %v2389, ptr %gcs2
  %v2390 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2388, i64 %v2389)
  %v2391 = load i8, ptr %ctx, align 8
  %c508 = zext i8 %v2391 to i64
  %v2392 = icmp ne i64 %c508, 0
  br i1 %v2392, label %b1616, label %b1615
b1611:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2393 = call i64 @df.box_int(i64 %v2380)
  store i64 %v2393, ptr %gcs1
  %v2394 = call i64 @df.box_int(i64 %v2381)
  store i64 %v2394, ptr %gcs2
  %sa2395_0 = getelementptr [3 x i64], ptr %sargs2395, i64 0, i64 0
  store i64 %v2393, ptr %sa2395_0
  %sa2395_1 = getelementptr [3 x i64], ptr %sargs2395, i64 0, i64 1
  store i64 %v2394, ptr %sa2395_1
  %sat2395 = getelementptr [3 x i64], ptr %sargs2395, i64 0, i64 2
  store i64 0, ptr %sat2395
  %sic2395 = getelementptr i64, ptr %area, i64 334
  %sf2395 = call ptr @df.selector(ptr %sic2395, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2395 = call i64 %sf2395(i64 %v0, ptr %sargs2395, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2396 = load i8, ptr %ctx, align 8
  %c509 = zext i8 %v2396 to i64
  %v2397 = icmp ne i64 %c509, 0
  br i1 %v2397, label %b1618, label %b1617
b1612:
  %v2398 = phi i64 [ %v2385, %b1613 ], [ %v2390, %b1615 ], [ %v2395, %b1617 ]
  store i64 %v2398, ptr %gcs0
  %v2399 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2400 = and i64 %v2398, 1
  %v2401 = icmp ne i64 %v2400, 0
  br i1 %v2401, label %b1619, label %b1620
b1613:
  br label %b1612
b1614:
  %gcvolta1614 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1614, ptr %ctxtopo, align 8
  ret i64 0
b1615:
  br label %b1612
b1616:
  %gcvolta1616 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1616, ptr %ctxtopo, align 8
  ret i64 0
b1617:
  br label %b1612
b1618:
  %gcvolta1618 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1618, ptr %ctxtopo, align 8
  ret i64 0
b1619:
  %v2402 = ashr i64 %v2398, 1
  %v2403 = mul i64 %v2402, %v2399
  store i64 0, ptr %gcs0
  %v2404 = call i64 @df.box_int(i64 %v2403)
  br label %b1621
b1620:
  %v2405 = call i64 @df.box_int(i64 %v2399)
  store i64 %v2405, ptr %gcs1
  %sa2406_0 = getelementptr [2 x i64], ptr %sargs2406, i64 0, i64 0
  store i64 %v2405, ptr %sa2406_0
  %sat2406 = getelementptr [2 x i64], ptr %sargs2406, i64 0, i64 1
  store i64 0, ptr %sat2406
  %sic2406 = getelementptr i64, ptr %area, i64 336
  %sf2406 = call ptr @df.selector(ptr %sic2406, i64 %v2398, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2406 = call i64 %sf2406(i64 %v2398, ptr %sargs2406, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2407 = load i8, ptr %ctx, align 8
  %c510 = zext i8 %v2407 to i64
  %v2408 = icmp ne i64 %c510, 0
  br i1 %v2408, label %b1623, label %b1622
b1621:
  %v2409 = phi i64 [ %v2404, %b1619 ], [ %v2406, %b1622 ]
  store i64 %v2409, ptr %gcs0
  %v2410 = call i64 @df.class(i64 %v2409)
  switch i64 %v2410, label %b1626 [ i64 4, label %b1624 i64 2, label %b1625 i64 3, label %b1625 ]
b1622:
  br label %b1621
b1623:
  %gcvolta1623 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1623, ptr %ctxtopo, align 8
  ret i64 0
b1624:
  store i64 0, ptr %gcs1
  %v2411 = call double @df.unbox_double(i64 %v2409)
  %v2412 = load i8, ptr %ctx, align 8
  %c511 = zext i8 %v2412 to i64
  %v2413 = icmp ne i64 %c511, 0
  br i1 %v2413, label %b1629, label %b1628
b1625:
  store i64 0, ptr %gcs1
  %v2417 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2409)
  %v2418 = load i8, ptr %ctx, align 8
  %c512 = zext i8 %v2418 to i64
  %v2419 = icmp ne i64 %c512, 0
  br i1 %v2419, label %b1633, label %b1632
b1626:
  store i64 0, ptr %gcs1
  %sat2420 = getelementptr [1 x i64], ptr %sargs2420, i64 0, i64 0
  store i64 0, ptr %sat2420
  %sic2420 = getelementptr i64, ptr %area, i64 338
  %sf2420 = call ptr @df.selector(ptr %sic2420, i64 %v2409, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2420 = call i64 %sf2420(i64 %v2409, ptr %sargs2420, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2420, ptr %gcs0
  %v2421 = load i8, ptr %ctx, align 8
  %c513 = zext i8 %v2421 to i64
  %v2422 = icmp ne i64 %c513, 0
  br i1 %v2422, label %b1635, label %b1634
b1627:
  %v2426 = phi i64 [ %v2414, %b1630 ], [ %v2417, %b1632 ], [ %v2423, %b1636 ]
  store i64 0, ptr %gcs0
  %v2427 = call i64 @df.box_int(i64 %v2426)
  %xcp2428 = load i8, ptr %ctx, align 8
  %xcn2428 = icmp ne i8 %xcp2428, 0
  %xce2428 = call i1 @llvm.expect.i1(i1 %xcn2428, i1 false)
  br i1 %xce2428, label %xc2428.limpar, label %xc2428.fim
xc2428.limpar:
  call void @dartforge_exception_clear()
  br label %xc2428.fim
xc2428.fim:
  %gcvolta1627 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1627, ptr %ctxtopo, align 8
  ret i64 %v2427
b1628:
  store i64 0, ptr %gcs0
  %v2414 = call i64 @dartforge_native_DartForge_double_toInt(double %v2411)
  %v2415 = load i8, ptr %ctx, align 8
  %c514 = zext i8 %v2415 to i64
  %v2416 = icmp ne i64 %c514, 0
  br i1 %v2416, label %b1631, label %b1630
b1629:
  %gcvolta1629 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1629, ptr %ctxtopo, align 8
  ret i64 0
b1630:
  br label %b1627
b1631:
  %gcvolta1631 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1631, ptr %ctxtopo, align 8
  ret i64 0
b1632:
  br label %b1627
b1633:
  %gcvolta1633 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1633, ptr %ctxtopo, align 8
  ret i64 0
b1634:
  %v2423 = call i64 @df.unbox_int(i64 %v2420)
  %v2424 = load i8, ptr %ctx, align 8
  %c515 = zext i8 %v2424 to i64
  %v2425 = icmp ne i64 %c515, 0
  br i1 %v2425, label %b1637, label %b1636
b1635:
  %gcvolta1635 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1635, ptr %ctxtopo, align 8
  ret i64 0
b1636:
  br label %b1627
b1637:
  %gcvolta1637 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1637, ptr %ctxtopo, align 8
  ret i64 0
b1639:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2433 = call i64 @df.box_int(i64 %v2430)
  store i64 %v2433, ptr %gcs1
  %v2434 = call i64 @df.box_int(i64 %v2431)
  store i64 %v2434, ptr %gcs2
  %v2435 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2433, i64 %v2434)
  %v2436 = load i8, ptr %ctx, align 8
  %c516 = zext i8 %v2436 to i64
  %v2437 = icmp ne i64 %c516, 0
  br i1 %v2437, label %b1644, label %b1643
b1640:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2438 = call i64 @df.box_int(i64 %v2430)
  store i64 %v2438, ptr %gcs1
  %v2439 = call i64 @df.box_int(i64 %v2431)
  store i64 %v2439, ptr %gcs2
  %v2440 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2438, i64 %v2439)
  %v2441 = load i8, ptr %ctx, align 8
  %c517 = zext i8 %v2441 to i64
  %v2442 = icmp ne i64 %c517, 0
  br i1 %v2442, label %b1646, label %b1645
b1641:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2443 = call i64 @df.box_int(i64 %v2430)
  store i64 %v2443, ptr %gcs1
  %v2444 = call i64 @df.box_int(i64 %v2431)
  store i64 %v2444, ptr %gcs2
  %sa2445_0 = getelementptr [3 x i64], ptr %sargs2445, i64 0, i64 0
  store i64 %v2443, ptr %sa2445_0
  %sa2445_1 = getelementptr [3 x i64], ptr %sargs2445, i64 0, i64 1
  store i64 %v2444, ptr %sa2445_1
  %sat2445 = getelementptr [3 x i64], ptr %sargs2445, i64 0, i64 2
  store i64 0, ptr %sat2445
  %sic2445 = getelementptr i64, ptr %area, i64 340
  %sf2445 = call ptr @df.selector(ptr %sic2445, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2445 = call i64 %sf2445(i64 %v0, ptr %sargs2445, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2446 = load i8, ptr %ctx, align 8
  %c518 = zext i8 %v2446 to i64
  %v2447 = icmp ne i64 %c518, 0
  br i1 %v2447, label %b1648, label %b1647
b1642:
  %v2448 = phi i64 [ %v2435, %b1643 ], [ %v2440, %b1645 ], [ %v2445, %b1647 ]
  store i64 %v2448, ptr %gcs0
  %v2449 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2450 = and i64 %v2448, 1
  %v2451 = icmp ne i64 %v2450, 0
  br i1 %v2451, label %b1649, label %b1650
b1643:
  br label %b1642
b1644:
  %gcvolta1644 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1644, ptr %ctxtopo, align 8
  ret i64 0
b1645:
  br label %b1642
b1646:
  %gcvolta1646 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1646, ptr %ctxtopo, align 8
  ret i64 0
b1647:
  br label %b1642
b1648:
  %gcvolta1648 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1648, ptr %ctxtopo, align 8
  ret i64 0
b1649:
  %v2452 = ashr i64 %v2448, 1
  %v2453 = mul i64 %v2452, %v2449
  store i64 0, ptr %gcs0
  %v2454 = call i64 @df.box_int(i64 %v2453)
  br label %b1651
b1650:
  %v2455 = call i64 @df.box_int(i64 %v2449)
  store i64 %v2455, ptr %gcs1
  %sa2456_0 = getelementptr [2 x i64], ptr %sargs2456, i64 0, i64 0
  store i64 %v2455, ptr %sa2456_0
  %sat2456 = getelementptr [2 x i64], ptr %sargs2456, i64 0, i64 1
  store i64 0, ptr %sat2456
  %sic2456 = getelementptr i64, ptr %area, i64 342
  %sf2456 = call ptr @df.selector(ptr %sic2456, i64 %v2448, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2456 = call i64 %sf2456(i64 %v2448, ptr %sargs2456, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2457 = load i8, ptr %ctx, align 8
  %c519 = zext i8 %v2457 to i64
  %v2458 = icmp ne i64 %c519, 0
  br i1 %v2458, label %b1653, label %b1652
b1651:
  %v2459 = phi i64 [ %v2454, %b1649 ], [ %v2456, %b1652 ]
  store i64 %v2459, ptr %gcs0
  %v2460 = call i64 @df.class(i64 %v2459)
  switch i64 %v2460, label %b1656 [ i64 4, label %b1654 i64 2, label %b1655 i64 3, label %b1655 ]
b1652:
  br label %b1651
b1653:
  %gcvolta1653 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1653, ptr %ctxtopo, align 8
  ret i64 0
b1654:
  store i64 0, ptr %gcs1
  %v2461 = call double @df.unbox_double(i64 %v2459)
  %v2462 = load i8, ptr %ctx, align 8
  %c520 = zext i8 %v2462 to i64
  %v2463 = icmp ne i64 %c520, 0
  br i1 %v2463, label %b1659, label %b1658
b1655:
  store i64 0, ptr %gcs1
  %v2467 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2459)
  %v2468 = load i8, ptr %ctx, align 8
  %c521 = zext i8 %v2468 to i64
  %v2469 = icmp ne i64 %c521, 0
  br i1 %v2469, label %b1663, label %b1662
b1656:
  store i64 0, ptr %gcs1
  %sat2470 = getelementptr [1 x i64], ptr %sargs2470, i64 0, i64 0
  store i64 0, ptr %sat2470
  %sic2470 = getelementptr i64, ptr %area, i64 344
  %sf2470 = call ptr @df.selector(ptr %sic2470, i64 %v2459, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2470 = call i64 %sf2470(i64 %v2459, ptr %sargs2470, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2470, ptr %gcs0
  %v2471 = load i8, ptr %ctx, align 8
  %c522 = zext i8 %v2471 to i64
  %v2472 = icmp ne i64 %c522, 0
  br i1 %v2472, label %b1665, label %b1664
b1657:
  %v2476 = phi i64 [ %v2464, %b1660 ], [ %v2467, %b1662 ], [ %v2473, %b1666 ]
  store i64 0, ptr %gcs0
  %v2477 = call i64 @df.box_int(i64 %v2476)
  %xcp2478 = load i8, ptr %ctx, align 8
  %xcn2478 = icmp ne i8 %xcp2478, 0
  %xce2478 = call i1 @llvm.expect.i1(i1 %xcn2478, i1 false)
  br i1 %xce2478, label %xc2478.limpar, label %xc2478.fim
xc2478.limpar:
  call void @dartforge_exception_clear()
  br label %xc2478.fim
xc2478.fim:
  %gcvolta1657 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1657, ptr %ctxtopo, align 8
  ret i64 %v2477
b1658:
  store i64 0, ptr %gcs0
  %v2464 = call i64 @dartforge_native_DartForge_double_toInt(double %v2461)
  %v2465 = load i8, ptr %ctx, align 8
  %c523 = zext i8 %v2465 to i64
  %v2466 = icmp ne i64 %c523, 0
  br i1 %v2466, label %b1661, label %b1660
b1659:
  %gcvolta1659 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1659, ptr %ctxtopo, align 8
  ret i64 0
b1660:
  br label %b1657
b1661:
  %gcvolta1661 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1661, ptr %ctxtopo, align 8
  ret i64 0
b1662:
  br label %b1657
b1663:
  %gcvolta1663 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1663, ptr %ctxtopo, align 8
  ret i64 0
b1664:
  %v2473 = call i64 @df.unbox_int(i64 %v2470)
  %v2474 = load i8, ptr %ctx, align 8
  %c524 = zext i8 %v2474 to i64
  %v2475 = icmp ne i64 %c524, 0
  br i1 %v2475, label %b1667, label %b1666
b1665:
  %gcvolta1665 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1665, ptr %ctxtopo, align 8
  ret i64 0
b1666:
  br label %b1657
b1667:
  %gcvolta1667 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1667, ptr %ctxtopo, align 8
  ret i64 0
b1669:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2483 = call i64 @df.box_int(i64 %v2480)
  store i64 %v2483, ptr %gcs1
  %v2484 = call i64 @df.box_int(i64 %v2481)
  store i64 %v2484, ptr %gcs2
  %v2485 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2483, i64 %v2484)
  %v2486 = load i8, ptr %ctx, align 8
  %c525 = zext i8 %v2486 to i64
  %v2487 = icmp ne i64 %c525, 0
  br i1 %v2487, label %b1674, label %b1673
b1670:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2488 = call i64 @df.box_int(i64 %v2480)
  store i64 %v2488, ptr %gcs1
  %v2489 = call i64 @df.box_int(i64 %v2481)
  store i64 %v2489, ptr %gcs2
  %v2490 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2488, i64 %v2489)
  %v2491 = load i8, ptr %ctx, align 8
  %c526 = zext i8 %v2491 to i64
  %v2492 = icmp ne i64 %c526, 0
  br i1 %v2492, label %b1676, label %b1675
b1671:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2493 = call i64 @df.box_int(i64 %v2480)
  store i64 %v2493, ptr %gcs1
  %v2494 = call i64 @df.box_int(i64 %v2481)
  store i64 %v2494, ptr %gcs2
  %sa2495_0 = getelementptr [3 x i64], ptr %sargs2495, i64 0, i64 0
  store i64 %v2493, ptr %sa2495_0
  %sa2495_1 = getelementptr [3 x i64], ptr %sargs2495, i64 0, i64 1
  store i64 %v2494, ptr %sa2495_1
  %sat2495 = getelementptr [3 x i64], ptr %sargs2495, i64 0, i64 2
  store i64 0, ptr %sat2495
  %sic2495 = getelementptr i64, ptr %area, i64 346
  %sf2495 = call ptr @df.selector(ptr %sic2495, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2495 = call i64 %sf2495(i64 %v0, ptr %sargs2495, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2496 = load i8, ptr %ctx, align 8
  %c527 = zext i8 %v2496 to i64
  %v2497 = icmp ne i64 %c527, 0
  br i1 %v2497, label %b1678, label %b1677
b1672:
  %v2498 = phi i64 [ %v2485, %b1673 ], [ %v2490, %b1675 ], [ %v2495, %b1677 ]
  store i64 %v2498, ptr %gcs0
  %v2499 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2500 = and i64 %v2498, 1
  %v2501 = icmp ne i64 %v2500, 0
  br i1 %v2501, label %b1679, label %b1680
b1673:
  br label %b1672
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
  %v2502 = ashr i64 %v2498, 1
  %v2503 = mul i64 %v2502, %v2499
  store i64 0, ptr %gcs0
  %v2504 = call i64 @df.box_int(i64 %v2503)
  br label %b1681
b1680:
  %v2505 = call i64 @df.box_int(i64 %v2499)
  store i64 %v2505, ptr %gcs1
  %sa2506_0 = getelementptr [2 x i64], ptr %sargs2506, i64 0, i64 0
  store i64 %v2505, ptr %sa2506_0
  %sat2506 = getelementptr [2 x i64], ptr %sargs2506, i64 0, i64 1
  store i64 0, ptr %sat2506
  %sic2506 = getelementptr i64, ptr %area, i64 348
  %sf2506 = call ptr @df.selector(ptr %sic2506, i64 %v2498, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2506 = call i64 %sf2506(i64 %v2498, ptr %sargs2506, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2507 = load i8, ptr %ctx, align 8
  %c528 = zext i8 %v2507 to i64
  %v2508 = icmp ne i64 %c528, 0
  br i1 %v2508, label %b1683, label %b1682
b1681:
  %v2509 = phi i64 [ %v2504, %b1679 ], [ %v2506, %b1682 ]
  store i64 %v2509, ptr %gcs0
  %v2510 = call i64 @df.class(i64 %v2509)
  switch i64 %v2510, label %b1686 [ i64 4, label %b1684 i64 2, label %b1685 i64 3, label %b1685 ]
b1682:
  br label %b1681
b1683:
  %gcvolta1683 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1683, ptr %ctxtopo, align 8
  ret i64 0
b1684:
  store i64 0, ptr %gcs1
  %v2511 = call double @df.unbox_double(i64 %v2509)
  %v2512 = load i8, ptr %ctx, align 8
  %c529 = zext i8 %v2512 to i64
  %v2513 = icmp ne i64 %c529, 0
  br i1 %v2513, label %b1689, label %b1688
b1685:
  store i64 0, ptr %gcs1
  %v2517 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2509)
  %v2518 = load i8, ptr %ctx, align 8
  %c530 = zext i8 %v2518 to i64
  %v2519 = icmp ne i64 %c530, 0
  br i1 %v2519, label %b1693, label %b1692
b1686:
  store i64 0, ptr %gcs1
  %sat2520 = getelementptr [1 x i64], ptr %sargs2520, i64 0, i64 0
  store i64 0, ptr %sat2520
  %sic2520 = getelementptr i64, ptr %area, i64 350
  %sf2520 = call ptr @df.selector(ptr %sic2520, i64 %v2509, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2520 = call i64 %sf2520(i64 %v2509, ptr %sargs2520, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2520, ptr %gcs0
  %v2521 = load i8, ptr %ctx, align 8
  %c531 = zext i8 %v2521 to i64
  %v2522 = icmp ne i64 %c531, 0
  br i1 %v2522, label %b1695, label %b1694
b1687:
  %v2526 = phi i64 [ %v2514, %b1690 ], [ %v2517, %b1692 ], [ %v2523, %b1696 ]
  store i64 0, ptr %gcs0
  %v2527 = call i64 @df.box_int(i64 %v2526)
  %xcp2528 = load i8, ptr %ctx, align 8
  %xcn2528 = icmp ne i8 %xcp2528, 0
  %xce2528 = call i1 @llvm.expect.i1(i1 %xcn2528, i1 false)
  br i1 %xce2528, label %xc2528.limpar, label %xc2528.fim
xc2528.limpar:
  call void @dartforge_exception_clear()
  br label %xc2528.fim
xc2528.fim:
  %gcvolta1687 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1687, ptr %ctxtopo, align 8
  ret i64 %v2527
b1688:
  store i64 0, ptr %gcs0
  %v2514 = call i64 @dartforge_native_DartForge_double_toInt(double %v2511)
  %v2515 = load i8, ptr %ctx, align 8
  %c532 = zext i8 %v2515 to i64
  %v2516 = icmp ne i64 %c532, 0
  br i1 %v2516, label %b1691, label %b1690
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
  %v2523 = call i64 @df.unbox_int(i64 %v2520)
  %v2524 = load i8, ptr %ctx, align 8
  %c533 = zext i8 %v2524 to i64
  %v2525 = icmp ne i64 %c533, 0
  br i1 %v2525, label %b1697, label %b1696
b1695:
  %gcvolta1695 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1695, ptr %ctxtopo, align 8
  ret i64 0
b1696:
  br label %b1687
b1697:
  %gcvolta1697 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1697, ptr %ctxtopo, align 8
  ret i64 0
b1699:
  %v2533 = ashr i64 %v0, 1
  %v2534 = icmp slt i64 %v2533, %v2530
  br label %b1701
b1700:
  %v2535 = call i64 @df.box_int(i64 %v2530)
  store i64 %v2535, ptr %gcs1
  %sa2536_0 = getelementptr [2 x i64], ptr %sargs2536, i64 0, i64 0
  store i64 %v2535, ptr %sa2536_0
  %sat2536 = getelementptr [2 x i64], ptr %sargs2536, i64 0, i64 1
  store i64 0, ptr %sat2536
  %sic2536 = getelementptr i64, ptr %area, i64 352
  %sf2536 = call ptr @df.selector(ptr %sic2536, i64 %v0, i64 -4020809642220582218, ptr @df.seln.23, i64 4)
  %v2536 = call i64 %sf2536(i64 %v0, ptr %sargs2536, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2536, ptr %gcs1
  %v2537 = load i8, ptr %ctx, align 8
  %c534 = zext i8 %v2537 to i64
  %v2538 = icmp ne i64 %c534, 0
  br i1 %v2538, label %b1703, label %b1702
b1701:
  %v2542 = phi i1 [ %v2534, %b1699 ], [ %v2539, %b1704 ]
  br i1 %v2542, label %b1706, label %b1707
b1702:
  %v2539 = call i1 @df.unbox_bool(i64 %v2536)
  %v2540 = load i8, ptr %ctx, align 8
  %c535 = zext i8 %v2540 to i64
  %v2541 = icmp ne i64 %c535, 0
  br i1 %v2541, label %b1705, label %b1704
b1703:
  %gcvolta1703 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1703, ptr %ctxtopo, align 8
  ret i64 0
b1704:
  br label %b1701
b1705:
  %gcvolta1705 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1705, ptr %ctxtopo, align 8
  ret i64 0
b1706:
  %v2544 = add i64 0, 1
  %v2545 = sub i64 0, %v2544
  %v2546 = add i64 0, 1
  %v2547 = call i64 @df.class(i64 %v0)
  switch i64 %v2547, label %b1711 [ i64 4, label %b1709 i64 2, label %b1710 i64 3, label %b1710 ]
b1707:
  %v2576 = add i64 0, 1
  %v2577 = sub i64 0, %v2576
  %v2578 = add i64 0, 1
  %v2579 = call i64 @df.class(i64 %v0)
  switch i64 %v2579, label %b1726 [ i64 4, label %b1724 i64 2, label %b1725 i64 3, label %b1725 ]
b1708:
  %v2607 = phi i64 [ %v2574, %b1721 ], [ %v2606, %b1736 ]
  store i64 %v2607, ptr %gcs0
  %v2608 = call i64 @df.class(i64 %v2607)
  switch i64 %v2608, label %b1741 [ i64 4, label %b1739 i64 2, label %b1740 i64 3, label %b1740 ]
b1709:
  store i64 0, ptr %gcs1
  %v2548 = call i64 @df.box_int(i64 %v2545)
  store i64 %v2548, ptr %gcs1
  %v2549 = call i64 @df.box_int(i64 %v2546)
  store i64 %v2549, ptr %gcs2
  %v2550 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2548, i64 %v2549)
  %v2551 = load i8, ptr %ctx, align 8
  %c536 = zext i8 %v2551 to i64
  %v2552 = icmp ne i64 %c536, 0
  br i1 %v2552, label %b1714, label %b1713
b1710:
  store i64 0, ptr %gcs1
  %v2553 = call i64 @df.box_int(i64 %v2545)
  store i64 %v2553, ptr %gcs1
  %v2554 = call i64 @df.box_int(i64 %v2546)
  store i64 %v2554, ptr %gcs2
  %v2555 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2553, i64 %v2554)
  %v2556 = load i8, ptr %ctx, align 8
  %c537 = zext i8 %v2556 to i64
  %v2557 = icmp ne i64 %c537, 0
  br i1 %v2557, label %b1716, label %b1715
b1711:
  store i64 0, ptr %gcs1
  %v2558 = call i64 @df.box_int(i64 %v2545)
  store i64 %v2558, ptr %gcs1
  %v2559 = call i64 @df.box_int(i64 %v2546)
  store i64 %v2559, ptr %gcs2
  %sa2560_0 = getelementptr [3 x i64], ptr %sargs2560, i64 0, i64 0
  store i64 %v2558, ptr %sa2560_0
  %sa2560_1 = getelementptr [3 x i64], ptr %sargs2560, i64 0, i64 1
  store i64 %v2559, ptr %sa2560_1
  %sat2560 = getelementptr [3 x i64], ptr %sargs2560, i64 0, i64 2
  store i64 0, ptr %sat2560
  %sic2560 = getelementptr i64, ptr %area, i64 354
  %sf2560 = call ptr @df.selector(ptr %sic2560, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2560 = call i64 %sf2560(i64 %v0, ptr %sargs2560, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2561 = load i8, ptr %ctx, align 8
  %c538 = zext i8 %v2561 to i64
  %v2562 = icmp ne i64 %c538, 0
  br i1 %v2562, label %b1718, label %b1717
b1712:
  %v2563 = phi i64 [ %v2550, %b1713 ], [ %v2555, %b1715 ], [ %v2560, %b1717 ]
  store i64 %v2563, ptr %gcs0
  %v2564 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2565 = and i64 %v2563, 1
  %v2566 = icmp ne i64 %v2565, 0
  br i1 %v2566, label %b1719, label %b1720
b1713:
  br label %b1712
b1714:
  %gcvolta1714 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1714, ptr %ctxtopo, align 8
  ret i64 0
b1715:
  br label %b1712
b1716:
  %gcvolta1716 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1716, ptr %ctxtopo, align 8
  ret i64 0
b1717:
  br label %b1712
b1718:
  %gcvolta1718 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1718, ptr %ctxtopo, align 8
  ret i64 0
b1719:
  %v2567 = ashr i64 %v2563, 1
  %v2568 = mul i64 %v2567, %v2564
  store i64 0, ptr %gcs0
  %v2569 = call i64 @df.box_int(i64 %v2568)
  br label %b1721
b1720:
  %v2570 = call i64 @df.box_int(i64 %v2564)
  store i64 %v2570, ptr %gcs1
  %sa2571_0 = getelementptr [2 x i64], ptr %sargs2571, i64 0, i64 0
  store i64 %v2570, ptr %sa2571_0
  %sat2571 = getelementptr [2 x i64], ptr %sargs2571, i64 0, i64 1
  store i64 0, ptr %sat2571
  %sic2571 = getelementptr i64, ptr %area, i64 356
  %sf2571 = call ptr @df.selector(ptr %sic2571, i64 %v2563, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2571 = call i64 %sf2571(i64 %v2563, ptr %sargs2571, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2572 = load i8, ptr %ctx, align 8
  %c539 = zext i8 %v2572 to i64
  %v2573 = icmp ne i64 %c539, 0
  br i1 %v2573, label %b1723, label %b1722
b1721:
  %v2574 = phi i64 [ %v2569, %b1719 ], [ %v2571, %b1722 ]
  br label %b1708
b1722:
  br label %b1721
b1723:
  %gcvolta1723 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1723, ptr %ctxtopo, align 8
  ret i64 0
b1724:
  store i64 0, ptr %gcs1
  %v2580 = call i64 @df.box_int(i64 %v2577)
  store i64 %v2580, ptr %gcs1
  %v2581 = call i64 @df.box_int(i64 %v2578)
  store i64 %v2581, ptr %gcs2
  %v2582 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2580, i64 %v2581)
  %v2583 = load i8, ptr %ctx, align 8
  %c540 = zext i8 %v2583 to i64
  %v2584 = icmp ne i64 %c540, 0
  br i1 %v2584, label %b1729, label %b1728
b1725:
  store i64 0, ptr %gcs1
  %v2585 = call i64 @df.box_int(i64 %v2577)
  store i64 %v2585, ptr %gcs1
  %v2586 = call i64 @df.box_int(i64 %v2578)
  store i64 %v2586, ptr %gcs2
  %v2587 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2585, i64 %v2586)
  %v2588 = load i8, ptr %ctx, align 8
  %c541 = zext i8 %v2588 to i64
  %v2589 = icmp ne i64 %c541, 0
  br i1 %v2589, label %b1731, label %b1730
b1726:
  store i64 0, ptr %gcs1
  %v2590 = call i64 @df.box_int(i64 %v2577)
  store i64 %v2590, ptr %gcs1
  %v2591 = call i64 @df.box_int(i64 %v2578)
  store i64 %v2591, ptr %gcs2
  %sa2592_0 = getelementptr [3 x i64], ptr %sargs2592, i64 0, i64 0
  store i64 %v2590, ptr %sa2592_0
  %sa2592_1 = getelementptr [3 x i64], ptr %sargs2592, i64 0, i64 1
  store i64 %v2591, ptr %sa2592_1
  %sat2592 = getelementptr [3 x i64], ptr %sargs2592, i64 0, i64 2
  store i64 0, ptr %sat2592
  %sic2592 = getelementptr i64, ptr %area, i64 358
  %sf2592 = call ptr @df.selector(ptr %sic2592, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2592 = call i64 %sf2592(i64 %v0, ptr %sargs2592, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2593 = load i8, ptr %ctx, align 8
  %c542 = zext i8 %v2593 to i64
  %v2594 = icmp ne i64 %c542, 0
  br i1 %v2594, label %b1733, label %b1732
b1727:
  %v2595 = phi i64 [ %v2582, %b1728 ], [ %v2587, %b1730 ], [ %v2592, %b1732 ]
  store i64 %v2595, ptr %gcs0
  %v2596 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2597 = and i64 %v2595, 1
  %v2598 = icmp ne i64 %v2597, 0
  br i1 %v2598, label %b1734, label %b1735
b1728:
  br label %b1727
b1729:
  %gcvolta1729 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1729, ptr %ctxtopo, align 8
  ret i64 0
b1730:
  br label %b1727
b1731:
  %gcvolta1731 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1731, ptr %ctxtopo, align 8
  ret i64 0
b1732:
  br label %b1727
b1733:
  %gcvolta1733 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1733, ptr %ctxtopo, align 8
  ret i64 0
b1734:
  %v2599 = ashr i64 %v2595, 1
  %v2600 = mul i64 %v2599, %v2596
  store i64 0, ptr %gcs0
  %v2601 = call i64 @df.box_int(i64 %v2600)
  br label %b1736
b1735:
  %v2602 = call i64 @df.box_int(i64 %v2596)
  store i64 %v2602, ptr %gcs1
  %sa2603_0 = getelementptr [2 x i64], ptr %sargs2603, i64 0, i64 0
  store i64 %v2602, ptr %sa2603_0
  %sat2603 = getelementptr [2 x i64], ptr %sargs2603, i64 0, i64 1
  store i64 0, ptr %sat2603
  %sic2603 = getelementptr i64, ptr %area, i64 360
  %sf2603 = call ptr @df.selector(ptr %sic2603, i64 %v2595, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2603 = call i64 %sf2603(i64 %v2595, ptr %sargs2603, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2604 = load i8, ptr %ctx, align 8
  %c543 = zext i8 %v2604 to i64
  %v2605 = icmp ne i64 %c543, 0
  br i1 %v2605, label %b1738, label %b1737
b1736:
  %v2606 = phi i64 [ %v2601, %b1734 ], [ %v2603, %b1737 ]
  br label %b1708
b1737:
  br label %b1736
b1738:
  %gcvolta1738 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1738, ptr %ctxtopo, align 8
  ret i64 0
b1739:
  store i64 0, ptr %gcs1
  %v2609 = call double @df.unbox_double(i64 %v2607)
  %v2610 = load i8, ptr %ctx, align 8
  %c544 = zext i8 %v2610 to i64
  %v2611 = icmp ne i64 %c544, 0
  br i1 %v2611, label %b1744, label %b1743
b1740:
  store i64 0, ptr %gcs1
  %v2615 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2607)
  %v2616 = load i8, ptr %ctx, align 8
  %c545 = zext i8 %v2616 to i64
  %v2617 = icmp ne i64 %c545, 0
  br i1 %v2617, label %b1748, label %b1747
b1741:
  store i64 0, ptr %gcs1
  %sat2618 = getelementptr [1 x i64], ptr %sargs2618, i64 0, i64 0
  store i64 0, ptr %sat2618
  %sic2618 = getelementptr i64, ptr %area, i64 362
  %sf2618 = call ptr @df.selector(ptr %sic2618, i64 %v2607, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2618 = call i64 %sf2618(i64 %v2607, ptr %sargs2618, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2618, ptr %gcs0
  %v2619 = load i8, ptr %ctx, align 8
  %c546 = zext i8 %v2619 to i64
  %v2620 = icmp ne i64 %c546, 0
  br i1 %v2620, label %b1750, label %b1749
b1742:
  %v2624 = phi i64 [ %v2612, %b1745 ], [ %v2615, %b1747 ], [ %v2621, %b1751 ]
  store i64 0, ptr %gcs0
  %v2625 = call i64 @df.box_int(i64 %v2624)
  %xcp2626 = load i8, ptr %ctx, align 8
  %xcn2626 = icmp ne i8 %xcp2626, 0
  %xce2626 = call i1 @llvm.expect.i1(i1 %xcn2626, i1 false)
  br i1 %xce2626, label %xc2626.limpar, label %xc2626.fim
xc2626.limpar:
  call void @dartforge_exception_clear()
  br label %xc2626.fim
xc2626.fim:
  %gcvolta1742 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1742, ptr %ctxtopo, align 8
  ret i64 %v2625
b1743:
  store i64 0, ptr %gcs0
  %v2612 = call i64 @dartforge_native_DartForge_double_toInt(double %v2609)
  %v2613 = load i8, ptr %ctx, align 8
  %c547 = zext i8 %v2613 to i64
  %v2614 = icmp ne i64 %c547, 0
  br i1 %v2614, label %b1746, label %b1745
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
  %v2621 = call i64 @df.unbox_int(i64 %v2618)
  %v2622 = load i8, ptr %ctx, align 8
  %c548 = zext i8 %v2622 to i64
  %v2623 = icmp ne i64 %c548, 0
  br i1 %v2623, label %b1752, label %b1751
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
  %v2631 = ashr i64 %v0, 1
  %v2632 = icmp slt i64 %v2631, %v2628
  br label %b1756
b1755:
  %v2633 = call i64 @df.box_int(i64 %v2628)
  store i64 %v2633, ptr %gcs1
  %sa2634_0 = getelementptr [2 x i64], ptr %sargs2634, i64 0, i64 0
  store i64 %v2633, ptr %sa2634_0
  %sat2634 = getelementptr [2 x i64], ptr %sargs2634, i64 0, i64 1
  store i64 0, ptr %sat2634
  %sic2634 = getelementptr i64, ptr %area, i64 364
  %sf2634 = call ptr @df.selector(ptr %sic2634, i64 %v0, i64 -4020809642220582218, ptr @df.seln.23, i64 4)
  %v2634 = call i64 %sf2634(i64 %v0, ptr %sargs2634, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2634, ptr %gcs1
  %v2635 = load i8, ptr %ctx, align 8
  %c549 = zext i8 %v2635 to i64
  %v2636 = icmp ne i64 %c549, 0
  br i1 %v2636, label %b1758, label %b1757
b1756:
  %v2640 = phi i1 [ %v2632, %b1754 ], [ %v2637, %b1759 ]
  br i1 %v2640, label %b1761, label %b1762
b1757:
  %v2637 = call i1 @df.unbox_bool(i64 %v2634)
  %v2638 = load i8, ptr %ctx, align 8
  %c550 = zext i8 %v2638 to i64
  %v2639 = icmp ne i64 %c550, 0
  br i1 %v2639, label %b1760, label %b1759
b1758:
  %gcvolta1758 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1758, ptr %ctxtopo, align 8
  ret i64 0
b1759:
  br label %b1756
b1760:
  %gcvolta1760 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1760, ptr %ctxtopo, align 8
  ret i64 0
b1761:
  %v2642 = add i64 0, 1
  %v2643 = sub i64 0, %v2642
  %v2644 = add i64 0, 1
  %v2645 = call i64 @df.class(i64 %v0)
  switch i64 %v2645, label %b1766 [ i64 4, label %b1764 i64 2, label %b1765 i64 3, label %b1765 ]
b1762:
  %v2674 = add i64 0, 1
  %v2675 = sub i64 0, %v2674
  %v2676 = add i64 0, 1
  %v2677 = call i64 @df.class(i64 %v0)
  switch i64 %v2677, label %b1781 [ i64 4, label %b1779 i64 2, label %b1780 i64 3, label %b1780 ]
b1763:
  %v2705 = phi i64 [ %v2672, %b1776 ], [ %v2704, %b1791 ]
  store i64 %v2705, ptr %gcs0
  %v2706 = call i64 @df.class(i64 %v2705)
  switch i64 %v2706, label %b1796 [ i64 4, label %b1794 i64 2, label %b1795 i64 3, label %b1795 ]
b1764:
  store i64 0, ptr %gcs1
  %v2646 = call i64 @df.box_int(i64 %v2643)
  store i64 %v2646, ptr %gcs1
  %v2647 = call i64 @df.box_int(i64 %v2644)
  store i64 %v2647, ptr %gcs2
  %v2648 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2646, i64 %v2647)
  %v2649 = load i8, ptr %ctx, align 8
  %c551 = zext i8 %v2649 to i64
  %v2650 = icmp ne i64 %c551, 0
  br i1 %v2650, label %b1769, label %b1768
b1765:
  store i64 0, ptr %gcs1
  %v2651 = call i64 @df.box_int(i64 %v2643)
  store i64 %v2651, ptr %gcs1
  %v2652 = call i64 @df.box_int(i64 %v2644)
  store i64 %v2652, ptr %gcs2
  %v2653 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2651, i64 %v2652)
  %v2654 = load i8, ptr %ctx, align 8
  %c552 = zext i8 %v2654 to i64
  %v2655 = icmp ne i64 %c552, 0
  br i1 %v2655, label %b1771, label %b1770
b1766:
  store i64 0, ptr %gcs1
  %v2656 = call i64 @df.box_int(i64 %v2643)
  store i64 %v2656, ptr %gcs1
  %v2657 = call i64 @df.box_int(i64 %v2644)
  store i64 %v2657, ptr %gcs2
  %sa2658_0 = getelementptr [3 x i64], ptr %sargs2658, i64 0, i64 0
  store i64 %v2656, ptr %sa2658_0
  %sa2658_1 = getelementptr [3 x i64], ptr %sargs2658, i64 0, i64 1
  store i64 %v2657, ptr %sa2658_1
  %sat2658 = getelementptr [3 x i64], ptr %sargs2658, i64 0, i64 2
  store i64 0, ptr %sat2658
  %sic2658 = getelementptr i64, ptr %area, i64 366
  %sf2658 = call ptr @df.selector(ptr %sic2658, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2658 = call i64 %sf2658(i64 %v0, ptr %sargs2658, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2659 = load i8, ptr %ctx, align 8
  %c553 = zext i8 %v2659 to i64
  %v2660 = icmp ne i64 %c553, 0
  br i1 %v2660, label %b1773, label %b1772
b1767:
  %v2661 = phi i64 [ %v2648, %b1768 ], [ %v2653, %b1770 ], [ %v2658, %b1772 ]
  store i64 %v2661, ptr %gcs0
  %v2662 = add i64 0, 32768
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2663 = and i64 %v2661, 1
  %v2664 = icmp ne i64 %v2663, 0
  br i1 %v2664, label %b1774, label %b1775
b1768:
  br label %b1767
b1769:
  %gcvolta1769 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1769, ptr %ctxtopo, align 8
  ret i64 0
b1770:
  br label %b1767
b1771:
  %gcvolta1771 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1771, ptr %ctxtopo, align 8
  ret i64 0
b1772:
  br label %b1767
b1773:
  %gcvolta1773 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1773, ptr %ctxtopo, align 8
  ret i64 0
b1774:
  %v2665 = ashr i64 %v2661, 1
  %v2666 = mul i64 %v2665, %v2662
  store i64 0, ptr %gcs0
  %v2667 = call i64 @df.box_int(i64 %v2666)
  br label %b1776
b1775:
  %v2668 = call i64 @df.box_int(i64 %v2662)
  store i64 %v2668, ptr %gcs1
  %sa2669_0 = getelementptr [2 x i64], ptr %sargs2669, i64 0, i64 0
  store i64 %v2668, ptr %sa2669_0
  %sat2669 = getelementptr [2 x i64], ptr %sargs2669, i64 0, i64 1
  store i64 0, ptr %sat2669
  %sic2669 = getelementptr i64, ptr %area, i64 368
  %sf2669 = call ptr @df.selector(ptr %sic2669, i64 %v2661, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2669 = call i64 %sf2669(i64 %v2661, ptr %sargs2669, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2670 = load i8, ptr %ctx, align 8
  %c554 = zext i8 %v2670 to i64
  %v2671 = icmp ne i64 %c554, 0
  br i1 %v2671, label %b1778, label %b1777
b1776:
  %v2672 = phi i64 [ %v2667, %b1774 ], [ %v2669, %b1777 ]
  br label %b1763
b1777:
  br label %b1776
b1778:
  %gcvolta1778 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1778, ptr %ctxtopo, align 8
  ret i64 0
b1779:
  store i64 0, ptr %gcs1
  %v2678 = call i64 @df.box_int(i64 %v2675)
  store i64 %v2678, ptr %gcs1
  %v2679 = call i64 @df.box_int(i64 %v2676)
  store i64 %v2679, ptr %gcs2
  %v2680 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2678, i64 %v2679)
  %v2681 = load i8, ptr %ctx, align 8
  %c555 = zext i8 %v2681 to i64
  %v2682 = icmp ne i64 %c555, 0
  br i1 %v2682, label %b1784, label %b1783
b1780:
  store i64 0, ptr %gcs1
  %v2683 = call i64 @df.box_int(i64 %v2675)
  store i64 %v2683, ptr %gcs1
  %v2684 = call i64 @df.box_int(i64 %v2676)
  store i64 %v2684, ptr %gcs2
  %v2685 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2683, i64 %v2684)
  %v2686 = load i8, ptr %ctx, align 8
  %c556 = zext i8 %v2686 to i64
  %v2687 = icmp ne i64 %c556, 0
  br i1 %v2687, label %b1786, label %b1785
b1781:
  store i64 0, ptr %gcs1
  %v2688 = call i64 @df.box_int(i64 %v2675)
  store i64 %v2688, ptr %gcs1
  %v2689 = call i64 @df.box_int(i64 %v2676)
  store i64 %v2689, ptr %gcs2
  %sa2690_0 = getelementptr [3 x i64], ptr %sargs2690, i64 0, i64 0
  store i64 %v2688, ptr %sa2690_0
  %sa2690_1 = getelementptr [3 x i64], ptr %sargs2690, i64 0, i64 1
  store i64 %v2689, ptr %sa2690_1
  %sat2690 = getelementptr [3 x i64], ptr %sargs2690, i64 0, i64 2
  store i64 0, ptr %sat2690
  %sic2690 = getelementptr i64, ptr %area, i64 370
  %sf2690 = call ptr @df.selector(ptr %sic2690, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2690 = call i64 %sf2690(i64 %v0, ptr %sargs2690, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2691 = load i8, ptr %ctx, align 8
  %c557 = zext i8 %v2691 to i64
  %v2692 = icmp ne i64 %c557, 0
  br i1 %v2692, label %b1788, label %b1787
b1782:
  %v2693 = phi i64 [ %v2680, %b1783 ], [ %v2685, %b1785 ], [ %v2690, %b1787 ]
  store i64 %v2693, ptr %gcs0
  %v2694 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2695 = and i64 %v2693, 1
  %v2696 = icmp ne i64 %v2695, 0
  br i1 %v2696, label %b1789, label %b1790
b1783:
  br label %b1782
b1784:
  %gcvolta1784 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1784, ptr %ctxtopo, align 8
  ret i64 0
b1785:
  br label %b1782
b1786:
  %gcvolta1786 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1786, ptr %ctxtopo, align 8
  ret i64 0
b1787:
  br label %b1782
b1788:
  %gcvolta1788 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1788, ptr %ctxtopo, align 8
  ret i64 0
b1789:
  %v2697 = ashr i64 %v2693, 1
  %v2698 = mul i64 %v2697, %v2694
  store i64 0, ptr %gcs0
  %v2699 = call i64 @df.box_int(i64 %v2698)
  br label %b1791
b1790:
  %v2700 = call i64 @df.box_int(i64 %v2694)
  store i64 %v2700, ptr %gcs1
  %sa2701_0 = getelementptr [2 x i64], ptr %sargs2701, i64 0, i64 0
  store i64 %v2700, ptr %sa2701_0
  %sat2701 = getelementptr [2 x i64], ptr %sargs2701, i64 0, i64 1
  store i64 0, ptr %sat2701
  %sic2701 = getelementptr i64, ptr %area, i64 372
  %sf2701 = call ptr @df.selector(ptr %sic2701, i64 %v2693, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2701 = call i64 %sf2701(i64 %v2693, ptr %sargs2701, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2702 = load i8, ptr %ctx, align 8
  %c558 = zext i8 %v2702 to i64
  %v2703 = icmp ne i64 %c558, 0
  br i1 %v2703, label %b1793, label %b1792
b1791:
  %v2704 = phi i64 [ %v2699, %b1789 ], [ %v2701, %b1792 ]
  br label %b1763
b1792:
  br label %b1791
b1793:
  %gcvolta1793 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1793, ptr %ctxtopo, align 8
  ret i64 0
b1794:
  store i64 0, ptr %gcs1
  %v2707 = call double @df.unbox_double(i64 %v2705)
  %v2708 = load i8, ptr %ctx, align 8
  %c559 = zext i8 %v2708 to i64
  %v2709 = icmp ne i64 %c559, 0
  br i1 %v2709, label %b1799, label %b1798
b1795:
  store i64 0, ptr %gcs1
  %v2713 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2705)
  %v2714 = load i8, ptr %ctx, align 8
  %c560 = zext i8 %v2714 to i64
  %v2715 = icmp ne i64 %c560, 0
  br i1 %v2715, label %b1803, label %b1802
b1796:
  store i64 0, ptr %gcs1
  %sat2716 = getelementptr [1 x i64], ptr %sargs2716, i64 0, i64 0
  store i64 0, ptr %sat2716
  %sic2716 = getelementptr i64, ptr %area, i64 374
  %sf2716 = call ptr @df.selector(ptr %sic2716, i64 %v2705, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2716 = call i64 %sf2716(i64 %v2705, ptr %sargs2716, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2716, ptr %gcs0
  %v2717 = load i8, ptr %ctx, align 8
  %c561 = zext i8 %v2717 to i64
  %v2718 = icmp ne i64 %c561, 0
  br i1 %v2718, label %b1805, label %b1804
b1797:
  %v2722 = phi i64 [ %v2710, %b1800 ], [ %v2713, %b1802 ], [ %v2719, %b1806 ]
  store i64 0, ptr %gcs0
  %v2723 = call i64 @df.box_int(i64 %v2722)
  %xcp2724 = load i8, ptr %ctx, align 8
  %xcn2724 = icmp ne i8 %xcp2724, 0
  %xce2724 = call i1 @llvm.expect.i1(i1 %xcn2724, i1 false)
  br i1 %xce2724, label %xc2724.limpar, label %xc2724.fim
xc2724.limpar:
  call void @dartforge_exception_clear()
  br label %xc2724.fim
xc2724.fim:
  %gcvolta1797 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1797, ptr %ctxtopo, align 8
  ret i64 %v2723
b1798:
  store i64 0, ptr %gcs0
  %v2710 = call i64 @dartforge_native_DartForge_double_toInt(double %v2707)
  %v2711 = load i8, ptr %ctx, align 8
  %c562 = zext i8 %v2711 to i64
  %v2712 = icmp ne i64 %c562, 0
  br i1 %v2712, label %b1801, label %b1800
b1799:
  %gcvolta1799 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1799, ptr %ctxtopo, align 8
  ret i64 0
b1800:
  br label %b1797
b1801:
  %gcvolta1801 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1801, ptr %ctxtopo, align 8
  ret i64 0
b1802:
  br label %b1797
b1803:
  %gcvolta1803 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1803, ptr %ctxtopo, align 8
  ret i64 0
b1804:
  %v2719 = call i64 @df.unbox_int(i64 %v2716)
  %v2720 = load i8, ptr %ctx, align 8
  %c563 = zext i8 %v2720 to i64
  %v2721 = icmp ne i64 %c563, 0
  br i1 %v2721, label %b1807, label %b1806
b1805:
  %gcvolta1805 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1805, ptr %ctxtopo, align 8
  ret i64 0
b1806:
  br label %b1797
b1807:
  %gcvolta1807 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1807, ptr %ctxtopo, align 8
  ret i64 0
b1809:
  %v2729 = ashr i64 %v0, 1
  %v2730 = icmp slt i64 %v2729, %v2726
  br label %b1811
b1810:
  %v2731 = call i64 @df.box_int(i64 %v2726)
  store i64 %v2731, ptr %gcs1
  %sa2732_0 = getelementptr [2 x i64], ptr %sargs2732, i64 0, i64 0
  store i64 %v2731, ptr %sa2732_0
  %sat2732 = getelementptr [2 x i64], ptr %sargs2732, i64 0, i64 1
  store i64 0, ptr %sat2732
  %sic2732 = getelementptr i64, ptr %area, i64 376
  %sf2732 = call ptr @df.selector(ptr %sic2732, i64 %v0, i64 -4020809642220582218, ptr @df.seln.23, i64 4)
  %v2732 = call i64 %sf2732(i64 %v0, ptr %sargs2732, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2732, ptr %gcs1
  %v2733 = load i8, ptr %ctx, align 8
  %c564 = zext i8 %v2733 to i64
  %v2734 = icmp ne i64 %c564, 0
  br i1 %v2734, label %b1813, label %b1812
b1811:
  %v2738 = phi i1 [ %v2730, %b1809 ], [ %v2735, %b1814 ]
  br i1 %v2738, label %b1816, label %b1817
b1812:
  %v2735 = call i1 @df.unbox_bool(i64 %v2732)
  %v2736 = load i8, ptr %ctx, align 8
  %c565 = zext i8 %v2736 to i64
  %v2737 = icmp ne i64 %c565, 0
  br i1 %v2737, label %b1815, label %b1814
b1813:
  %gcvolta1813 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1813, ptr %ctxtopo, align 8
  ret i64 0
b1814:
  br label %b1811
b1815:
  %gcvolta1815 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1815, ptr %ctxtopo, align 8
  ret i64 0
b1816:
  %v2740 = add i64 0, 1
  %v2741 = sub i64 0, %v2740
  %v2742 = add i64 0, 1
  %v2743 = call i64 @df.class(i64 %v0)
  switch i64 %v2743, label %b1821 [ i64 4, label %b1819 i64 2, label %b1820 i64 3, label %b1820 ]
b1817:
  %v2772 = add i64 0, 1
  %v2773 = sub i64 0, %v2772
  %v2774 = add i64 0, 1
  %v2775 = call i64 @df.class(i64 %v0)
  switch i64 %v2775, label %b1836 [ i64 4, label %b1834 i64 2, label %b1835 i64 3, label %b1835 ]
b1818:
  %v2803 = phi i64 [ %v2770, %b1831 ], [ %v2802, %b1846 ]
  store i64 %v2803, ptr %gcs0
  %v2804 = call i64 @df.class(i64 %v2803)
  switch i64 %v2804, label %b1851 [ i64 4, label %b1849 i64 2, label %b1850 i64 3, label %b1850 ]
b1819:
  store i64 0, ptr %gcs1
  %v2744 = call i64 @df.box_int(i64 %v2741)
  store i64 %v2744, ptr %gcs1
  %v2745 = call i64 @df.box_int(i64 %v2742)
  store i64 %v2745, ptr %gcs2
  %v2746 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2744, i64 %v2745)
  %v2747 = load i8, ptr %ctx, align 8
  %c566 = zext i8 %v2747 to i64
  %v2748 = icmp ne i64 %c566, 0
  br i1 %v2748, label %b1824, label %b1823
b1820:
  store i64 0, ptr %gcs1
  %v2749 = call i64 @df.box_int(i64 %v2741)
  store i64 %v2749, ptr %gcs1
  %v2750 = call i64 @df.box_int(i64 %v2742)
  store i64 %v2750, ptr %gcs2
  %v2751 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2749, i64 %v2750)
  %v2752 = load i8, ptr %ctx, align 8
  %c567 = zext i8 %v2752 to i64
  %v2753 = icmp ne i64 %c567, 0
  br i1 %v2753, label %b1826, label %b1825
b1821:
  store i64 0, ptr %gcs1
  %v2754 = call i64 @df.box_int(i64 %v2741)
  store i64 %v2754, ptr %gcs1
  %v2755 = call i64 @df.box_int(i64 %v2742)
  store i64 %v2755, ptr %gcs2
  %sa2756_0 = getelementptr [3 x i64], ptr %sargs2756, i64 0, i64 0
  store i64 %v2754, ptr %sa2756_0
  %sa2756_1 = getelementptr [3 x i64], ptr %sargs2756, i64 0, i64 1
  store i64 %v2755, ptr %sa2756_1
  %sat2756 = getelementptr [3 x i64], ptr %sargs2756, i64 0, i64 2
  store i64 0, ptr %sat2756
  %sic2756 = getelementptr i64, ptr %area, i64 378
  %sf2756 = call ptr @df.selector(ptr %sic2756, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2756 = call i64 %sf2756(i64 %v0, ptr %sargs2756, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2757 = load i8, ptr %ctx, align 8
  %c568 = zext i8 %v2757 to i64
  %v2758 = icmp ne i64 %c568, 0
  br i1 %v2758, label %b1828, label %b1827
b1822:
  %v2759 = phi i64 [ %v2746, %b1823 ], [ %v2751, %b1825 ], [ %v2756, %b1827 ]
  store i64 %v2759, ptr %gcs0
  %v2760 = add i64 0, 2147483648
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2761 = and i64 %v2759, 1
  %v2762 = icmp ne i64 %v2761, 0
  br i1 %v2762, label %b1829, label %b1830
b1823:
  br label %b1822
b1824:
  %gcvolta1824 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1824, ptr %ctxtopo, align 8
  ret i64 0
b1825:
  br label %b1822
b1826:
  %gcvolta1826 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1826, ptr %ctxtopo, align 8
  ret i64 0
b1827:
  br label %b1822
b1828:
  %gcvolta1828 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1828, ptr %ctxtopo, align 8
  ret i64 0
b1829:
  %v2763 = ashr i64 %v2759, 1
  %v2764 = mul i64 %v2763, %v2760
  store i64 0, ptr %gcs0
  %v2765 = call i64 @df.box_int(i64 %v2764)
  br label %b1831
b1830:
  %v2766 = call i64 @df.box_int(i64 %v2760)
  store i64 %v2766, ptr %gcs1
  %sa2767_0 = getelementptr [2 x i64], ptr %sargs2767, i64 0, i64 0
  store i64 %v2766, ptr %sa2767_0
  %sat2767 = getelementptr [2 x i64], ptr %sargs2767, i64 0, i64 1
  store i64 0, ptr %sat2767
  %sic2767 = getelementptr i64, ptr %area, i64 380
  %sf2767 = call ptr @df.selector(ptr %sic2767, i64 %v2759, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2767 = call i64 %sf2767(i64 %v2759, ptr %sargs2767, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2768 = load i8, ptr %ctx, align 8
  %c569 = zext i8 %v2768 to i64
  %v2769 = icmp ne i64 %c569, 0
  br i1 %v2769, label %b1833, label %b1832
b1831:
  %v2770 = phi i64 [ %v2765, %b1829 ], [ %v2767, %b1832 ]
  br label %b1818
b1832:
  br label %b1831
b1833:
  %gcvolta1833 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1833, ptr %ctxtopo, align 8
  ret i64 0
b1834:
  store i64 0, ptr %gcs1
  %v2776 = call i64 @df.box_int(i64 %v2773)
  store i64 %v2776, ptr %gcs1
  %v2777 = call i64 @df.box_int(i64 %v2774)
  store i64 %v2777, ptr %gcs2
  %v2778 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2776, i64 %v2777)
  %v2779 = load i8, ptr %ctx, align 8
  %c570 = zext i8 %v2779 to i64
  %v2780 = icmp ne i64 %c570, 0
  br i1 %v2780, label %b1839, label %b1838
b1835:
  store i64 0, ptr %gcs1
  %v2781 = call i64 @df.box_int(i64 %v2773)
  store i64 %v2781, ptr %gcs1
  %v2782 = call i64 @df.box_int(i64 %v2774)
  store i64 %v2782, ptr %gcs2
  %v2783 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2781, i64 %v2782)
  %v2784 = load i8, ptr %ctx, align 8
  %c571 = zext i8 %v2784 to i64
  %v2785 = icmp ne i64 %c571, 0
  br i1 %v2785, label %b1841, label %b1840
b1836:
  store i64 0, ptr %gcs1
  %v2786 = call i64 @df.box_int(i64 %v2773)
  store i64 %v2786, ptr %gcs1
  %v2787 = call i64 @df.box_int(i64 %v2774)
  store i64 %v2787, ptr %gcs2
  %sa2788_0 = getelementptr [3 x i64], ptr %sargs2788, i64 0, i64 0
  store i64 %v2786, ptr %sa2788_0
  %sa2788_1 = getelementptr [3 x i64], ptr %sargs2788, i64 0, i64 1
  store i64 %v2787, ptr %sa2788_1
  %sat2788 = getelementptr [3 x i64], ptr %sargs2788, i64 0, i64 2
  store i64 0, ptr %sat2788
  %sic2788 = getelementptr i64, ptr %area, i64 382
  %sf2788 = call ptr @df.selector(ptr %sic2788, i64 %v0, i64 -1664458868869587675, ptr @df.seln.21, i64 8)
  %v2788 = call i64 %sf2788(i64 %v0, ptr %sargs2788, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2789 = load i8, ptr %ctx, align 8
  %c572 = zext i8 %v2789 to i64
  %v2790 = icmp ne i64 %c572, 0
  br i1 %v2790, label %b1843, label %b1842
b1837:
  %v2791 = phi i64 [ %v2778, %b1838 ], [ %v2783, %b1840 ], [ %v2788, %b1842 ]
  store i64 %v2791, ptr %gcs0
  %v2792 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2793 = and i64 %v2791, 1
  %v2794 = icmp ne i64 %v2793, 0
  br i1 %v2794, label %b1844, label %b1845
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
  %v2795 = ashr i64 %v2791, 1
  %v2796 = mul i64 %v2795, %v2792
  store i64 0, ptr %gcs0
  %v2797 = call i64 @df.box_int(i64 %v2796)
  br label %b1846
b1845:
  %v2798 = call i64 @df.box_int(i64 %v2792)
  store i64 %v2798, ptr %gcs1
  %sa2799_0 = getelementptr [2 x i64], ptr %sargs2799, i64 0, i64 0
  store i64 %v2798, ptr %sa2799_0
  %sat2799 = getelementptr [2 x i64], ptr %sargs2799, i64 0, i64 1
  store i64 0, ptr %sat2799
  %sic2799 = getelementptr i64, ptr %area, i64 384
  %sf2799 = call ptr @df.selector(ptr %sic2799, i64 %v2791, i64 -4020825035383377172, ptr @df.seln.19, i64 4)
  %v2799 = call i64 %sf2799(i64 %v2791, ptr %sargs2799, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2800 = load i8, ptr %ctx, align 8
  %c573 = zext i8 %v2800 to i64
  %v2801 = icmp ne i64 %c573, 0
  br i1 %v2801, label %b1848, label %b1847
b1846:
  %v2802 = phi i64 [ %v2797, %b1844 ], [ %v2799, %b1847 ]
  br label %b1818
b1847:
  br label %b1846
b1848:
  %gcvolta1848 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1848, ptr %ctxtopo, align 8
  ret i64 0
b1849:
  store i64 0, ptr %gcs1
  %v2805 = call double @df.unbox_double(i64 %v2803)
  %v2806 = load i8, ptr %ctx, align 8
  %c574 = zext i8 %v2806 to i64
  %v2807 = icmp ne i64 %c574, 0
  br i1 %v2807, label %b1854, label %b1853
b1850:
  store i64 0, ptr %gcs1
  %v2811 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2803)
  %v2812 = load i8, ptr %ctx, align 8
  %c575 = zext i8 %v2812 to i64
  %v2813 = icmp ne i64 %c575, 0
  br i1 %v2813, label %b1858, label %b1857
b1851:
  store i64 0, ptr %gcs1
  %sat2814 = getelementptr [1 x i64], ptr %sargs2814, i64 0, i64 0
  store i64 0, ptr %sat2814
  %sic2814 = getelementptr i64, ptr %area, i64 386
  %sf2814 = call ptr @df.selector(ptr %sic2814, i64 %v2803, i64 4857594363590788828, ptr @df.seln.22, i64 8)
  %v2814 = call i64 %sf2814(i64 %v2803, ptr %sargs2814, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2814, ptr %gcs0
  %v2815 = load i8, ptr %ctx, align 8
  %c576 = zext i8 %v2815 to i64
  %v2816 = icmp ne i64 %c576, 0
  br i1 %v2816, label %b1860, label %b1859
b1852:
  %v2820 = phi i64 [ %v2808, %b1855 ], [ %v2811, %b1857 ], [ %v2817, %b1861 ]
  store i64 0, ptr %gcs0
  %v2821 = call i64 @df.box_int(i64 %v2820)
  %xcp2822 = load i8, ptr %ctx, align 8
  %xcn2822 = icmp ne i8 %xcp2822, 0
  %xce2822 = call i1 @llvm.expect.i1(i1 %xcn2822, i1 false)
  br i1 %xce2822, label %xc2822.limpar, label %xc2822.fim
xc2822.limpar:
  call void @dartforge_exception_clear()
  br label %xc2822.fim
xc2822.fim:
  %gcvolta1852 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1852, ptr %ctxtopo, align 8
  ret i64 %v2821
b1853:
  store i64 0, ptr %gcs0
  %v2808 = call i64 @dartforge_native_DartForge_double_toInt(double %v2805)
  %v2809 = load i8, ptr %ctx, align 8
  %c577 = zext i8 %v2809 to i64
  %v2810 = icmp ne i64 %c577, 0
  br i1 %v2810, label %b1856, label %b1855
b1854:
  %gcvolta1854 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1854, ptr %ctxtopo, align 8
  ret i64 0
b1855:
  br label %b1852
b1856:
  %gcvolta1856 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1856, ptr %ctxtopo, align 8
  ret i64 0
b1857:
  br label %b1852
b1858:
  %gcvolta1858 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1858, ptr %ctxtopo, align 8
  ret i64 0
b1859:
  %v2817 = call i64 @df.unbox_int(i64 %v2814)
  %v2818 = load i8, ptr %ctx, align 8
  %c578 = zext i8 %v2818 to i64
  %v2819 = icmp ne i64 %c578, 0
  br i1 %v2819, label %b1862, label %b1861
b1860:
  %gcvolta1860 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1860, ptr %ctxtopo, align 8
  ret i64 0
b1861:
  br label %b1852
b1862:
  %gcvolta1862 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1862, ptr %ctxtopo, align 8
  ret i64 0
}
