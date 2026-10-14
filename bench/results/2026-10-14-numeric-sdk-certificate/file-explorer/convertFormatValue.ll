define i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..convertFormatValue(i64 %v0, i64 %v3, i64 %v6) nounwind {
b0:
  %sargs16 = alloca [2 x i64]
  %sargs82 = alloca [2 x i64]
  %sargs94 = alloca [2 x i64]
  %sargs271 = alloca [2 x i64]
  %sargs163 = alloca [2 x i64]
  %sargs185 = alloca [2 x i64]
  %sargs198 = alloca [2 x i64]
  %sargs211 = alloca [2 x i64]
  %sargs224 = alloca [2 x i64]
  %sargs237 = alloca [2 x i64]
  %sargs250 = alloca [2 x i64]
  %sargs263 = alloca [2 x i64]
  %sargs454 = alloca [2 x i64]
  %sargs336 = alloca [2 x i64]
  %sargs381 = alloca [2 x i64]
  %sargs394 = alloca [2 x i64]
  %sargs407 = alloca [2 x i64]
  %sargs420 = alloca [2 x i64]
  %sargs433 = alloca [2 x i64]
  %sargs446 = alloca [2 x i64]
  %sargs657 = alloca [2 x i64]
  %sargs519 = alloca [2 x i64]
  %sargs587 = alloca [2 x i64]
  %sargs600 = alloca [2 x i64]
  %sargs636 = alloca [2 x i64]
  %sargs649 = alloca [2 x i64]
  %sargs890 = alloca [2 x i64]
  %sargs722 = alloca [2 x i64]
  %sargs882 = alloca [2 x i64]
  %sargs1133 = alloca [2 x i64]
  %sargs955 = alloca [2 x i64]
  %sargs1436 = alloca [2 x i64]
  %sargs1198 = alloca [2 x i64]
  %sargs1217 = alloca [2 x i64]
  %sargs1256 = alloca [2 x i64]
  %sargs1295 = alloca [2 x i64]
  %sargs1334 = alloca [2 x i64]
  %sargs1373 = alloca [2 x i64]
  %sargs1415 = alloca [2 x i64]
  %sargs1428 = alloca [2 x i64]
  %sargs1759 = alloca [2 x i64]
  %sargs1501 = alloca [2 x i64]
  %sargs1520 = alloca [2 x i64]
  %sargs1559 = alloca [2 x i64]
  %sargs1598 = alloca [2 x i64]
  %sargs1637 = alloca [2 x i64]
  %sargs1676 = alloca [2 x i64]
  %sargs2082 = alloca [2 x i64]
  %sargs1824 = alloca [2 x i64]
  %sargs1843 = alloca [2 x i64]
  %sargs1882 = alloca [2 x i64]
  %sargs1921 = alloca [2 x i64]
  %sargs1960 = alloca [2 x i64]
  %sargs1999 = alloca [2 x i64]
  %sargs2164 = alloca [3 x i64]
  %sargs2191 = alloca [3 x i64]
  %sargs2218 = alloca [3 x i64]
  %sargs2245 = alloca [3 x i64]
  %sargs2272 = alloca [3 x i64]
  %sargs2147 = alloca [2 x i64]
  %sargs2174 = alloca [2 x i64]
  %sargs2178 = alloca [1 x i64]
  %sargs2201 = alloca [2 x i64]
  %sargs2205 = alloca [1 x i64]
  %sargs2228 = alloca [2 x i64]
  %sargs2232 = alloca [1 x i64]
  %sargs2255 = alloca [2 x i64]
  %sargs2259 = alloca [1 x i64]
  %sargs2282 = alloca [2 x i64]
  %sargs2286 = alloca [1 x i64]
  %sargs2301 = alloca [2 x i64]
  %sargs2314 = alloca [3 x i64]
  %sargs2334 = alloca [3 x i64]
  %sargs2349 = alloca [1 x i64]
  %sargs2324 = alloca [2 x i64]
  %sargs2344 = alloca [2 x i64]
  %sargs2364 = alloca [2 x i64]
  %sargs2377 = alloca [3 x i64]
  %sargs2397 = alloca [3 x i64]
  %sargs2412 = alloca [1 x i64]
  %sargs2387 = alloca [2 x i64]
  %sargs2407 = alloca [2 x i64]
  %sargs2427 = alloca [2 x i64]
  %sargs2440 = alloca [3 x i64]
  %sargs2460 = alloca [3 x i64]
  %sargs2475 = alloca [1 x i64]
  %sargs2450 = alloca [2 x i64]
  %sargs2470 = alloca [2 x i64]
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
  %sic16 = getelementptr i64, ptr %area, i64 141690
  %sf16 = call ptr @df.selector(ptr %sic16, i64 %v3, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
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
  %v2091 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v2091, ptr %gcs1
  %v2092 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v2092 to i64
  %v2093 = icmp ne i64 %c5, 0
  br i1 %v2093, label %b1380, label %b1379
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
  %v107 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v107, ptr %gcs1
  %v108 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v108 to i64
  %v109 = icmp ne i64 %c10, 0
  br i1 %v109, label %b110, label %b109
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
  %v280 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v280, ptr %gcs1
  %v281 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v281 to i64
  %v282 = icmp ne i64 %c12, 0
  br i1 %v282, label %b232, label %b231
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
  %v463 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v463, ptr %gcs1
  %v464 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v464 to i64
  %v465 = icmp ne i64 %c14, 0
  br i1 %v465, label %b358, label %b357
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
  %v666 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v666, ptr %gcs1
  %v667 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v667 to i64
  %v668 = icmp ne i64 %c16, 0
  br i1 %v668, label %b492, label %b491
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
  %v899 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v899, ptr %gcs1
  %v900 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v900 to i64
  %v901 = icmp ne i64 %c18, 0
  br i1 %v901, label %b638, label %b637
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
  %v1142 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1142, ptr %gcs1
  %v1143 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v1143 to i64
  %v1144 = icmp ne i64 %c20, 0
  br i1 %v1144, label %b788, label %b787
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
  %v1445 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1445, ptr %gcs1
  %v1446 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v1446 to i64
  %v1447 = icmp ne i64 %c22, 0
  br i1 %v1447, label %b980, label %b979
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
  %v1768 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  store i64 %v1768, ptr %gcs1
  %v1769 = load i8, ptr %ctx, align 8
  %c24 = zext i8 %v1769 to i64
  %v1770 = icmp ne i64 %c24, 0
  br i1 %v1770, label %b1180, label %b1179
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
  %sa82_0 = getelementptr [2 x i64], ptr %sargs82, i64 0, i64 0
  store i64 %v76, ptr %sa82_0
  %sat82 = getelementptr [2 x i64], ptr %sargs82, i64 0, i64 1
  store i64 0, ptr %sat82
  %sic82 = getelementptr i64, ptr %area, i64 141692
  %sf82 = call ptr @df.selector(ptr %sic82, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v82 = call i64 %sf82(i64 %v0, ptr %sargs82, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v82, ptr %gcs0
  %v83 = load i8, ptr %ctx, align 8
  %c27 = zext i8 %v83 to i64
  %v84 = icmp ne i64 %c27, 0
  br i1 %v84, label %b79, label %b78
b77:
  %v88 = phi i1 [ %v81, %b75 ], [ %v85, %b80 ]
  br i1 %v88, label %b82, label %b83
b78:
  store i64 0, ptr %gcs1
  %v85 = call i1 @df.unbox_bool(i64 %v82)
  %v86 = load i8, ptr %ctx, align 8
  %c28 = zext i8 %v86 to i64
  %v87 = icmp ne i64 %c28, 0
  br i1 %v87, label %b81, label %b80
b79:
  %gcvolta79 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta79, ptr %ctxtopo, align 8
  ret i64 0
b80:
  br label %b77
b81:
  %gcvolta81 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta81, ptr %ctxtopo, align 8
  ret i64 0
b82:
  %v89 = add i64 0, 0
  br label %b84
b83:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v90 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..formatMaxValue()
  store i64 %v90, ptr %gcs0
  %v91 = load i8, ptr %ctx, align 8
  %c29 = zext i8 %v91 to i64
  %v92 = icmp ne i64 %c29, 0
  br i1 %v92, label %b86, label %b85
b84:
  %v103 = phi i64 [ %v89, %b82 ], [ %v100, %b92 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v104 = call i64 @df.box_int(i64 %v103)
  %xcp105 = load i8, ptr %ctx, align 8
  %xcn105 = icmp ne i8 %xcp105, 0
  %xce105 = call i1 @llvm.expect.i1(i1 %xcn105, i1 false)
  br i1 %xce105, label %xc105.limpar, label %xc105.fim
xc105.limpar:
  call void @dartforge_exception_clear()
  br label %xc105.fim
xc105.fim:
  %gcvolta84 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta84, ptr %ctxtopo, align 8
  ret i64 %v104
b85:
  %sa94_0 = getelementptr [2 x i64], ptr %sargs94, i64 0, i64 0
  store i64 %v6, ptr %sa94_0
  %sat94 = getelementptr [2 x i64], ptr %sargs94, i64 0, i64 1
  store i64 0, ptr %sat94
  %sic94 = getelementptr i64, ptr %area, i64 141694
  %sf94 = call ptr @df.selector(ptr %sic94, i64 %v90, i64 2870274114568068918, ptr @df.seln.4, i64 5)
  %v94 = call i64 %sf94(i64 %v90, ptr %sargs94, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v94, ptr %gcs0
  %v95 = load i8, ptr %ctx, align 8
  %c30 = zext i8 %v95 to i64
  %v96 = icmp ne i64 %c30, 0
  br i1 %v96, label %b88, label %b87
b86:
  %gcvolta86 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta86, ptr %ctxtopo, align 8
  ret i64 0
b87:
  store i64 0, ptr %gcs2
  %v97 = icmp eq i64 %v94, 0
  br i1 %v97, label %b89, label %b90
b88:
  %gcvolta88 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta88, ptr %ctxtopo, align 8
  ret i64 0
b89:
  store i64 0, ptr %gcs0
  %v98 = call i64 @dartforge_null_check_error_new()
  store i64 %v98, ptr %gcs0
  call void @dartforge_exception_throw(i64 %v98, i8 3)
  %gcvolta89 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta89, ptr %ctxtopo, align 8
  ret i64 0
b90:
  %v100 = call i64 @df.unbox_int(i64 %v94)
  %v101 = load i8, ptr %ctx, align 8
  %c31 = zext i8 %v101 to i64
  %v102 = icmp ne i64 %c31, 0
  br i1 %v102, label %b93, label %b92
b92:
  br label %b84
b93:
  %gcvolta93 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta93, ptr %ctxtopo, align 8
  ret i64 0
b107:
  %v269 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v270 = call i64 @df.box_int(i64 %v269)
  store i64 %v270, ptr %gcs1
  %sa271_0 = getelementptr [2 x i64], ptr %sargs271, i64 0, i64 0
  store i64 %v270, ptr %sa271_0
  %sat271 = getelementptr [2 x i64], ptr %sargs271, i64 0, i64 1
  store i64 0, ptr %sat271
  %sic271 = getelementptr i64, ptr %area, i64 141696
  %sf271 = call ptr @df.selector(ptr %sic271, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v271 = call i64 %sf271(i64 %v0, ptr %sargs271, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v271, ptr %gcs0
  %v272 = load i8, ptr %ctx, align 8
  %c32 = zext i8 %v272 to i64
  %v273 = icmp ne i64 %c32, 0
  br i1 %v273, label %b213, label %b212
b108:
  store i64 0, ptr %gcs1
  %v111 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v111, ptr %gcs1
  %v112 = load i8, ptr %ctx, align 8
  %c33 = zext i8 %v112 to i64
  %v113 = icmp ne i64 %c33, 0
  br i1 %v113, label %b114, label %b113
b109:
  %v110 = icmp eq i64 %v107, %v6
  br i1 %v110, label %b111, label %b108
b110:
  %gcvolta110 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta110, ptr %ctxtopo, align 8
  ret i64 0
b111:
  %v156 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v157 = call i64 @df.box_int(i64 %v156)
  store i64 %v157, ptr %gcs1
  %v158 = icmp eq i64 %v0, 0
  %v159 = icmp eq i64 %v157, 0
  %c34 = zext i1 %v158 to i64
  %c35 = zext i1 %v159 to i64
  %v160 = or i64 %c34, %c35
  %v161 = icmp ne i64 %v160, 0
  br i1 %v161, label %b158, label %b159
b112:
  store i64 0, ptr %gcs1
  %v115 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v115, ptr %gcs1
  %v116 = load i8, ptr %ctx, align 8
  %c36 = zext i8 %v116 to i64
  %v117 = icmp ne i64 %c36, 0
  br i1 %v117, label %b118, label %b117
b113:
  %v114 = icmp eq i64 %v111, %v6
  br i1 %v114, label %b115, label %b112
b114:
  %gcvolta114 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta114, ptr %ctxtopo, align 8
  ret i64 0
b115:
  %xcp176 = load i8, ptr %ctx, align 8
  %xcn176 = icmp ne i8 %xcp176, 0
  %xce176 = call i1 @llvm.expect.i1(i1 %xcn176, i1 false)
  br i1 %xce176, label %xc176.limpar, label %xc176.fim
xc176.limpar:
  call void @dartforge_exception_clear()
  br label %xc176.fim
xc176.fim:
  %gcvolta115 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta115, ptr %ctxtopo, align 8
  ret i64 %v0
b116:
  store i64 0, ptr %gcs1
  %v119 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v119, ptr %gcs1
  %v120 = load i8, ptr %ctx, align 8
  %c37 = zext i8 %v120 to i64
  %v121 = icmp ne i64 %c37, 0
  br i1 %v121, label %b122, label %b121
b117:
  %v118 = icmp eq i64 %v115, %v6
  br i1 %v118, label %b119, label %b116
b118:
  %gcvolta118 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta118, ptr %ctxtopo, align 8
  ret i64 0
b119:
  %v178 = add i64 0, 5
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v179 = and i64 %v0, 1
  %v180 = icmp ne i64 %v179, 0
  br i1 %v180, label %b170, label %b171
b120:
  store i64 0, ptr %gcs1
  %v123 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v123, ptr %gcs1
  %v124 = load i8, ptr %ctx, align 8
  %c38 = zext i8 %v124 to i64
  %v125 = icmp ne i64 %c38, 0
  br i1 %v125, label %b126, label %b125
b121:
  %v122 = icmp eq i64 %v119, %v6
  br i1 %v122, label %b123, label %b120
b122:
  %gcvolta122 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta122, ptr %ctxtopo, align 8
  ret i64 0
b123:
  %v191 = add i64 0, 75
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v192 = and i64 %v0, 1
  %v193 = icmp ne i64 %v192, 0
  br i1 %v193, label %b176, label %b177
b124:
  store i64 0, ptr %gcs1
  %v127 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v127, ptr %gcs1
  %v128 = load i8, ptr %ctx, align 8
  %c39 = zext i8 %v128 to i64
  %v129 = icmp ne i64 %c39, 0
  br i1 %v129, label %b130, label %b129
b125:
  %v126 = icmp eq i64 %v123, %v6
  br i1 %v126, label %b127, label %b124
b126:
  %gcvolta126 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta126, ptr %ctxtopo, align 8
  ret i64 0
b127:
  %v204 = add i64 0, 21845
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v205 = and i64 %v0, 1
  %v206 = icmp ne i64 %v205, 0
  br i1 %v206, label %b182, label %b183
b128:
  store i64 0, ptr %gcs1
  %v131 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v131, ptr %gcs1
  %v132 = load i8, ptr %ctx, align 8
  %c40 = zext i8 %v132 to i64
  %v133 = icmp ne i64 %c40, 0
  br i1 %v133, label %b134, label %b133
b129:
  %v130 = icmp eq i64 %v127, %v6
  br i1 %v130, label %b131, label %b128
b130:
  %gcvolta130 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta130, ptr %ctxtopo, align 8
  ret i64 0
b131:
  %v217 = add i64 0, 1431655765
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v218 = and i64 %v0, 1
  %v219 = icmp ne i64 %v218, 0
  br i1 %v219, label %b188, label %b189
b132:
  store i64 0, ptr %gcs1
  %v135 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v135, ptr %gcs1
  %v136 = load i8, ptr %ctx, align 8
  %c41 = zext i8 %v136 to i64
  %v137 = icmp ne i64 %c41, 0
  br i1 %v137, label %b138, label %b137
b133:
  %v134 = icmp eq i64 %v131, %v6
  br i1 %v134, label %b135, label %b132
b134:
  %gcvolta134 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta134, ptr %ctxtopo, align 8
  ret i64 0
b135:
  %v230 = add i64 0, 42
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v231 = and i64 %v0, 1
  %v232 = icmp ne i64 %v231, 0
  br i1 %v232, label %b194, label %b195
b136:
  store i64 0, ptr %gcs1
  %v139 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v139, ptr %gcs1
  %v140 = load i8, ptr %ctx, align 8
  %c42 = zext i8 %v140 to i64
  %v141 = icmp ne i64 %c42, 0
  br i1 %v141, label %b142, label %b141
b137:
  %v138 = icmp eq i64 %v135, %v6
  br i1 %v138, label %b139, label %b136
b138:
  %gcvolta138 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta138, ptr %ctxtopo, align 8
  ret i64 0
b139:
  %v243 = add i64 0, 10922
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v244 = and i64 %v0, 1
  %v245 = icmp ne i64 %v244, 0
  br i1 %v245, label %b200, label %b201
b140:
  store i64 0, ptr %gcs1
  %v143 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v143, ptr %gcs1
  %v144 = load i8, ptr %ctx, align 8
  %c43 = zext i8 %v144 to i64
  %v145 = icmp ne i64 %c43, 0
  br i1 %v145, label %b146, label %b145
b141:
  %v142 = icmp eq i64 %v139, %v6
  br i1 %v142, label %b143, label %b140
b142:
  %gcvolta142 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta142, ptr %ctxtopo, align 8
  ret i64 0
b143:
  %v256 = add i64 0, 715827882
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v257 = and i64 %v0, 1
  %v258 = icmp ne i64 %v257, 0
  br i1 %v258, label %b206, label %b207
b144:
  store i64 0, ptr %gcs1
  %v147 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v147, ptr %gcs1
  %v148 = load i8, ptr %ctx, align 8
  %c44 = zext i8 %v148 to i64
  %v149 = icmp ne i64 %c44, 0
  br i1 %v149, label %b151, label %b150
b145:
  %v146 = icmp eq i64 %v143, %v6
  br i1 %v146, label %b147, label %b144
b146:
  %gcvolta146 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta146, ptr %ctxtopo, align 8
  ret i64 0
b147:
  br label %b107
b149:
  store i64 0, ptr %gcs1
  %v151 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v151, ptr %gcs1
  %v152 = load i8, ptr %ctx, align 8
  %c45 = zext i8 %v152 to i64
  %v153 = icmp ne i64 %c45, 0
  br i1 %v153, label %b156, label %b155
b150:
  %v150 = icmp eq i64 %v147, %v6
  br i1 %v150, label %b152, label %b149
b151:
  %gcvolta151 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta151, ptr %ctxtopo, align 8
  ret i64 0
b152:
  br label %b107
b154:
  br label %b12
b155:
  %v154 = icmp eq i64 %v151, %v6
  br i1 %v154, label %b157, label %b154
b156:
  %gcvolta156 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta156, ptr %ctxtopo, align 8
  ret i64 0
b157:
  br label %b107
b158:
  %v162 = icmp eq i64 %v0, %v157
  br label %b160
b159:
  %sa163_0 = getelementptr [2 x i64], ptr %sargs163, i64 0, i64 0
  store i64 %v157, ptr %sa163_0
  %sat163 = getelementptr [2 x i64], ptr %sargs163, i64 0, i64 1
  store i64 0, ptr %sat163
  %sic163 = getelementptr i64, ptr %area, i64 141698
  %sf163 = call ptr @df.selector(ptr %sic163, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v163 = call i64 %sf163(i64 %v0, ptr %sargs163, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v163, ptr %gcs0
  %v164 = load i8, ptr %ctx, align 8
  %c46 = zext i8 %v164 to i64
  %v165 = icmp ne i64 %c46, 0
  br i1 %v165, label %b162, label %b161
b160:
  %v169 = phi i1 [ %v162, %b158 ], [ %v166, %b163 ]
  br i1 %v169, label %b165, label %b166
b161:
  store i64 0, ptr %gcs1
  %v166 = call i1 @df.unbox_bool(i64 %v163)
  %v167 = load i8, ptr %ctx, align 8
  %c47 = zext i8 %v167 to i64
  %v168 = icmp ne i64 %c47, 0
  br i1 %v168, label %b164, label %b163
b162:
  %gcvolta162 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta162, ptr %ctxtopo, align 8
  ret i64 0
b163:
  br label %b160
b164:
  %gcvolta164 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta164, ptr %ctxtopo, align 8
  ret i64 0
b165:
  %v170 = add i64 0, 0
  br label %b167
b166:
  %v171 = add i64 0, 1
  br label %b167
b167:
  %v172 = phi i64 [ %v170, %b165 ], [ %v171, %b166 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v173 = call i64 @df.box_int(i64 %v172)
  %xcp174 = load i8, ptr %ctx, align 8
  %xcn174 = icmp ne i8 %xcp174, 0
  %xce174 = call i1 @llvm.expect.i1(i1 %xcn174, i1 false)
  br i1 %xce174, label %xc174.limpar, label %xc174.fim
xc174.limpar:
  call void @dartforge_exception_clear()
  br label %xc174.fim
xc174.fim:
  %gcvolta167 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta167, ptr %ctxtopo, align 8
  ret i64 %v173
b170:
  %v181 = ashr i64 %v0, 1
  %v182 = mul i64 %v181, %v178
  store i64 0, ptr %gcs0
  %v183 = call i64 @df.box_int(i64 %v182)
  br label %b172
b171:
  %v184 = call i64 @df.box_int(i64 %v178)
  store i64 %v184, ptr %gcs1
  %sa185_0 = getelementptr [2 x i64], ptr %sargs185, i64 0, i64 0
  store i64 %v184, ptr %sa185_0
  %sat185 = getelementptr [2 x i64], ptr %sargs185, i64 0, i64 1
  store i64 0, ptr %sat185
  %sic185 = getelementptr i64, ptr %area, i64 141700
  %sf185 = call ptr @df.selector(ptr %sic185, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v185 = call i64 %sf185(i64 %v0, ptr %sargs185, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v186 = load i8, ptr %ctx, align 8
  %c48 = zext i8 %v186 to i64
  %v187 = icmp ne i64 %c48, 0
  br i1 %v187, label %b174, label %b173
b172:
  %v188 = phi i64 [ %v183, %b170 ], [ %v185, %b173 ]
  %xcp189 = load i8, ptr %ctx, align 8
  %xcn189 = icmp ne i8 %xcp189, 0
  %xce189 = call i1 @llvm.expect.i1(i1 %xcn189, i1 false)
  br i1 %xce189, label %xc189.limpar, label %xc189.fim
xc189.limpar:
  call void @dartforge_exception_clear()
  br label %xc189.fim
xc189.fim:
  %gcvolta172 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta172, ptr %ctxtopo, align 8
  ret i64 %v188
b173:
  br label %b172
b174:
  %gcvolta174 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta174, ptr %ctxtopo, align 8
  ret i64 0
b176:
  %v194 = ashr i64 %v0, 1
  %v195 = mul i64 %v194, %v191
  store i64 0, ptr %gcs0
  %v196 = call i64 @df.box_int(i64 %v195)
  br label %b178
b177:
  %v197 = call i64 @df.box_int(i64 %v191)
  store i64 %v197, ptr %gcs1
  %sa198_0 = getelementptr [2 x i64], ptr %sargs198, i64 0, i64 0
  store i64 %v197, ptr %sa198_0
  %sat198 = getelementptr [2 x i64], ptr %sargs198, i64 0, i64 1
  store i64 0, ptr %sat198
  %sic198 = getelementptr i64, ptr %area, i64 141702
  %sf198 = call ptr @df.selector(ptr %sic198, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v198 = call i64 %sf198(i64 %v0, ptr %sargs198, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v199 = load i8, ptr %ctx, align 8
  %c49 = zext i8 %v199 to i64
  %v200 = icmp ne i64 %c49, 0
  br i1 %v200, label %b180, label %b179
b178:
  %v201 = phi i64 [ %v196, %b176 ], [ %v198, %b179 ]
  %xcp202 = load i8, ptr %ctx, align 8
  %xcn202 = icmp ne i8 %xcp202, 0
  %xce202 = call i1 @llvm.expect.i1(i1 %xcn202, i1 false)
  br i1 %xce202, label %xc202.limpar, label %xc202.fim
xc202.limpar:
  call void @dartforge_exception_clear()
  br label %xc202.fim
xc202.fim:
  %gcvolta178 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta178, ptr %ctxtopo, align 8
  ret i64 %v201
b179:
  br label %b178
b180:
  %gcvolta180 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta180, ptr %ctxtopo, align 8
  ret i64 0
b182:
  %v207 = ashr i64 %v0, 1
  %v208 = mul i64 %v207, %v204
  store i64 0, ptr %gcs0
  %v209 = call i64 @df.box_int(i64 %v208)
  br label %b184
b183:
  %v210 = call i64 @df.box_int(i64 %v204)
  store i64 %v210, ptr %gcs1
  %sa211_0 = getelementptr [2 x i64], ptr %sargs211, i64 0, i64 0
  store i64 %v210, ptr %sa211_0
  %sat211 = getelementptr [2 x i64], ptr %sargs211, i64 0, i64 1
  store i64 0, ptr %sat211
  %sic211 = getelementptr i64, ptr %area, i64 141704
  %sf211 = call ptr @df.selector(ptr %sic211, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v211 = call i64 %sf211(i64 %v0, ptr %sargs211, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v212 = load i8, ptr %ctx, align 8
  %c50 = zext i8 %v212 to i64
  %v213 = icmp ne i64 %c50, 0
  br i1 %v213, label %b186, label %b185
b184:
  %v214 = phi i64 [ %v209, %b182 ], [ %v211, %b185 ]
  %xcp215 = load i8, ptr %ctx, align 8
  %xcn215 = icmp ne i8 %xcp215, 0
  %xce215 = call i1 @llvm.expect.i1(i1 %xcn215, i1 false)
  br i1 %xce215, label %xc215.limpar, label %xc215.fim
xc215.limpar:
  call void @dartforge_exception_clear()
  br label %xc215.fim
xc215.fim:
  %gcvolta184 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta184, ptr %ctxtopo, align 8
  ret i64 %v214
b185:
  br label %b184
b186:
  %gcvolta186 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta186, ptr %ctxtopo, align 8
  ret i64 0
b188:
  %v220 = ashr i64 %v0, 1
  %v221 = mul i64 %v220, %v217
  store i64 0, ptr %gcs0
  %v222 = call i64 @df.box_int(i64 %v221)
  br label %b190
b189:
  %v223 = call i64 @df.box_int(i64 %v217)
  store i64 %v223, ptr %gcs1
  %sa224_0 = getelementptr [2 x i64], ptr %sargs224, i64 0, i64 0
  store i64 %v223, ptr %sa224_0
  %sat224 = getelementptr [2 x i64], ptr %sargs224, i64 0, i64 1
  store i64 0, ptr %sat224
  %sic224 = getelementptr i64, ptr %area, i64 141706
  %sf224 = call ptr @df.selector(ptr %sic224, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v224 = call i64 %sf224(i64 %v0, ptr %sargs224, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v225 = load i8, ptr %ctx, align 8
  %c51 = zext i8 %v225 to i64
  %v226 = icmp ne i64 %c51, 0
  br i1 %v226, label %b192, label %b191
b190:
  %v227 = phi i64 [ %v222, %b188 ], [ %v224, %b191 ]
  %xcp228 = load i8, ptr %ctx, align 8
  %xcn228 = icmp ne i8 %xcp228, 0
  %xce228 = call i1 @llvm.expect.i1(i1 %xcn228, i1 false)
  br i1 %xce228, label %xc228.limpar, label %xc228.fim
xc228.limpar:
  call void @dartforge_exception_clear()
  br label %xc228.fim
xc228.fim:
  %gcvolta190 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta190, ptr %ctxtopo, align 8
  ret i64 %v227
b191:
  br label %b190
b192:
  %gcvolta192 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta192, ptr %ctxtopo, align 8
  ret i64 0
b194:
  %v233 = ashr i64 %v0, 1
  %v234 = mul i64 %v233, %v230
  store i64 0, ptr %gcs0
  %v235 = call i64 @df.box_int(i64 %v234)
  br label %b196
b195:
  %v236 = call i64 @df.box_int(i64 %v230)
  store i64 %v236, ptr %gcs1
  %sa237_0 = getelementptr [2 x i64], ptr %sargs237, i64 0, i64 0
  store i64 %v236, ptr %sa237_0
  %sat237 = getelementptr [2 x i64], ptr %sargs237, i64 0, i64 1
  store i64 0, ptr %sat237
  %sic237 = getelementptr i64, ptr %area, i64 141708
  %sf237 = call ptr @df.selector(ptr %sic237, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v237 = call i64 %sf237(i64 %v0, ptr %sargs237, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v238 = load i8, ptr %ctx, align 8
  %c52 = zext i8 %v238 to i64
  %v239 = icmp ne i64 %c52, 0
  br i1 %v239, label %b198, label %b197
b196:
  %v240 = phi i64 [ %v235, %b194 ], [ %v237, %b197 ]
  %xcp241 = load i8, ptr %ctx, align 8
  %xcn241 = icmp ne i8 %xcp241, 0
  %xce241 = call i1 @llvm.expect.i1(i1 %xcn241, i1 false)
  br i1 %xce241, label %xc241.limpar, label %xc241.fim
xc241.limpar:
  call void @dartforge_exception_clear()
  br label %xc241.fim
xc241.fim:
  %gcvolta196 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta196, ptr %ctxtopo, align 8
  ret i64 %v240
b197:
  br label %b196
b198:
  %gcvolta198 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta198, ptr %ctxtopo, align 8
  ret i64 0
b200:
  %v246 = ashr i64 %v0, 1
  %v247 = mul i64 %v246, %v243
  store i64 0, ptr %gcs0
  %v248 = call i64 @df.box_int(i64 %v247)
  br label %b202
b201:
  %v249 = call i64 @df.box_int(i64 %v243)
  store i64 %v249, ptr %gcs1
  %sa250_0 = getelementptr [2 x i64], ptr %sargs250, i64 0, i64 0
  store i64 %v249, ptr %sa250_0
  %sat250 = getelementptr [2 x i64], ptr %sargs250, i64 0, i64 1
  store i64 0, ptr %sat250
  %sic250 = getelementptr i64, ptr %area, i64 141710
  %sf250 = call ptr @df.selector(ptr %sic250, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v250 = call i64 %sf250(i64 %v0, ptr %sargs250, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v251 = load i8, ptr %ctx, align 8
  %c53 = zext i8 %v251 to i64
  %v252 = icmp ne i64 %c53, 0
  br i1 %v252, label %b204, label %b203
b202:
  %v253 = phi i64 [ %v248, %b200 ], [ %v250, %b203 ]
  %xcp254 = load i8, ptr %ctx, align 8
  %xcn254 = icmp ne i8 %xcp254, 0
  %xce254 = call i1 @llvm.expect.i1(i1 %xcn254, i1 false)
  br i1 %xce254, label %xc254.limpar, label %xc254.fim
xc254.limpar:
  call void @dartforge_exception_clear()
  br label %xc254.fim
xc254.fim:
  %gcvolta202 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta202, ptr %ctxtopo, align 8
  ret i64 %v253
b203:
  br label %b202
b204:
  %gcvolta204 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta204, ptr %ctxtopo, align 8
  ret i64 0
b206:
  %v259 = ashr i64 %v0, 1
  %v260 = mul i64 %v259, %v256
  store i64 0, ptr %gcs0
  %v261 = call i64 @df.box_int(i64 %v260)
  br label %b208
b207:
  %v262 = call i64 @df.box_int(i64 %v256)
  store i64 %v262, ptr %gcs1
  %sa263_0 = getelementptr [2 x i64], ptr %sargs263, i64 0, i64 0
  store i64 %v262, ptr %sa263_0
  %sat263 = getelementptr [2 x i64], ptr %sargs263, i64 0, i64 1
  store i64 0, ptr %sat263
  %sic263 = getelementptr i64, ptr %area, i64 141712
  %sf263 = call ptr @df.selector(ptr %sic263, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v263 = call i64 %sf263(i64 %v0, ptr %sargs263, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v264 = load i8, ptr %ctx, align 8
  %c54 = zext i8 %v264 to i64
  %v265 = icmp ne i64 %c54, 0
  br i1 %v265, label %b210, label %b209
b208:
  %v266 = phi i64 [ %v261, %b206 ], [ %v263, %b209 ]
  %xcp267 = load i8, ptr %ctx, align 8
  %xcn267 = icmp ne i8 %xcp267, 0
  %xce267 = call i1 @llvm.expect.i1(i1 %xcn267, i1 false)
  br i1 %xce267, label %xc267.limpar, label %xc267.fim
xc267.limpar:
  call void @dartforge_exception_clear()
  br label %xc267.fim
xc267.fim:
  %gcvolta208 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta208, ptr %ctxtopo, align 8
  ret i64 %v266
b209:
  br label %b208
b210:
  %gcvolta210 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta210, ptr %ctxtopo, align 8
  ret i64 0
b212:
  store i64 0, ptr %gcs1
  %v274 = call double @df.unbox_double(i64 %v271)
  %v275 = load i8, ptr %ctx, align 8
  %c55 = zext i8 %v275 to i64
  %v276 = icmp ne i64 %c55, 0
  br i1 %v276, label %b215, label %b214
b213:
  %gcvolta213 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta213, ptr %ctxtopo, align 8
  ret i64 0
b214:
  %xcp278 = load i8, ptr %ctx, align 8
  %xcn278 = icmp ne i8 %xcp278, 0
  %xce278 = call i1 @llvm.expect.i1(i1 %xcn278, i1 false)
  br i1 %xce278, label %xc278.limpar, label %xc278.fim
xc278.limpar:
  call void @dartforge_exception_clear()
  br label %xc278.fim
xc278.fim:
  %gcvolta214 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta214, ptr %ctxtopo, align 8
  ret i64 %v271
b215:
  %gcvolta215 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta215, ptr %ctxtopo, align 8
  ret i64 0
b229:
  %v452 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v453 = call i64 @df.box_int(i64 %v452)
  store i64 %v453, ptr %gcs1
  %sa454_0 = getelementptr [2 x i64], ptr %sargs454, i64 0, i64 0
  store i64 %v453, ptr %sa454_0
  %sat454 = getelementptr [2 x i64], ptr %sargs454, i64 0, i64 1
  store i64 0, ptr %sat454
  %sic454 = getelementptr i64, ptr %area, i64 141714
  %sf454 = call ptr @df.selector(ptr %sic454, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v454 = call i64 %sf454(i64 %v0, ptr %sargs454, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v454, ptr %gcs0
  %v455 = load i8, ptr %ctx, align 8
  %c56 = zext i8 %v455 to i64
  %v456 = icmp ne i64 %c56, 0
  br i1 %v456, label %b339, label %b338
b230:
  store i64 0, ptr %gcs1
  %v284 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v284, ptr %gcs1
  %v285 = load i8, ptr %ctx, align 8
  %c57 = zext i8 %v285 to i64
  %v286 = icmp ne i64 %c57, 0
  br i1 %v286, label %b236, label %b235
b231:
  %v283 = icmp eq i64 %v280, %v6
  br i1 %v283, label %b233, label %b230
b232:
  %gcvolta232 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta232, ptr %ctxtopo, align 8
  ret i64 0
b233:
  %v329 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v330 = call i64 @df.box_int(i64 %v329)
  store i64 %v330, ptr %gcs1
  %v331 = icmp eq i64 %v0, 0
  %v332 = icmp eq i64 %v330, 0
  %c58 = zext i1 %v331 to i64
  %c59 = zext i1 %v332 to i64
  %v333 = or i64 %c58, %c59
  %v334 = icmp ne i64 %v333, 0
  br i1 %v334, label %b280, label %b281
b234:
  store i64 0, ptr %gcs1
  %v288 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v288, ptr %gcs1
  %v289 = load i8, ptr %ctx, align 8
  %c60 = zext i8 %v289 to i64
  %v290 = icmp ne i64 %c60, 0
  br i1 %v290, label %b240, label %b239
b235:
  %v287 = icmp eq i64 %v284, %v6
  br i1 %v287, label %b237, label %b234
b236:
  %gcvolta236 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta236, ptr %ctxtopo, align 8
  ret i64 0
b237:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v349 = icmp eq i64 %v0, 0
  %v350 = call i64 @df.class(i64 %v0)
  %v351 = call i8 @df.subclass(i64 %v350, i64 674)
  %c61 = zext i8 %v351 to i64
  %v352 = icmp ne i64 %c61, 0
  %v353 = xor i1 %v349, true
  %c62 = zext i1 %v352 to i64
  %c63 = zext i1 %v353 to i64
  %v354 = and i64 %c62, %c63
  %v355 = icmp ne i64 %v354, 0
  br i1 %v355, label %b292, label %b291
b238:
  store i64 0, ptr %gcs1
  %v292 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v292, ptr %gcs1
  %v293 = load i8, ptr %ctx, align 8
  %c64 = zext i8 %v293 to i64
  %v294 = icmp ne i64 %c64, 0
  br i1 %v294, label %b244, label %b243
b239:
  %v291 = icmp eq i64 %v288, %v6
  br i1 %v291, label %b241, label %b238
b240:
  %gcvolta240 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta240, ptr %ctxtopo, align 8
  ret i64 0
b241:
  %xcp372 = load i8, ptr %ctx, align 8
  %xcn372 = icmp ne i8 %xcp372, 0
  %xce372 = call i1 @llvm.expect.i1(i1 %xcn372, i1 false)
  br i1 %xce372, label %xc372.limpar, label %xc372.fim
xc372.limpar:
  call void @dartforge_exception_clear()
  br label %xc372.fim
xc372.fim:
  %gcvolta241 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta241, ptr %ctxtopo, align 8
  ret i64 %v0
b242:
  store i64 0, ptr %gcs1
  %v296 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v296, ptr %gcs1
  %v297 = load i8, ptr %ctx, align 8
  %c65 = zext i8 %v297 to i64
  %v298 = icmp ne i64 %c65, 0
  br i1 %v298, label %b248, label %b247
b243:
  %v295 = icmp eq i64 %v292, %v6
  br i1 %v295, label %b245, label %b242
b244:
  %gcvolta244 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta244, ptr %ctxtopo, align 8
  ret i64 0
b245:
  %v374 = add i64 0, 17
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v375 = and i64 %v0, 1
  %v376 = icmp ne i64 %v375, 0
  br i1 %v376, label %b302, label %b303
b246:
  store i64 0, ptr %gcs1
  %v300 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v300, ptr %gcs1
  %v301 = load i8, ptr %ctx, align 8
  %c66 = zext i8 %v301 to i64
  %v302 = icmp ne i64 %c66, 0
  br i1 %v302, label %b252, label %b251
b247:
  %v299 = icmp eq i64 %v296, %v6
  br i1 %v299, label %b249, label %b246
b248:
  %gcvolta248 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta248, ptr %ctxtopo, align 8
  ret i64 0
b249:
  %v387 = add i64 0, 4369
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v388 = and i64 %v0, 1
  %v389 = icmp ne i64 %v388, 0
  br i1 %v389, label %b308, label %b309
b250:
  store i64 0, ptr %gcs1
  %v304 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v304, ptr %gcs1
  %v305 = load i8, ptr %ctx, align 8
  %c67 = zext i8 %v305 to i64
  %v306 = icmp ne i64 %c67, 0
  br i1 %v306, label %b256, label %b255
b251:
  %v303 = icmp eq i64 %v300, %v6
  br i1 %v303, label %b253, label %b250
b252:
  %gcvolta252 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta252, ptr %ctxtopo, align 8
  ret i64 0
b253:
  %v400 = add i64 0, 286331153
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v401 = and i64 %v0, 1
  %v402 = icmp ne i64 %v401, 0
  br i1 %v402, label %b314, label %b315
b254:
  store i64 0, ptr %gcs1
  %v308 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v308, ptr %gcs1
  %v309 = load i8, ptr %ctx, align 8
  %c68 = zext i8 %v309 to i64
  %v310 = icmp ne i64 %c68, 0
  br i1 %v310, label %b260, label %b259
b255:
  %v307 = icmp eq i64 %v304, %v6
  br i1 %v307, label %b257, label %b254
b256:
  %gcvolta256 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta256, ptr %ctxtopo, align 8
  ret i64 0
b257:
  %v413 = add i64 0, 8
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v414 = and i64 %v0, 1
  %v415 = icmp ne i64 %v414, 0
  br i1 %v415, label %b320, label %b321
b258:
  store i64 0, ptr %gcs1
  %v312 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v312, ptr %gcs1
  %v313 = load i8, ptr %ctx, align 8
  %c69 = zext i8 %v313 to i64
  %v314 = icmp ne i64 %c69, 0
  br i1 %v314, label %b264, label %b263
b259:
  %v311 = icmp eq i64 %v308, %v6
  br i1 %v311, label %b261, label %b258
b260:
  %gcvolta260 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta260, ptr %ctxtopo, align 8
  ret i64 0
b261:
  %v426 = add i64 0, 2184
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v427 = and i64 %v0, 1
  %v428 = icmp ne i64 %v427, 0
  br i1 %v428, label %b326, label %b327
b262:
  store i64 0, ptr %gcs1
  %v316 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v316, ptr %gcs1
  %v317 = load i8, ptr %ctx, align 8
  %c70 = zext i8 %v317 to i64
  %v318 = icmp ne i64 %c70, 0
  br i1 %v318, label %b268, label %b267
b263:
  %v315 = icmp eq i64 %v312, %v6
  br i1 %v315, label %b265, label %b262
b264:
  %gcvolta264 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta264, ptr %ctxtopo, align 8
  ret i64 0
b265:
  %v439 = add i64 0, 143165576
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v440 = and i64 %v0, 1
  %v441 = icmp ne i64 %v440, 0
  br i1 %v441, label %b332, label %b333
b266:
  store i64 0, ptr %gcs1
  %v320 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v320, ptr %gcs1
  %v321 = load i8, ptr %ctx, align 8
  %c71 = zext i8 %v321 to i64
  %v322 = icmp ne i64 %c71, 0
  br i1 %v322, label %b273, label %b272
b267:
  %v319 = icmp eq i64 %v316, %v6
  br i1 %v319, label %b269, label %b266
b268:
  %gcvolta268 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta268, ptr %ctxtopo, align 8
  ret i64 0
b269:
  br label %b229
b271:
  store i64 0, ptr %gcs1
  %v324 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v324, ptr %gcs1
  %v325 = load i8, ptr %ctx, align 8
  %c72 = zext i8 %v325 to i64
  %v326 = icmp ne i64 %c72, 0
  br i1 %v326, label %b278, label %b277
b272:
  %v323 = icmp eq i64 %v320, %v6
  br i1 %v323, label %b274, label %b271
b273:
  %gcvolta273 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta273, ptr %ctxtopo, align 8
  ret i64 0
b274:
  br label %b229
b276:
  br label %b12
b277:
  %v327 = icmp eq i64 %v324, %v6
  br i1 %v327, label %b279, label %b276
b278:
  %gcvolta278 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta278, ptr %ctxtopo, align 8
  ret i64 0
b279:
  br label %b229
b280:
  %v335 = icmp eq i64 %v0, %v330
  br label %b282
b281:
  %sa336_0 = getelementptr [2 x i64], ptr %sargs336, i64 0, i64 0
  store i64 %v330, ptr %sa336_0
  %sat336 = getelementptr [2 x i64], ptr %sargs336, i64 0, i64 1
  store i64 0, ptr %sat336
  %sic336 = getelementptr i64, ptr %area, i64 141716
  %sf336 = call ptr @df.selector(ptr %sic336, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v336 = call i64 %sf336(i64 %v0, ptr %sargs336, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v336, ptr %gcs0
  %v337 = load i8, ptr %ctx, align 8
  %c73 = zext i8 %v337 to i64
  %v338 = icmp ne i64 %c73, 0
  br i1 %v338, label %b284, label %b283
b282:
  %v342 = phi i1 [ %v335, %b280 ], [ %v339, %b285 ]
  br i1 %v342, label %b287, label %b288
b283:
  store i64 0, ptr %gcs1
  %v339 = call i1 @df.unbox_bool(i64 %v336)
  %v340 = load i8, ptr %ctx, align 8
  %c74 = zext i8 %v340 to i64
  %v341 = icmp ne i64 %c74, 0
  br i1 %v341, label %b286, label %b285
b284:
  %gcvolta284 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta284, ptr %ctxtopo, align 8
  ret i64 0
b285:
  br label %b282
b286:
  %gcvolta286 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta286, ptr %ctxtopo, align 8
  ret i64 0
b287:
  %v343 = add i64 0, 0
  br label %b289
b288:
  %v344 = add i64 0, 1
  br label %b289
b289:
  %v345 = phi i64 [ %v343, %b287 ], [ %v344, %b288 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v346 = call i64 @df.box_int(i64 %v345)
  %xcp347 = load i8, ptr %ctx, align 8
  %xcn347 = icmp ne i8 %xcp347, 0
  %xce347 = call i1 @llvm.expect.i1(i1 %xcn347, i1 false)
  br i1 %xce347, label %xc347.limpar, label %xc347.fim
xc347.limpar:
  call void @dartforge_exception_clear()
  br label %xc347.fim
xc347.fim:
  %gcvolta289 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta289, ptr %ctxtopo, align 8
  ret i64 %v346
b291:
  %ga356 = getelementptr i64, ptr %area, i64 10809
  %v356 = load i8, ptr %ga356
  %c75 = zext i8 %v356 to i64
  %v357 = icmp ne i64 %c75, 0
  br i1 %v357, label %b293, label %b294
b292:
  %v364 = call i64 @df.unbox_int(i64 %v0)
  %v365 = load i8, ptr %ctx, align 8
  %c76 = zext i8 %v365 to i64
  %v366 = icmp ne i64 %c76, 0
  br i1 %v366, label %b299, label %b298
b293:
  %ga358 = getelementptr i64, ptr %area, i64 10808
  %v358 = load i64, ptr %ga358
  br label %b295
b294:
  %v359 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b295
b295:
  %v360 = phi i64 [ %v358, %b293 ], [ %v359, %b294 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v360, i64 0, i64 0)
  %v362 = load i8, ptr %ctx, align 8
  %c77 = zext i8 %v362 to i64
  %v363 = icmp ne i64 %c77, 0
  br i1 %v363, label %b297, label %b296
b296:
  br label %b292
b297:
  %gcvolta297 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta297, ptr %ctxtopo, align 8
  ret i64 0
b298:
  %v367 = add i64 0, 1
  %v368 = ashr i64 %v364, %v367
  store i64 0, ptr %gcs0
  %v369 = call i64 @df.box_int(i64 %v368)
  %xcp370 = load i8, ptr %ctx, align 8
  %xcn370 = icmp ne i8 %xcp370, 0
  %xce370 = call i1 @llvm.expect.i1(i1 %xcn370, i1 false)
  br i1 %xce370, label %xc370.limpar, label %xc370.fim
xc370.limpar:
  call void @dartforge_exception_clear()
  br label %xc370.fim
xc370.fim:
  %gcvolta298 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta298, ptr %ctxtopo, align 8
  ret i64 %v369
b299:
  %gcvolta299 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta299, ptr %ctxtopo, align 8
  ret i64 0
b302:
  %v377 = ashr i64 %v0, 1
  %v378 = mul i64 %v377, %v374
  store i64 0, ptr %gcs0
  %v379 = call i64 @df.box_int(i64 %v378)
  br label %b304
b303:
  %v380 = call i64 @df.box_int(i64 %v374)
  store i64 %v380, ptr %gcs1
  %sa381_0 = getelementptr [2 x i64], ptr %sargs381, i64 0, i64 0
  store i64 %v380, ptr %sa381_0
  %sat381 = getelementptr [2 x i64], ptr %sargs381, i64 0, i64 1
  store i64 0, ptr %sat381
  %sic381 = getelementptr i64, ptr %area, i64 141718
  %sf381 = call ptr @df.selector(ptr %sic381, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v381 = call i64 %sf381(i64 %v0, ptr %sargs381, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v382 = load i8, ptr %ctx, align 8
  %c78 = zext i8 %v382 to i64
  %v383 = icmp ne i64 %c78, 0
  br i1 %v383, label %b306, label %b305
b304:
  %v384 = phi i64 [ %v379, %b302 ], [ %v381, %b305 ]
  %xcp385 = load i8, ptr %ctx, align 8
  %xcn385 = icmp ne i8 %xcp385, 0
  %xce385 = call i1 @llvm.expect.i1(i1 %xcn385, i1 false)
  br i1 %xce385, label %xc385.limpar, label %xc385.fim
xc385.limpar:
  call void @dartforge_exception_clear()
  br label %xc385.fim
xc385.fim:
  %gcvolta304 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta304, ptr %ctxtopo, align 8
  ret i64 %v384
b305:
  br label %b304
b306:
  %gcvolta306 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta306, ptr %ctxtopo, align 8
  ret i64 0
b308:
  %v390 = ashr i64 %v0, 1
  %v391 = mul i64 %v390, %v387
  store i64 0, ptr %gcs0
  %v392 = call i64 @df.box_int(i64 %v391)
  br label %b310
b309:
  %v393 = call i64 @df.box_int(i64 %v387)
  store i64 %v393, ptr %gcs1
  %sa394_0 = getelementptr [2 x i64], ptr %sargs394, i64 0, i64 0
  store i64 %v393, ptr %sa394_0
  %sat394 = getelementptr [2 x i64], ptr %sargs394, i64 0, i64 1
  store i64 0, ptr %sat394
  %sic394 = getelementptr i64, ptr %area, i64 141720
  %sf394 = call ptr @df.selector(ptr %sic394, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v394 = call i64 %sf394(i64 %v0, ptr %sargs394, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v395 = load i8, ptr %ctx, align 8
  %c79 = zext i8 %v395 to i64
  %v396 = icmp ne i64 %c79, 0
  br i1 %v396, label %b312, label %b311
b310:
  %v397 = phi i64 [ %v392, %b308 ], [ %v394, %b311 ]
  %xcp398 = load i8, ptr %ctx, align 8
  %xcn398 = icmp ne i8 %xcp398, 0
  %xce398 = call i1 @llvm.expect.i1(i1 %xcn398, i1 false)
  br i1 %xce398, label %xc398.limpar, label %xc398.fim
xc398.limpar:
  call void @dartforge_exception_clear()
  br label %xc398.fim
xc398.fim:
  %gcvolta310 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta310, ptr %ctxtopo, align 8
  ret i64 %v397
b311:
  br label %b310
b312:
  %gcvolta312 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta312, ptr %ctxtopo, align 8
  ret i64 0
b314:
  %v403 = ashr i64 %v0, 1
  %v404 = mul i64 %v403, %v400
  store i64 0, ptr %gcs0
  %v405 = call i64 @df.box_int(i64 %v404)
  br label %b316
b315:
  %v406 = call i64 @df.box_int(i64 %v400)
  store i64 %v406, ptr %gcs1
  %sa407_0 = getelementptr [2 x i64], ptr %sargs407, i64 0, i64 0
  store i64 %v406, ptr %sa407_0
  %sat407 = getelementptr [2 x i64], ptr %sargs407, i64 0, i64 1
  store i64 0, ptr %sat407
  %sic407 = getelementptr i64, ptr %area, i64 141722
  %sf407 = call ptr @df.selector(ptr %sic407, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v407 = call i64 %sf407(i64 %v0, ptr %sargs407, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v408 = load i8, ptr %ctx, align 8
  %c80 = zext i8 %v408 to i64
  %v409 = icmp ne i64 %c80, 0
  br i1 %v409, label %b318, label %b317
b316:
  %v410 = phi i64 [ %v405, %b314 ], [ %v407, %b317 ]
  %xcp411 = load i8, ptr %ctx, align 8
  %xcn411 = icmp ne i8 %xcp411, 0
  %xce411 = call i1 @llvm.expect.i1(i1 %xcn411, i1 false)
  br i1 %xce411, label %xc411.limpar, label %xc411.fim
xc411.limpar:
  call void @dartforge_exception_clear()
  br label %xc411.fim
xc411.fim:
  %gcvolta316 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta316, ptr %ctxtopo, align 8
  ret i64 %v410
b317:
  br label %b316
b318:
  %gcvolta318 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta318, ptr %ctxtopo, align 8
  ret i64 0
b320:
  %v416 = ashr i64 %v0, 1
  %v417 = mul i64 %v416, %v413
  store i64 0, ptr %gcs0
  %v418 = call i64 @df.box_int(i64 %v417)
  br label %b322
b321:
  %v419 = call i64 @df.box_int(i64 %v413)
  store i64 %v419, ptr %gcs1
  %sa420_0 = getelementptr [2 x i64], ptr %sargs420, i64 0, i64 0
  store i64 %v419, ptr %sa420_0
  %sat420 = getelementptr [2 x i64], ptr %sargs420, i64 0, i64 1
  store i64 0, ptr %sat420
  %sic420 = getelementptr i64, ptr %area, i64 141724
  %sf420 = call ptr @df.selector(ptr %sic420, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v420 = call i64 %sf420(i64 %v0, ptr %sargs420, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v421 = load i8, ptr %ctx, align 8
  %c81 = zext i8 %v421 to i64
  %v422 = icmp ne i64 %c81, 0
  br i1 %v422, label %b324, label %b323
b322:
  %v423 = phi i64 [ %v418, %b320 ], [ %v420, %b323 ]
  %xcp424 = load i8, ptr %ctx, align 8
  %xcn424 = icmp ne i8 %xcp424, 0
  %xce424 = call i1 @llvm.expect.i1(i1 %xcn424, i1 false)
  br i1 %xce424, label %xc424.limpar, label %xc424.fim
xc424.limpar:
  call void @dartforge_exception_clear()
  br label %xc424.fim
xc424.fim:
  %gcvolta322 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta322, ptr %ctxtopo, align 8
  ret i64 %v423
b323:
  br label %b322
b324:
  %gcvolta324 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta324, ptr %ctxtopo, align 8
  ret i64 0
b326:
  %v429 = ashr i64 %v0, 1
  %v430 = mul i64 %v429, %v426
  store i64 0, ptr %gcs0
  %v431 = call i64 @df.box_int(i64 %v430)
  br label %b328
b327:
  %v432 = call i64 @df.box_int(i64 %v426)
  store i64 %v432, ptr %gcs1
  %sa433_0 = getelementptr [2 x i64], ptr %sargs433, i64 0, i64 0
  store i64 %v432, ptr %sa433_0
  %sat433 = getelementptr [2 x i64], ptr %sargs433, i64 0, i64 1
  store i64 0, ptr %sat433
  %sic433 = getelementptr i64, ptr %area, i64 141726
  %sf433 = call ptr @df.selector(ptr %sic433, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v433 = call i64 %sf433(i64 %v0, ptr %sargs433, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v434 = load i8, ptr %ctx, align 8
  %c82 = zext i8 %v434 to i64
  %v435 = icmp ne i64 %c82, 0
  br i1 %v435, label %b330, label %b329
b328:
  %v436 = phi i64 [ %v431, %b326 ], [ %v433, %b329 ]
  %xcp437 = load i8, ptr %ctx, align 8
  %xcn437 = icmp ne i8 %xcp437, 0
  %xce437 = call i1 @llvm.expect.i1(i1 %xcn437, i1 false)
  br i1 %xce437, label %xc437.limpar, label %xc437.fim
xc437.limpar:
  call void @dartforge_exception_clear()
  br label %xc437.fim
xc437.fim:
  %gcvolta328 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta328, ptr %ctxtopo, align 8
  ret i64 %v436
b329:
  br label %b328
b330:
  %gcvolta330 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta330, ptr %ctxtopo, align 8
  ret i64 0
b332:
  %v442 = ashr i64 %v0, 1
  %v443 = mul i64 %v442, %v439
  store i64 0, ptr %gcs0
  %v444 = call i64 @df.box_int(i64 %v443)
  br label %b334
b333:
  %v445 = call i64 @df.box_int(i64 %v439)
  store i64 %v445, ptr %gcs1
  %sa446_0 = getelementptr [2 x i64], ptr %sargs446, i64 0, i64 0
  store i64 %v445, ptr %sa446_0
  %sat446 = getelementptr [2 x i64], ptr %sargs446, i64 0, i64 1
  store i64 0, ptr %sat446
  %sic446 = getelementptr i64, ptr %area, i64 141728
  %sf446 = call ptr @df.selector(ptr %sic446, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v446 = call i64 %sf446(i64 %v0, ptr %sargs446, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v447 = load i8, ptr %ctx, align 8
  %c83 = zext i8 %v447 to i64
  %v448 = icmp ne i64 %c83, 0
  br i1 %v448, label %b336, label %b335
b334:
  %v449 = phi i64 [ %v444, %b332 ], [ %v446, %b335 ]
  %xcp450 = load i8, ptr %ctx, align 8
  %xcn450 = icmp ne i8 %xcp450, 0
  %xce450 = call i1 @llvm.expect.i1(i1 %xcn450, i1 false)
  br i1 %xce450, label %xc450.limpar, label %xc450.fim
xc450.limpar:
  call void @dartforge_exception_clear()
  br label %xc450.fim
xc450.fim:
  %gcvolta334 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta334, ptr %ctxtopo, align 8
  ret i64 %v449
b335:
  br label %b334
b336:
  %gcvolta336 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta336, ptr %ctxtopo, align 8
  ret i64 0
b338:
  store i64 0, ptr %gcs1
  %v457 = call double @df.unbox_double(i64 %v454)
  %v458 = load i8, ptr %ctx, align 8
  %c84 = zext i8 %v458 to i64
  %v459 = icmp ne i64 %c84, 0
  br i1 %v459, label %b341, label %b340
b339:
  %gcvolta339 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta339, ptr %ctxtopo, align 8
  ret i64 0
b340:
  %xcp461 = load i8, ptr %ctx, align 8
  %xcn461 = icmp ne i8 %xcp461, 0
  %xce461 = call i1 @llvm.expect.i1(i1 %xcn461, i1 false)
  br i1 %xce461, label %xc461.limpar, label %xc461.fim
xc461.limpar:
  call void @dartforge_exception_clear()
  br label %xc461.fim
xc461.fim:
  %gcvolta340 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta340, ptr %ctxtopo, align 8
  ret i64 %v454
b341:
  %gcvolta341 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta341, ptr %ctxtopo, align 8
  ret i64 0
b355:
  %v655 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v656 = call i64 @df.box_int(i64 %v655)
  store i64 %v656, ptr %gcs1
  %sa657_0 = getelementptr [2 x i64], ptr %sargs657, i64 0, i64 0
  store i64 %v656, ptr %sa657_0
  %sat657 = getelementptr [2 x i64], ptr %sargs657, i64 0, i64 1
  store i64 0, ptr %sat657
  %sic657 = getelementptr i64, ptr %area, i64 141730
  %sf657 = call ptr @df.selector(ptr %sic657, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v657 = call i64 %sf657(i64 %v0, ptr %sargs657, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v657, ptr %gcs0
  %v658 = load i8, ptr %ctx, align 8
  %c85 = zext i8 %v658 to i64
  %v659 = icmp ne i64 %c85, 0
  br i1 %v659, label %b473, label %b472
b356:
  store i64 0, ptr %gcs1
  %v467 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v467, ptr %gcs1
  %v468 = load i8, ptr %ctx, align 8
  %c86 = zext i8 %v468 to i64
  %v469 = icmp ne i64 %c86, 0
  br i1 %v469, label %b362, label %b361
b357:
  %v466 = icmp eq i64 %v463, %v6
  br i1 %v466, label %b359, label %b356
b358:
  %gcvolta358 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta358, ptr %ctxtopo, align 8
  ret i64 0
b359:
  %v512 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v513 = call i64 @df.box_int(i64 %v512)
  store i64 %v513, ptr %gcs1
  %v514 = icmp eq i64 %v0, 0
  %v515 = icmp eq i64 %v513, 0
  %c87 = zext i1 %v514 to i64
  %c88 = zext i1 %v515 to i64
  %v516 = or i64 %c87, %c88
  %v517 = icmp ne i64 %v516, 0
  br i1 %v517, label %b406, label %b407
b360:
  store i64 0, ptr %gcs1
  %v471 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v471, ptr %gcs1
  %v472 = load i8, ptr %ctx, align 8
  %c89 = zext i8 %v472 to i64
  %v473 = icmp ne i64 %c89, 0
  br i1 %v473, label %b366, label %b365
b361:
  %v470 = icmp eq i64 %v467, %v6
  br i1 %v470, label %b363, label %b360
b362:
  %gcvolta362 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta362, ptr %ctxtopo, align 8
  ret i64 0
b363:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v532 = icmp eq i64 %v0, 0
  %v533 = call i64 @df.class(i64 %v0)
  %v534 = call i8 @df.subclass(i64 %v533, i64 674)
  %c90 = zext i8 %v534 to i64
  %v535 = icmp ne i64 %c90, 0
  %v536 = xor i1 %v532, true
  %c91 = zext i1 %v535 to i64
  %c92 = zext i1 %v536 to i64
  %v537 = and i64 %c91, %c92
  %v538 = icmp ne i64 %v537, 0
  br i1 %v538, label %b418, label %b417
b364:
  store i64 0, ptr %gcs1
  %v475 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v475, ptr %gcs1
  %v476 = load i8, ptr %ctx, align 8
  %c93 = zext i8 %v476 to i64
  %v477 = icmp ne i64 %c93, 0
  br i1 %v477, label %b370, label %b369
b365:
  %v474 = icmp eq i64 %v471, %v6
  br i1 %v474, label %b367, label %b364
b366:
  %gcvolta366 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta366, ptr %ctxtopo, align 8
  ret i64 0
b367:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v555 = icmp eq i64 %v0, 0
  %v556 = call i64 @df.class(i64 %v0)
  %v557 = call i8 @df.subclass(i64 %v556, i64 674)
  %c94 = zext i8 %v557 to i64
  %v558 = icmp ne i64 %c94, 0
  %v559 = xor i1 %v555, true
  %c95 = zext i1 %v558 to i64
  %c96 = zext i1 %v559 to i64
  %v560 = and i64 %c95, %c96
  %v561 = icmp ne i64 %v560, 0
  br i1 %v561, label %b428, label %b427
b368:
  store i64 0, ptr %gcs1
  %v479 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v479, ptr %gcs1
  %v480 = load i8, ptr %ctx, align 8
  %c97 = zext i8 %v480 to i64
  %v481 = icmp ne i64 %c97, 0
  br i1 %v481, label %b374, label %b373
b369:
  %v478 = icmp eq i64 %v475, %v6
  br i1 %v478, label %b371, label %b368
b370:
  %gcvolta370 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta370, ptr %ctxtopo, align 8
  ret i64 0
b371:
  %xcp578 = load i8, ptr %ctx, align 8
  %xcn578 = icmp ne i8 %xcp578, 0
  %xce578 = call i1 @llvm.expect.i1(i1 %xcn578, i1 false)
  br i1 %xce578, label %xc578.limpar, label %xc578.fim
xc578.limpar:
  call void @dartforge_exception_clear()
  br label %xc578.fim
xc578.fim:
  %gcvolta371 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta371, ptr %ctxtopo, align 8
  ret i64 %v0
b372:
  store i64 0, ptr %gcs1
  %v483 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v483, ptr %gcs1
  %v484 = load i8, ptr %ctx, align 8
  %c98 = zext i8 %v484 to i64
  %v485 = icmp ne i64 %c98, 0
  br i1 %v485, label %b378, label %b377
b373:
  %v482 = icmp eq i64 %v479, %v6
  br i1 %v482, label %b375, label %b372
b374:
  %gcvolta374 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta374, ptr %ctxtopo, align 8
  ret i64 0
b375:
  %v580 = add i64 0, 257
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v581 = and i64 %v0, 1
  %v582 = icmp ne i64 %v581, 0
  br i1 %v582, label %b438, label %b439
b376:
  store i64 0, ptr %gcs1
  %v487 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v487, ptr %gcs1
  %v488 = load i8, ptr %ctx, align 8
  %c99 = zext i8 %v488 to i64
  %v489 = icmp ne i64 %c99, 0
  br i1 %v489, label %b382, label %b381
b377:
  %v486 = icmp eq i64 %v483, %v6
  br i1 %v486, label %b379, label %b376
b378:
  %gcvolta378 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta378, ptr %ctxtopo, align 8
  ret i64 0
b379:
  %v593 = add i64 0, 16843009
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v594 = and i64 %v0, 1
  %v595 = icmp ne i64 %v594, 0
  br i1 %v595, label %b444, label %b445
b380:
  store i64 0, ptr %gcs1
  %v491 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v491, ptr %gcs1
  %v492 = load i8, ptr %ctx, align 8
  %c100 = zext i8 %v492 to i64
  %v493 = icmp ne i64 %c100, 0
  br i1 %v493, label %b386, label %b385
b381:
  %v490 = icmp eq i64 %v487, %v6
  br i1 %v490, label %b383, label %b380
b382:
  %gcvolta382 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta382, ptr %ctxtopo, align 8
  ret i64 0
b383:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v606 = icmp eq i64 %v0, 0
  %v607 = call i64 @df.class(i64 %v0)
  %v608 = call i8 @df.subclass(i64 %v607, i64 674)
  %c101 = zext i8 %v608 to i64
  %v609 = icmp ne i64 %c101, 0
  %v610 = xor i1 %v606, true
  %c102 = zext i1 %v609 to i64
  %c103 = zext i1 %v610 to i64
  %v611 = and i64 %c102, %c103
  %v612 = icmp ne i64 %v611, 0
  br i1 %v612, label %b451, label %b450
b384:
  store i64 0, ptr %gcs1
  %v495 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v495, ptr %gcs1
  %v496 = load i8, ptr %ctx, align 8
  %c104 = zext i8 %v496 to i64
  %v497 = icmp ne i64 %c104, 0
  br i1 %v497, label %b390, label %b389
b385:
  %v494 = icmp eq i64 %v491, %v6
  br i1 %v494, label %b387, label %b384
b386:
  %gcvolta386 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta386, ptr %ctxtopo, align 8
  ret i64 0
b387:
  %v629 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v630 = and i64 %v0, 1
  %v631 = icmp ne i64 %v630, 0
  br i1 %v631, label %b460, label %b461
b388:
  store i64 0, ptr %gcs1
  %v499 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v499, ptr %gcs1
  %v500 = load i8, ptr %ctx, align 8
  %c105 = zext i8 %v500 to i64
  %v501 = icmp ne i64 %c105, 0
  br i1 %v501, label %b394, label %b393
b389:
  %v498 = icmp eq i64 %v495, %v6
  br i1 %v498, label %b391, label %b388
b390:
  %gcvolta390 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta390, ptr %ctxtopo, align 8
  ret i64 0
b391:
  %v642 = add i64 0, 8421504
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v643 = and i64 %v0, 1
  %v644 = icmp ne i64 %v643, 0
  br i1 %v644, label %b466, label %b467
b392:
  store i64 0, ptr %gcs1
  %v503 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v503, ptr %gcs1
  %v504 = load i8, ptr %ctx, align 8
  %c106 = zext i8 %v504 to i64
  %v505 = icmp ne i64 %c106, 0
  br i1 %v505, label %b399, label %b398
b393:
  %v502 = icmp eq i64 %v499, %v6
  br i1 %v502, label %b395, label %b392
b394:
  %gcvolta394 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta394, ptr %ctxtopo, align 8
  ret i64 0
b395:
  br label %b355
b397:
  store i64 0, ptr %gcs1
  %v507 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v507, ptr %gcs1
  %v508 = load i8, ptr %ctx, align 8
  %c107 = zext i8 %v508 to i64
  %v509 = icmp ne i64 %c107, 0
  br i1 %v509, label %b404, label %b403
b398:
  %v506 = icmp eq i64 %v503, %v6
  br i1 %v506, label %b400, label %b397
b399:
  %gcvolta399 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta399, ptr %ctxtopo, align 8
  ret i64 0
b400:
  br label %b355
b402:
  br label %b12
b403:
  %v510 = icmp eq i64 %v507, %v6
  br i1 %v510, label %b405, label %b402
b404:
  %gcvolta404 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta404, ptr %ctxtopo, align 8
  ret i64 0
b405:
  br label %b355
b406:
  %v518 = icmp eq i64 %v0, %v513
  br label %b408
b407:
  %sa519_0 = getelementptr [2 x i64], ptr %sargs519, i64 0, i64 0
  store i64 %v513, ptr %sa519_0
  %sat519 = getelementptr [2 x i64], ptr %sargs519, i64 0, i64 1
  store i64 0, ptr %sat519
  %sic519 = getelementptr i64, ptr %area, i64 141732
  %sf519 = call ptr @df.selector(ptr %sic519, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v519 = call i64 %sf519(i64 %v0, ptr %sargs519, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v519, ptr %gcs0
  %v520 = load i8, ptr %ctx, align 8
  %c108 = zext i8 %v520 to i64
  %v521 = icmp ne i64 %c108, 0
  br i1 %v521, label %b410, label %b409
b408:
  %v525 = phi i1 [ %v518, %b406 ], [ %v522, %b411 ]
  br i1 %v525, label %b413, label %b414
b409:
  store i64 0, ptr %gcs1
  %v522 = call i1 @df.unbox_bool(i64 %v519)
  %v523 = load i8, ptr %ctx, align 8
  %c109 = zext i8 %v523 to i64
  %v524 = icmp ne i64 %c109, 0
  br i1 %v524, label %b412, label %b411
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
b413:
  %v526 = add i64 0, 0
  br label %b415
b414:
  %v527 = add i64 0, 1
  br label %b415
b415:
  %v528 = phi i64 [ %v526, %b413 ], [ %v527, %b414 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v529 = call i64 @df.box_int(i64 %v528)
  %xcp530 = load i8, ptr %ctx, align 8
  %xcn530 = icmp ne i8 %xcp530, 0
  %xce530 = call i1 @llvm.expect.i1(i1 %xcn530, i1 false)
  br i1 %xce530, label %xc530.limpar, label %xc530.fim
xc530.limpar:
  call void @dartforge_exception_clear()
  br label %xc530.fim
xc530.fim:
  %gcvolta415 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta415, ptr %ctxtopo, align 8
  ret i64 %v529
b417:
  %ga539 = getelementptr i64, ptr %area, i64 10809
  %v539 = load i8, ptr %ga539
  %c110 = zext i8 %v539 to i64
  %v540 = icmp ne i64 %c110, 0
  br i1 %v540, label %b419, label %b420
b418:
  %v547 = call i64 @df.unbox_int(i64 %v0)
  %v548 = load i8, ptr %ctx, align 8
  %c111 = zext i8 %v548 to i64
  %v549 = icmp ne i64 %c111, 0
  br i1 %v549, label %b425, label %b424
b419:
  %ga541 = getelementptr i64, ptr %area, i64 10808
  %v541 = load i64, ptr %ga541
  br label %b421
b420:
  %v542 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b421
b421:
  %v543 = phi i64 [ %v541, %b419 ], [ %v542, %b420 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v543, i64 0, i64 0)
  %v545 = load i8, ptr %ctx, align 8
  %c112 = zext i8 %v545 to i64
  %v546 = icmp ne i64 %c112, 0
  br i1 %v546, label %b423, label %b422
b422:
  br label %b418
b423:
  %gcvolta423 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta423, ptr %ctxtopo, align 8
  ret i64 0
b424:
  %v550 = add i64 0, 6
  %v551 = ashr i64 %v547, %v550
  store i64 0, ptr %gcs0
  %v552 = call i64 @df.box_int(i64 %v551)
  %xcp553 = load i8, ptr %ctx, align 8
  %xcn553 = icmp ne i8 %xcp553, 0
  %xce553 = call i1 @llvm.expect.i1(i1 %xcn553, i1 false)
  br i1 %xce553, label %xc553.limpar, label %xc553.fim
xc553.limpar:
  call void @dartforge_exception_clear()
  br label %xc553.fim
xc553.fim:
  %gcvolta424 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta424, ptr %ctxtopo, align 8
  ret i64 %v552
b425:
  %gcvolta425 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta425, ptr %ctxtopo, align 8
  ret i64 0
b427:
  %ga562 = getelementptr i64, ptr %area, i64 10809
  %v562 = load i8, ptr %ga562
  %c113 = zext i8 %v562 to i64
  %v563 = icmp ne i64 %c113, 0
  br i1 %v563, label %b429, label %b430
b428:
  %v570 = call i64 @df.unbox_int(i64 %v0)
  %v571 = load i8, ptr %ctx, align 8
  %c114 = zext i8 %v571 to i64
  %v572 = icmp ne i64 %c114, 0
  br i1 %v572, label %b435, label %b434
b429:
  %ga564 = getelementptr i64, ptr %area, i64 10808
  %v564 = load i64, ptr %ga564
  br label %b431
b430:
  %v565 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b431
b431:
  %v566 = phi i64 [ %v564, %b429 ], [ %v565, %b430 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v566, i64 0, i64 0)
  %v568 = load i8, ptr %ctx, align 8
  %c115 = zext i8 %v568 to i64
  %v569 = icmp ne i64 %c115, 0
  br i1 %v569, label %b433, label %b432
b432:
  br label %b428
b433:
  %gcvolta433 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta433, ptr %ctxtopo, align 8
  ret i64 0
b434:
  %v573 = add i64 0, 4
  %v574 = ashr i64 %v570, %v573
  store i64 0, ptr %gcs0
  %v575 = call i64 @df.box_int(i64 %v574)
  %xcp576 = load i8, ptr %ctx, align 8
  %xcn576 = icmp ne i8 %xcp576, 0
  %xce576 = call i1 @llvm.expect.i1(i1 %xcn576, i1 false)
  br i1 %xce576, label %xc576.limpar, label %xc576.fim
xc576.limpar:
  call void @dartforge_exception_clear()
  br label %xc576.fim
xc576.fim:
  %gcvolta434 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta434, ptr %ctxtopo, align 8
  ret i64 %v575
b435:
  %gcvolta435 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta435, ptr %ctxtopo, align 8
  ret i64 0
b438:
  %v583 = ashr i64 %v0, 1
  %v584 = mul i64 %v583, %v580
  store i64 0, ptr %gcs0
  %v585 = call i64 @df.box_int(i64 %v584)
  br label %b440
b439:
  %v586 = call i64 @df.box_int(i64 %v580)
  store i64 %v586, ptr %gcs1
  %sa587_0 = getelementptr [2 x i64], ptr %sargs587, i64 0, i64 0
  store i64 %v586, ptr %sa587_0
  %sat587 = getelementptr [2 x i64], ptr %sargs587, i64 0, i64 1
  store i64 0, ptr %sat587
  %sic587 = getelementptr i64, ptr %area, i64 141734
  %sf587 = call ptr @df.selector(ptr %sic587, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v587 = call i64 %sf587(i64 %v0, ptr %sargs587, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v588 = load i8, ptr %ctx, align 8
  %c116 = zext i8 %v588 to i64
  %v589 = icmp ne i64 %c116, 0
  br i1 %v589, label %b442, label %b441
b440:
  %v590 = phi i64 [ %v585, %b438 ], [ %v587, %b441 ]
  %xcp591 = load i8, ptr %ctx, align 8
  %xcn591 = icmp ne i8 %xcp591, 0
  %xce591 = call i1 @llvm.expect.i1(i1 %xcn591, i1 false)
  br i1 %xce591, label %xc591.limpar, label %xc591.fim
xc591.limpar:
  call void @dartforge_exception_clear()
  br label %xc591.fim
xc591.fim:
  %gcvolta440 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta440, ptr %ctxtopo, align 8
  ret i64 %v590
b441:
  br label %b440
b442:
  %gcvolta442 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta442, ptr %ctxtopo, align 8
  ret i64 0
b444:
  %v596 = ashr i64 %v0, 1
  %v597 = mul i64 %v596, %v593
  store i64 0, ptr %gcs0
  %v598 = call i64 @df.box_int(i64 %v597)
  br label %b446
b445:
  %v599 = call i64 @df.box_int(i64 %v593)
  store i64 %v599, ptr %gcs1
  %sa600_0 = getelementptr [2 x i64], ptr %sargs600, i64 0, i64 0
  store i64 %v599, ptr %sa600_0
  %sat600 = getelementptr [2 x i64], ptr %sargs600, i64 0, i64 1
  store i64 0, ptr %sat600
  %sic600 = getelementptr i64, ptr %area, i64 141736
  %sf600 = call ptr @df.selector(ptr %sic600, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v600 = call i64 %sf600(i64 %v0, ptr %sargs600, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v601 = load i8, ptr %ctx, align 8
  %c117 = zext i8 %v601 to i64
  %v602 = icmp ne i64 %c117, 0
  br i1 %v602, label %b448, label %b447
b446:
  %v603 = phi i64 [ %v598, %b444 ], [ %v600, %b447 ]
  %xcp604 = load i8, ptr %ctx, align 8
  %xcn604 = icmp ne i8 %xcp604, 0
  %xce604 = call i1 @llvm.expect.i1(i1 %xcn604, i1 false)
  br i1 %xce604, label %xc604.limpar, label %xc604.fim
xc604.limpar:
  call void @dartforge_exception_clear()
  br label %xc604.fim
xc604.fim:
  %gcvolta446 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta446, ptr %ctxtopo, align 8
  ret i64 %v603
b447:
  br label %b446
b448:
  %gcvolta448 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta448, ptr %ctxtopo, align 8
  ret i64 0
b450:
  %ga613 = getelementptr i64, ptr %area, i64 10809
  %v613 = load i8, ptr %ga613
  %c118 = zext i8 %v613 to i64
  %v614 = icmp ne i64 %c118, 0
  br i1 %v614, label %b452, label %b453
b451:
  %v621 = call i64 @df.unbox_int(i64 %v0)
  %v622 = load i8, ptr %ctx, align 8
  %c119 = zext i8 %v622 to i64
  %v623 = icmp ne i64 %c119, 0
  br i1 %v623, label %b458, label %b457
b452:
  %ga615 = getelementptr i64, ptr %area, i64 10808
  %v615 = load i64, ptr %ga615
  br label %b454
b453:
  %v616 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b454
b454:
  %v617 = phi i64 [ %v615, %b452 ], [ %v616, %b453 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v617, i64 0, i64 0)
  %v619 = load i8, ptr %ctx, align 8
  %c120 = zext i8 %v619 to i64
  %v620 = icmp ne i64 %c120, 0
  br i1 %v620, label %b456, label %b455
b455:
  br label %b451
b456:
  %gcvolta456 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta456, ptr %ctxtopo, align 8
  ret i64 0
b457:
  %v624 = add i64 0, 1
  %v625 = ashr i64 %v621, %v624
  store i64 0, ptr %gcs0
  %v626 = call i64 @df.box_int(i64 %v625)
  %xcp627 = load i8, ptr %ctx, align 8
  %xcn627 = icmp ne i8 %xcp627, 0
  %xce627 = call i1 @llvm.expect.i1(i1 %xcn627, i1 false)
  br i1 %xce627, label %xc627.limpar, label %xc627.fim
xc627.limpar:
  call void @dartforge_exception_clear()
  br label %xc627.fim
xc627.fim:
  %gcvolta457 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta457, ptr %ctxtopo, align 8
  ret i64 %v626
b458:
  %gcvolta458 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta458, ptr %ctxtopo, align 8
  ret i64 0
b460:
  %v632 = ashr i64 %v0, 1
  %v633 = mul i64 %v632, %v629
  store i64 0, ptr %gcs0
  %v634 = call i64 @df.box_int(i64 %v633)
  br label %b462
b461:
  %v635 = call i64 @df.box_int(i64 %v629)
  store i64 %v635, ptr %gcs1
  %sa636_0 = getelementptr [2 x i64], ptr %sargs636, i64 0, i64 0
  store i64 %v635, ptr %sa636_0
  %sat636 = getelementptr [2 x i64], ptr %sargs636, i64 0, i64 1
  store i64 0, ptr %sat636
  %sic636 = getelementptr i64, ptr %area, i64 141738
  %sf636 = call ptr @df.selector(ptr %sic636, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v636 = call i64 %sf636(i64 %v0, ptr %sargs636, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v637 = load i8, ptr %ctx, align 8
  %c121 = zext i8 %v637 to i64
  %v638 = icmp ne i64 %c121, 0
  br i1 %v638, label %b464, label %b463
b462:
  %v639 = phi i64 [ %v634, %b460 ], [ %v636, %b463 ]
  %xcp640 = load i8, ptr %ctx, align 8
  %xcn640 = icmp ne i8 %xcp640, 0
  %xce640 = call i1 @llvm.expect.i1(i1 %xcn640, i1 false)
  br i1 %xce640, label %xc640.limpar, label %xc640.fim
xc640.limpar:
  call void @dartforge_exception_clear()
  br label %xc640.fim
xc640.fim:
  %gcvolta462 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta462, ptr %ctxtopo, align 8
  ret i64 %v639
b463:
  br label %b462
b464:
  %gcvolta464 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta464, ptr %ctxtopo, align 8
  ret i64 0
b466:
  %v645 = ashr i64 %v0, 1
  %v646 = mul i64 %v645, %v642
  store i64 0, ptr %gcs0
  %v647 = call i64 @df.box_int(i64 %v646)
  br label %b468
b467:
  %v648 = call i64 @df.box_int(i64 %v642)
  store i64 %v648, ptr %gcs1
  %sa649_0 = getelementptr [2 x i64], ptr %sargs649, i64 0, i64 0
  store i64 %v648, ptr %sa649_0
  %sat649 = getelementptr [2 x i64], ptr %sargs649, i64 0, i64 1
  store i64 0, ptr %sat649
  %sic649 = getelementptr i64, ptr %area, i64 141740
  %sf649 = call ptr @df.selector(ptr %sic649, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v649 = call i64 %sf649(i64 %v0, ptr %sargs649, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v650 = load i8, ptr %ctx, align 8
  %c122 = zext i8 %v650 to i64
  %v651 = icmp ne i64 %c122, 0
  br i1 %v651, label %b470, label %b469
b468:
  %v652 = phi i64 [ %v647, %b466 ], [ %v649, %b469 ]
  %xcp653 = load i8, ptr %ctx, align 8
  %xcn653 = icmp ne i8 %xcp653, 0
  %xce653 = call i1 @llvm.expect.i1(i1 %xcn653, i1 false)
  br i1 %xce653, label %xc653.limpar, label %xc653.fim
xc653.limpar:
  call void @dartforge_exception_clear()
  br label %xc653.fim
xc653.fim:
  %gcvolta468 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta468, ptr %ctxtopo, align 8
  ret i64 %v652
b469:
  br label %b468
b470:
  %gcvolta470 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta470, ptr %ctxtopo, align 8
  ret i64 0
b472:
  store i64 0, ptr %gcs1
  %v660 = call double @df.unbox_double(i64 %v657)
  %v661 = load i8, ptr %ctx, align 8
  %c123 = zext i8 %v661 to i64
  %v662 = icmp ne i64 %c123, 0
  br i1 %v662, label %b475, label %b474
b473:
  %gcvolta473 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta473, ptr %ctxtopo, align 8
  ret i64 0
b474:
  %xcp664 = load i8, ptr %ctx, align 8
  %xcn664 = icmp ne i8 %xcp664, 0
  %xce664 = call i1 @llvm.expect.i1(i1 %xcn664, i1 false)
  br i1 %xce664, label %xc664.limpar, label %xc664.fim
xc664.limpar:
  call void @dartforge_exception_clear()
  br label %xc664.fim
xc664.fim:
  %gcvolta474 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta474, ptr %ctxtopo, align 8
  ret i64 %v657
b475:
  %gcvolta475 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta475, ptr %ctxtopo, align 8
  ret i64 0
b489:
  %v888 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v889 = call i64 @df.box_int(i64 %v888)
  store i64 %v889, ptr %gcs1
  %sa890_0 = getelementptr [2 x i64], ptr %sargs890, i64 0, i64 0
  store i64 %v889, ptr %sa890_0
  %sat890 = getelementptr [2 x i64], ptr %sargs890, i64 0, i64 1
  store i64 0, ptr %sat890
  %sic890 = getelementptr i64, ptr %area, i64 141742
  %sf890 = call ptr @df.selector(ptr %sic890, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v890 = call i64 %sf890(i64 %v0, ptr %sargs890, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v890, ptr %gcs0
  %v891 = load i8, ptr %ctx, align 8
  %c124 = zext i8 %v891 to i64
  %v892 = icmp ne i64 %c124, 0
  br i1 %v892, label %b619, label %b618
b490:
  store i64 0, ptr %gcs1
  %v670 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v670, ptr %gcs1
  %v671 = load i8, ptr %ctx, align 8
  %c125 = zext i8 %v671 to i64
  %v672 = icmp ne i64 %c125, 0
  br i1 %v672, label %b496, label %b495
b491:
  %v669 = icmp eq i64 %v666, %v6
  br i1 %v669, label %b493, label %b490
b492:
  %gcvolta492 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta492, ptr %ctxtopo, align 8
  ret i64 0
b493:
  %v715 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v716 = call i64 @df.box_int(i64 %v715)
  store i64 %v716, ptr %gcs1
  %v717 = icmp eq i64 %v0, 0
  %v718 = icmp eq i64 %v716, 0
  %c126 = zext i1 %v717 to i64
  %c127 = zext i1 %v718 to i64
  %v719 = or i64 %c126, %c127
  %v720 = icmp ne i64 %v719, 0
  br i1 %v720, label %b540, label %b541
b494:
  store i64 0, ptr %gcs1
  %v674 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v674, ptr %gcs1
  %v675 = load i8, ptr %ctx, align 8
  %c128 = zext i8 %v675 to i64
  %v676 = icmp ne i64 %c128, 0
  br i1 %v676, label %b500, label %b499
b495:
  %v673 = icmp eq i64 %v670, %v6
  br i1 %v673, label %b497, label %b494
b496:
  %gcvolta496 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta496, ptr %ctxtopo, align 8
  ret i64 0
b497:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v735 = icmp eq i64 %v0, 0
  %v736 = call i64 @df.class(i64 %v0)
  %v737 = call i8 @df.subclass(i64 %v736, i64 674)
  %c129 = zext i8 %v737 to i64
  %v738 = icmp ne i64 %c129, 0
  %v739 = xor i1 %v735, true
  %c130 = zext i1 %v738 to i64
  %c131 = zext i1 %v739 to i64
  %v740 = and i64 %c130, %c131
  %v741 = icmp ne i64 %v740, 0
  br i1 %v741, label %b552, label %b551
b498:
  store i64 0, ptr %gcs1
  %v678 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v678, ptr %gcs1
  %v679 = load i8, ptr %ctx, align 8
  %c132 = zext i8 %v679 to i64
  %v680 = icmp ne i64 %c132, 0
  br i1 %v680, label %b504, label %b503
b499:
  %v677 = icmp eq i64 %v674, %v6
  br i1 %v677, label %b501, label %b498
b500:
  %gcvolta500 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta500, ptr %ctxtopo, align 8
  ret i64 0
b501:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v758 = icmp eq i64 %v0, 0
  %v759 = call i64 @df.class(i64 %v0)
  %v760 = call i8 @df.subclass(i64 %v759, i64 674)
  %c133 = zext i8 %v760 to i64
  %v761 = icmp ne i64 %c133, 0
  %v762 = xor i1 %v758, true
  %c134 = zext i1 %v761 to i64
  %c135 = zext i1 %v762 to i64
  %v763 = and i64 %c134, %c135
  %v764 = icmp ne i64 %v763, 0
  br i1 %v764, label %b562, label %b561
b502:
  store i64 0, ptr %gcs1
  %v682 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v682, ptr %gcs1
  %v683 = load i8, ptr %ctx, align 8
  %c136 = zext i8 %v683 to i64
  %v684 = icmp ne i64 %c136, 0
  br i1 %v684, label %b508, label %b507
b503:
  %v681 = icmp eq i64 %v678, %v6
  br i1 %v681, label %b505, label %b502
b504:
  %gcvolta504 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta504, ptr %ctxtopo, align 8
  ret i64 0
b505:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v781 = icmp eq i64 %v0, 0
  %v782 = call i64 @df.class(i64 %v0)
  %v783 = call i8 @df.subclass(i64 %v782, i64 674)
  %c137 = zext i8 %v783 to i64
  %v784 = icmp ne i64 %c137, 0
  %v785 = xor i1 %v781, true
  %c138 = zext i1 %v784 to i64
  %c139 = zext i1 %v785 to i64
  %v786 = and i64 %c138, %c139
  %v787 = icmp ne i64 %v786, 0
  br i1 %v787, label %b572, label %b571
b506:
  store i64 0, ptr %gcs1
  %v686 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v686, ptr %gcs1
  %v687 = load i8, ptr %ctx, align 8
  %c140 = zext i8 %v687 to i64
  %v688 = icmp ne i64 %c140, 0
  br i1 %v688, label %b512, label %b511
b507:
  %v685 = icmp eq i64 %v682, %v6
  br i1 %v685, label %b509, label %b506
b508:
  %gcvolta508 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta508, ptr %ctxtopo, align 8
  ret i64 0
b509:
  %xcp804 = load i8, ptr %ctx, align 8
  %xcn804 = icmp ne i8 %xcp804, 0
  %xce804 = call i1 @llvm.expect.i1(i1 %xcn804, i1 false)
  br i1 %xce804, label %xc804.limpar, label %xc804.fim
xc804.limpar:
  call void @dartforge_exception_clear()
  br label %xc804.fim
xc804.fim:
  %gcvolta509 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta509, ptr %ctxtopo, align 8
  ret i64 %v0
b510:
  store i64 0, ptr %gcs1
  %v690 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v690, ptr %gcs1
  %v691 = load i8, ptr %ctx, align 8
  %c141 = zext i8 %v691 to i64
  %v692 = icmp ne i64 %c141, 0
  br i1 %v692, label %b516, label %b515
b511:
  %v689 = icmp eq i64 %v686, %v6
  br i1 %v689, label %b513, label %b510
b512:
  %gcvolta512 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta512, ptr %ctxtopo, align 8
  ret i64 0
b513:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v806 = icmp eq i64 %v0, 0
  %v807 = call i64 @df.class(i64 %v0)
  %v808 = call i8 @df.subclass(i64 %v807, i64 674)
  %c142 = zext i8 %v808 to i64
  %v809 = icmp ne i64 %c142, 0
  %v810 = xor i1 %v806, true
  %c143 = zext i1 %v809 to i64
  %c144 = zext i1 %v810 to i64
  %v811 = and i64 %c143, %c144
  %v812 = icmp ne i64 %v811, 0
  br i1 %v812, label %b583, label %b582
b514:
  store i64 0, ptr %gcs1
  %v694 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v694, ptr %gcs1
  %v695 = load i8, ptr %ctx, align 8
  %c145 = zext i8 %v695 to i64
  %v696 = icmp ne i64 %c145, 0
  br i1 %v696, label %b520, label %b519
b515:
  %v693 = icmp eq i64 %v690, %v6
  br i1 %v693, label %b517, label %b514
b516:
  %gcvolta516 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta516, ptr %ctxtopo, align 8
  ret i64 0
b517:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v829 = icmp eq i64 %v0, 0
  %v830 = call i64 @df.class(i64 %v0)
  %v831 = call i8 @df.subclass(i64 %v830, i64 674)
  %c146 = zext i8 %v831 to i64
  %v832 = icmp ne i64 %c146, 0
  %v833 = xor i1 %v829, true
  %c147 = zext i1 %v832 to i64
  %c148 = zext i1 %v833 to i64
  %v834 = and i64 %c147, %c148
  %v835 = icmp ne i64 %v834, 0
  br i1 %v835, label %b593, label %b592
b518:
  store i64 0, ptr %gcs1
  %v698 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v698, ptr %gcs1
  %v699 = load i8, ptr %ctx, align 8
  %c149 = zext i8 %v699 to i64
  %v700 = icmp ne i64 %c149, 0
  br i1 %v700, label %b524, label %b523
b519:
  %v697 = icmp eq i64 %v694, %v6
  br i1 %v697, label %b521, label %b518
b520:
  %gcvolta520 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta520, ptr %ctxtopo, align 8
  ret i64 0
b521:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v852 = icmp eq i64 %v0, 0
  %v853 = call i64 @df.class(i64 %v0)
  %v854 = call i8 @df.subclass(i64 %v853, i64 674)
  %c150 = zext i8 %v854 to i64
  %v855 = icmp ne i64 %c150, 0
  %v856 = xor i1 %v852, true
  %c151 = zext i1 %v855 to i64
  %c152 = zext i1 %v856 to i64
  %v857 = and i64 %c151, %c152
  %v858 = icmp ne i64 %v857, 0
  br i1 %v858, label %b603, label %b602
b522:
  store i64 0, ptr %gcs1
  %v702 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v702, ptr %gcs1
  %v703 = load i8, ptr %ctx, align 8
  %c153 = zext i8 %v703 to i64
  %v704 = icmp ne i64 %c153, 0
  br i1 %v704, label %b528, label %b527
b523:
  %v701 = icmp eq i64 %v698, %v6
  br i1 %v701, label %b525, label %b522
b524:
  %gcvolta524 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta524, ptr %ctxtopo, align 8
  ret i64 0
b525:
  %v875 = add i64 0, 524296
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v876 = and i64 %v0, 1
  %v877 = icmp ne i64 %v876, 0
  br i1 %v877, label %b612, label %b613
b526:
  store i64 0, ptr %gcs1
  %v706 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v706, ptr %gcs1
  %v707 = load i8, ptr %ctx, align 8
  %c154 = zext i8 %v707 to i64
  %v708 = icmp ne i64 %c154, 0
  br i1 %v708, label %b533, label %b532
b527:
  %v705 = icmp eq i64 %v702, %v6
  br i1 %v705, label %b529, label %b526
b528:
  %gcvolta528 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta528, ptr %ctxtopo, align 8
  ret i64 0
b529:
  br label %b489
b531:
  store i64 0, ptr %gcs1
  %v710 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v710, ptr %gcs1
  %v711 = load i8, ptr %ctx, align 8
  %c155 = zext i8 %v711 to i64
  %v712 = icmp ne i64 %c155, 0
  br i1 %v712, label %b538, label %b537
b532:
  %v709 = icmp eq i64 %v706, %v6
  br i1 %v709, label %b534, label %b531
b533:
  %gcvolta533 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta533, ptr %ctxtopo, align 8
  ret i64 0
b534:
  br label %b489
b536:
  br label %b12
b537:
  %v713 = icmp eq i64 %v710, %v6
  br i1 %v713, label %b539, label %b536
b538:
  %gcvolta538 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta538, ptr %ctxtopo, align 8
  ret i64 0
b539:
  br label %b489
b540:
  %v721 = icmp eq i64 %v0, %v716
  br label %b542
b541:
  %sa722_0 = getelementptr [2 x i64], ptr %sargs722, i64 0, i64 0
  store i64 %v716, ptr %sa722_0
  %sat722 = getelementptr [2 x i64], ptr %sargs722, i64 0, i64 1
  store i64 0, ptr %sat722
  %sic722 = getelementptr i64, ptr %area, i64 141744
  %sf722 = call ptr @df.selector(ptr %sic722, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v722 = call i64 %sf722(i64 %v0, ptr %sargs722, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v722, ptr %gcs0
  %v723 = load i8, ptr %ctx, align 8
  %c156 = zext i8 %v723 to i64
  %v724 = icmp ne i64 %c156, 0
  br i1 %v724, label %b544, label %b543
b542:
  %v728 = phi i1 [ %v721, %b540 ], [ %v725, %b545 ]
  br i1 %v728, label %b547, label %b548
b543:
  store i64 0, ptr %gcs1
  %v725 = call i1 @df.unbox_bool(i64 %v722)
  %v726 = load i8, ptr %ctx, align 8
  %c157 = zext i8 %v726 to i64
  %v727 = icmp ne i64 %c157, 0
  br i1 %v727, label %b546, label %b545
b544:
  %gcvolta544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta544, ptr %ctxtopo, align 8
  ret i64 0
b545:
  br label %b542
b546:
  %gcvolta546 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta546, ptr %ctxtopo, align 8
  ret i64 0
b547:
  %v729 = add i64 0, 0
  br label %b549
b548:
  %v730 = add i64 0, 1
  br label %b549
b549:
  %v731 = phi i64 [ %v729, %b547 ], [ %v730, %b548 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v732 = call i64 @df.box_int(i64 %v731)
  %xcp733 = load i8, ptr %ctx, align 8
  %xcn733 = icmp ne i8 %xcp733, 0
  %xce733 = call i1 @llvm.expect.i1(i1 %xcn733, i1 false)
  br i1 %xce733, label %xc733.limpar, label %xc733.fim
xc733.limpar:
  call void @dartforge_exception_clear()
  br label %xc733.fim
xc733.fim:
  %gcvolta549 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta549, ptr %ctxtopo, align 8
  ret i64 %v732
b551:
  %ga742 = getelementptr i64, ptr %area, i64 10809
  %v742 = load i8, ptr %ga742
  %c158 = zext i8 %v742 to i64
  %v743 = icmp ne i64 %c158, 0
  br i1 %v743, label %b553, label %b554
b552:
  %v750 = call i64 @df.unbox_int(i64 %v0)
  %v751 = load i8, ptr %ctx, align 8
  %c159 = zext i8 %v751 to i64
  %v752 = icmp ne i64 %c159, 0
  br i1 %v752, label %b559, label %b558
b553:
  %ga744 = getelementptr i64, ptr %area, i64 10808
  %v744 = load i64, ptr %ga744
  br label %b555
b554:
  %v745 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b555
b555:
  %v746 = phi i64 [ %v744, %b553 ], [ %v745, %b554 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v746, i64 0, i64 0)
  %v748 = load i8, ptr %ctx, align 8
  %c160 = zext i8 %v748 to i64
  %v749 = icmp ne i64 %c160, 0
  br i1 %v749, label %b557, label %b556
b556:
  br label %b552
b557:
  %gcvolta557 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta557, ptr %ctxtopo, align 8
  ret i64 0
b558:
  %v753 = add i64 0, 14
  %v754 = ashr i64 %v750, %v753
  store i64 0, ptr %gcs0
  %v755 = call i64 @df.box_int(i64 %v754)
  %xcp756 = load i8, ptr %ctx, align 8
  %xcn756 = icmp ne i8 %xcp756, 0
  %xce756 = call i1 @llvm.expect.i1(i1 %xcn756, i1 false)
  br i1 %xce756, label %xc756.limpar, label %xc756.fim
xc756.limpar:
  call void @dartforge_exception_clear()
  br label %xc756.fim
xc756.fim:
  %gcvolta558 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta558, ptr %ctxtopo, align 8
  ret i64 %v755
b559:
  %gcvolta559 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta559, ptr %ctxtopo, align 8
  ret i64 0
b561:
  %ga765 = getelementptr i64, ptr %area, i64 10809
  %v765 = load i8, ptr %ga765
  %c161 = zext i8 %v765 to i64
  %v766 = icmp ne i64 %c161, 0
  br i1 %v766, label %b563, label %b564
b562:
  %v773 = call i64 @df.unbox_int(i64 %v0)
  %v774 = load i8, ptr %ctx, align 8
  %c162 = zext i8 %v774 to i64
  %v775 = icmp ne i64 %c162, 0
  br i1 %v775, label %b569, label %b568
b563:
  %ga767 = getelementptr i64, ptr %area, i64 10808
  %v767 = load i64, ptr %ga767
  br label %b565
b564:
  %v768 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b565
b565:
  %v769 = phi i64 [ %v767, %b563 ], [ %v768, %b564 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v769, i64 0, i64 0)
  %v771 = load i8, ptr %ctx, align 8
  %c163 = zext i8 %v771 to i64
  %v772 = icmp ne i64 %c163, 0
  br i1 %v772, label %b567, label %b566
b566:
  br label %b562
b567:
  %gcvolta567 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta567, ptr %ctxtopo, align 8
  ret i64 0
b568:
  %v776 = add i64 0, 12
  %v777 = ashr i64 %v773, %v776
  store i64 0, ptr %gcs0
  %v778 = call i64 @df.box_int(i64 %v777)
  %xcp779 = load i8, ptr %ctx, align 8
  %xcn779 = icmp ne i8 %xcp779, 0
  %xce779 = call i1 @llvm.expect.i1(i1 %xcn779, i1 false)
  br i1 %xce779, label %xc779.limpar, label %xc779.fim
xc779.limpar:
  call void @dartforge_exception_clear()
  br label %xc779.fim
xc779.fim:
  %gcvolta568 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta568, ptr %ctxtopo, align 8
  ret i64 %v778
b569:
  %gcvolta569 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta569, ptr %ctxtopo, align 8
  ret i64 0
b571:
  %ga788 = getelementptr i64, ptr %area, i64 10809
  %v788 = load i8, ptr %ga788
  %c164 = zext i8 %v788 to i64
  %v789 = icmp ne i64 %c164, 0
  br i1 %v789, label %b573, label %b574
b572:
  %v796 = call i64 @df.unbox_int(i64 %v0)
  %v797 = load i8, ptr %ctx, align 8
  %c165 = zext i8 %v797 to i64
  %v798 = icmp ne i64 %c165, 0
  br i1 %v798, label %b579, label %b578
b573:
  %ga790 = getelementptr i64, ptr %area, i64 10808
  %v790 = load i64, ptr %ga790
  br label %b575
b574:
  %v791 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b575
b575:
  %v792 = phi i64 [ %v790, %b573 ], [ %v791, %b574 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v792, i64 0, i64 0)
  %v794 = load i8, ptr %ctx, align 8
  %c166 = zext i8 %v794 to i64
  %v795 = icmp ne i64 %c166, 0
  br i1 %v795, label %b577, label %b576
b576:
  br label %b572
b577:
  %gcvolta577 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta577, ptr %ctxtopo, align 8
  ret i64 0
b578:
  %v799 = add i64 0, 8
  %v800 = ashr i64 %v796, %v799
  store i64 0, ptr %gcs0
  %v801 = call i64 @df.box_int(i64 %v800)
  %xcp802 = load i8, ptr %ctx, align 8
  %xcn802 = icmp ne i8 %xcp802, 0
  %xce802 = call i1 @llvm.expect.i1(i1 %xcn802, i1 false)
  br i1 %xce802, label %xc802.limpar, label %xc802.fim
xc802.limpar:
  call void @dartforge_exception_clear()
  br label %xc802.fim
xc802.fim:
  %gcvolta578 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta578, ptr %ctxtopo, align 8
  ret i64 %v801
b579:
  %gcvolta579 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta579, ptr %ctxtopo, align 8
  ret i64 0
b582:
  %ga813 = getelementptr i64, ptr %area, i64 10809
  %v813 = load i8, ptr %ga813
  %c167 = zext i8 %v813 to i64
  %v814 = icmp ne i64 %c167, 0
  br i1 %v814, label %b584, label %b585
b583:
  %v821 = call i64 @df.unbox_int(i64 %v0)
  %v822 = load i8, ptr %ctx, align 8
  %c168 = zext i8 %v822 to i64
  %v823 = icmp ne i64 %c168, 0
  br i1 %v823, label %b590, label %b589
b584:
  %ga815 = getelementptr i64, ptr %area, i64 10808
  %v815 = load i64, ptr %ga815
  br label %b586
b585:
  %v816 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b586
b586:
  %v817 = phi i64 [ %v815, %b584 ], [ %v816, %b585 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v817, i64 0, i64 0)
  %v819 = load i8, ptr %ctx, align 8
  %c169 = zext i8 %v819 to i64
  %v820 = icmp ne i64 %c169, 0
  br i1 %v820, label %b588, label %b587
b587:
  br label %b583
b588:
  %gcvolta588 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta588, ptr %ctxtopo, align 8
  ret i64 0
b589:
  %v824 = add i64 0, 8
  %v825 = shl i64 %v821, %v824
  store i64 0, ptr %gcs0
  %v826 = call i64 @df.box_int(i64 %v825)
  %xcp827 = load i8, ptr %ctx, align 8
  %xcn827 = icmp ne i8 %xcp827, 0
  %xce827 = call i1 @llvm.expect.i1(i1 %xcn827, i1 false)
  br i1 %xce827, label %xc827.limpar, label %xc827.fim
xc827.limpar:
  call void @dartforge_exception_clear()
  br label %xc827.fim
xc827.fim:
  %gcvolta589 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta589, ptr %ctxtopo, align 8
  ret i64 %v826
b590:
  %gcvolta590 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta590, ptr %ctxtopo, align 8
  ret i64 0
b592:
  %ga836 = getelementptr i64, ptr %area, i64 10809
  %v836 = load i8, ptr %ga836
  %c170 = zext i8 %v836 to i64
  %v837 = icmp ne i64 %c170, 0
  br i1 %v837, label %b594, label %b595
b593:
  %v844 = call i64 @df.unbox_int(i64 %v0)
  %v845 = load i8, ptr %ctx, align 8
  %c171 = zext i8 %v845 to i64
  %v846 = icmp ne i64 %c171, 0
  br i1 %v846, label %b600, label %b599
b594:
  %ga838 = getelementptr i64, ptr %area, i64 10808
  %v838 = load i64, ptr %ga838
  br label %b596
b595:
  %v839 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b596
b596:
  %v840 = phi i64 [ %v838, %b594 ], [ %v839, %b595 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v840, i64 0, i64 0)
  %v842 = load i8, ptr %ctx, align 8
  %c172 = zext i8 %v842 to i64
  %v843 = icmp ne i64 %c172, 0
  br i1 %v843, label %b598, label %b597
b597:
  br label %b593
b598:
  %gcvolta598 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta598, ptr %ctxtopo, align 8
  ret i64 0
b599:
  %v847 = add i64 0, 9
  %v848 = ashr i64 %v844, %v847
  store i64 0, ptr %gcs0
  %v849 = call i64 @df.box_int(i64 %v848)
  %xcp850 = load i8, ptr %ctx, align 8
  %xcn850 = icmp ne i8 %xcp850, 0
  %xce850 = call i1 @llvm.expect.i1(i1 %xcn850, i1 false)
  br i1 %xce850, label %xc850.limpar, label %xc850.fim
xc850.limpar:
  call void @dartforge_exception_clear()
  br label %xc850.fim
xc850.fim:
  %gcvolta599 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta599, ptr %ctxtopo, align 8
  ret i64 %v849
b600:
  %gcvolta600 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta600, ptr %ctxtopo, align 8
  ret i64 0
b602:
  %ga859 = getelementptr i64, ptr %area, i64 10809
  %v859 = load i8, ptr %ga859
  %c173 = zext i8 %v859 to i64
  %v860 = icmp ne i64 %c173, 0
  br i1 %v860, label %b604, label %b605
b603:
  %v867 = call i64 @df.unbox_int(i64 %v0)
  %v868 = load i8, ptr %ctx, align 8
  %c174 = zext i8 %v868 to i64
  %v869 = icmp ne i64 %c174, 0
  br i1 %v869, label %b610, label %b609
b604:
  %ga861 = getelementptr i64, ptr %area, i64 10808
  %v861 = load i64, ptr %ga861
  br label %b606
b605:
  %v862 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b606
b606:
  %v863 = phi i64 [ %v861, %b604 ], [ %v862, %b605 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v863, i64 0, i64 0)
  %v865 = load i8, ptr %ctx, align 8
  %c175 = zext i8 %v865 to i64
  %v866 = icmp ne i64 %c175, 0
  br i1 %v866, label %b608, label %b607
b607:
  br label %b603
b608:
  %gcvolta608 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta608, ptr %ctxtopo, align 8
  ret i64 0
b609:
  %v870 = add i64 0, 1
  %v871 = ashr i64 %v867, %v870
  store i64 0, ptr %gcs0
  %v872 = call i64 @df.box_int(i64 %v871)
  %xcp873 = load i8, ptr %ctx, align 8
  %xcn873 = icmp ne i8 %xcp873, 0
  %xce873 = call i1 @llvm.expect.i1(i1 %xcn873, i1 false)
  br i1 %xce873, label %xc873.limpar, label %xc873.fim
xc873.limpar:
  call void @dartforge_exception_clear()
  br label %xc873.fim
xc873.fim:
  %gcvolta609 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta609, ptr %ctxtopo, align 8
  ret i64 %v872
b610:
  %gcvolta610 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta610, ptr %ctxtopo, align 8
  ret i64 0
b612:
  %v878 = ashr i64 %v0, 1
  %v879 = mul i64 %v878, %v875
  store i64 0, ptr %gcs0
  %v880 = call i64 @df.box_int(i64 %v879)
  br label %b614
b613:
  %v881 = call i64 @df.box_int(i64 %v875)
  store i64 %v881, ptr %gcs1
  %sa882_0 = getelementptr [2 x i64], ptr %sargs882, i64 0, i64 0
  store i64 %v881, ptr %sa882_0
  %sat882 = getelementptr [2 x i64], ptr %sargs882, i64 0, i64 1
  store i64 0, ptr %sat882
  %sic882 = getelementptr i64, ptr %area, i64 141746
  %sf882 = call ptr @df.selector(ptr %sic882, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v882 = call i64 %sf882(i64 %v0, ptr %sargs882, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v883 = load i8, ptr %ctx, align 8
  %c176 = zext i8 %v883 to i64
  %v884 = icmp ne i64 %c176, 0
  br i1 %v884, label %b616, label %b615
b614:
  %v885 = phi i64 [ %v880, %b612 ], [ %v882, %b615 ]
  %xcp886 = load i8, ptr %ctx, align 8
  %xcn886 = icmp ne i8 %xcp886, 0
  %xce886 = call i1 @llvm.expect.i1(i1 %xcn886, i1 false)
  br i1 %xce886, label %xc886.limpar, label %xc886.fim
xc886.limpar:
  call void @dartforge_exception_clear()
  br label %xc886.fim
xc886.fim:
  %gcvolta614 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta614, ptr %ctxtopo, align 8
  ret i64 %v885
b615:
  br label %b614
b616:
  %gcvolta616 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta616, ptr %ctxtopo, align 8
  ret i64 0
b618:
  store i64 0, ptr %gcs1
  %v893 = call double @df.unbox_double(i64 %v890)
  %v894 = load i8, ptr %ctx, align 8
  %c177 = zext i8 %v894 to i64
  %v895 = icmp ne i64 %c177, 0
  br i1 %v895, label %b621, label %b620
b619:
  %gcvolta619 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta619, ptr %ctxtopo, align 8
  ret i64 0
b620:
  %xcp897 = load i8, ptr %ctx, align 8
  %xcn897 = icmp ne i8 %xcp897, 0
  %xce897 = call i1 @llvm.expect.i1(i1 %xcn897, i1 false)
  br i1 %xce897, label %xc897.limpar, label %xc897.fim
xc897.limpar:
  call void @dartforge_exception_clear()
  br label %xc897.fim
xc897.fim:
  %gcvolta620 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta620, ptr %ctxtopo, align 8
  ret i64 %v890
b621:
  %gcvolta621 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta621, ptr %ctxtopo, align 8
  ret i64 0
b635:
  %v1131 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1132 = call i64 @df.box_int(i64 %v1131)
  store i64 %v1132, ptr %gcs1
  %sa1133_0 = getelementptr [2 x i64], ptr %sargs1133, i64 0, i64 0
  store i64 %v1132, ptr %sa1133_0
  %sat1133 = getelementptr [2 x i64], ptr %sargs1133, i64 0, i64 1
  store i64 0, ptr %sat1133
  %sic1133 = getelementptr i64, ptr %area, i64 141748
  %sf1133 = call ptr @df.selector(ptr %sic1133, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v1133 = call i64 %sf1133(i64 %v0, ptr %sargs1133, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1133, ptr %gcs0
  %v1134 = load i8, ptr %ctx, align 8
  %c178 = zext i8 %v1134 to i64
  %v1135 = icmp ne i64 %c178, 0
  br i1 %v1135, label %b769, label %b768
b636:
  store i64 0, ptr %gcs1
  %v903 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v903, ptr %gcs1
  %v904 = load i8, ptr %ctx, align 8
  %c179 = zext i8 %v904 to i64
  %v905 = icmp ne i64 %c179, 0
  br i1 %v905, label %b642, label %b641
b637:
  %v902 = icmp eq i64 %v899, %v6
  br i1 %v902, label %b639, label %b636
b638:
  %gcvolta638 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta638, ptr %ctxtopo, align 8
  ret i64 0
b639:
  %v948 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v949 = call i64 @df.box_int(i64 %v948)
  store i64 %v949, ptr %gcs1
  %v950 = icmp eq i64 %v0, 0
  %v951 = icmp eq i64 %v949, 0
  %c180 = zext i1 %v950 to i64
  %c181 = zext i1 %v951 to i64
  %v952 = or i64 %c180, %c181
  %v953 = icmp ne i64 %v952, 0
  br i1 %v953, label %b686, label %b687
b640:
  store i64 0, ptr %gcs1
  %v907 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v907, ptr %gcs1
  %v908 = load i8, ptr %ctx, align 8
  %c182 = zext i8 %v908 to i64
  %v909 = icmp ne i64 %c182, 0
  br i1 %v909, label %b646, label %b645
b641:
  %v906 = icmp eq i64 %v903, %v6
  br i1 %v906, label %b643, label %b640
b642:
  %gcvolta642 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta642, ptr %ctxtopo, align 8
  ret i64 0
b643:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v968 = icmp eq i64 %v0, 0
  %v969 = call i64 @df.class(i64 %v0)
  %v970 = call i8 @df.subclass(i64 %v969, i64 674)
  %c183 = zext i8 %v970 to i64
  %v971 = icmp ne i64 %c183, 0
  %v972 = xor i1 %v968, true
  %c184 = zext i1 %v971 to i64
  %c185 = zext i1 %v972 to i64
  %v973 = and i64 %c184, %c185
  %v974 = icmp ne i64 %v973, 0
  br i1 %v974, label %b698, label %b697
b644:
  store i64 0, ptr %gcs1
  %v911 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v911, ptr %gcs1
  %v912 = load i8, ptr %ctx, align 8
  %c186 = zext i8 %v912 to i64
  %v913 = icmp ne i64 %c186, 0
  br i1 %v913, label %b650, label %b649
b645:
  %v910 = icmp eq i64 %v907, %v6
  br i1 %v910, label %b647, label %b644
b646:
  %gcvolta646 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta646, ptr %ctxtopo, align 8
  ret i64 0
b647:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v991 = icmp eq i64 %v0, 0
  %v992 = call i64 @df.class(i64 %v0)
  %v993 = call i8 @df.subclass(i64 %v992, i64 674)
  %c187 = zext i8 %v993 to i64
  %v994 = icmp ne i64 %c187, 0
  %v995 = xor i1 %v991, true
  %c188 = zext i1 %v994 to i64
  %c189 = zext i1 %v995 to i64
  %v996 = and i64 %c188, %c189
  %v997 = icmp ne i64 %v996, 0
  br i1 %v997, label %b708, label %b707
b648:
  store i64 0, ptr %gcs1
  %v915 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v915, ptr %gcs1
  %v916 = load i8, ptr %ctx, align 8
  %c190 = zext i8 %v916 to i64
  %v917 = icmp ne i64 %c190, 0
  br i1 %v917, label %b654, label %b653
b649:
  %v914 = icmp eq i64 %v911, %v6
  br i1 %v914, label %b651, label %b648
b650:
  %gcvolta650 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta650, ptr %ctxtopo, align 8
  ret i64 0
b651:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1014 = icmp eq i64 %v0, 0
  %v1015 = call i64 @df.class(i64 %v0)
  %v1016 = call i8 @df.subclass(i64 %v1015, i64 674)
  %c191 = zext i8 %v1016 to i64
  %v1017 = icmp ne i64 %c191, 0
  %v1018 = xor i1 %v1014, true
  %c192 = zext i1 %v1017 to i64
  %c193 = zext i1 %v1018 to i64
  %v1019 = and i64 %c192, %c193
  %v1020 = icmp ne i64 %v1019, 0
  br i1 %v1020, label %b718, label %b717
b652:
  store i64 0, ptr %gcs1
  %v919 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v919, ptr %gcs1
  %v920 = load i8, ptr %ctx, align 8
  %c194 = zext i8 %v920 to i64
  %v921 = icmp ne i64 %c194, 0
  br i1 %v921, label %b658, label %b657
b653:
  %v918 = icmp eq i64 %v915, %v6
  br i1 %v918, label %b655, label %b652
b654:
  %gcvolta654 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta654, ptr %ctxtopo, align 8
  ret i64 0
b655:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1037 = icmp eq i64 %v0, 0
  %v1038 = call i64 @df.class(i64 %v0)
  %v1039 = call i8 @df.subclass(i64 %v1038, i64 674)
  %c195 = zext i8 %v1039 to i64
  %v1040 = icmp ne i64 %c195, 0
  %v1041 = xor i1 %v1037, true
  %c196 = zext i1 %v1040 to i64
  %c197 = zext i1 %v1041 to i64
  %v1042 = and i64 %c196, %c197
  %v1043 = icmp ne i64 %v1042, 0
  br i1 %v1043, label %b728, label %b727
b656:
  store i64 0, ptr %gcs1
  %v923 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v923, ptr %gcs1
  %v924 = load i8, ptr %ctx, align 8
  %c198 = zext i8 %v924 to i64
  %v925 = icmp ne i64 %c198, 0
  br i1 %v925, label %b662, label %b661
b657:
  %v922 = icmp eq i64 %v919, %v6
  br i1 %v922, label %b659, label %b656
b658:
  %gcvolta658 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta658, ptr %ctxtopo, align 8
  ret i64 0
b659:
  %xcp1060 = load i8, ptr %ctx, align 8
  %xcn1060 = icmp ne i8 %xcp1060, 0
  %xce1060 = call i1 @llvm.expect.i1(i1 %xcn1060, i1 false)
  br i1 %xce1060, label %xc1060.limpar, label %xc1060.fim
xc1060.limpar:
  call void @dartforge_exception_clear()
  br label %xc1060.fim
xc1060.fim:
  %gcvolta659 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta659, ptr %ctxtopo, align 8
  ret i64 %v0
b660:
  store i64 0, ptr %gcs1
  %v927 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v927, ptr %gcs1
  %v928 = load i8, ptr %ctx, align 8
  %c199 = zext i8 %v928 to i64
  %v929 = icmp ne i64 %c199, 0
  br i1 %v929, label %b666, label %b665
b661:
  %v926 = icmp eq i64 %v923, %v6
  br i1 %v926, label %b663, label %b660
b662:
  %gcvolta662 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta662, ptr %ctxtopo, align 8
  ret i64 0
b663:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1062 = icmp eq i64 %v0, 0
  %v1063 = call i64 @df.class(i64 %v0)
  %v1064 = call i8 @df.subclass(i64 %v1063, i64 674)
  %c200 = zext i8 %v1064 to i64
  %v1065 = icmp ne i64 %c200, 0
  %v1066 = xor i1 %v1062, true
  %c201 = zext i1 %v1065 to i64
  %c202 = zext i1 %v1066 to i64
  %v1067 = and i64 %c201, %c202
  %v1068 = icmp ne i64 %v1067, 0
  br i1 %v1068, label %b739, label %b738
b664:
  store i64 0, ptr %gcs1
  %v931 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v931, ptr %gcs1
  %v932 = load i8, ptr %ctx, align 8
  %c203 = zext i8 %v932 to i64
  %v933 = icmp ne i64 %c203, 0
  br i1 %v933, label %b670, label %b669
b665:
  %v930 = icmp eq i64 %v927, %v6
  br i1 %v930, label %b667, label %b664
b666:
  %gcvolta666 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta666, ptr %ctxtopo, align 8
  ret i64 0
b667:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1085 = icmp eq i64 %v0, 0
  %v1086 = call i64 @df.class(i64 %v0)
  %v1087 = call i8 @df.subclass(i64 %v1086, i64 674)
  %c204 = zext i8 %v1087 to i64
  %v1088 = icmp ne i64 %c204, 0
  %v1089 = xor i1 %v1085, true
  %c205 = zext i1 %v1088 to i64
  %c206 = zext i1 %v1089 to i64
  %v1090 = and i64 %c205, %c206
  %v1091 = icmp ne i64 %v1090, 0
  br i1 %v1091, label %b749, label %b748
b668:
  store i64 0, ptr %gcs1
  %v935 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v935, ptr %gcs1
  %v936 = load i8, ptr %ctx, align 8
  %c207 = zext i8 %v936 to i64
  %v937 = icmp ne i64 %c207, 0
  br i1 %v937, label %b674, label %b673
b669:
  %v934 = icmp eq i64 %v931, %v6
  br i1 %v934, label %b671, label %b668
b670:
  %gcvolta670 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta670, ptr %ctxtopo, align 8
  ret i64 0
b671:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1108 = icmp eq i64 %v0, 0
  %v1109 = call i64 @df.class(i64 %v0)
  %v1110 = call i8 @df.subclass(i64 %v1109, i64 674)
  %c208 = zext i8 %v1110 to i64
  %v1111 = icmp ne i64 %c208, 0
  %v1112 = xor i1 %v1108, true
  %c209 = zext i1 %v1111 to i64
  %c210 = zext i1 %v1112 to i64
  %v1113 = and i64 %c209, %c210
  %v1114 = icmp ne i64 %v1113, 0
  br i1 %v1114, label %b759, label %b758
b672:
  store i64 0, ptr %gcs1
  %v939 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v939, ptr %gcs1
  %v940 = load i8, ptr %ctx, align 8
  %c211 = zext i8 %v940 to i64
  %v941 = icmp ne i64 %c211, 0
  br i1 %v941, label %b679, label %b678
b673:
  %v938 = icmp eq i64 %v935, %v6
  br i1 %v938, label %b675, label %b672
b674:
  %gcvolta674 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta674, ptr %ctxtopo, align 8
  ret i64 0
b675:
  br label %b635
b677:
  store i64 0, ptr %gcs1
  %v943 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v943, ptr %gcs1
  %v944 = load i8, ptr %ctx, align 8
  %c212 = zext i8 %v944 to i64
  %v945 = icmp ne i64 %c212, 0
  br i1 %v945, label %b684, label %b683
b678:
  %v942 = icmp eq i64 %v939, %v6
  br i1 %v942, label %b680, label %b677
b679:
  %gcvolta679 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta679, ptr %ctxtopo, align 8
  ret i64 0
b680:
  br label %b635
b682:
  br label %b12
b683:
  %v946 = icmp eq i64 %v943, %v6
  br i1 %v946, label %b685, label %b682
b684:
  %gcvolta684 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta684, ptr %ctxtopo, align 8
  ret i64 0
b685:
  br label %b635
b686:
  %v954 = icmp eq i64 %v0, %v949
  br label %b688
b687:
  %sa955_0 = getelementptr [2 x i64], ptr %sargs955, i64 0, i64 0
  store i64 %v949, ptr %sa955_0
  %sat955 = getelementptr [2 x i64], ptr %sargs955, i64 0, i64 1
  store i64 0, ptr %sat955
  %sic955 = getelementptr i64, ptr %area, i64 141750
  %sf955 = call ptr @df.selector(ptr %sic955, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v955 = call i64 %sf955(i64 %v0, ptr %sargs955, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v955, ptr %gcs0
  %v956 = load i8, ptr %ctx, align 8
  %c213 = zext i8 %v956 to i64
  %v957 = icmp ne i64 %c213, 0
  br i1 %v957, label %b690, label %b689
b688:
  %v961 = phi i1 [ %v954, %b686 ], [ %v958, %b691 ]
  br i1 %v961, label %b693, label %b694
b689:
  store i64 0, ptr %gcs1
  %v958 = call i1 @df.unbox_bool(i64 %v955)
  %v959 = load i8, ptr %ctx, align 8
  %c214 = zext i8 %v959 to i64
  %v960 = icmp ne i64 %c214, 0
  br i1 %v960, label %b692, label %b691
b690:
  %gcvolta690 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta690, ptr %ctxtopo, align 8
  ret i64 0
b691:
  br label %b688
b692:
  %gcvolta692 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta692, ptr %ctxtopo, align 8
  ret i64 0
b693:
  %v962 = add i64 0, 0
  br label %b695
b694:
  %v963 = add i64 0, 1
  br label %b695
b695:
  %v964 = phi i64 [ %v962, %b693 ], [ %v963, %b694 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v965 = call i64 @df.box_int(i64 %v964)
  %xcp966 = load i8, ptr %ctx, align 8
  %xcn966 = icmp ne i8 %xcp966, 0
  %xce966 = call i1 @llvm.expect.i1(i1 %xcn966, i1 false)
  br i1 %xce966, label %xc966.limpar, label %xc966.fim
xc966.limpar:
  call void @dartforge_exception_clear()
  br label %xc966.fim
xc966.fim:
  %gcvolta695 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta695, ptr %ctxtopo, align 8
  ret i64 %v965
b697:
  %ga975 = getelementptr i64, ptr %area, i64 10809
  %v975 = load i8, ptr %ga975
  %c215 = zext i8 %v975 to i64
  %v976 = icmp ne i64 %c215, 0
  br i1 %v976, label %b699, label %b700
b698:
  %v983 = call i64 @df.unbox_int(i64 %v0)
  %v984 = load i8, ptr %ctx, align 8
  %c216 = zext i8 %v984 to i64
  %v985 = icmp ne i64 %c216, 0
  br i1 %v985, label %b705, label %b704
b699:
  %ga977 = getelementptr i64, ptr %area, i64 10808
  %v977 = load i64, ptr %ga977
  br label %b701
b700:
  %v978 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b701
b701:
  %v979 = phi i64 [ %v977, %b699 ], [ %v978, %b700 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v979, i64 0, i64 0)
  %v981 = load i8, ptr %ctx, align 8
  %c217 = zext i8 %v981 to i64
  %v982 = icmp ne i64 %c217, 0
  br i1 %v982, label %b703, label %b702
b702:
  br label %b698
b703:
  %gcvolta703 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta703, ptr %ctxtopo, align 8
  ret i64 0
b704:
  %v986 = add i64 0, 30
  %v987 = ashr i64 %v983, %v986
  store i64 0, ptr %gcs0
  %v988 = call i64 @df.box_int(i64 %v987)
  %xcp989 = load i8, ptr %ctx, align 8
  %xcn989 = icmp ne i8 %xcp989, 0
  %xce989 = call i1 @llvm.expect.i1(i1 %xcn989, i1 false)
  br i1 %xce989, label %xc989.limpar, label %xc989.fim
xc989.limpar:
  call void @dartforge_exception_clear()
  br label %xc989.fim
xc989.fim:
  %gcvolta704 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta704, ptr %ctxtopo, align 8
  ret i64 %v988
b705:
  %gcvolta705 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta705, ptr %ctxtopo, align 8
  ret i64 0
b707:
  %ga998 = getelementptr i64, ptr %area, i64 10809
  %v998 = load i8, ptr %ga998
  %c218 = zext i8 %v998 to i64
  %v999 = icmp ne i64 %c218, 0
  br i1 %v999, label %b709, label %b710
b708:
  %v1006 = call i64 @df.unbox_int(i64 %v0)
  %v1007 = load i8, ptr %ctx, align 8
  %c219 = zext i8 %v1007 to i64
  %v1008 = icmp ne i64 %c219, 0
  br i1 %v1008, label %b715, label %b714
b709:
  %ga1000 = getelementptr i64, ptr %area, i64 10808
  %v1000 = load i64, ptr %ga1000
  br label %b711
b710:
  %v1001 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b711
b711:
  %v1002 = phi i64 [ %v1000, %b709 ], [ %v1001, %b710 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1002, i64 0, i64 0)
  %v1004 = load i8, ptr %ctx, align 8
  %c220 = zext i8 %v1004 to i64
  %v1005 = icmp ne i64 %c220, 0
  br i1 %v1005, label %b713, label %b712
b712:
  br label %b708
b713:
  %gcvolta713 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta713, ptr %ctxtopo, align 8
  ret i64 0
b714:
  %v1009 = add i64 0, 28
  %v1010 = ashr i64 %v1006, %v1009
  store i64 0, ptr %gcs0
  %v1011 = call i64 @df.box_int(i64 %v1010)
  %xcp1012 = load i8, ptr %ctx, align 8
  %xcn1012 = icmp ne i8 %xcp1012, 0
  %xce1012 = call i1 @llvm.expect.i1(i1 %xcn1012, i1 false)
  br i1 %xce1012, label %xc1012.limpar, label %xc1012.fim
xc1012.limpar:
  call void @dartforge_exception_clear()
  br label %xc1012.fim
xc1012.fim:
  %gcvolta714 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta714, ptr %ctxtopo, align 8
  ret i64 %v1011
b715:
  %gcvolta715 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta715, ptr %ctxtopo, align 8
  ret i64 0
b717:
  %ga1021 = getelementptr i64, ptr %area, i64 10809
  %v1021 = load i8, ptr %ga1021
  %c221 = zext i8 %v1021 to i64
  %v1022 = icmp ne i64 %c221, 0
  br i1 %v1022, label %b719, label %b720
b718:
  %v1029 = call i64 @df.unbox_int(i64 %v0)
  %v1030 = load i8, ptr %ctx, align 8
  %c222 = zext i8 %v1030 to i64
  %v1031 = icmp ne i64 %c222, 0
  br i1 %v1031, label %b725, label %b724
b719:
  %ga1023 = getelementptr i64, ptr %area, i64 10808
  %v1023 = load i64, ptr %ga1023
  br label %b721
b720:
  %v1024 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b721
b721:
  %v1025 = phi i64 [ %v1023, %b719 ], [ %v1024, %b720 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1025, i64 0, i64 0)
  %v1027 = load i8, ptr %ctx, align 8
  %c223 = zext i8 %v1027 to i64
  %v1028 = icmp ne i64 %c223, 0
  br i1 %v1028, label %b723, label %b722
b722:
  br label %b718
b723:
  %gcvolta723 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta723, ptr %ctxtopo, align 8
  ret i64 0
b724:
  %v1032 = add i64 0, 24
  %v1033 = ashr i64 %v1029, %v1032
  store i64 0, ptr %gcs0
  %v1034 = call i64 @df.box_int(i64 %v1033)
  %xcp1035 = load i8, ptr %ctx, align 8
  %xcn1035 = icmp ne i8 %xcp1035, 0
  %xce1035 = call i1 @llvm.expect.i1(i1 %xcn1035, i1 false)
  br i1 %xce1035, label %xc1035.limpar, label %xc1035.fim
xc1035.limpar:
  call void @dartforge_exception_clear()
  br label %xc1035.fim
xc1035.fim:
  %gcvolta724 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta724, ptr %ctxtopo, align 8
  ret i64 %v1034
b725:
  %gcvolta725 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta725, ptr %ctxtopo, align 8
  ret i64 0
b727:
  %ga1044 = getelementptr i64, ptr %area, i64 10809
  %v1044 = load i8, ptr %ga1044
  %c224 = zext i8 %v1044 to i64
  %v1045 = icmp ne i64 %c224, 0
  br i1 %v1045, label %b729, label %b730
b728:
  %v1052 = call i64 @df.unbox_int(i64 %v0)
  %v1053 = load i8, ptr %ctx, align 8
  %c225 = zext i8 %v1053 to i64
  %v1054 = icmp ne i64 %c225, 0
  br i1 %v1054, label %b735, label %b734
b729:
  %ga1046 = getelementptr i64, ptr %area, i64 10808
  %v1046 = load i64, ptr %ga1046
  br label %b731
b730:
  %v1047 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b731
b731:
  %v1048 = phi i64 [ %v1046, %b729 ], [ %v1047, %b730 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1048, i64 0, i64 0)
  %v1050 = load i8, ptr %ctx, align 8
  %c226 = zext i8 %v1050 to i64
  %v1051 = icmp ne i64 %c226, 0
  br i1 %v1051, label %b733, label %b732
b732:
  br label %b728
b733:
  %gcvolta733 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta733, ptr %ctxtopo, align 8
  ret i64 0
b734:
  %v1055 = add i64 0, 16
  %v1056 = ashr i64 %v1052, %v1055
  store i64 0, ptr %gcs0
  %v1057 = call i64 @df.box_int(i64 %v1056)
  %xcp1058 = load i8, ptr %ctx, align 8
  %xcn1058 = icmp ne i8 %xcp1058, 0
  %xce1058 = call i1 @llvm.expect.i1(i1 %xcn1058, i1 false)
  br i1 %xce1058, label %xc1058.limpar, label %xc1058.fim
xc1058.limpar:
  call void @dartforge_exception_clear()
  br label %xc1058.fim
xc1058.fim:
  %gcvolta734 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta734, ptr %ctxtopo, align 8
  ret i64 %v1057
b735:
  %gcvolta735 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta735, ptr %ctxtopo, align 8
  ret i64 0
b738:
  %ga1069 = getelementptr i64, ptr %area, i64 10809
  %v1069 = load i8, ptr %ga1069
  %c227 = zext i8 %v1069 to i64
  %v1070 = icmp ne i64 %c227, 0
  br i1 %v1070, label %b740, label %b741
b739:
  %v1077 = call i64 @df.unbox_int(i64 %v0)
  %v1078 = load i8, ptr %ctx, align 8
  %c228 = zext i8 %v1078 to i64
  %v1079 = icmp ne i64 %c228, 0
  br i1 %v1079, label %b746, label %b745
b740:
  %ga1071 = getelementptr i64, ptr %area, i64 10808
  %v1071 = load i64, ptr %ga1071
  br label %b742
b741:
  %v1072 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b742
b742:
  %v1073 = phi i64 [ %v1071, %b740 ], [ %v1072, %b741 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1073, i64 0, i64 0)
  %v1075 = load i8, ptr %ctx, align 8
  %c229 = zext i8 %v1075 to i64
  %v1076 = icmp ne i64 %c229, 0
  br i1 %v1076, label %b744, label %b743
b743:
  br label %b739
b744:
  %gcvolta744 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta744, ptr %ctxtopo, align 8
  ret i64 0
b745:
  %v1080 = add i64 0, 25
  %v1081 = ashr i64 %v1077, %v1080
  store i64 0, ptr %gcs0
  %v1082 = call i64 @df.box_int(i64 %v1081)
  %xcp1083 = load i8, ptr %ctx, align 8
  %xcn1083 = icmp ne i8 %xcp1083, 0
  %xce1083 = call i1 @llvm.expect.i1(i1 %xcn1083, i1 false)
  br i1 %xce1083, label %xc1083.limpar, label %xc1083.fim
xc1083.limpar:
  call void @dartforge_exception_clear()
  br label %xc1083.fim
xc1083.fim:
  %gcvolta745 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta745, ptr %ctxtopo, align 8
  ret i64 %v1082
b746:
  %gcvolta746 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta746, ptr %ctxtopo, align 8
  ret i64 0
b748:
  %ga1092 = getelementptr i64, ptr %area, i64 10809
  %v1092 = load i8, ptr %ga1092
  %c230 = zext i8 %v1092 to i64
  %v1093 = icmp ne i64 %c230, 0
  br i1 %v1093, label %b750, label %b751
b749:
  %v1100 = call i64 @df.unbox_int(i64 %v0)
  %v1101 = load i8, ptr %ctx, align 8
  %c231 = zext i8 %v1101 to i64
  %v1102 = icmp ne i64 %c231, 0
  br i1 %v1102, label %b756, label %b755
b750:
  %ga1094 = getelementptr i64, ptr %area, i64 10808
  %v1094 = load i64, ptr %ga1094
  br label %b752
b751:
  %v1095 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b752
b752:
  %v1096 = phi i64 [ %v1094, %b750 ], [ %v1095, %b751 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1096, i64 0, i64 0)
  %v1098 = load i8, ptr %ctx, align 8
  %c232 = zext i8 %v1098 to i64
  %v1099 = icmp ne i64 %c232, 0
  br i1 %v1099, label %b754, label %b753
b753:
  br label %b749
b754:
  %gcvolta754 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta754, ptr %ctxtopo, align 8
  ret i64 0
b755:
  %v1103 = add i64 0, 17
  %v1104 = ashr i64 %v1100, %v1103
  store i64 0, ptr %gcs0
  %v1105 = call i64 @df.box_int(i64 %v1104)
  %xcp1106 = load i8, ptr %ctx, align 8
  %xcn1106 = icmp ne i8 %xcp1106, 0
  %xce1106 = call i1 @llvm.expect.i1(i1 %xcn1106, i1 false)
  br i1 %xce1106, label %xc1106.limpar, label %xc1106.fim
xc1106.limpar:
  call void @dartforge_exception_clear()
  br label %xc1106.fim
xc1106.fim:
  %gcvolta755 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta755, ptr %ctxtopo, align 8
  ret i64 %v1105
b756:
  %gcvolta756 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta756, ptr %ctxtopo, align 8
  ret i64 0
b758:
  %ga1115 = getelementptr i64, ptr %area, i64 10809
  %v1115 = load i8, ptr %ga1115
  %c233 = zext i8 %v1115 to i64
  %v1116 = icmp ne i64 %c233, 0
  br i1 %v1116, label %b760, label %b761
b759:
  %v1123 = call i64 @df.unbox_int(i64 %v0)
  %v1124 = load i8, ptr %ctx, align 8
  %c234 = zext i8 %v1124 to i64
  %v1125 = icmp ne i64 %c234, 0
  br i1 %v1125, label %b766, label %b765
b760:
  %ga1117 = getelementptr i64, ptr %area, i64 10808
  %v1117 = load i64, ptr %ga1117
  br label %b762
b761:
  %v1118 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b762
b762:
  %v1119 = phi i64 [ %v1117, %b760 ], [ %v1118, %b761 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1119, i64 0, i64 0)
  %v1121 = load i8, ptr %ctx, align 8
  %c235 = zext i8 %v1121 to i64
  %v1122 = icmp ne i64 %c235, 0
  br i1 %v1122, label %b764, label %b763
b763:
  br label %b759
b764:
  %gcvolta764 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta764, ptr %ctxtopo, align 8
  ret i64 0
b765:
  %v1126 = add i64 0, 1
  %v1127 = ashr i64 %v1123, %v1126
  store i64 0, ptr %gcs0
  %v1128 = call i64 @df.box_int(i64 %v1127)
  %xcp1129 = load i8, ptr %ctx, align 8
  %xcn1129 = icmp ne i8 %xcp1129, 0
  %xce1129 = call i1 @llvm.expect.i1(i1 %xcn1129, i1 false)
  br i1 %xce1129, label %xc1129.limpar, label %xc1129.fim
xc1129.limpar:
  call void @dartforge_exception_clear()
  br label %xc1129.fim
xc1129.fim:
  %gcvolta765 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta765, ptr %ctxtopo, align 8
  ret i64 %v1128
b766:
  %gcvolta766 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta766, ptr %ctxtopo, align 8
  ret i64 0
b768:
  store i64 0, ptr %gcs1
  %v1136 = call double @df.unbox_double(i64 %v1133)
  %v1137 = load i8, ptr %ctx, align 8
  %c236 = zext i8 %v1137 to i64
  %v1138 = icmp ne i64 %c236, 0
  br i1 %v1138, label %b771, label %b770
b769:
  %gcvolta769 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta769, ptr %ctxtopo, align 8
  ret i64 0
b770:
  %xcp1140 = load i8, ptr %ctx, align 8
  %xcn1140 = icmp ne i8 %xcp1140, 0
  %xce1140 = call i1 @llvm.expect.i1(i1 %xcn1140, i1 false)
  br i1 %xce1140, label %xc1140.limpar, label %xc1140.fim
xc1140.limpar:
  call void @dartforge_exception_clear()
  br label %xc1140.fim
xc1140.fim:
  %gcvolta770 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta770, ptr %ctxtopo, align 8
  ret i64 %v1133
b771:
  %gcvolta771 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta771, ptr %ctxtopo, align 8
  ret i64 0
b785:
  %v1434 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1435 = call i64 @df.box_int(i64 %v1434)
  store i64 %v1435, ptr %gcs1
  %sa1436_0 = getelementptr [2 x i64], ptr %sargs1436, i64 0, i64 0
  store i64 %v1435, ptr %sa1436_0
  %sat1436 = getelementptr [2 x i64], ptr %sargs1436, i64 0, i64 1
  store i64 0, ptr %sat1436
  %sic1436 = getelementptr i64, ptr %area, i64 141752
  %sf1436 = call ptr @df.selector(ptr %sic1436, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v1436 = call i64 %sf1436(i64 %v0, ptr %sargs1436, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1436, ptr %gcs0
  %v1437 = load i8, ptr %ctx, align 8
  %c237 = zext i8 %v1437 to i64
  %v1438 = icmp ne i64 %c237, 0
  br i1 %v1438, label %b961, label %b960
b786:
  store i64 0, ptr %gcs1
  %v1146 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1146, ptr %gcs1
  %v1147 = load i8, ptr %ctx, align 8
  %c238 = zext i8 %v1147 to i64
  %v1148 = icmp ne i64 %c238, 0
  br i1 %v1148, label %b792, label %b791
b787:
  %v1145 = icmp eq i64 %v1142, %v6
  br i1 %v1145, label %b789, label %b786
b788:
  %gcvolta788 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta788, ptr %ctxtopo, align 8
  ret i64 0
b789:
  %v1191 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1192 = call i64 @df.box_int(i64 %v1191)
  store i64 %v1192, ptr %gcs1
  %v1193 = icmp eq i64 %v0, 0
  %v1194 = icmp eq i64 %v1192, 0
  %c239 = zext i1 %v1193 to i64
  %c240 = zext i1 %v1194 to i64
  %v1195 = or i64 %c239, %c240
  %v1196 = icmp ne i64 %v1195, 0
  br i1 %v1196, label %b836, label %b837
b790:
  store i64 0, ptr %gcs1
  %v1150 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1150, ptr %gcs1
  %v1151 = load i8, ptr %ctx, align 8
  %c241 = zext i8 %v1151 to i64
  %v1152 = icmp ne i64 %c241, 0
  br i1 %v1152, label %b796, label %b795
b791:
  %v1149 = icmp eq i64 %v1146, %v6
  br i1 %v1149, label %b793, label %b790
b792:
  %gcvolta792 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta792, ptr %ctxtopo, align 8
  ret i64 0
b793:
  %v1211 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1212 = and i64 %v0, 1
  %v1213 = icmp ne i64 %v1212, 0
  br i1 %v1213, label %b847, label %b848
b794:
  store i64 0, ptr %gcs1
  %v1154 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1154, ptr %gcs1
  %v1155 = load i8, ptr %ctx, align 8
  %c242 = zext i8 %v1155 to i64
  %v1156 = icmp ne i64 %c242, 0
  br i1 %v1156, label %b800, label %b799
b795:
  %v1153 = icmp eq i64 %v1150, %v6
  br i1 %v1153, label %b797, label %b794
b796:
  %gcvolta796 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta796, ptr %ctxtopo, align 8
  ret i64 0
b797:
  %v1250 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1251 = and i64 %v0, 1
  %v1252 = icmp ne i64 %v1251, 0
  br i1 %v1252, label %b867, label %b868
b798:
  store i64 0, ptr %gcs1
  %v1158 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1158, ptr %gcs1
  %v1159 = load i8, ptr %ctx, align 8
  %c243 = zext i8 %v1159 to i64
  %v1160 = icmp ne i64 %c243, 0
  br i1 %v1160, label %b804, label %b803
b799:
  %v1157 = icmp eq i64 %v1154, %v6
  br i1 %v1157, label %b801, label %b798
b800:
  %gcvolta800 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta800, ptr %ctxtopo, align 8
  ret i64 0
b801:
  %v1289 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1290 = and i64 %v0, 1
  %v1291 = icmp ne i64 %v1290, 0
  br i1 %v1291, label %b887, label %b888
b802:
  store i64 0, ptr %gcs1
  %v1162 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1162, ptr %gcs1
  %v1163 = load i8, ptr %ctx, align 8
  %c244 = zext i8 %v1163 to i64
  %v1164 = icmp ne i64 %c244, 0
  br i1 %v1164, label %b808, label %b807
b803:
  %v1161 = icmp eq i64 %v1158, %v6
  br i1 %v1161, label %b805, label %b802
b804:
  %gcvolta804 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta804, ptr %ctxtopo, align 8
  ret i64 0
b805:
  %v1328 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1329 = and i64 %v0, 1
  %v1330 = icmp ne i64 %v1329, 0
  br i1 %v1330, label %b907, label %b908
b806:
  store i64 0, ptr %gcs1
  %v1166 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1166, ptr %gcs1
  %v1167 = load i8, ptr %ctx, align 8
  %c245 = zext i8 %v1167 to i64
  %v1168 = icmp ne i64 %c245, 0
  br i1 %v1168, label %b812, label %b811
b807:
  %v1165 = icmp eq i64 %v1162, %v6
  br i1 %v1165, label %b809, label %b806
b808:
  %gcvolta808 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta808, ptr %ctxtopo, align 8
  ret i64 0
b809:
  %v1367 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1368 = and i64 %v0, 1
  %v1369 = icmp ne i64 %v1368, 0
  br i1 %v1369, label %b927, label %b928
b810:
  store i64 0, ptr %gcs1
  %v1170 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1170, ptr %gcs1
  %v1171 = load i8, ptr %ctx, align 8
  %c246 = zext i8 %v1171 to i64
  %v1172 = icmp ne i64 %c246, 0
  br i1 %v1172, label %b816, label %b815
b811:
  %v1169 = icmp eq i64 %v1166, %v6
  br i1 %v1169, label %b813, label %b810
b812:
  %gcvolta812 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta812, ptr %ctxtopo, align 8
  ret i64 0
b813:
  %xcp1406 = load i8, ptr %ctx, align 8
  %xcn1406 = icmp ne i8 %xcp1406, 0
  %xce1406 = call i1 @llvm.expect.i1(i1 %xcn1406, i1 false)
  br i1 %xce1406, label %xc1406.limpar, label %xc1406.fim
xc1406.limpar:
  call void @dartforge_exception_clear()
  br label %xc1406.fim
xc1406.fim:
  %gcvolta813 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta813, ptr %ctxtopo, align 8
  ret i64 %v0
b814:
  store i64 0, ptr %gcs1
  %v1174 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1174, ptr %gcs1
  %v1175 = load i8, ptr %ctx, align 8
  %c247 = zext i8 %v1175 to i64
  %v1176 = icmp ne i64 %c247, 0
  br i1 %v1176, label %b820, label %b819
b815:
  %v1173 = icmp eq i64 %v1170, %v6
  br i1 %v1173, label %b817, label %b814
b816:
  %gcvolta816 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta816, ptr %ctxtopo, align 8
  ret i64 0
b817:
  %v1408 = add i64 0, 258
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1409 = and i64 %v0, 1
  %v1410 = icmp ne i64 %v1409, 0
  br i1 %v1410, label %b948, label %b949
b818:
  store i64 0, ptr %gcs1
  %v1178 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1178, ptr %gcs1
  %v1179 = load i8, ptr %ctx, align 8
  %c248 = zext i8 %v1179 to i64
  %v1180 = icmp ne i64 %c248, 0
  br i1 %v1180, label %b824, label %b823
b819:
  %v1177 = icmp eq i64 %v1174, %v6
  br i1 %v1177, label %b821, label %b818
b820:
  %gcvolta820 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta820, ptr %ctxtopo, align 8
  ret i64 0
b821:
  %v1421 = add i64 0, 16909320
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1422 = and i64 %v0, 1
  %v1423 = icmp ne i64 %v1422, 0
  br i1 %v1423, label %b954, label %b955
b822:
  store i64 0, ptr %gcs1
  %v1182 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1182, ptr %gcs1
  %v1183 = load i8, ptr %ctx, align 8
  %c249 = zext i8 %v1183 to i64
  %v1184 = icmp ne i64 %c249, 0
  br i1 %v1184, label %b829, label %b828
b823:
  %v1181 = icmp eq i64 %v1178, %v6
  br i1 %v1181, label %b825, label %b822
b824:
  %gcvolta824 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta824, ptr %ctxtopo, align 8
  ret i64 0
b825:
  br label %b785
b827:
  store i64 0, ptr %gcs1
  %v1186 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1186, ptr %gcs1
  %v1187 = load i8, ptr %ctx, align 8
  %c250 = zext i8 %v1187 to i64
  %v1188 = icmp ne i64 %c250, 0
  br i1 %v1188, label %b834, label %b833
b828:
  %v1185 = icmp eq i64 %v1182, %v6
  br i1 %v1185, label %b830, label %b827
b829:
  %gcvolta829 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta829, ptr %ctxtopo, align 8
  ret i64 0
b830:
  br label %b785
b832:
  br label %b12
b833:
  %v1189 = icmp eq i64 %v1186, %v6
  br i1 %v1189, label %b835, label %b832
b834:
  %gcvolta834 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta834, ptr %ctxtopo, align 8
  ret i64 0
b835:
  br label %b785
b836:
  %v1197 = icmp eq i64 %v0, %v1192
  br label %b838
b837:
  %sa1198_0 = getelementptr [2 x i64], ptr %sargs1198, i64 0, i64 0
  store i64 %v1192, ptr %sa1198_0
  %sat1198 = getelementptr [2 x i64], ptr %sargs1198, i64 0, i64 1
  store i64 0, ptr %sat1198
  %sic1198 = getelementptr i64, ptr %area, i64 141754
  %sf1198 = call ptr @df.selector(ptr %sic1198, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v1198 = call i64 %sf1198(i64 %v0, ptr %sargs1198, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1198, ptr %gcs0
  %v1199 = load i8, ptr %ctx, align 8
  %c251 = zext i8 %v1199 to i64
  %v1200 = icmp ne i64 %c251, 0
  br i1 %v1200, label %b840, label %b839
b838:
  %v1204 = phi i1 [ %v1197, %b836 ], [ %v1201, %b841 ]
  br i1 %v1204, label %b843, label %b844
b839:
  store i64 0, ptr %gcs1
  %v1201 = call i1 @df.unbox_bool(i64 %v1198)
  %v1202 = load i8, ptr %ctx, align 8
  %c252 = zext i8 %v1202 to i64
  %v1203 = icmp ne i64 %c252, 0
  br i1 %v1203, label %b842, label %b841
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
  %v1205 = add i64 0, 0
  br label %b845
b844:
  %v1206 = add i64 0, 1
  br label %b845
b845:
  %v1207 = phi i64 [ %v1205, %b843 ], [ %v1206, %b844 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1208 = call i64 @df.box_int(i64 %v1207)
  %xcp1209 = load i8, ptr %ctx, align 8
  %xcn1209 = icmp ne i8 %xcp1209, 0
  %xce1209 = call i1 @llvm.expect.i1(i1 %xcn1209, i1 false)
  br i1 %xce1209, label %xc1209.limpar, label %xc1209.fim
xc1209.limpar:
  call void @dartforge_exception_clear()
  br label %xc1209.fim
xc1209.fim:
  %gcvolta845 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta845, ptr %ctxtopo, align 8
  ret i64 %v1208
b847:
  %v1214 = ashr i64 %v0, 1
  %v1215 = icmp sle i64 %v1214, %v1211
  br label %b849
b848:
  %v1216 = call i64 @df.box_int(i64 %v1211)
  store i64 %v1216, ptr %gcs1
  %sa1217_0 = getelementptr [2 x i64], ptr %sargs1217, i64 0, i64 0
  store i64 %v1216, ptr %sa1217_0
  %sat1217 = getelementptr [2 x i64], ptr %sargs1217, i64 0, i64 1
  store i64 0, ptr %sat1217
  %sic1217 = getelementptr i64, ptr %area, i64 141756
  %sf1217 = call ptr @df.selector(ptr %sic1217, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1217 = call i64 %sf1217(i64 %v0, ptr %sargs1217, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1217, ptr %gcs1
  %v1218 = load i8, ptr %ctx, align 8
  %c253 = zext i8 %v1218 to i64
  %v1219 = icmp ne i64 %c253, 0
  br i1 %v1219, label %b851, label %b850
b849:
  %v1223 = phi i1 [ %v1215, %b847 ], [ %v1220, %b852 ]
  br i1 %v1223, label %b854, label %b855
b850:
  %v1220 = call i1 @df.unbox_bool(i64 %v1217)
  %v1221 = load i8, ptr %ctx, align 8
  %c254 = zext i8 %v1221 to i64
  %v1222 = icmp ne i64 %c254, 0
  br i1 %v1222, label %b853, label %b852
b851:
  %gcvolta851 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta851, ptr %ctxtopo, align 8
  ret i64 0
b852:
  br label %b849
b853:
  %gcvolta853 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta853, ptr %ctxtopo, align 8
  ret i64 0
b854:
  %v1224 = add i64 0, 0
  br label %b856
b855:
  store i64 0, ptr %gcs1
  %v1226 = icmp eq i64 %v0, 0
  %v1227 = call i64 @df.class(i64 %v0)
  %v1228 = call i8 @df.subclass(i64 %v1227, i64 674)
  %c255 = zext i8 %v1228 to i64
  %v1229 = icmp ne i64 %c255, 0
  %v1230 = xor i1 %v1226, true
  %c256 = zext i1 %v1229 to i64
  %c257 = zext i1 %v1230 to i64
  %v1231 = and i64 %c256, %c257
  %v1232 = icmp ne i64 %v1231, 0
  br i1 %v1232, label %b858, label %b857
b856:
  %v1246 = phi i64 [ %v1224, %b854 ], [ %v1245, %b864 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1247 = call i64 @df.box_int(i64 %v1246)
  %xcp1248 = load i8, ptr %ctx, align 8
  %xcn1248 = icmp ne i8 %xcp1248, 0
  %xce1248 = call i1 @llvm.expect.i1(i1 %xcn1248, i1 false)
  br i1 %xce1248, label %xc1248.limpar, label %xc1248.fim
xc1248.limpar:
  call void @dartforge_exception_clear()
  br label %xc1248.fim
xc1248.fim:
  %gcvolta856 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta856, ptr %ctxtopo, align 8
  ret i64 %v1247
b857:
  %ga1233 = getelementptr i64, ptr %area, i64 10809
  %v1233 = load i8, ptr %ga1233
  %c258 = zext i8 %v1233 to i64
  %v1234 = icmp ne i64 %c258, 0
  br i1 %v1234, label %b859, label %b860
b858:
  %v1241 = call i64 @df.unbox_int(i64 %v0)
  %v1242 = load i8, ptr %ctx, align 8
  %c259 = zext i8 %v1242 to i64
  %v1243 = icmp ne i64 %c259, 0
  br i1 %v1243, label %b865, label %b864
b859:
  %ga1235 = getelementptr i64, ptr %area, i64 10808
  %v1235 = load i64, ptr %ga1235
  br label %b861
b860:
  %v1236 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b861
b861:
  %v1237 = phi i64 [ %v1235, %b859 ], [ %v1236, %b860 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1237, i64 0, i64 0)
  %v1239 = load i8, ptr %ctx, align 8
  %c260 = zext i8 %v1239 to i64
  %v1240 = icmp ne i64 %c260, 0
  br i1 %v1240, label %b863, label %b862
b862:
  br label %b858
b863:
  %gcvolta863 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta863, ptr %ctxtopo, align 8
  ret i64 0
b864:
  %v1244 = add i64 0, 5
  %v1245 = ashr i64 %v1241, %v1244
  br label %b856
b865:
  %gcvolta865 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta865, ptr %ctxtopo, align 8
  ret i64 0
b867:
  %v1253 = ashr i64 %v0, 1
  %v1254 = icmp sle i64 %v1253, %v1250
  br label %b869
b868:
  %v1255 = call i64 @df.box_int(i64 %v1250)
  store i64 %v1255, ptr %gcs1
  %sa1256_0 = getelementptr [2 x i64], ptr %sargs1256, i64 0, i64 0
  store i64 %v1255, ptr %sa1256_0
  %sat1256 = getelementptr [2 x i64], ptr %sargs1256, i64 0, i64 1
  store i64 0, ptr %sat1256
  %sic1256 = getelementptr i64, ptr %area, i64 141758
  %sf1256 = call ptr @df.selector(ptr %sic1256, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1256 = call i64 %sf1256(i64 %v0, ptr %sargs1256, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1256, ptr %gcs1
  %v1257 = load i8, ptr %ctx, align 8
  %c261 = zext i8 %v1257 to i64
  %v1258 = icmp ne i64 %c261, 0
  br i1 %v1258, label %b871, label %b870
b869:
  %v1262 = phi i1 [ %v1254, %b867 ], [ %v1259, %b872 ]
  br i1 %v1262, label %b874, label %b875
b870:
  %v1259 = call i1 @df.unbox_bool(i64 %v1256)
  %v1260 = load i8, ptr %ctx, align 8
  %c262 = zext i8 %v1260 to i64
  %v1261 = icmp ne i64 %c262, 0
  br i1 %v1261, label %b873, label %b872
b871:
  %gcvolta871 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta871, ptr %ctxtopo, align 8
  ret i64 0
b872:
  br label %b869
b873:
  %gcvolta873 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta873, ptr %ctxtopo, align 8
  ret i64 0
b874:
  %v1263 = add i64 0, 0
  br label %b876
b875:
  store i64 0, ptr %gcs1
  %v1265 = icmp eq i64 %v0, 0
  %v1266 = call i64 @df.class(i64 %v0)
  %v1267 = call i8 @df.subclass(i64 %v1266, i64 674)
  %c263 = zext i8 %v1267 to i64
  %v1268 = icmp ne i64 %c263, 0
  %v1269 = xor i1 %v1265, true
  %c264 = zext i1 %v1268 to i64
  %c265 = zext i1 %v1269 to i64
  %v1270 = and i64 %c264, %c265
  %v1271 = icmp ne i64 %v1270, 0
  br i1 %v1271, label %b878, label %b877
b876:
  %v1285 = phi i64 [ %v1263, %b874 ], [ %v1284, %b884 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1286 = call i64 @df.box_int(i64 %v1285)
  %xcp1287 = load i8, ptr %ctx, align 8
  %xcn1287 = icmp ne i8 %xcp1287, 0
  %xce1287 = call i1 @llvm.expect.i1(i1 %xcn1287, i1 false)
  br i1 %xce1287, label %xc1287.limpar, label %xc1287.fim
xc1287.limpar:
  call void @dartforge_exception_clear()
  br label %xc1287.fim
xc1287.fim:
  %gcvolta876 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta876, ptr %ctxtopo, align 8
  ret i64 %v1286
b877:
  %ga1272 = getelementptr i64, ptr %area, i64 10809
  %v1272 = load i8, ptr %ga1272
  %c266 = zext i8 %v1272 to i64
  %v1273 = icmp ne i64 %c266, 0
  br i1 %v1273, label %b879, label %b880
b878:
  %v1280 = call i64 @df.unbox_int(i64 %v0)
  %v1281 = load i8, ptr %ctx, align 8
  %c267 = zext i8 %v1281 to i64
  %v1282 = icmp ne i64 %c267, 0
  br i1 %v1282, label %b885, label %b884
b879:
  %ga1274 = getelementptr i64, ptr %area, i64 10808
  %v1274 = load i64, ptr %ga1274
  br label %b881
b880:
  %v1275 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b881
b881:
  %v1276 = phi i64 [ %v1274, %b879 ], [ %v1275, %b880 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1276, i64 0, i64 0)
  %v1278 = load i8, ptr %ctx, align 8
  %c268 = zext i8 %v1278 to i64
  %v1279 = icmp ne i64 %c268, 0
  br i1 %v1279, label %b883, label %b882
b882:
  br label %b878
b883:
  %gcvolta883 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta883, ptr %ctxtopo, align 8
  ret i64 0
b884:
  %v1283 = add i64 0, 3
  %v1284 = ashr i64 %v1280, %v1283
  br label %b876
b885:
  %gcvolta885 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta885, ptr %ctxtopo, align 8
  ret i64 0
b887:
  %v1292 = ashr i64 %v0, 1
  %v1293 = icmp sle i64 %v1292, %v1289
  br label %b889
b888:
  %v1294 = call i64 @df.box_int(i64 %v1289)
  store i64 %v1294, ptr %gcs1
  %sa1295_0 = getelementptr [2 x i64], ptr %sargs1295, i64 0, i64 0
  store i64 %v1294, ptr %sa1295_0
  %sat1295 = getelementptr [2 x i64], ptr %sargs1295, i64 0, i64 1
  store i64 0, ptr %sat1295
  %sic1295 = getelementptr i64, ptr %area, i64 141760
  %sf1295 = call ptr @df.selector(ptr %sic1295, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1295 = call i64 %sf1295(i64 %v0, ptr %sargs1295, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1295, ptr %gcs1
  %v1296 = load i8, ptr %ctx, align 8
  %c269 = zext i8 %v1296 to i64
  %v1297 = icmp ne i64 %c269, 0
  br i1 %v1297, label %b891, label %b890
b889:
  %v1301 = phi i1 [ %v1293, %b887 ], [ %v1298, %b892 ]
  br i1 %v1301, label %b894, label %b895
b890:
  %v1298 = call i1 @df.unbox_bool(i64 %v1295)
  %v1299 = load i8, ptr %ctx, align 8
  %c270 = zext i8 %v1299 to i64
  %v1300 = icmp ne i64 %c270, 0
  br i1 %v1300, label %b893, label %b892
b891:
  %gcvolta891 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta891, ptr %ctxtopo, align 8
  ret i64 0
b892:
  br label %b889
b893:
  %gcvolta893 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta893, ptr %ctxtopo, align 8
  ret i64 0
b894:
  %v1302 = add i64 0, 0
  br label %b896
b895:
  store i64 0, ptr %gcs1
  %v1304 = icmp eq i64 %v0, 0
  %v1305 = call i64 @df.class(i64 %v0)
  %v1306 = call i8 @df.subclass(i64 %v1305, i64 674)
  %c271 = zext i8 %v1306 to i64
  %v1307 = icmp ne i64 %c271, 0
  %v1308 = xor i1 %v1304, true
  %c272 = zext i1 %v1307 to i64
  %c273 = zext i1 %v1308 to i64
  %v1309 = and i64 %c272, %c273
  %v1310 = icmp ne i64 %v1309, 0
  br i1 %v1310, label %b898, label %b897
b896:
  %v1324 = phi i64 [ %v1302, %b894 ], [ %v1323, %b904 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1325 = call i64 @df.box_int(i64 %v1324)
  %xcp1326 = load i8, ptr %ctx, align 8
  %xcn1326 = icmp ne i8 %xcp1326, 0
  %xce1326 = call i1 @llvm.expect.i1(i1 %xcn1326, i1 false)
  br i1 %xce1326, label %xc1326.limpar, label %xc1326.fim
xc1326.limpar:
  call void @dartforge_exception_clear()
  br label %xc1326.fim
xc1326.fim:
  %gcvolta896 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta896, ptr %ctxtopo, align 8
  ret i64 %v1325
b897:
  %ga1311 = getelementptr i64, ptr %area, i64 10809
  %v1311 = load i8, ptr %ga1311
  %c274 = zext i8 %v1311 to i64
  %v1312 = icmp ne i64 %c274, 0
  br i1 %v1312, label %b899, label %b900
b898:
  %v1319 = call i64 @df.unbox_int(i64 %v0)
  %v1320 = load i8, ptr %ctx, align 8
  %c275 = zext i8 %v1320 to i64
  %v1321 = icmp ne i64 %c275, 0
  br i1 %v1321, label %b905, label %b904
b899:
  %ga1313 = getelementptr i64, ptr %area, i64 10808
  %v1313 = load i64, ptr %ga1313
  br label %b901
b900:
  %v1314 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b901
b901:
  %v1315 = phi i64 [ %v1313, %b899 ], [ %v1314, %b900 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1315, i64 0, i64 0)
  %v1317 = load i8, ptr %ctx, align 8
  %c276 = zext i8 %v1317 to i64
  %v1318 = icmp ne i64 %c276, 0
  br i1 %v1318, label %b903, label %b902
b902:
  br label %b898
b903:
  %gcvolta903 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta903, ptr %ctxtopo, align 8
  ret i64 0
b904:
  %v1322 = add i64 0, 1
  %v1323 = shl i64 %v1319, %v1322
  br label %b896
b905:
  %gcvolta905 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta905, ptr %ctxtopo, align 8
  ret i64 0
b907:
  %v1331 = ashr i64 %v0, 1
  %v1332 = icmp sle i64 %v1331, %v1328
  br label %b909
b908:
  %v1333 = call i64 @df.box_int(i64 %v1328)
  store i64 %v1333, ptr %gcs1
  %sa1334_0 = getelementptr [2 x i64], ptr %sargs1334, i64 0, i64 0
  store i64 %v1333, ptr %sa1334_0
  %sat1334 = getelementptr [2 x i64], ptr %sargs1334, i64 0, i64 1
  store i64 0, ptr %sat1334
  %sic1334 = getelementptr i64, ptr %area, i64 141762
  %sf1334 = call ptr @df.selector(ptr %sic1334, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1334 = call i64 %sf1334(i64 %v0, ptr %sargs1334, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1334, ptr %gcs1
  %v1335 = load i8, ptr %ctx, align 8
  %c277 = zext i8 %v1335 to i64
  %v1336 = icmp ne i64 %c277, 0
  br i1 %v1336, label %b911, label %b910
b909:
  %v1340 = phi i1 [ %v1332, %b907 ], [ %v1337, %b912 ]
  br i1 %v1340, label %b914, label %b915
b910:
  %v1337 = call i1 @df.unbox_bool(i64 %v1334)
  %v1338 = load i8, ptr %ctx, align 8
  %c278 = zext i8 %v1338 to i64
  %v1339 = icmp ne i64 %c278, 0
  br i1 %v1339, label %b913, label %b912
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
  %v1341 = add i64 0, 0
  br label %b916
b915:
  store i64 0, ptr %gcs1
  %v1343 = icmp eq i64 %v0, 0
  %v1344 = call i64 @df.class(i64 %v0)
  %v1345 = call i8 @df.subclass(i64 %v1344, i64 674)
  %c279 = zext i8 %v1345 to i64
  %v1346 = icmp ne i64 %c279, 0
  %v1347 = xor i1 %v1343, true
  %c280 = zext i1 %v1346 to i64
  %c281 = zext i1 %v1347 to i64
  %v1348 = and i64 %c280, %c281
  %v1349 = icmp ne i64 %v1348, 0
  br i1 %v1349, label %b918, label %b917
b916:
  %v1363 = phi i64 [ %v1341, %b914 ], [ %v1362, %b924 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1364 = call i64 @df.box_int(i64 %v1363)
  %xcp1365 = load i8, ptr %ctx, align 8
  %xcn1365 = icmp ne i8 %xcp1365, 0
  %xce1365 = call i1 @llvm.expect.i1(i1 %xcn1365, i1 false)
  br i1 %xce1365, label %xc1365.limpar, label %xc1365.fim
xc1365.limpar:
  call void @dartforge_exception_clear()
  br label %xc1365.fim
xc1365.fim:
  %gcvolta916 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta916, ptr %ctxtopo, align 8
  ret i64 %v1364
b917:
  %ga1350 = getelementptr i64, ptr %area, i64 10809
  %v1350 = load i8, ptr %ga1350
  %c282 = zext i8 %v1350 to i64
  %v1351 = icmp ne i64 %c282, 0
  br i1 %v1351, label %b919, label %b920
b918:
  %v1358 = call i64 @df.unbox_int(i64 %v0)
  %v1359 = load i8, ptr %ctx, align 8
  %c283 = zext i8 %v1359 to i64
  %v1360 = icmp ne i64 %c283, 0
  br i1 %v1360, label %b925, label %b924
b919:
  %ga1352 = getelementptr i64, ptr %area, i64 10808
  %v1352 = load i64, ptr %ga1352
  br label %b921
b920:
  %v1353 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b921
b921:
  %v1354 = phi i64 [ %v1352, %b919 ], [ %v1353, %b920 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1354, i64 0, i64 0)
  %v1356 = load i8, ptr %ctx, align 8
  %c284 = zext i8 %v1356 to i64
  %v1357 = icmp ne i64 %c284, 0
  br i1 %v1357, label %b923, label %b922
b922:
  br label %b918
b923:
  %gcvolta923 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta923, ptr %ctxtopo, align 8
  ret i64 0
b924:
  %v1361 = add i64 0, 516
  %v1362 = mul i64 %v1358, %v1361
  br label %b916
b925:
  %gcvolta925 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta925, ptr %ctxtopo, align 8
  ret i64 0
b927:
  %v1370 = ashr i64 %v0, 1
  %v1371 = icmp sle i64 %v1370, %v1367
  br label %b929
b928:
  %v1372 = call i64 @df.box_int(i64 %v1367)
  store i64 %v1372, ptr %gcs1
  %sa1373_0 = getelementptr [2 x i64], ptr %sargs1373, i64 0, i64 0
  store i64 %v1372, ptr %sa1373_0
  %sat1373 = getelementptr [2 x i64], ptr %sargs1373, i64 0, i64 1
  store i64 0, ptr %sat1373
  %sic1373 = getelementptr i64, ptr %area, i64 141764
  %sf1373 = call ptr @df.selector(ptr %sic1373, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1373 = call i64 %sf1373(i64 %v0, ptr %sargs1373, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1373, ptr %gcs1
  %v1374 = load i8, ptr %ctx, align 8
  %c285 = zext i8 %v1374 to i64
  %v1375 = icmp ne i64 %c285, 0
  br i1 %v1375, label %b931, label %b930
b929:
  %v1379 = phi i1 [ %v1371, %b927 ], [ %v1376, %b932 ]
  br i1 %v1379, label %b934, label %b935
b930:
  %v1376 = call i1 @df.unbox_bool(i64 %v1373)
  %v1377 = load i8, ptr %ctx, align 8
  %c286 = zext i8 %v1377 to i64
  %v1378 = icmp ne i64 %c286, 0
  br i1 %v1378, label %b933, label %b932
b931:
  %gcvolta931 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta931, ptr %ctxtopo, align 8
  ret i64 0
b932:
  br label %b929
b933:
  %gcvolta933 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta933, ptr %ctxtopo, align 8
  ret i64 0
b934:
  %v1380 = add i64 0, 0
  br label %b936
b935:
  store i64 0, ptr %gcs1
  %v1382 = icmp eq i64 %v0, 0
  %v1383 = call i64 @df.class(i64 %v0)
  %v1384 = call i8 @df.subclass(i64 %v1383, i64 674)
  %c287 = zext i8 %v1384 to i64
  %v1385 = icmp ne i64 %c287, 0
  %v1386 = xor i1 %v1382, true
  %c288 = zext i1 %v1385 to i64
  %c289 = zext i1 %v1386 to i64
  %v1387 = and i64 %c288, %c289
  %v1388 = icmp ne i64 %v1387, 0
  br i1 %v1388, label %b938, label %b937
b936:
  %v1402 = phi i64 [ %v1380, %b934 ], [ %v1401, %b944 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1403 = call i64 @df.box_int(i64 %v1402)
  %xcp1404 = load i8, ptr %ctx, align 8
  %xcn1404 = icmp ne i8 %xcp1404, 0
  %xce1404 = call i1 @llvm.expect.i1(i1 %xcn1404, i1 false)
  br i1 %xce1404, label %xc1404.limpar, label %xc1404.fim
xc1404.limpar:
  call void @dartforge_exception_clear()
  br label %xc1404.fim
xc1404.fim:
  %gcvolta936 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta936, ptr %ctxtopo, align 8
  ret i64 %v1403
b937:
  %ga1389 = getelementptr i64, ptr %area, i64 10809
  %v1389 = load i8, ptr %ga1389
  %c290 = zext i8 %v1389 to i64
  %v1390 = icmp ne i64 %c290, 0
  br i1 %v1390, label %b939, label %b940
b938:
  %v1397 = call i64 @df.unbox_int(i64 %v0)
  %v1398 = load i8, ptr %ctx, align 8
  %c291 = zext i8 %v1398 to i64
  %v1399 = icmp ne i64 %c291, 0
  br i1 %v1399, label %b945, label %b944
b939:
  %ga1391 = getelementptr i64, ptr %area, i64 10808
  %v1391 = load i64, ptr %ga1391
  br label %b941
b940:
  %v1392 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b941
b941:
  %v1393 = phi i64 [ %v1391, %b939 ], [ %v1392, %b940 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1393, i64 0, i64 0)
  %v1395 = load i8, ptr %ctx, align 8
  %c292 = zext i8 %v1395 to i64
  %v1396 = icmp ne i64 %c292, 0
  br i1 %v1396, label %b943, label %b942
b942:
  br label %b938
b943:
  %gcvolta943 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta943, ptr %ctxtopo, align 8
  ret i64 0
b944:
  %v1400 = add i64 0, 33818640
  %v1401 = mul i64 %v1397, %v1400
  br label %b936
b945:
  %gcvolta945 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta945, ptr %ctxtopo, align 8
  ret i64 0
b948:
  %v1411 = ashr i64 %v0, 1
  %v1412 = mul i64 %v1411, %v1408
  store i64 0, ptr %gcs0
  %v1413 = call i64 @df.box_int(i64 %v1412)
  br label %b950
b949:
  %v1414 = call i64 @df.box_int(i64 %v1408)
  store i64 %v1414, ptr %gcs1
  %sa1415_0 = getelementptr [2 x i64], ptr %sargs1415, i64 0, i64 0
  store i64 %v1414, ptr %sa1415_0
  %sat1415 = getelementptr [2 x i64], ptr %sargs1415, i64 0, i64 1
  store i64 0, ptr %sat1415
  %sic1415 = getelementptr i64, ptr %area, i64 141766
  %sf1415 = call ptr @df.selector(ptr %sic1415, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v1415 = call i64 %sf1415(i64 %v0, ptr %sargs1415, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1416 = load i8, ptr %ctx, align 8
  %c293 = zext i8 %v1416 to i64
  %v1417 = icmp ne i64 %c293, 0
  br i1 %v1417, label %b952, label %b951
b950:
  %v1418 = phi i64 [ %v1413, %b948 ], [ %v1415, %b951 ]
  %xcp1419 = load i8, ptr %ctx, align 8
  %xcn1419 = icmp ne i8 %xcp1419, 0
  %xce1419 = call i1 @llvm.expect.i1(i1 %xcn1419, i1 false)
  br i1 %xce1419, label %xc1419.limpar, label %xc1419.fim
xc1419.limpar:
  call void @dartforge_exception_clear()
  br label %xc1419.fim
xc1419.fim:
  %gcvolta950 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta950, ptr %ctxtopo, align 8
  ret i64 %v1418
b951:
  br label %b950
b952:
  %gcvolta952 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta952, ptr %ctxtopo, align 8
  ret i64 0
b954:
  %v1424 = ashr i64 %v0, 1
  %v1425 = mul i64 %v1424, %v1421
  store i64 0, ptr %gcs0
  %v1426 = call i64 @df.box_int(i64 %v1425)
  br label %b956
b955:
  %v1427 = call i64 @df.box_int(i64 %v1421)
  store i64 %v1427, ptr %gcs1
  %sa1428_0 = getelementptr [2 x i64], ptr %sargs1428, i64 0, i64 0
  store i64 %v1427, ptr %sa1428_0
  %sat1428 = getelementptr [2 x i64], ptr %sargs1428, i64 0, i64 1
  store i64 0, ptr %sat1428
  %sic1428 = getelementptr i64, ptr %area, i64 141768
  %sf1428 = call ptr @df.selector(ptr %sic1428, i64 %v0, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v1428 = call i64 %sf1428(i64 %v0, ptr %sargs1428, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v1429 = load i8, ptr %ctx, align 8
  %c294 = zext i8 %v1429 to i64
  %v1430 = icmp ne i64 %c294, 0
  br i1 %v1430, label %b958, label %b957
b956:
  %v1431 = phi i64 [ %v1426, %b954 ], [ %v1428, %b957 ]
  %xcp1432 = load i8, ptr %ctx, align 8
  %xcn1432 = icmp ne i8 %xcp1432, 0
  %xce1432 = call i1 @llvm.expect.i1(i1 %xcn1432, i1 false)
  br i1 %xce1432, label %xc1432.limpar, label %xc1432.fim
xc1432.limpar:
  call void @dartforge_exception_clear()
  br label %xc1432.fim
xc1432.fim:
  %gcvolta956 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta956, ptr %ctxtopo, align 8
  ret i64 %v1431
b957:
  br label %b956
b958:
  %gcvolta958 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta958, ptr %ctxtopo, align 8
  ret i64 0
b960:
  store i64 0, ptr %gcs1
  %v1439 = call double @df.unbox_double(i64 %v1436)
  %v1440 = load i8, ptr %ctx, align 8
  %c295 = zext i8 %v1440 to i64
  %v1441 = icmp ne i64 %c295, 0
  br i1 %v1441, label %b963, label %b962
b961:
  %gcvolta961 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta961, ptr %ctxtopo, align 8
  ret i64 0
b962:
  %xcp1443 = load i8, ptr %ctx, align 8
  %xcn1443 = icmp ne i8 %xcp1443, 0
  %xce1443 = call i1 @llvm.expect.i1(i1 %xcn1443, i1 false)
  br i1 %xce1443, label %xc1443.limpar, label %xc1443.fim
xc1443.limpar:
  call void @dartforge_exception_clear()
  br label %xc1443.fim
xc1443.fim:
  %gcvolta962 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta962, ptr %ctxtopo, align 8
  ret i64 %v1436
b963:
  %gcvolta963 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta963, ptr %ctxtopo, align 8
  ret i64 0
b977:
  %v1757 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1758 = call i64 @df.box_int(i64 %v1757)
  store i64 %v1758, ptr %gcs1
  %sa1759_0 = getelementptr [2 x i64], ptr %sargs1759, i64 0, i64 0
  store i64 %v1758, ptr %sa1759_0
  %sat1759 = getelementptr [2 x i64], ptr %sargs1759, i64 0, i64 1
  store i64 0, ptr %sat1759
  %sic1759 = getelementptr i64, ptr %area, i64 141770
  %sf1759 = call ptr @df.selector(ptr %sic1759, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v1759 = call i64 %sf1759(i64 %v0, ptr %sargs1759, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1759, ptr %gcs0
  %v1760 = load i8, ptr %ctx, align 8
  %c296 = zext i8 %v1760 to i64
  %v1761 = icmp ne i64 %c296, 0
  br i1 %v1761, label %b1161, label %b1160
b978:
  store i64 0, ptr %gcs1
  %v1449 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1449, ptr %gcs1
  %v1450 = load i8, ptr %ctx, align 8
  %c297 = zext i8 %v1450 to i64
  %v1451 = icmp ne i64 %c297, 0
  br i1 %v1451, label %b984, label %b983
b979:
  %v1448 = icmp eq i64 %v1445, %v6
  br i1 %v1448, label %b981, label %b978
b980:
  %gcvolta980 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta980, ptr %ctxtopo, align 8
  ret i64 0
b981:
  %v1494 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1495 = call i64 @df.box_int(i64 %v1494)
  store i64 %v1495, ptr %gcs1
  %v1496 = icmp eq i64 %v0, 0
  %v1497 = icmp eq i64 %v1495, 0
  %c298 = zext i1 %v1496 to i64
  %c299 = zext i1 %v1497 to i64
  %v1498 = or i64 %c298, %c299
  %v1499 = icmp ne i64 %v1498, 0
  br i1 %v1499, label %b1028, label %b1029
b982:
  store i64 0, ptr %gcs1
  %v1453 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1453, ptr %gcs1
  %v1454 = load i8, ptr %ctx, align 8
  %c300 = zext i8 %v1454 to i64
  %v1455 = icmp ne i64 %c300, 0
  br i1 %v1455, label %b988, label %b987
b983:
  %v1452 = icmp eq i64 %v1449, %v6
  br i1 %v1452, label %b985, label %b982
b984:
  %gcvolta984 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta984, ptr %ctxtopo, align 8
  ret i64 0
b985:
  %v1514 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1515 = and i64 %v0, 1
  %v1516 = icmp ne i64 %v1515, 0
  br i1 %v1516, label %b1039, label %b1040
b986:
  store i64 0, ptr %gcs1
  %v1457 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1457, ptr %gcs1
  %v1458 = load i8, ptr %ctx, align 8
  %c301 = zext i8 %v1458 to i64
  %v1459 = icmp ne i64 %c301, 0
  br i1 %v1459, label %b992, label %b991
b987:
  %v1456 = icmp eq i64 %v1453, %v6
  br i1 %v1456, label %b989, label %b986
b988:
  %gcvolta988 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta988, ptr %ctxtopo, align 8
  ret i64 0
b989:
  %v1553 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1554 = and i64 %v0, 1
  %v1555 = icmp ne i64 %v1554, 0
  br i1 %v1555, label %b1059, label %b1060
b990:
  store i64 0, ptr %gcs1
  %v1461 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1461, ptr %gcs1
  %v1462 = load i8, ptr %ctx, align 8
  %c302 = zext i8 %v1462 to i64
  %v1463 = icmp ne i64 %c302, 0
  br i1 %v1463, label %b996, label %b995
b991:
  %v1460 = icmp eq i64 %v1457, %v6
  br i1 %v1460, label %b993, label %b990
b992:
  %gcvolta992 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta992, ptr %ctxtopo, align 8
  ret i64 0
b993:
  %v1592 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1593 = and i64 %v0, 1
  %v1594 = icmp ne i64 %v1593, 0
  br i1 %v1594, label %b1079, label %b1080
b994:
  store i64 0, ptr %gcs1
  %v1465 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1465, ptr %gcs1
  %v1466 = load i8, ptr %ctx, align 8
  %c303 = zext i8 %v1466 to i64
  %v1467 = icmp ne i64 %c303, 0
  br i1 %v1467, label %b1000, label %b999
b995:
  %v1464 = icmp eq i64 %v1461, %v6
  br i1 %v1464, label %b997, label %b994
b996:
  %gcvolta996 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta996, ptr %ctxtopo, align 8
  ret i64 0
b997:
  %v1631 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1632 = and i64 %v0, 1
  %v1633 = icmp ne i64 %v1632, 0
  br i1 %v1633, label %b1099, label %b1100
b998:
  store i64 0, ptr %gcs1
  %v1469 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1469, ptr %gcs1
  %v1470 = load i8, ptr %ctx, align 8
  %c304 = zext i8 %v1470 to i64
  %v1471 = icmp ne i64 %c304, 0
  br i1 %v1471, label %b1004, label %b1003
b999:
  %v1468 = icmp eq i64 %v1465, %v6
  br i1 %v1468, label %b1001, label %b998
b1000:
  %gcvolta1000 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1000, ptr %ctxtopo, align 8
  ret i64 0
b1001:
  %v1670 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1671 = and i64 %v0, 1
  %v1672 = icmp ne i64 %v1671, 0
  br i1 %v1672, label %b1119, label %b1120
b1002:
  store i64 0, ptr %gcs1
  %v1473 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1473, ptr %gcs1
  %v1474 = load i8, ptr %ctx, align 8
  %c305 = zext i8 %v1474 to i64
  %v1475 = icmp ne i64 %c305, 0
  br i1 %v1475, label %b1008, label %b1007
b1003:
  %v1472 = icmp eq i64 %v1469, %v6
  br i1 %v1472, label %b1005, label %b1002
b1004:
  %gcvolta1004 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1004, ptr %ctxtopo, align 8
  ret i64 0
b1005:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1709 = icmp eq i64 %v0, 0
  %v1710 = call i64 @df.class(i64 %v0)
  %v1711 = call i8 @df.subclass(i64 %v1710, i64 674)
  %c306 = zext i8 %v1711 to i64
  %v1712 = icmp ne i64 %c306, 0
  %v1713 = xor i1 %v1709, true
  %c307 = zext i1 %v1712 to i64
  %c308 = zext i1 %v1713 to i64
  %v1714 = and i64 %c307, %c308
  %v1715 = icmp ne i64 %v1714, 0
  br i1 %v1715, label %b1140, label %b1139
b1006:
  store i64 0, ptr %gcs1
  %v1477 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1477, ptr %gcs1
  %v1478 = load i8, ptr %ctx, align 8
  %c309 = zext i8 %v1478 to i64
  %v1479 = icmp ne i64 %c309, 0
  br i1 %v1479, label %b1012, label %b1011
b1007:
  %v1476 = icmp eq i64 %v1473, %v6
  br i1 %v1476, label %b1009, label %b1006
b1008:
  %gcvolta1008 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1008, ptr %ctxtopo, align 8
  ret i64 0
b1009:
  %xcp1732 = load i8, ptr %ctx, align 8
  %xcn1732 = icmp ne i8 %xcp1732, 0
  %xce1732 = call i1 @llvm.expect.i1(i1 %xcn1732, i1 false)
  br i1 %xce1732, label %xc1732.limpar, label %xc1732.fim
xc1732.limpar:
  call void @dartforge_exception_clear()
  br label %xc1732.fim
xc1732.fim:
  %gcvolta1009 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1009, ptr %ctxtopo, align 8
  ret i64 %v0
b1010:
  store i64 0, ptr %gcs1
  %v1481 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1481, ptr %gcs1
  %v1482 = load i8, ptr %ctx, align 8
  %c310 = zext i8 %v1482 to i64
  %v1483 = icmp ne i64 %c310, 0
  br i1 %v1483, label %b1016, label %b1015
b1011:
  %v1480 = icmp eq i64 %v1477, %v6
  br i1 %v1480, label %b1013, label %b1010
b1012:
  %gcvolta1012 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1012, ptr %ctxtopo, align 8
  ret i64 0
b1013:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1734 = icmp eq i64 %v0, 0
  %v1735 = call i64 @df.class(i64 %v0)
  %v1736 = call i8 @df.subclass(i64 %v1735, i64 674)
  %c311 = zext i8 %v1736 to i64
  %v1737 = icmp ne i64 %c311, 0
  %v1738 = xor i1 %v1734, true
  %c312 = zext i1 %v1737 to i64
  %c313 = zext i1 %v1738 to i64
  %v1739 = and i64 %c312, %c313
  %v1740 = icmp ne i64 %v1739, 0
  br i1 %v1740, label %b1151, label %b1150
b1014:
  store i64 0, ptr %gcs1
  %v1485 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1485, ptr %gcs1
  %v1486 = load i8, ptr %ctx, align 8
  %c314 = zext i8 %v1486 to i64
  %v1487 = icmp ne i64 %c314, 0
  br i1 %v1487, label %b1021, label %b1020
b1015:
  %v1484 = icmp eq i64 %v1481, %v6
  br i1 %v1484, label %b1017, label %b1014
b1016:
  %gcvolta1016 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1016, ptr %ctxtopo, align 8
  ret i64 0
b1017:
  br label %b977
b1019:
  store i64 0, ptr %gcs1
  %v1489 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1489, ptr %gcs1
  %v1490 = load i8, ptr %ctx, align 8
  %c315 = zext i8 %v1490 to i64
  %v1491 = icmp ne i64 %c315, 0
  br i1 %v1491, label %b1026, label %b1025
b1020:
  %v1488 = icmp eq i64 %v1485, %v6
  br i1 %v1488, label %b1022, label %b1019
b1021:
  %gcvolta1021 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1021, ptr %ctxtopo, align 8
  ret i64 0
b1022:
  br label %b977
b1024:
  br label %b12
b1025:
  %v1492 = icmp eq i64 %v1489, %v6
  br i1 %v1492, label %b1027, label %b1024
b1026:
  %gcvolta1026 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1026, ptr %ctxtopo, align 8
  ret i64 0
b1027:
  br label %b977
b1028:
  %v1500 = icmp eq i64 %v0, %v1495
  br label %b1030
b1029:
  %sa1501_0 = getelementptr [2 x i64], ptr %sargs1501, i64 0, i64 0
  store i64 %v1495, ptr %sa1501_0
  %sat1501 = getelementptr [2 x i64], ptr %sargs1501, i64 0, i64 1
  store i64 0, ptr %sat1501
  %sic1501 = getelementptr i64, ptr %area, i64 141772
  %sf1501 = call ptr @df.selector(ptr %sic1501, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v1501 = call i64 %sf1501(i64 %v0, ptr %sargs1501, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1501, ptr %gcs0
  %v1502 = load i8, ptr %ctx, align 8
  %c316 = zext i8 %v1502 to i64
  %v1503 = icmp ne i64 %c316, 0
  br i1 %v1503, label %b1032, label %b1031
b1030:
  %v1507 = phi i1 [ %v1500, %b1028 ], [ %v1504, %b1033 ]
  br i1 %v1507, label %b1035, label %b1036
b1031:
  store i64 0, ptr %gcs1
  %v1504 = call i1 @df.unbox_bool(i64 %v1501)
  %v1505 = load i8, ptr %ctx, align 8
  %c317 = zext i8 %v1505 to i64
  %v1506 = icmp ne i64 %c317, 0
  br i1 %v1506, label %b1034, label %b1033
b1032:
  %gcvolta1032 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1032, ptr %ctxtopo, align 8
  ret i64 0
b1033:
  br label %b1030
b1034:
  %gcvolta1034 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1034, ptr %ctxtopo, align 8
  ret i64 0
b1035:
  %v1508 = add i64 0, 0
  br label %b1037
b1036:
  %v1509 = add i64 0, 1
  br label %b1037
b1037:
  %v1510 = phi i64 [ %v1508, %b1035 ], [ %v1509, %b1036 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1511 = call i64 @df.box_int(i64 %v1510)
  %xcp1512 = load i8, ptr %ctx, align 8
  %xcn1512 = icmp ne i8 %xcp1512, 0
  %xce1512 = call i1 @llvm.expect.i1(i1 %xcn1512, i1 false)
  br i1 %xce1512, label %xc1512.limpar, label %xc1512.fim
xc1512.limpar:
  call void @dartforge_exception_clear()
  br label %xc1512.fim
xc1512.fim:
  %gcvolta1037 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1037, ptr %ctxtopo, align 8
  ret i64 %v1511
b1039:
  %v1517 = ashr i64 %v0, 1
  %v1518 = icmp sle i64 %v1517, %v1514
  br label %b1041
b1040:
  %v1519 = call i64 @df.box_int(i64 %v1514)
  store i64 %v1519, ptr %gcs1
  %sa1520_0 = getelementptr [2 x i64], ptr %sargs1520, i64 0, i64 0
  store i64 %v1519, ptr %sa1520_0
  %sat1520 = getelementptr [2 x i64], ptr %sargs1520, i64 0, i64 1
  store i64 0, ptr %sat1520
  %sic1520 = getelementptr i64, ptr %area, i64 141774
  %sf1520 = call ptr @df.selector(ptr %sic1520, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1520 = call i64 %sf1520(i64 %v0, ptr %sargs1520, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1520, ptr %gcs1
  %v1521 = load i8, ptr %ctx, align 8
  %c318 = zext i8 %v1521 to i64
  %v1522 = icmp ne i64 %c318, 0
  br i1 %v1522, label %b1043, label %b1042
b1041:
  %v1526 = phi i1 [ %v1518, %b1039 ], [ %v1523, %b1044 ]
  br i1 %v1526, label %b1046, label %b1047
b1042:
  %v1523 = call i1 @df.unbox_bool(i64 %v1520)
  %v1524 = load i8, ptr %ctx, align 8
  %c319 = zext i8 %v1524 to i64
  %v1525 = icmp ne i64 %c319, 0
  br i1 %v1525, label %b1045, label %b1044
b1043:
  %gcvolta1043 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1043, ptr %ctxtopo, align 8
  ret i64 0
b1044:
  br label %b1041
b1045:
  %gcvolta1045 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1045, ptr %ctxtopo, align 8
  ret i64 0
b1046:
  %v1527 = add i64 0, 0
  br label %b1048
b1047:
  store i64 0, ptr %gcs1
  %v1529 = icmp eq i64 %v0, 0
  %v1530 = call i64 @df.class(i64 %v0)
  %v1531 = call i8 @df.subclass(i64 %v1530, i64 674)
  %c320 = zext i8 %v1531 to i64
  %v1532 = icmp ne i64 %c320, 0
  %v1533 = xor i1 %v1529, true
  %c321 = zext i1 %v1532 to i64
  %c322 = zext i1 %v1533 to i64
  %v1534 = and i64 %c321, %c322
  %v1535 = icmp ne i64 %v1534, 0
  br i1 %v1535, label %b1050, label %b1049
b1048:
  %v1549 = phi i64 [ %v1527, %b1046 ], [ %v1548, %b1056 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1550 = call i64 @df.box_int(i64 %v1549)
  %xcp1551 = load i8, ptr %ctx, align 8
  %xcn1551 = icmp ne i8 %xcp1551, 0
  %xce1551 = call i1 @llvm.expect.i1(i1 %xcn1551, i1 false)
  br i1 %xce1551, label %xc1551.limpar, label %xc1551.fim
xc1551.limpar:
  call void @dartforge_exception_clear()
  br label %xc1551.fim
xc1551.fim:
  %gcvolta1048 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1048, ptr %ctxtopo, align 8
  ret i64 %v1550
b1049:
  %ga1536 = getelementptr i64, ptr %area, i64 10809
  %v1536 = load i8, ptr %ga1536
  %c323 = zext i8 %v1536 to i64
  %v1537 = icmp ne i64 %c323, 0
  br i1 %v1537, label %b1051, label %b1052
b1050:
  %v1544 = call i64 @df.unbox_int(i64 %v0)
  %v1545 = load i8, ptr %ctx, align 8
  %c324 = zext i8 %v1545 to i64
  %v1546 = icmp ne i64 %c324, 0
  br i1 %v1546, label %b1057, label %b1056
b1051:
  %ga1538 = getelementptr i64, ptr %area, i64 10808
  %v1538 = load i64, ptr %ga1538
  br label %b1053
b1052:
  %v1539 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1053
b1053:
  %v1540 = phi i64 [ %v1538, %b1051 ], [ %v1539, %b1052 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1540, i64 0, i64 0)
  %v1542 = load i8, ptr %ctx, align 8
  %c325 = zext i8 %v1542 to i64
  %v1543 = icmp ne i64 %c325, 0
  br i1 %v1543, label %b1055, label %b1054
b1054:
  br label %b1050
b1055:
  %gcvolta1055 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1055, ptr %ctxtopo, align 8
  ret i64 0
b1056:
  %v1547 = add i64 0, 15
  %v1548 = ashr i64 %v1544, %v1547
  br label %b1048
b1057:
  %gcvolta1057 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1057, ptr %ctxtopo, align 8
  ret i64 0
b1059:
  %v1556 = ashr i64 %v0, 1
  %v1557 = icmp sle i64 %v1556, %v1553
  br label %b1061
b1060:
  %v1558 = call i64 @df.box_int(i64 %v1553)
  store i64 %v1558, ptr %gcs1
  %sa1559_0 = getelementptr [2 x i64], ptr %sargs1559, i64 0, i64 0
  store i64 %v1558, ptr %sa1559_0
  %sat1559 = getelementptr [2 x i64], ptr %sargs1559, i64 0, i64 1
  store i64 0, ptr %sat1559
  %sic1559 = getelementptr i64, ptr %area, i64 141776
  %sf1559 = call ptr @df.selector(ptr %sic1559, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1559 = call i64 %sf1559(i64 %v0, ptr %sargs1559, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1559, ptr %gcs1
  %v1560 = load i8, ptr %ctx, align 8
  %c326 = zext i8 %v1560 to i64
  %v1561 = icmp ne i64 %c326, 0
  br i1 %v1561, label %b1063, label %b1062
b1061:
  %v1565 = phi i1 [ %v1557, %b1059 ], [ %v1562, %b1064 ]
  br i1 %v1565, label %b1066, label %b1067
b1062:
  %v1562 = call i1 @df.unbox_bool(i64 %v1559)
  %v1563 = load i8, ptr %ctx, align 8
  %c327 = zext i8 %v1563 to i64
  %v1564 = icmp ne i64 %c327, 0
  br i1 %v1564, label %b1065, label %b1064
b1063:
  %gcvolta1063 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1063, ptr %ctxtopo, align 8
  ret i64 0
b1064:
  br label %b1061
b1065:
  %gcvolta1065 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1065, ptr %ctxtopo, align 8
  ret i64 0
b1066:
  %v1566 = add i64 0, 0
  br label %b1068
b1067:
  store i64 0, ptr %gcs1
  %v1568 = icmp eq i64 %v0, 0
  %v1569 = call i64 @df.class(i64 %v0)
  %v1570 = call i8 @df.subclass(i64 %v1569, i64 674)
  %c328 = zext i8 %v1570 to i64
  %v1571 = icmp ne i64 %c328, 0
  %v1572 = xor i1 %v1568, true
  %c329 = zext i1 %v1571 to i64
  %c330 = zext i1 %v1572 to i64
  %v1573 = and i64 %c329, %c330
  %v1574 = icmp ne i64 %v1573, 0
  br i1 %v1574, label %b1070, label %b1069
b1068:
  %v1588 = phi i64 [ %v1566, %b1066 ], [ %v1587, %b1076 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1589 = call i64 @df.box_int(i64 %v1588)
  %xcp1590 = load i8, ptr %ctx, align 8
  %xcn1590 = icmp ne i8 %xcp1590, 0
  %xce1590 = call i1 @llvm.expect.i1(i1 %xcn1590, i1 false)
  br i1 %xce1590, label %xc1590.limpar, label %xc1590.fim
xc1590.limpar:
  call void @dartforge_exception_clear()
  br label %xc1590.fim
xc1590.fim:
  %gcvolta1068 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1068, ptr %ctxtopo, align 8
  ret i64 %v1589
b1069:
  %ga1575 = getelementptr i64, ptr %area, i64 10809
  %v1575 = load i8, ptr %ga1575
  %c331 = zext i8 %v1575 to i64
  %v1576 = icmp ne i64 %c331, 0
  br i1 %v1576, label %b1071, label %b1072
b1070:
  %v1583 = call i64 @df.unbox_int(i64 %v0)
  %v1584 = load i8, ptr %ctx, align 8
  %c332 = zext i8 %v1584 to i64
  %v1585 = icmp ne i64 %c332, 0
  br i1 %v1585, label %b1077, label %b1076
b1071:
  %ga1577 = getelementptr i64, ptr %area, i64 10808
  %v1577 = load i64, ptr %ga1577
  br label %b1073
b1072:
  %v1578 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1073
b1073:
  %v1579 = phi i64 [ %v1577, %b1071 ], [ %v1578, %b1072 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1579, i64 0, i64 0)
  %v1581 = load i8, ptr %ctx, align 8
  %c333 = zext i8 %v1581 to i64
  %v1582 = icmp ne i64 %c333, 0
  br i1 %v1582, label %b1075, label %b1074
b1074:
  br label %b1070
b1075:
  %gcvolta1075 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1075, ptr %ctxtopo, align 8
  ret i64 0
b1076:
  %v1586 = add i64 0, 11
  %v1587 = ashr i64 %v1583, %v1586
  br label %b1068
b1077:
  %gcvolta1077 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1077, ptr %ctxtopo, align 8
  ret i64 0
b1079:
  %v1595 = ashr i64 %v0, 1
  %v1596 = icmp sle i64 %v1595, %v1592
  br label %b1081
b1080:
  %v1597 = call i64 @df.box_int(i64 %v1592)
  store i64 %v1597, ptr %gcs1
  %sa1598_0 = getelementptr [2 x i64], ptr %sargs1598, i64 0, i64 0
  store i64 %v1597, ptr %sa1598_0
  %sat1598 = getelementptr [2 x i64], ptr %sargs1598, i64 0, i64 1
  store i64 0, ptr %sat1598
  %sic1598 = getelementptr i64, ptr %area, i64 141778
  %sf1598 = call ptr @df.selector(ptr %sic1598, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1598 = call i64 %sf1598(i64 %v0, ptr %sargs1598, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1598, ptr %gcs1
  %v1599 = load i8, ptr %ctx, align 8
  %c334 = zext i8 %v1599 to i64
  %v1600 = icmp ne i64 %c334, 0
  br i1 %v1600, label %b1083, label %b1082
b1081:
  %v1604 = phi i1 [ %v1596, %b1079 ], [ %v1601, %b1084 ]
  br i1 %v1604, label %b1086, label %b1087
b1082:
  %v1601 = call i1 @df.unbox_bool(i64 %v1598)
  %v1602 = load i8, ptr %ctx, align 8
  %c335 = zext i8 %v1602 to i64
  %v1603 = icmp ne i64 %c335, 0
  br i1 %v1603, label %b1085, label %b1084
b1083:
  %gcvolta1083 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1083, ptr %ctxtopo, align 8
  ret i64 0
b1084:
  br label %b1081
b1085:
  %gcvolta1085 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1085, ptr %ctxtopo, align 8
  ret i64 0
b1086:
  %v1605 = add i64 0, 0
  br label %b1088
b1087:
  store i64 0, ptr %gcs1
  %v1607 = icmp eq i64 %v0, 0
  %v1608 = call i64 @df.class(i64 %v0)
  %v1609 = call i8 @df.subclass(i64 %v1608, i64 674)
  %c336 = zext i8 %v1609 to i64
  %v1610 = icmp ne i64 %c336, 0
  %v1611 = xor i1 %v1607, true
  %c337 = zext i1 %v1610 to i64
  %c338 = zext i1 %v1611 to i64
  %v1612 = and i64 %c337, %c338
  %v1613 = icmp ne i64 %v1612, 0
  br i1 %v1613, label %b1090, label %b1089
b1088:
  %v1627 = phi i64 [ %v1605, %b1086 ], [ %v1626, %b1096 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1628 = call i64 @df.box_int(i64 %v1627)
  %xcp1629 = load i8, ptr %ctx, align 8
  %xcn1629 = icmp ne i8 %xcp1629, 0
  %xce1629 = call i1 @llvm.expect.i1(i1 %xcn1629, i1 false)
  br i1 %xce1629, label %xc1629.limpar, label %xc1629.fim
xc1629.limpar:
  call void @dartforge_exception_clear()
  br label %xc1629.fim
xc1629.fim:
  %gcvolta1088 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1088, ptr %ctxtopo, align 8
  ret i64 %v1628
b1089:
  %ga1614 = getelementptr i64, ptr %area, i64 10809
  %v1614 = load i8, ptr %ga1614
  %c339 = zext i8 %v1614 to i64
  %v1615 = icmp ne i64 %c339, 0
  br i1 %v1615, label %b1091, label %b1092
b1090:
  %v1622 = call i64 @df.unbox_int(i64 %v0)
  %v1623 = load i8, ptr %ctx, align 8
  %c340 = zext i8 %v1623 to i64
  %v1624 = icmp ne i64 %c340, 0
  br i1 %v1624, label %b1097, label %b1096
b1091:
  %ga1616 = getelementptr i64, ptr %area, i64 10808
  %v1616 = load i64, ptr %ga1616
  br label %b1093
b1092:
  %v1617 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1093
b1093:
  %v1618 = phi i64 [ %v1616, %b1091 ], [ %v1617, %b1092 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1618, i64 0, i64 0)
  %v1620 = load i8, ptr %ctx, align 8
  %c341 = zext i8 %v1620 to i64
  %v1621 = icmp ne i64 %c341, 0
  br i1 %v1621, label %b1095, label %b1094
b1094:
  br label %b1090
b1095:
  %gcvolta1095 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1095, ptr %ctxtopo, align 8
  ret i64 0
b1096:
  %v1625 = add i64 0, 7
  %v1626 = ashr i64 %v1622, %v1625
  br label %b1088
b1097:
  %gcvolta1097 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1097, ptr %ctxtopo, align 8
  ret i64 0
b1099:
  %v1634 = ashr i64 %v0, 1
  %v1635 = icmp sle i64 %v1634, %v1631
  br label %b1101
b1100:
  %v1636 = call i64 @df.box_int(i64 %v1631)
  store i64 %v1636, ptr %gcs1
  %sa1637_0 = getelementptr [2 x i64], ptr %sargs1637, i64 0, i64 0
  store i64 %v1636, ptr %sa1637_0
  %sat1637 = getelementptr [2 x i64], ptr %sargs1637, i64 0, i64 1
  store i64 0, ptr %sat1637
  %sic1637 = getelementptr i64, ptr %area, i64 141780
  %sf1637 = call ptr @df.selector(ptr %sic1637, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1637 = call i64 %sf1637(i64 %v0, ptr %sargs1637, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1637, ptr %gcs1
  %v1638 = load i8, ptr %ctx, align 8
  %c342 = zext i8 %v1638 to i64
  %v1639 = icmp ne i64 %c342, 0
  br i1 %v1639, label %b1103, label %b1102
b1101:
  %v1643 = phi i1 [ %v1635, %b1099 ], [ %v1640, %b1104 ]
  br i1 %v1643, label %b1106, label %b1107
b1102:
  %v1640 = call i1 @df.unbox_bool(i64 %v1637)
  %v1641 = load i8, ptr %ctx, align 8
  %c343 = zext i8 %v1641 to i64
  %v1642 = icmp ne i64 %c343, 0
  br i1 %v1642, label %b1105, label %b1104
b1103:
  %gcvolta1103 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1103, ptr %ctxtopo, align 8
  ret i64 0
b1104:
  br label %b1101
b1105:
  %gcvolta1105 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1105, ptr %ctxtopo, align 8
  ret i64 0
b1106:
  %v1644 = add i64 0, 0
  br label %b1108
b1107:
  store i64 0, ptr %gcs1
  %v1646 = icmp eq i64 %v0, 0
  %v1647 = call i64 @df.class(i64 %v0)
  %v1648 = call i8 @df.subclass(i64 %v1647, i64 674)
  %c344 = zext i8 %v1648 to i64
  %v1649 = icmp ne i64 %c344, 0
  %v1650 = xor i1 %v1646, true
  %c345 = zext i1 %v1649 to i64
  %c346 = zext i1 %v1650 to i64
  %v1651 = and i64 %c345, %c346
  %v1652 = icmp ne i64 %v1651, 0
  br i1 %v1652, label %b1110, label %b1109
b1108:
  %v1666 = phi i64 [ %v1644, %b1106 ], [ %v1665, %b1116 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1667 = call i64 @df.box_int(i64 %v1666)
  %xcp1668 = load i8, ptr %ctx, align 8
  %xcn1668 = icmp ne i8 %xcp1668, 0
  %xce1668 = call i1 @llvm.expect.i1(i1 %xcn1668, i1 false)
  br i1 %xce1668, label %xc1668.limpar, label %xc1668.fim
xc1668.limpar:
  call void @dartforge_exception_clear()
  br label %xc1668.fim
xc1668.fim:
  %gcvolta1108 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1108, ptr %ctxtopo, align 8
  ret i64 %v1667
b1109:
  %ga1653 = getelementptr i64, ptr %area, i64 10809
  %v1653 = load i8, ptr %ga1653
  %c347 = zext i8 %v1653 to i64
  %v1654 = icmp ne i64 %c347, 0
  br i1 %v1654, label %b1111, label %b1112
b1110:
  %v1661 = call i64 @df.unbox_int(i64 %v0)
  %v1662 = load i8, ptr %ctx, align 8
  %c348 = zext i8 %v1662 to i64
  %v1663 = icmp ne i64 %c348, 0
  br i1 %v1663, label %b1117, label %b1116
b1111:
  %ga1655 = getelementptr i64, ptr %area, i64 10808
  %v1655 = load i64, ptr %ga1655
  br label %b1113
b1112:
  %v1656 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1113
b1113:
  %v1657 = phi i64 [ %v1655, %b1111 ], [ %v1656, %b1112 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1657, i64 0, i64 0)
  %v1659 = load i8, ptr %ctx, align 8
  %c349 = zext i8 %v1659 to i64
  %v1660 = icmp ne i64 %c349, 0
  br i1 %v1660, label %b1115, label %b1114
b1114:
  br label %b1110
b1115:
  %gcvolta1115 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1115, ptr %ctxtopo, align 8
  ret i64 0
b1116:
  %v1664 = add i64 0, 1
  %v1665 = shl i64 %v1661, %v1664
  br label %b1108
b1117:
  %gcvolta1117 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1117, ptr %ctxtopo, align 8
  ret i64 0
b1119:
  %v1673 = ashr i64 %v0, 1
  %v1674 = icmp sle i64 %v1673, %v1670
  br label %b1121
b1120:
  %v1675 = call i64 @df.box_int(i64 %v1670)
  store i64 %v1675, ptr %gcs1
  %sa1676_0 = getelementptr [2 x i64], ptr %sargs1676, i64 0, i64 0
  store i64 %v1675, ptr %sa1676_0
  %sat1676 = getelementptr [2 x i64], ptr %sargs1676, i64 0, i64 1
  store i64 0, ptr %sat1676
  %sic1676 = getelementptr i64, ptr %area, i64 141782
  %sf1676 = call ptr @df.selector(ptr %sic1676, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1676 = call i64 %sf1676(i64 %v0, ptr %sargs1676, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1676, ptr %gcs1
  %v1677 = load i8, ptr %ctx, align 8
  %c350 = zext i8 %v1677 to i64
  %v1678 = icmp ne i64 %c350, 0
  br i1 %v1678, label %b1123, label %b1122
b1121:
  %v1682 = phi i1 [ %v1674, %b1119 ], [ %v1679, %b1124 ]
  br i1 %v1682, label %b1126, label %b1127
b1122:
  %v1679 = call i1 @df.unbox_bool(i64 %v1676)
  %v1680 = load i8, ptr %ctx, align 8
  %c351 = zext i8 %v1680 to i64
  %v1681 = icmp ne i64 %c351, 0
  br i1 %v1681, label %b1125, label %b1124
b1123:
  %gcvolta1123 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1123, ptr %ctxtopo, align 8
  ret i64 0
b1124:
  br label %b1121
b1125:
  %gcvolta1125 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1125, ptr %ctxtopo, align 8
  ret i64 0
b1126:
  %v1683 = add i64 0, 0
  br label %b1128
b1127:
  store i64 0, ptr %gcs1
  %v1685 = icmp eq i64 %v0, 0
  %v1686 = call i64 @df.class(i64 %v0)
  %v1687 = call i8 @df.subclass(i64 %v1686, i64 674)
  %c352 = zext i8 %v1687 to i64
  %v1688 = icmp ne i64 %c352, 0
  %v1689 = xor i1 %v1685, true
  %c353 = zext i1 %v1688 to i64
  %c354 = zext i1 %v1689 to i64
  %v1690 = and i64 %c353, %c354
  %v1691 = icmp ne i64 %v1690, 0
  br i1 %v1691, label %b1130, label %b1129
b1128:
  %v1705 = phi i64 [ %v1683, %b1126 ], [ %v1704, %b1136 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1706 = call i64 @df.box_int(i64 %v1705)
  %xcp1707 = load i8, ptr %ctx, align 8
  %xcn1707 = icmp ne i8 %xcp1707, 0
  %xce1707 = call i1 @llvm.expect.i1(i1 %xcn1707, i1 false)
  br i1 %xce1707, label %xc1707.limpar, label %xc1707.fim
xc1707.limpar:
  call void @dartforge_exception_clear()
  br label %xc1707.fim
xc1707.fim:
  %gcvolta1128 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1128, ptr %ctxtopo, align 8
  ret i64 %v1706
b1129:
  %ga1692 = getelementptr i64, ptr %area, i64 10809
  %v1692 = load i8, ptr %ga1692
  %c355 = zext i8 %v1692 to i64
  %v1693 = icmp ne i64 %c355, 0
  br i1 %v1693, label %b1131, label %b1132
b1130:
  %v1700 = call i64 @df.unbox_int(i64 %v0)
  %v1701 = load i8, ptr %ctx, align 8
  %c356 = zext i8 %v1701 to i64
  %v1702 = icmp ne i64 %c356, 0
  br i1 %v1702, label %b1137, label %b1136
b1131:
  %ga1694 = getelementptr i64, ptr %area, i64 10808
  %v1694 = load i64, ptr %ga1694
  br label %b1133
b1132:
  %v1695 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1133
b1133:
  %v1696 = phi i64 [ %v1694, %b1131 ], [ %v1695, %b1132 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1696, i64 0, i64 0)
  %v1698 = load i8, ptr %ctx, align 8
  %c357 = zext i8 %v1698 to i64
  %v1699 = icmp ne i64 %c357, 0
  br i1 %v1699, label %b1135, label %b1134
b1134:
  br label %b1130
b1135:
  %gcvolta1135 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1135, ptr %ctxtopo, align 8
  ret i64 0
b1136:
  %v1703 = add i64 0, 131076
  %v1704 = mul i64 %v1700, %v1703
  br label %b1128
b1137:
  %gcvolta1137 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1137, ptr %ctxtopo, align 8
  ret i64 0
b1139:
  %ga1716 = getelementptr i64, ptr %area, i64 10809
  %v1716 = load i8, ptr %ga1716
  %c358 = zext i8 %v1716 to i64
  %v1717 = icmp ne i64 %c358, 0
  br i1 %v1717, label %b1141, label %b1142
b1140:
  %v1724 = call i64 @df.unbox_int(i64 %v0)
  %v1725 = load i8, ptr %ctx, align 8
  %c359 = zext i8 %v1725 to i64
  %v1726 = icmp ne i64 %c359, 0
  br i1 %v1726, label %b1147, label %b1146
b1141:
  %ga1718 = getelementptr i64, ptr %area, i64 10808
  %v1718 = load i64, ptr %ga1718
  br label %b1143
b1142:
  %v1719 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1143
b1143:
  %v1720 = phi i64 [ %v1718, %b1141 ], [ %v1719, %b1142 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1720, i64 0, i64 0)
  %v1722 = load i8, ptr %ctx, align 8
  %c360 = zext i8 %v1722 to i64
  %v1723 = icmp ne i64 %c360, 0
  br i1 %v1723, label %b1145, label %b1144
b1144:
  br label %b1140
b1145:
  %gcvolta1145 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1145, ptr %ctxtopo, align 8
  ret i64 0
b1146:
  %v1727 = add i64 0, 8
  %v1728 = ashr i64 %v1724, %v1727
  store i64 0, ptr %gcs0
  %v1729 = call i64 @df.box_int(i64 %v1728)
  %xcp1730 = load i8, ptr %ctx, align 8
  %xcn1730 = icmp ne i8 %xcp1730, 0
  %xce1730 = call i1 @llvm.expect.i1(i1 %xcn1730, i1 false)
  br i1 %xce1730, label %xc1730.limpar, label %xc1730.fim
xc1730.limpar:
  call void @dartforge_exception_clear()
  br label %xc1730.fim
xc1730.fim:
  %gcvolta1146 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1146, ptr %ctxtopo, align 8
  ret i64 %v1729
b1147:
  %gcvolta1147 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1147, ptr %ctxtopo, align 8
  ret i64 0
b1150:
  %ga1741 = getelementptr i64, ptr %area, i64 10809
  %v1741 = load i8, ptr %ga1741
  %c361 = zext i8 %v1741 to i64
  %v1742 = icmp ne i64 %c361, 0
  br i1 %v1742, label %b1152, label %b1153
b1151:
  %v1749 = call i64 @df.unbox_int(i64 %v0)
  %v1750 = load i8, ptr %ctx, align 8
  %c362 = zext i8 %v1750 to i64
  %v1751 = icmp ne i64 %c362, 0
  br i1 %v1751, label %b1158, label %b1157
b1152:
  %ga1743 = getelementptr i64, ptr %area, i64 10808
  %v1743 = load i64, ptr %ga1743
  br label %b1154
b1153:
  %v1744 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1154
b1154:
  %v1745 = phi i64 [ %v1743, %b1152 ], [ %v1744, %b1153 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1745, i64 0, i64 0)
  %v1747 = load i8, ptr %ctx, align 8
  %c363 = zext i8 %v1747 to i64
  %v1748 = icmp ne i64 %c363, 0
  br i1 %v1748, label %b1156, label %b1155
b1155:
  br label %b1151
b1156:
  %gcvolta1156 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1156, ptr %ctxtopo, align 8
  ret i64 0
b1157:
  %v1752 = add i64 0, 65538
  %v1753 = mul i64 %v1749, %v1752
  store i64 0, ptr %gcs0
  %v1754 = call i64 @df.box_int(i64 %v1753)
  %xcp1755 = load i8, ptr %ctx, align 8
  %xcn1755 = icmp ne i8 %xcp1755, 0
  %xce1755 = call i1 @llvm.expect.i1(i1 %xcn1755, i1 false)
  br i1 %xce1755, label %xc1755.limpar, label %xc1755.fim
xc1755.limpar:
  call void @dartforge_exception_clear()
  br label %xc1755.fim
xc1755.fim:
  %gcvolta1157 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1157, ptr %ctxtopo, align 8
  ret i64 %v1754
b1158:
  %gcvolta1158 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1158, ptr %ctxtopo, align 8
  ret i64 0
b1160:
  store i64 0, ptr %gcs1
  %v1762 = call double @df.unbox_double(i64 %v1759)
  %v1763 = load i8, ptr %ctx, align 8
  %c364 = zext i8 %v1763 to i64
  %v1764 = icmp ne i64 %c364, 0
  br i1 %v1764, label %b1163, label %b1162
b1161:
  %gcvolta1161 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1161, ptr %ctxtopo, align 8
  ret i64 0
b1162:
  %xcp1766 = load i8, ptr %ctx, align 8
  %xcn1766 = icmp ne i8 %xcp1766, 0
  %xce1766 = call i1 @llvm.expect.i1(i1 %xcn1766, i1 false)
  br i1 %xce1766, label %xc1766.limpar, label %xc1766.fim
xc1766.limpar:
  call void @dartforge_exception_clear()
  br label %xc1766.fim
xc1766.fim:
  %gcvolta1162 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1162, ptr %ctxtopo, align 8
  ret i64 %v1759
b1163:
  %gcvolta1163 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1163, ptr %ctxtopo, align 8
  ret i64 0
b1177:
  %v2080 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2081 = call i64 @df.box_int(i64 %v2080)
  store i64 %v2081, ptr %gcs1
  %sa2082_0 = getelementptr [2 x i64], ptr %sargs2082, i64 0, i64 0
  store i64 %v2081, ptr %sa2082_0
  %sat2082 = getelementptr [2 x i64], ptr %sargs2082, i64 0, i64 1
  store i64 0, ptr %sat2082
  %sic2082 = getelementptr i64, ptr %area, i64 141784
  %sf2082 = call ptr @df.selector(ptr %sic2082, i64 %v0, i64 -4020828333918261805, ptr @df.seln.331, i64 4)
  %v2082 = call i64 %sf2082(i64 %v0, ptr %sargs2082, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2082, ptr %gcs0
  %v2083 = load i8, ptr %ctx, align 8
  %c365 = zext i8 %v2083 to i64
  %v2084 = icmp ne i64 %c365, 0
  br i1 %v2084, label %b1361, label %b1360
b1178:
  store i64 0, ptr %gcs1
  %v1772 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v1772, ptr %gcs1
  %v1773 = load i8, ptr %ctx, align 8
  %c366 = zext i8 %v1773 to i64
  %v1774 = icmp ne i64 %c366, 0
  br i1 %v1774, label %b1184, label %b1183
b1179:
  %v1771 = icmp eq i64 %v1768, %v6
  br i1 %v1771, label %b1181, label %b1178
b1180:
  %gcvolta1180 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1180, ptr %ctxtopo, align 8
  ret i64 0
b1181:
  %v1817 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1818 = call i64 @df.box_int(i64 %v1817)
  store i64 %v1818, ptr %gcs1
  %v1819 = icmp eq i64 %v0, 0
  %v1820 = icmp eq i64 %v1818, 0
  %c367 = zext i1 %v1819 to i64
  %c368 = zext i1 %v1820 to i64
  %v1821 = or i64 %c367, %c368
  %v1822 = icmp ne i64 %v1821, 0
  br i1 %v1822, label %b1228, label %b1229
b1182:
  store i64 0, ptr %gcs1
  %v1776 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v1776, ptr %gcs1
  %v1777 = load i8, ptr %ctx, align 8
  %c369 = zext i8 %v1777 to i64
  %v1778 = icmp ne i64 %c369, 0
  br i1 %v1778, label %b1188, label %b1187
b1183:
  %v1775 = icmp eq i64 %v1772, %v6
  br i1 %v1775, label %b1185, label %b1182
b1184:
  %gcvolta1184 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1184, ptr %ctxtopo, align 8
  ret i64 0
b1185:
  %v1837 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1838 = and i64 %v0, 1
  %v1839 = icmp ne i64 %v1838, 0
  br i1 %v1839, label %b1239, label %b1240
b1186:
  store i64 0, ptr %gcs1
  %v1780 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v1780, ptr %gcs1
  %v1781 = load i8, ptr %ctx, align 8
  %c370 = zext i8 %v1781 to i64
  %v1782 = icmp ne i64 %c370, 0
  br i1 %v1782, label %b1192, label %b1191
b1187:
  %v1779 = icmp eq i64 %v1776, %v6
  br i1 %v1779, label %b1189, label %b1186
b1188:
  %gcvolta1188 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1188, ptr %ctxtopo, align 8
  ret i64 0
b1189:
  %v1876 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1877 = and i64 %v0, 1
  %v1878 = icmp ne i64 %v1877, 0
  br i1 %v1878, label %b1259, label %b1260
b1190:
  store i64 0, ptr %gcs1
  %v1784 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v1784, ptr %gcs1
  %v1785 = load i8, ptr %ctx, align 8
  %c371 = zext i8 %v1785 to i64
  %v1786 = icmp ne i64 %c371, 0
  br i1 %v1786, label %b1196, label %b1195
b1191:
  %v1783 = icmp eq i64 %v1780, %v6
  br i1 %v1783, label %b1193, label %b1190
b1192:
  %gcvolta1192 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1192, ptr %ctxtopo, align 8
  ret i64 0
b1193:
  %v1915 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1916 = and i64 %v0, 1
  %v1917 = icmp ne i64 %v1916, 0
  br i1 %v1917, label %b1279, label %b1280
b1194:
  store i64 0, ptr %gcs1
  %v1788 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v1788, ptr %gcs1
  %v1789 = load i8, ptr %ctx, align 8
  %c372 = zext i8 %v1789 to i64
  %v1790 = icmp ne i64 %c372, 0
  br i1 %v1790, label %b1200, label %b1199
b1195:
  %v1787 = icmp eq i64 %v1784, %v6
  br i1 %v1787, label %b1197, label %b1194
b1196:
  %gcvolta1196 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1196, ptr %ctxtopo, align 8
  ret i64 0
b1197:
  %v1954 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1955 = and i64 %v0, 1
  %v1956 = icmp ne i64 %v1955, 0
  br i1 %v1956, label %b1299, label %b1300
b1198:
  store i64 0, ptr %gcs1
  %v1792 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v1792, ptr %gcs1
  %v1793 = load i8, ptr %ctx, align 8
  %c373 = zext i8 %v1793 to i64
  %v1794 = icmp ne i64 %c373, 0
  br i1 %v1794, label %b1204, label %b1203
b1199:
  %v1791 = icmp eq i64 %v1788, %v6
  br i1 %v1791, label %b1201, label %b1198
b1200:
  %gcvolta1200 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1200, ptr %ctxtopo, align 8
  ret i64 0
b1201:
  %v1993 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1994 = and i64 %v0, 1
  %v1995 = icmp ne i64 %v1994, 0
  br i1 %v1995, label %b1319, label %b1320
b1202:
  store i64 0, ptr %gcs1
  %v1796 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v1796, ptr %gcs1
  %v1797 = load i8, ptr %ctx, align 8
  %c374 = zext i8 %v1797 to i64
  %v1798 = icmp ne i64 %c374, 0
  br i1 %v1798, label %b1208, label %b1207
b1203:
  %v1795 = icmp eq i64 %v1792, %v6
  br i1 %v1795, label %b1205, label %b1202
b1204:
  %gcvolta1204 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1204, ptr %ctxtopo, align 8
  ret i64 0
b1205:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2032 = icmp eq i64 %v0, 0
  %v2033 = call i64 @df.class(i64 %v0)
  %v2034 = call i8 @df.subclass(i64 %v2033, i64 674)
  %c375 = zext i8 %v2034 to i64
  %v2035 = icmp ne i64 %c375, 0
  %v2036 = xor i1 %v2032, true
  %c376 = zext i1 %v2035 to i64
  %c377 = zext i1 %v2036 to i64
  %v2037 = and i64 %c376, %c377
  %v2038 = icmp ne i64 %v2037, 0
  br i1 %v2038, label %b1340, label %b1339
b1206:
  store i64 0, ptr %gcs1
  %v1800 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v1800, ptr %gcs1
  %v1801 = load i8, ptr %ctx, align 8
  %c378 = zext i8 %v1801 to i64
  %v1802 = icmp ne i64 %c378, 0
  br i1 %v1802, label %b1212, label %b1211
b1207:
  %v1799 = icmp eq i64 %v1796, %v6
  br i1 %v1799, label %b1209, label %b1206
b1208:
  %gcvolta1208 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1208, ptr %ctxtopo, align 8
  ret i64 0
b1209:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2055 = icmp eq i64 %v0, 0
  %v2056 = call i64 @df.class(i64 %v0)
  %v2057 = call i8 @df.subclass(i64 %v2056, i64 674)
  %c379 = zext i8 %v2057 to i64
  %v2058 = icmp ne i64 %c379, 0
  %v2059 = xor i1 %v2055, true
  %c380 = zext i1 %v2058 to i64
  %c381 = zext i1 %v2059 to i64
  %v2060 = and i64 %c380, %c381
  %v2061 = icmp ne i64 %v2060, 0
  br i1 %v2061, label %b1350, label %b1349
b1210:
  store i64 0, ptr %gcs1
  %v1804 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v1804, ptr %gcs1
  %v1805 = load i8, ptr %ctx, align 8
  %c382 = zext i8 %v1805 to i64
  %v1806 = icmp ne i64 %c382, 0
  br i1 %v1806, label %b1216, label %b1215
b1211:
  %v1803 = icmp eq i64 %v1800, %v6
  br i1 %v1803, label %b1213, label %b1210
b1212:
  %gcvolta1212 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1212, ptr %ctxtopo, align 8
  ret i64 0
b1213:
  %xcp2078 = load i8, ptr %ctx, align 8
  %xcn2078 = icmp ne i8 %xcp2078, 0
  %xce2078 = call i1 @llvm.expect.i1(i1 %xcn2078, i1 false)
  br i1 %xce2078, label %xc2078.limpar, label %xc2078.fim
xc2078.limpar:
  call void @dartforge_exception_clear()
  br label %xc2078.fim
xc2078.fim:
  %gcvolta1213 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1213, ptr %ctxtopo, align 8
  ret i64 %v0
b1214:
  store i64 0, ptr %gcs1
  %v1808 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v1808, ptr %gcs1
  %v1809 = load i8, ptr %ctx, align 8
  %c383 = zext i8 %v1809 to i64
  %v1810 = icmp ne i64 %c383, 0
  br i1 %v1810, label %b1221, label %b1220
b1215:
  %v1807 = icmp eq i64 %v1804, %v6
  br i1 %v1807, label %b1217, label %b1214
b1216:
  %gcvolta1216 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1216, ptr %ctxtopo, align 8
  ret i64 0
b1217:
  br label %b1177
b1219:
  store i64 0, ptr %gcs1
  %v1812 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v1812, ptr %gcs1
  %v1813 = load i8, ptr %ctx, align 8
  %c384 = zext i8 %v1813 to i64
  %v1814 = icmp ne i64 %c384, 0
  br i1 %v1814, label %b1226, label %b1225
b1220:
  %v1811 = icmp eq i64 %v1808, %v6
  br i1 %v1811, label %b1222, label %b1219
b1221:
  %gcvolta1221 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1221, ptr %ctxtopo, align 8
  ret i64 0
b1222:
  br label %b1177
b1224:
  br label %b12
b1225:
  %v1815 = icmp eq i64 %v1812, %v6
  br i1 %v1815, label %b1227, label %b1224
b1226:
  %gcvolta1226 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1226, ptr %ctxtopo, align 8
  ret i64 0
b1227:
  br label %b1177
b1228:
  %v1823 = icmp eq i64 %v0, %v1818
  br label %b1230
b1229:
  %sa1824_0 = getelementptr [2 x i64], ptr %sargs1824, i64 0, i64 0
  store i64 %v1818, ptr %sa1824_0
  %sat1824 = getelementptr [2 x i64], ptr %sargs1824, i64 0, i64 1
  store i64 0, ptr %sat1824
  %sic1824 = getelementptr i64, ptr %area, i64 141786
  %sf1824 = call ptr @df.selector(ptr %sic1824, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v1824 = call i64 %sf1824(i64 %v0, ptr %sargs1824, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1824, ptr %gcs0
  %v1825 = load i8, ptr %ctx, align 8
  %c385 = zext i8 %v1825 to i64
  %v1826 = icmp ne i64 %c385, 0
  br i1 %v1826, label %b1232, label %b1231
b1230:
  %v1830 = phi i1 [ %v1823, %b1228 ], [ %v1827, %b1233 ]
  br i1 %v1830, label %b1235, label %b1236
b1231:
  store i64 0, ptr %gcs1
  %v1827 = call i1 @df.unbox_bool(i64 %v1824)
  %v1828 = load i8, ptr %ctx, align 8
  %c386 = zext i8 %v1828 to i64
  %v1829 = icmp ne i64 %c386, 0
  br i1 %v1829, label %b1234, label %b1233
b1232:
  %gcvolta1232 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1232, ptr %ctxtopo, align 8
  ret i64 0
b1233:
  br label %b1230
b1234:
  %gcvolta1234 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1234, ptr %ctxtopo, align 8
  ret i64 0
b1235:
  %v1831 = add i64 0, 0
  br label %b1237
b1236:
  %v1832 = add i64 0, 1
  br label %b1237
b1237:
  %v1833 = phi i64 [ %v1831, %b1235 ], [ %v1832, %b1236 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1834 = call i64 @df.box_int(i64 %v1833)
  %xcp1835 = load i8, ptr %ctx, align 8
  %xcn1835 = icmp ne i8 %xcp1835, 0
  %xce1835 = call i1 @llvm.expect.i1(i1 %xcn1835, i1 false)
  br i1 %xce1835, label %xc1835.limpar, label %xc1835.fim
xc1835.limpar:
  call void @dartforge_exception_clear()
  br label %xc1835.fim
xc1835.fim:
  %gcvolta1237 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1237, ptr %ctxtopo, align 8
  ret i64 %v1834
b1239:
  %v1840 = ashr i64 %v0, 1
  %v1841 = icmp sle i64 %v1840, %v1837
  br label %b1241
b1240:
  %v1842 = call i64 @df.box_int(i64 %v1837)
  store i64 %v1842, ptr %gcs1
  %sa1843_0 = getelementptr [2 x i64], ptr %sargs1843, i64 0, i64 0
  store i64 %v1842, ptr %sa1843_0
  %sat1843 = getelementptr [2 x i64], ptr %sargs1843, i64 0, i64 1
  store i64 0, ptr %sat1843
  %sic1843 = getelementptr i64, ptr %area, i64 141788
  %sf1843 = call ptr @df.selector(ptr %sic1843, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1843 = call i64 %sf1843(i64 %v0, ptr %sargs1843, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1843, ptr %gcs1
  %v1844 = load i8, ptr %ctx, align 8
  %c387 = zext i8 %v1844 to i64
  %v1845 = icmp ne i64 %c387, 0
  br i1 %v1845, label %b1243, label %b1242
b1241:
  %v1849 = phi i1 [ %v1841, %b1239 ], [ %v1846, %b1244 ]
  br i1 %v1849, label %b1246, label %b1247
b1242:
  %v1846 = call i1 @df.unbox_bool(i64 %v1843)
  %v1847 = load i8, ptr %ctx, align 8
  %c388 = zext i8 %v1847 to i64
  %v1848 = icmp ne i64 %c388, 0
  br i1 %v1848, label %b1245, label %b1244
b1243:
  %gcvolta1243 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1243, ptr %ctxtopo, align 8
  ret i64 0
b1244:
  br label %b1241
b1245:
  %gcvolta1245 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1245, ptr %ctxtopo, align 8
  ret i64 0
b1246:
  %v1850 = add i64 0, 0
  br label %b1248
b1247:
  store i64 0, ptr %gcs1
  %v1852 = icmp eq i64 %v0, 0
  %v1853 = call i64 @df.class(i64 %v0)
  %v1854 = call i8 @df.subclass(i64 %v1853, i64 674)
  %c389 = zext i8 %v1854 to i64
  %v1855 = icmp ne i64 %c389, 0
  %v1856 = xor i1 %v1852, true
  %c390 = zext i1 %v1855 to i64
  %c391 = zext i1 %v1856 to i64
  %v1857 = and i64 %c390, %c391
  %v1858 = icmp ne i64 %v1857, 0
  br i1 %v1858, label %b1250, label %b1249
b1248:
  %v1872 = phi i64 [ %v1850, %b1246 ], [ %v1871, %b1256 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1873 = call i64 @df.box_int(i64 %v1872)
  %xcp1874 = load i8, ptr %ctx, align 8
  %xcn1874 = icmp ne i8 %xcp1874, 0
  %xce1874 = call i1 @llvm.expect.i1(i1 %xcn1874, i1 false)
  br i1 %xce1874, label %xc1874.limpar, label %xc1874.fim
xc1874.limpar:
  call void @dartforge_exception_clear()
  br label %xc1874.fim
xc1874.fim:
  %gcvolta1248 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1248, ptr %ctxtopo, align 8
  ret i64 %v1873
b1249:
  %ga1859 = getelementptr i64, ptr %area, i64 10809
  %v1859 = load i8, ptr %ga1859
  %c392 = zext i8 %v1859 to i64
  %v1860 = icmp ne i64 %c392, 0
  br i1 %v1860, label %b1251, label %b1252
b1250:
  %v1867 = call i64 @df.unbox_int(i64 %v0)
  %v1868 = load i8, ptr %ctx, align 8
  %c393 = zext i8 %v1868 to i64
  %v1869 = icmp ne i64 %c393, 0
  br i1 %v1869, label %b1257, label %b1256
b1251:
  %ga1861 = getelementptr i64, ptr %area, i64 10808
  %v1861 = load i64, ptr %ga1861
  br label %b1253
b1252:
  %v1862 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1253
b1253:
  %v1863 = phi i64 [ %v1861, %b1251 ], [ %v1862, %b1252 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1863, i64 0, i64 0)
  %v1865 = load i8, ptr %ctx, align 8
  %c394 = zext i8 %v1865 to i64
  %v1866 = icmp ne i64 %c394, 0
  br i1 %v1866, label %b1255, label %b1254
b1254:
  br label %b1250
b1255:
  %gcvolta1255 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1255, ptr %ctxtopo, align 8
  ret i64 0
b1256:
  %v1870 = add i64 0, 29
  %v1871 = ashr i64 %v1867, %v1870
  br label %b1248
b1257:
  %gcvolta1257 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1257, ptr %ctxtopo, align 8
  ret i64 0
b1259:
  %v1879 = ashr i64 %v0, 1
  %v1880 = icmp sle i64 %v1879, %v1876
  br label %b1261
b1260:
  %v1881 = call i64 @df.box_int(i64 %v1876)
  store i64 %v1881, ptr %gcs1
  %sa1882_0 = getelementptr [2 x i64], ptr %sargs1882, i64 0, i64 0
  store i64 %v1881, ptr %sa1882_0
  %sat1882 = getelementptr [2 x i64], ptr %sargs1882, i64 0, i64 1
  store i64 0, ptr %sat1882
  %sic1882 = getelementptr i64, ptr %area, i64 141790
  %sf1882 = call ptr @df.selector(ptr %sic1882, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1882 = call i64 %sf1882(i64 %v0, ptr %sargs1882, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1882, ptr %gcs1
  %v1883 = load i8, ptr %ctx, align 8
  %c395 = zext i8 %v1883 to i64
  %v1884 = icmp ne i64 %c395, 0
  br i1 %v1884, label %b1263, label %b1262
b1261:
  %v1888 = phi i1 [ %v1880, %b1259 ], [ %v1885, %b1264 ]
  br i1 %v1888, label %b1266, label %b1267
b1262:
  %v1885 = call i1 @df.unbox_bool(i64 %v1882)
  %v1886 = load i8, ptr %ctx, align 8
  %c396 = zext i8 %v1886 to i64
  %v1887 = icmp ne i64 %c396, 0
  br i1 %v1887, label %b1265, label %b1264
b1263:
  %gcvolta1263 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1263, ptr %ctxtopo, align 8
  ret i64 0
b1264:
  br label %b1261
b1265:
  %gcvolta1265 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1265, ptr %ctxtopo, align 8
  ret i64 0
b1266:
  %v1889 = add i64 0, 0
  br label %b1268
b1267:
  store i64 0, ptr %gcs1
  %v1891 = icmp eq i64 %v0, 0
  %v1892 = call i64 @df.class(i64 %v0)
  %v1893 = call i8 @df.subclass(i64 %v1892, i64 674)
  %c397 = zext i8 %v1893 to i64
  %v1894 = icmp ne i64 %c397, 0
  %v1895 = xor i1 %v1891, true
  %c398 = zext i1 %v1894 to i64
  %c399 = zext i1 %v1895 to i64
  %v1896 = and i64 %c398, %c399
  %v1897 = icmp ne i64 %v1896, 0
  br i1 %v1897, label %b1270, label %b1269
b1268:
  %v1911 = phi i64 [ %v1889, %b1266 ], [ %v1910, %b1276 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1912 = call i64 @df.box_int(i64 %v1911)
  %xcp1913 = load i8, ptr %ctx, align 8
  %xcn1913 = icmp ne i8 %xcp1913, 0
  %xce1913 = call i1 @llvm.expect.i1(i1 %xcn1913, i1 false)
  br i1 %xce1913, label %xc1913.limpar, label %xc1913.fim
xc1913.limpar:
  call void @dartforge_exception_clear()
  br label %xc1913.fim
xc1913.fim:
  %gcvolta1268 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1268, ptr %ctxtopo, align 8
  ret i64 %v1912
b1269:
  %ga1898 = getelementptr i64, ptr %area, i64 10809
  %v1898 = load i8, ptr %ga1898
  %c400 = zext i8 %v1898 to i64
  %v1899 = icmp ne i64 %c400, 0
  br i1 %v1899, label %b1271, label %b1272
b1270:
  %v1906 = call i64 @df.unbox_int(i64 %v0)
  %v1907 = load i8, ptr %ctx, align 8
  %c401 = zext i8 %v1907 to i64
  %v1908 = icmp ne i64 %c401, 0
  br i1 %v1908, label %b1277, label %b1276
b1271:
  %ga1900 = getelementptr i64, ptr %area, i64 10808
  %v1900 = load i64, ptr %ga1900
  br label %b1273
b1272:
  %v1901 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1273
b1273:
  %v1902 = phi i64 [ %v1900, %b1271 ], [ %v1901, %b1272 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1902, i64 0, i64 0)
  %v1904 = load i8, ptr %ctx, align 8
  %c402 = zext i8 %v1904 to i64
  %v1905 = icmp ne i64 %c402, 0
  br i1 %v1905, label %b1275, label %b1274
b1274:
  br label %b1270
b1275:
  %gcvolta1275 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1275, ptr %ctxtopo, align 8
  ret i64 0
b1276:
  %v1909 = add i64 0, 27
  %v1910 = ashr i64 %v1906, %v1909
  br label %b1268
b1277:
  %gcvolta1277 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1277, ptr %ctxtopo, align 8
  ret i64 0
b1279:
  %v1918 = ashr i64 %v0, 1
  %v1919 = icmp sle i64 %v1918, %v1915
  br label %b1281
b1280:
  %v1920 = call i64 @df.box_int(i64 %v1915)
  store i64 %v1920, ptr %gcs1
  %sa1921_0 = getelementptr [2 x i64], ptr %sargs1921, i64 0, i64 0
  store i64 %v1920, ptr %sa1921_0
  %sat1921 = getelementptr [2 x i64], ptr %sargs1921, i64 0, i64 1
  store i64 0, ptr %sat1921
  %sic1921 = getelementptr i64, ptr %area, i64 141792
  %sf1921 = call ptr @df.selector(ptr %sic1921, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1921 = call i64 %sf1921(i64 %v0, ptr %sargs1921, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1921, ptr %gcs1
  %v1922 = load i8, ptr %ctx, align 8
  %c403 = zext i8 %v1922 to i64
  %v1923 = icmp ne i64 %c403, 0
  br i1 %v1923, label %b1283, label %b1282
b1281:
  %v1927 = phi i1 [ %v1919, %b1279 ], [ %v1924, %b1284 ]
  br i1 %v1927, label %b1286, label %b1287
b1282:
  %v1924 = call i1 @df.unbox_bool(i64 %v1921)
  %v1925 = load i8, ptr %ctx, align 8
  %c404 = zext i8 %v1925 to i64
  %v1926 = icmp ne i64 %c404, 0
  br i1 %v1926, label %b1285, label %b1284
b1283:
  %gcvolta1283 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1283, ptr %ctxtopo, align 8
  ret i64 0
b1284:
  br label %b1281
b1285:
  %gcvolta1285 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1285, ptr %ctxtopo, align 8
  ret i64 0
b1286:
  %v1928 = add i64 0, 0
  br label %b1288
b1287:
  store i64 0, ptr %gcs1
  %v1930 = icmp eq i64 %v0, 0
  %v1931 = call i64 @df.class(i64 %v0)
  %v1932 = call i8 @df.subclass(i64 %v1931, i64 674)
  %c405 = zext i8 %v1932 to i64
  %v1933 = icmp ne i64 %c405, 0
  %v1934 = xor i1 %v1930, true
  %c406 = zext i1 %v1933 to i64
  %c407 = zext i1 %v1934 to i64
  %v1935 = and i64 %c406, %c407
  %v1936 = icmp ne i64 %v1935, 0
  br i1 %v1936, label %b1290, label %b1289
b1288:
  %v1950 = phi i64 [ %v1928, %b1286 ], [ %v1949, %b1296 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1951 = call i64 @df.box_int(i64 %v1950)
  %xcp1952 = load i8, ptr %ctx, align 8
  %xcn1952 = icmp ne i8 %xcp1952, 0
  %xce1952 = call i1 @llvm.expect.i1(i1 %xcn1952, i1 false)
  br i1 %xce1952, label %xc1952.limpar, label %xc1952.fim
xc1952.limpar:
  call void @dartforge_exception_clear()
  br label %xc1952.fim
xc1952.fim:
  %gcvolta1288 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1288, ptr %ctxtopo, align 8
  ret i64 %v1951
b1289:
  %ga1937 = getelementptr i64, ptr %area, i64 10809
  %v1937 = load i8, ptr %ga1937
  %c408 = zext i8 %v1937 to i64
  %v1938 = icmp ne i64 %c408, 0
  br i1 %v1938, label %b1291, label %b1292
b1290:
  %v1945 = call i64 @df.unbox_int(i64 %v0)
  %v1946 = load i8, ptr %ctx, align 8
  %c409 = zext i8 %v1946 to i64
  %v1947 = icmp ne i64 %c409, 0
  br i1 %v1947, label %b1297, label %b1296
b1291:
  %ga1939 = getelementptr i64, ptr %area, i64 10808
  %v1939 = load i64, ptr %ga1939
  br label %b1293
b1292:
  %v1940 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1293
b1293:
  %v1941 = phi i64 [ %v1939, %b1291 ], [ %v1940, %b1292 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1941, i64 0, i64 0)
  %v1943 = load i8, ptr %ctx, align 8
  %c410 = zext i8 %v1943 to i64
  %v1944 = icmp ne i64 %c410, 0
  br i1 %v1944, label %b1295, label %b1294
b1294:
  br label %b1290
b1295:
  %gcvolta1295 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1295, ptr %ctxtopo, align 8
  ret i64 0
b1296:
  %v1948 = add i64 0, 23
  %v1949 = ashr i64 %v1945, %v1948
  br label %b1288
b1297:
  %gcvolta1297 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1297, ptr %ctxtopo, align 8
  ret i64 0
b1299:
  %v1957 = ashr i64 %v0, 1
  %v1958 = icmp sle i64 %v1957, %v1954
  br label %b1301
b1300:
  %v1959 = call i64 @df.box_int(i64 %v1954)
  store i64 %v1959, ptr %gcs1
  %sa1960_0 = getelementptr [2 x i64], ptr %sargs1960, i64 0, i64 0
  store i64 %v1959, ptr %sa1960_0
  %sat1960 = getelementptr [2 x i64], ptr %sargs1960, i64 0, i64 1
  store i64 0, ptr %sat1960
  %sic1960 = getelementptr i64, ptr %area, i64 141794
  %sf1960 = call ptr @df.selector(ptr %sic1960, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1960 = call i64 %sf1960(i64 %v0, ptr %sargs1960, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1960, ptr %gcs1
  %v1961 = load i8, ptr %ctx, align 8
  %c411 = zext i8 %v1961 to i64
  %v1962 = icmp ne i64 %c411, 0
  br i1 %v1962, label %b1303, label %b1302
b1301:
  %v1966 = phi i1 [ %v1958, %b1299 ], [ %v1963, %b1304 ]
  br i1 %v1966, label %b1306, label %b1307
b1302:
  %v1963 = call i1 @df.unbox_bool(i64 %v1960)
  %v1964 = load i8, ptr %ctx, align 8
  %c412 = zext i8 %v1964 to i64
  %v1965 = icmp ne i64 %c412, 0
  br i1 %v1965, label %b1305, label %b1304
b1303:
  %gcvolta1303 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1303, ptr %ctxtopo, align 8
  ret i64 0
b1304:
  br label %b1301
b1305:
  %gcvolta1305 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1305, ptr %ctxtopo, align 8
  ret i64 0
b1306:
  %v1967 = add i64 0, 0
  br label %b1308
b1307:
  store i64 0, ptr %gcs1
  %v1969 = icmp eq i64 %v0, 0
  %v1970 = call i64 @df.class(i64 %v0)
  %v1971 = call i8 @df.subclass(i64 %v1970, i64 674)
  %c413 = zext i8 %v1971 to i64
  %v1972 = icmp ne i64 %c413, 0
  %v1973 = xor i1 %v1969, true
  %c414 = zext i1 %v1972 to i64
  %c415 = zext i1 %v1973 to i64
  %v1974 = and i64 %c414, %c415
  %v1975 = icmp ne i64 %v1974, 0
  br i1 %v1975, label %b1310, label %b1309
b1308:
  %v1989 = phi i64 [ %v1967, %b1306 ], [ %v1988, %b1316 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v1990 = call i64 @df.box_int(i64 %v1989)
  %xcp1991 = load i8, ptr %ctx, align 8
  %xcn1991 = icmp ne i8 %xcp1991, 0
  %xce1991 = call i1 @llvm.expect.i1(i1 %xcn1991, i1 false)
  br i1 %xce1991, label %xc1991.limpar, label %xc1991.fim
xc1991.limpar:
  call void @dartforge_exception_clear()
  br label %xc1991.fim
xc1991.fim:
  %gcvolta1308 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1308, ptr %ctxtopo, align 8
  ret i64 %v1990
b1309:
  %ga1976 = getelementptr i64, ptr %area, i64 10809
  %v1976 = load i8, ptr %ga1976
  %c416 = zext i8 %v1976 to i64
  %v1977 = icmp ne i64 %c416, 0
  br i1 %v1977, label %b1311, label %b1312
b1310:
  %v1984 = call i64 @df.unbox_int(i64 %v0)
  %v1985 = load i8, ptr %ctx, align 8
  %c417 = zext i8 %v1985 to i64
  %v1986 = icmp ne i64 %c417, 0
  br i1 %v1986, label %b1317, label %b1316
b1311:
  %ga1978 = getelementptr i64, ptr %area, i64 10808
  %v1978 = load i64, ptr %ga1978
  br label %b1313
b1312:
  %v1979 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1313
b1313:
  %v1980 = phi i64 [ %v1978, %b1311 ], [ %v1979, %b1312 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v1980, i64 0, i64 0)
  %v1982 = load i8, ptr %ctx, align 8
  %c418 = zext i8 %v1982 to i64
  %v1983 = icmp ne i64 %c418, 0
  br i1 %v1983, label %b1315, label %b1314
b1314:
  br label %b1310
b1315:
  %gcvolta1315 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1315, ptr %ctxtopo, align 8
  ret i64 0
b1316:
  %v1987 = add i64 0, 16
  %v1988 = ashr i64 %v1984, %v1987
  br label %b1308
b1317:
  %gcvolta1317 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1317, ptr %ctxtopo, align 8
  ret i64 0
b1319:
  %v1996 = ashr i64 %v0, 1
  %v1997 = icmp sle i64 %v1996, %v1993
  br label %b1321
b1320:
  %v1998 = call i64 @df.box_int(i64 %v1993)
  store i64 %v1998, ptr %gcs1
  %sa1999_0 = getelementptr [2 x i64], ptr %sargs1999, i64 0, i64 0
  store i64 %v1998, ptr %sa1999_0
  %sat1999 = getelementptr [2 x i64], ptr %sargs1999, i64 0, i64 1
  store i64 0, ptr %sat1999
  %sic1999 = getelementptr i64, ptr %area, i64 141796
  %sf1999 = call ptr @df.selector(ptr %sic1999, i64 %v0, i64 2959114654109919793, ptr @df.seln.97, i64 5)
  %v1999 = call i64 %sf1999(i64 %v0, ptr %sargs1999, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v1999, ptr %gcs1
  %v2000 = load i8, ptr %ctx, align 8
  %c419 = zext i8 %v2000 to i64
  %v2001 = icmp ne i64 %c419, 0
  br i1 %v2001, label %b1323, label %b1322
b1321:
  %v2005 = phi i1 [ %v1997, %b1319 ], [ %v2002, %b1324 ]
  br i1 %v2005, label %b1326, label %b1327
b1322:
  %v2002 = call i1 @df.unbox_bool(i64 %v1999)
  %v2003 = load i8, ptr %ctx, align 8
  %c420 = zext i8 %v2003 to i64
  %v2004 = icmp ne i64 %c420, 0
  br i1 %v2004, label %b1325, label %b1324
b1323:
  %gcvolta1323 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1323, ptr %ctxtopo, align 8
  ret i64 0
b1324:
  br label %b1321
b1325:
  %gcvolta1325 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1325, ptr %ctxtopo, align 8
  ret i64 0
b1326:
  %v2006 = add i64 0, 0
  br label %b1328
b1327:
  store i64 0, ptr %gcs1
  %v2008 = icmp eq i64 %v0, 0
  %v2009 = call i64 @df.class(i64 %v0)
  %v2010 = call i8 @df.subclass(i64 %v2009, i64 674)
  %c421 = zext i8 %v2010 to i64
  %v2011 = icmp ne i64 %c421, 0
  %v2012 = xor i1 %v2008, true
  %c422 = zext i1 %v2011 to i64
  %c423 = zext i1 %v2012 to i64
  %v2013 = and i64 %c422, %c423
  %v2014 = icmp ne i64 %v2013, 0
  br i1 %v2014, label %b1330, label %b1329
b1328:
  %v2028 = phi i64 [ %v2006, %b1326 ], [ %v2027, %b1336 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2029 = call i64 @df.box_int(i64 %v2028)
  %xcp2030 = load i8, ptr %ctx, align 8
  %xcn2030 = icmp ne i8 %xcp2030, 0
  %xce2030 = call i1 @llvm.expect.i1(i1 %xcn2030, i1 false)
  br i1 %xce2030, label %xc2030.limpar, label %xc2030.fim
xc2030.limpar:
  call void @dartforge_exception_clear()
  br label %xc2030.fim
xc2030.fim:
  %gcvolta1328 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1328, ptr %ctxtopo, align 8
  ret i64 %v2029
b1329:
  %ga2015 = getelementptr i64, ptr %area, i64 10809
  %v2015 = load i8, ptr %ga2015
  %c424 = zext i8 %v2015 to i64
  %v2016 = icmp ne i64 %c424, 0
  br i1 %v2016, label %b1331, label %b1332
b1330:
  %v2023 = call i64 @df.unbox_int(i64 %v0)
  %v2024 = load i8, ptr %ctx, align 8
  %c425 = zext i8 %v2024 to i64
  %v2025 = icmp ne i64 %c425, 0
  br i1 %v2025, label %b1337, label %b1336
b1331:
  %ga2017 = getelementptr i64, ptr %area, i64 10808
  %v2017 = load i64, ptr %ga2017
  br label %b1333
b1332:
  %v2018 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1333
b1333:
  %v2019 = phi i64 [ %v2017, %b1331 ], [ %v2018, %b1332 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2019, i64 0, i64 0)
  %v2021 = load i8, ptr %ctx, align 8
  %c426 = zext i8 %v2021 to i64
  %v2022 = icmp ne i64 %c426, 0
  br i1 %v2022, label %b1335, label %b1334
b1334:
  br label %b1330
b1335:
  %gcvolta1335 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1335, ptr %ctxtopo, align 8
  ret i64 0
b1336:
  %v2026 = add i64 0, 1
  %v2027 = shl i64 %v2023, %v2026
  br label %b1328
b1337:
  %gcvolta1337 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1337, ptr %ctxtopo, align 8
  ret i64 0
b1339:
  %ga2039 = getelementptr i64, ptr %area, i64 10809
  %v2039 = load i8, ptr %ga2039
  %c427 = zext i8 %v2039 to i64
  %v2040 = icmp ne i64 %c427, 0
  br i1 %v2040, label %b1341, label %b1342
b1340:
  %v2047 = call i64 @df.unbox_int(i64 %v0)
  %v2048 = load i8, ptr %ctx, align 8
  %c428 = zext i8 %v2048 to i64
  %v2049 = icmp ne i64 %c428, 0
  br i1 %v2049, label %b1347, label %b1346
b1341:
  %ga2041 = getelementptr i64, ptr %area, i64 10808
  %v2041 = load i64, ptr %ga2041
  br label %b1343
b1342:
  %v2042 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1343
b1343:
  %v2043 = phi i64 [ %v2041, %b1341 ], [ %v2042, %b1342 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2043, i64 0, i64 0)
  %v2045 = load i8, ptr %ctx, align 8
  %c429 = zext i8 %v2045 to i64
  %v2046 = icmp ne i64 %c429, 0
  br i1 %v2046, label %b1345, label %b1344
b1344:
  br label %b1340
b1345:
  %gcvolta1345 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1345, ptr %ctxtopo, align 8
  ret i64 0
b1346:
  %v2050 = add i64 0, 24
  %v2051 = ashr i64 %v2047, %v2050
  store i64 0, ptr %gcs0
  %v2052 = call i64 @df.box_int(i64 %v2051)
  %xcp2053 = load i8, ptr %ctx, align 8
  %xcn2053 = icmp ne i8 %xcp2053, 0
  %xce2053 = call i1 @llvm.expect.i1(i1 %xcn2053, i1 false)
  br i1 %xce2053, label %xc2053.limpar, label %xc2053.fim
xc2053.limpar:
  call void @dartforge_exception_clear()
  br label %xc2053.fim
xc2053.fim:
  %gcvolta1346 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1346, ptr %ctxtopo, align 8
  ret i64 %v2052
b1347:
  %gcvolta1347 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1347, ptr %ctxtopo, align 8
  ret i64 0
b1349:
  %ga2062 = getelementptr i64, ptr %area, i64 10809
  %v2062 = load i8, ptr %ga2062
  %c430 = zext i8 %v2062 to i64
  %v2063 = icmp ne i64 %c430, 0
  br i1 %v2063, label %b1351, label %b1352
b1350:
  %v2070 = call i64 @df.unbox_int(i64 %v0)
  %v2071 = load i8, ptr %ctx, align 8
  %c431 = zext i8 %v2071 to i64
  %v2072 = icmp ne i64 %c431, 0
  br i1 %v2072, label %b1357, label %b1356
b1351:
  %ga2064 = getelementptr i64, ptr %area, i64 10808
  %v2064 = load i64, ptr %ga2064
  br label %b1353
b1352:
  %v2065 = call i64 @df.rti.877bf89dd991b2af.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart()
  br label %b1353
b1353:
  %v2066 = phi i64 [ %v2064, %b1351 ], [ %v2065, %b1352 ]
  call void @dartforge_rti_as_with_context(i64 %v0, i64 %v2066, i64 0, i64 0)
  %v2068 = load i8, ptr %ctx, align 8
  %c432 = zext i8 %v2068 to i64
  %v2069 = icmp ne i64 %c432, 0
  br i1 %v2069, label %b1355, label %b1354
b1354:
  br label %b1350
b1355:
  %gcvolta1355 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1355, ptr %ctxtopo, align 8
  ret i64 0
b1356:
  %v2073 = add i64 0, 16
  %v2074 = ashr i64 %v2070, %v2073
  store i64 0, ptr %gcs0
  %v2075 = call i64 @df.box_int(i64 %v2074)
  %xcp2076 = load i8, ptr %ctx, align 8
  %xcn2076 = icmp ne i8 %xcp2076, 0
  %xce2076 = call i1 @llvm.expect.i1(i1 %xcn2076, i1 false)
  br i1 %xce2076, label %xc2076.limpar, label %xc2076.fim
xc2076.limpar:
  call void @dartforge_exception_clear()
  br label %xc2076.fim
xc2076.fim:
  %gcvolta1356 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1356, ptr %ctxtopo, align 8
  ret i64 %v2075
b1357:
  %gcvolta1357 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1357, ptr %ctxtopo, align 8
  ret i64 0
b1360:
  store i64 0, ptr %gcs1
  %v2085 = call double @df.unbox_double(i64 %v2082)
  %v2086 = load i8, ptr %ctx, align 8
  %c433 = zext i8 %v2086 to i64
  %v2087 = icmp ne i64 %c433, 0
  br i1 %v2087, label %b1363, label %b1362
b1361:
  %gcvolta1361 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1361, ptr %ctxtopo, align 8
  ret i64 0
b1362:
  %xcp2089 = load i8, ptr %ctx, align 8
  %xcn2089 = icmp ne i8 %xcp2089, 0
  %xce2089 = call i1 @llvm.expect.i1(i1 %xcn2089, i1 false)
  br i1 %xce2089, label %xc2089.limpar, label %xc2089.fim
xc2089.limpar:
  call void @dartforge_exception_clear()
  br label %xc2089.fim
xc2089.fim:
  %gcvolta1362 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1362, ptr %ctxtopo, align 8
  ret i64 %v2082
b1363:
  %gcvolta1363 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1363, ptr %ctxtopo, align 8
  ret i64 0
b1377:
  %xcp2484 = load i8, ptr %ctx, align 8
  %xcn2484 = icmp ne i8 %xcp2484, 0
  %xce2484 = call i1 @llvm.expect.i1(i1 %xcn2484, i1 false)
  br i1 %xce2484, label %xc2484.limpar, label %xc2484.fim
xc2484.limpar:
  call void @dartforge_exception_clear()
  br label %xc2484.fim
xc2484.fim:
  %gcvolta1377 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1377, ptr %ctxtopo, align 8
  ret i64 %v0
b1378:
  store i64 0, ptr %gcs1
  %v2095 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  store i64 %v2095, ptr %gcs1
  %v2096 = load i8, ptr %ctx, align 8
  %c434 = zext i8 %v2096 to i64
  %v2097 = icmp ne i64 %c434, 0
  br i1 %v2097, label %b1384, label %b1383
b1379:
  %v2094 = icmp eq i64 %v2091, %v6
  br i1 %v2094, label %b1381, label %b1378
b1380:
  %gcvolta1380 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1380, ptr %ctxtopo, align 8
  ret i64 0
b1381:
  %v2140 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2141 = call i64 @df.box_int(i64 %v2140)
  store i64 %v2141, ptr %gcs1
  %v2142 = icmp eq i64 %v0, 0
  %v2143 = icmp eq i64 %v2141, 0
  %c435 = zext i1 %v2142 to i64
  %c436 = zext i1 %v2143 to i64
  %v2144 = or i64 %c435, %c436
  %v2145 = icmp ne i64 %v2144, 0
  br i1 %v2145, label %b1428, label %b1429
b1382:
  store i64 0, ptr %gcs1
  %v2099 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  store i64 %v2099, ptr %gcs1
  %v2100 = load i8, ptr %ctx, align 8
  %c437 = zext i8 %v2100 to i64
  %v2101 = icmp ne i64 %c437, 0
  br i1 %v2101, label %b1388, label %b1387
b1383:
  %v2098 = icmp eq i64 %v2095, %v6
  br i1 %v2098, label %b1385, label %b1382
b1384:
  %gcvolta1384 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1384, ptr %ctxtopo, align 8
  ret i64 0
b1385:
  %v2160 = add i64 0, 0
  %v2161 = add i64 0, 1
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2162 = call i64 @df.box_int(i64 %v2160)
  store i64 %v2162, ptr %gcs1
  %v2163 = call i64 @df.box_int(i64 %v2161)
  store i64 %v2163, ptr %gcs2
  %sa2164_0 = getelementptr [3 x i64], ptr %sargs2164, i64 0, i64 0
  store i64 %v2162, ptr %sa2164_0
  %sa2164_1 = getelementptr [3 x i64], ptr %sargs2164, i64 0, i64 1
  store i64 %v2163, ptr %sa2164_1
  %sat2164 = getelementptr [3 x i64], ptr %sargs2164, i64 0, i64 2
  store i64 0, ptr %sat2164
  %sic2164 = getelementptr i64, ptr %area, i64 141798
  %sf2164 = call ptr @df.selector(ptr %sic2164, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2164 = call i64 %sf2164(i64 %v0, ptr %sargs2164, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2164, ptr %gcs0
  %v2165 = load i8, ptr %ctx, align 8
  %c438 = zext i8 %v2165 to i64
  %v2166 = icmp ne i64 %c438, 0
  br i1 %v2166, label %b1440, label %b1439
b1386:
  store i64 0, ptr %gcs1
  %v2103 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  store i64 %v2103, ptr %gcs1
  %v2104 = load i8, ptr %ctx, align 8
  %c439 = zext i8 %v2104 to i64
  %v2105 = icmp ne i64 %c439, 0
  br i1 %v2105, label %b1392, label %b1391
b1387:
  %v2102 = icmp eq i64 %v2099, %v6
  br i1 %v2102, label %b1389, label %b1386
b1388:
  %gcvolta1388 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1388, ptr %ctxtopo, align 8
  ret i64 0
b1389:
  %v2187 = add i64 0, 0
  %v2188 = add i64 0, 1
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2189 = call i64 @df.box_int(i64 %v2187)
  store i64 %v2189, ptr %gcs1
  %v2190 = call i64 @df.box_int(i64 %v2188)
  store i64 %v2190, ptr %gcs2
  %sa2191_0 = getelementptr [3 x i64], ptr %sargs2191, i64 0, i64 0
  store i64 %v2189, ptr %sa2191_0
  %sa2191_1 = getelementptr [3 x i64], ptr %sargs2191, i64 0, i64 1
  store i64 %v2190, ptr %sa2191_1
  %sat2191 = getelementptr [3 x i64], ptr %sargs2191, i64 0, i64 2
  store i64 0, ptr %sat2191
  %sic2191 = getelementptr i64, ptr %area, i64 141800
  %sf2191 = call ptr @df.selector(ptr %sic2191, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2191 = call i64 %sf2191(i64 %v0, ptr %sargs2191, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2191, ptr %gcs0
  %v2192 = load i8, ptr %ctx, align 8
  %c440 = zext i8 %v2192 to i64
  %v2193 = icmp ne i64 %c440, 0
  br i1 %v2193, label %b1452, label %b1451
b1390:
  store i64 0, ptr %gcs1
  %v2107 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  store i64 %v2107, ptr %gcs1
  %v2108 = load i8, ptr %ctx, align 8
  %c441 = zext i8 %v2108 to i64
  %v2109 = icmp ne i64 %c441, 0
  br i1 %v2109, label %b1396, label %b1395
b1391:
  %v2106 = icmp eq i64 %v2103, %v6
  br i1 %v2106, label %b1393, label %b1390
b1392:
  %gcvolta1392 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1392, ptr %ctxtopo, align 8
  ret i64 0
b1393:
  %v2214 = add i64 0, 0
  %v2215 = add i64 0, 1
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2216 = call i64 @df.box_int(i64 %v2214)
  store i64 %v2216, ptr %gcs1
  %v2217 = call i64 @df.box_int(i64 %v2215)
  store i64 %v2217, ptr %gcs2
  %sa2218_0 = getelementptr [3 x i64], ptr %sargs2218, i64 0, i64 0
  store i64 %v2216, ptr %sa2218_0
  %sa2218_1 = getelementptr [3 x i64], ptr %sargs2218, i64 0, i64 1
  store i64 %v2217, ptr %sa2218_1
  %sat2218 = getelementptr [3 x i64], ptr %sargs2218, i64 0, i64 2
  store i64 0, ptr %sat2218
  %sic2218 = getelementptr i64, ptr %area, i64 141802
  %sf2218 = call ptr @df.selector(ptr %sic2218, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2218 = call i64 %sf2218(i64 %v0, ptr %sargs2218, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2218, ptr %gcs0
  %v2219 = load i8, ptr %ctx, align 8
  %c442 = zext i8 %v2219 to i64
  %v2220 = icmp ne i64 %c442, 0
  br i1 %v2220, label %b1464, label %b1463
b1394:
  store i64 0, ptr %gcs1
  %v2111 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  store i64 %v2111, ptr %gcs1
  %v2112 = load i8, ptr %ctx, align 8
  %c443 = zext i8 %v2112 to i64
  %v2113 = icmp ne i64 %c443, 0
  br i1 %v2113, label %b1400, label %b1399
b1395:
  %v2110 = icmp eq i64 %v2107, %v6
  br i1 %v2110, label %b1397, label %b1394
b1396:
  %gcvolta1396 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1396, ptr %ctxtopo, align 8
  ret i64 0
b1397:
  %v2241 = add i64 0, 0
  %v2242 = add i64 0, 1
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2243 = call i64 @df.box_int(i64 %v2241)
  store i64 %v2243, ptr %gcs1
  %v2244 = call i64 @df.box_int(i64 %v2242)
  store i64 %v2244, ptr %gcs2
  %sa2245_0 = getelementptr [3 x i64], ptr %sargs2245, i64 0, i64 0
  store i64 %v2243, ptr %sa2245_0
  %sa2245_1 = getelementptr [3 x i64], ptr %sargs2245, i64 0, i64 1
  store i64 %v2244, ptr %sa2245_1
  %sat2245 = getelementptr [3 x i64], ptr %sargs2245, i64 0, i64 2
  store i64 0, ptr %sat2245
  %sic2245 = getelementptr i64, ptr %area, i64 141804
  %sf2245 = call ptr @df.selector(ptr %sic2245, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2245 = call i64 %sf2245(i64 %v0, ptr %sargs2245, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2245, ptr %gcs0
  %v2246 = load i8, ptr %ctx, align 8
  %c444 = zext i8 %v2246 to i64
  %v2247 = icmp ne i64 %c444, 0
  br i1 %v2247, label %b1476, label %b1475
b1398:
  store i64 0, ptr %gcs1
  %v2115 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  store i64 %v2115, ptr %gcs1
  %v2116 = load i8, ptr %ctx, align 8
  %c445 = zext i8 %v2116 to i64
  %v2117 = icmp ne i64 %c445, 0
  br i1 %v2117, label %b1404, label %b1403
b1399:
  %v2114 = icmp eq i64 %v2111, %v6
  br i1 %v2114, label %b1401, label %b1398
b1400:
  %gcvolta1400 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1400, ptr %ctxtopo, align 8
  ret i64 0
b1401:
  %v2268 = add i64 0, 0
  %v2269 = add i64 0, 1
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2270 = call i64 @df.box_int(i64 %v2268)
  store i64 %v2270, ptr %gcs1
  %v2271 = call i64 @df.box_int(i64 %v2269)
  store i64 %v2271, ptr %gcs2
  %sa2272_0 = getelementptr [3 x i64], ptr %sargs2272, i64 0, i64 0
  store i64 %v2270, ptr %sa2272_0
  %sa2272_1 = getelementptr [3 x i64], ptr %sargs2272, i64 0, i64 1
  store i64 %v2271, ptr %sa2272_1
  %sat2272 = getelementptr [3 x i64], ptr %sargs2272, i64 0, i64 2
  store i64 0, ptr %sat2272
  %sic2272 = getelementptr i64, ptr %area, i64 141806
  %sf2272 = call ptr @df.selector(ptr %sic2272, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2272 = call i64 %sf2272(i64 %v0, ptr %sargs2272, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2272, ptr %gcs0
  %v2273 = load i8, ptr %ctx, align 8
  %c446 = zext i8 %v2273 to i64
  %v2274 = icmp ne i64 %c446, 0
  br i1 %v2274, label %b1488, label %b1487
b1402:
  store i64 0, ptr %gcs1
  %v2119 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  store i64 %v2119, ptr %gcs1
  %v2120 = load i8, ptr %ctx, align 8
  %c447 = zext i8 %v2120 to i64
  %v2121 = icmp ne i64 %c447, 0
  br i1 %v2121, label %b1408, label %b1407
b1403:
  %v2118 = icmp eq i64 %v2115, %v6
  br i1 %v2118, label %b1405, label %b1402
b1404:
  %gcvolta1404 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1404, ptr %ctxtopo, align 8
  ret i64 0
b1405:
  %v2295 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2296 = and i64 %v0, 1
  %v2297 = icmp ne i64 %v2296, 0
  br i1 %v2297, label %b1499, label %b1500
b1406:
  store i64 0, ptr %gcs1
  %v2123 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  store i64 %v2123, ptr %gcs1
  %v2124 = load i8, ptr %ctx, align 8
  %c448 = zext i8 %v2124 to i64
  %v2125 = icmp ne i64 %c448, 0
  br i1 %v2125, label %b1412, label %b1411
b1407:
  %v2122 = icmp eq i64 %v2119, %v6
  br i1 %v2122, label %b1409, label %b1406
b1408:
  %gcvolta1408 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1408, ptr %ctxtopo, align 8
  ret i64 0
b1409:
  %v2358 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2359 = and i64 %v0, 1
  %v2360 = icmp ne i64 %v2359, 0
  br i1 %v2360, label %b1528, label %b1529
b1410:
  store i64 0, ptr %gcs1
  %v2127 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  store i64 %v2127, ptr %gcs1
  %v2128 = load i8, ptr %ctx, align 8
  %c449 = zext i8 %v2128 to i64
  %v2129 = icmp ne i64 %c449, 0
  br i1 %v2129, label %b1416, label %b1415
b1411:
  %v2126 = icmp eq i64 %v2123, %v6
  br i1 %v2126, label %b1413, label %b1410
b1412:
  %gcvolta1412 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1412, ptr %ctxtopo, align 8
  ret i64 0
b1413:
  %v2421 = add i64 0, 0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2422 = and i64 %v0, 1
  %v2423 = icmp ne i64 %v2422, 0
  br i1 %v2423, label %b1557, label %b1558
b1414:
  store i64 0, ptr %gcs1
  %v2131 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  store i64 %v2131, ptr %gcs1
  %v2132 = load i8, ptr %ctx, align 8
  %c450 = zext i8 %v2132 to i64
  %v2133 = icmp ne i64 %c450, 0
  br i1 %v2133, label %b1421, label %b1420
b1415:
  %v2130 = icmp eq i64 %v2127, %v6
  br i1 %v2130, label %b1417, label %b1414
b1416:
  %gcvolta1416 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1416, ptr %ctxtopo, align 8
  ret i64 0
b1417:
  br label %b1377
b1419:
  store i64 0, ptr %gcs1
  %v2135 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float64()
  store i64 %v2135, ptr %gcs1
  %v2136 = load i8, ptr %ctx, align 8
  %c451 = zext i8 %v2136 to i64
  %v2137 = icmp ne i64 %c451, 0
  br i1 %v2137, label %b1426, label %b1425
b1420:
  %v2134 = icmp eq i64 %v2131, %v6
  br i1 %v2134, label %b1422, label %b1419
b1421:
  %gcvolta1421 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1421, ptr %ctxtopo, align 8
  ret i64 0
b1422:
  br label %b1377
b1424:
  br label %b12
b1425:
  %v2138 = icmp eq i64 %v2135, %v6
  br i1 %v2138, label %b1427, label %b1424
b1426:
  %gcvolta1426 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1426, ptr %ctxtopo, align 8
  ret i64 0
b1427:
  br label %b1377
b1428:
  %v2146 = icmp eq i64 %v0, %v2141
  br label %b1430
b1429:
  %sa2147_0 = getelementptr [2 x i64], ptr %sargs2147, i64 0, i64 0
  store i64 %v2141, ptr %sa2147_0
  %sat2147 = getelementptr [2 x i64], ptr %sargs2147, i64 0, i64 1
  store i64 0, ptr %sat2147
  %sic2147 = getelementptr i64, ptr %area, i64 141808
  %sf2147 = call ptr @df.selector(ptr %sic2147, i64 %v0, i64 -9163797671095228218, ptr @df.seln.13, i64 4)
  %v2147 = call i64 %sf2147(i64 %v0, ptr %sargs2147, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2147, ptr %gcs0
  %v2148 = load i8, ptr %ctx, align 8
  %c452 = zext i8 %v2148 to i64
  %v2149 = icmp ne i64 %c452, 0
  br i1 %v2149, label %b1432, label %b1431
b1430:
  %v2153 = phi i1 [ %v2146, %b1428 ], [ %v2150, %b1433 ]
  br i1 %v2153, label %b1435, label %b1436
b1431:
  store i64 0, ptr %gcs1
  %v2150 = call i1 @df.unbox_bool(i64 %v2147)
  %v2151 = load i8, ptr %ctx, align 8
  %c453 = zext i8 %v2151 to i64
  %v2152 = icmp ne i64 %c453, 0
  br i1 %v2152, label %b1434, label %b1433
b1432:
  %gcvolta1432 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1432, ptr %ctxtopo, align 8
  ret i64 0
b1433:
  br label %b1430
b1434:
  %gcvolta1434 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1434, ptr %ctxtopo, align 8
  ret i64 0
b1435:
  %v2154 = add i64 0, 0
  br label %b1437
b1436:
  %v2155 = add i64 0, 1
  br label %b1437
b1437:
  %v2156 = phi i64 [ %v2154, %b1435 ], [ %v2155, %b1436 ]
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %v2157 = call i64 @df.box_int(i64 %v2156)
  %xcp2158 = load i8, ptr %ctx, align 8
  %xcn2158 = icmp ne i8 %xcp2158, 0
  %xce2158 = call i1 @llvm.expect.i1(i1 %xcn2158, i1 false)
  br i1 %xce2158, label %xc2158.limpar, label %xc2158.fim
xc2158.limpar:
  call void @dartforge_exception_clear()
  br label %xc2158.fim
xc2158.fim:
  %gcvolta1437 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1437, ptr %ctxtopo, align 8
  ret i64 %v2157
b1439:
  %v2167 = add i64 0, 3
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2168 = and i64 %v2164, 1
  %v2169 = icmp ne i64 %v2168, 0
  br i1 %v2169, label %b1441, label %b1442
b1440:
  %gcvolta1440 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1440, ptr %ctxtopo, align 8
  ret i64 0
b1441:
  %v2170 = ashr i64 %v2164, 1
  %v2171 = mul i64 %v2170, %v2167
  store i64 0, ptr %gcs0
  %v2172 = call i64 @df.box_int(i64 %v2171)
  br label %b1443
b1442:
  %v2173 = call i64 @df.box_int(i64 %v2167)
  store i64 %v2173, ptr %gcs1
  %sa2174_0 = getelementptr [2 x i64], ptr %sargs2174, i64 0, i64 0
  store i64 %v2173, ptr %sa2174_0
  %sat2174 = getelementptr [2 x i64], ptr %sargs2174, i64 0, i64 1
  store i64 0, ptr %sat2174
  %sic2174 = getelementptr i64, ptr %area, i64 141810
  %sf2174 = call ptr @df.selector(ptr %sic2174, i64 %v2164, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2174 = call i64 %sf2174(i64 %v2164, ptr %sargs2174, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2175 = load i8, ptr %ctx, align 8
  %c454 = zext i8 %v2175 to i64
  %v2176 = icmp ne i64 %c454, 0
  br i1 %v2176, label %b1445, label %b1444
b1443:
  %v2177 = phi i64 [ %v2172, %b1441 ], [ %v2174, %b1444 ]
  store i64 %v2177, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2178 = getelementptr [1 x i64], ptr %sargs2178, i64 0, i64 0
  store i64 0, ptr %sat2178
  %sic2178 = getelementptr i64, ptr %area, i64 141812
  %sf2178 = call ptr @df.selector(ptr %sic2178, i64 %v2177, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2178 = call i64 %sf2178(i64 %v2177, ptr %sargs2178, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2178, ptr %gcs0
  %v2179 = load i8, ptr %ctx, align 8
  %c455 = zext i8 %v2179 to i64
  %v2180 = icmp ne i64 %c455, 0
  br i1 %v2180, label %b1447, label %b1446
b1444:
  br label %b1443
b1445:
  %gcvolta1445 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1445, ptr %ctxtopo, align 8
  ret i64 0
b1446:
  %v2181 = call i64 @df.unbox_int(i64 %v2178)
  %v2182 = load i8, ptr %ctx, align 8
  %c456 = zext i8 %v2182 to i64
  %v2183 = icmp ne i64 %c456, 0
  br i1 %v2183, label %b1449, label %b1448
b1447:
  %gcvolta1447 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1447, ptr %ctxtopo, align 8
  ret i64 0
b1448:
  store i64 0, ptr %gcs0
  %v2184 = call i64 @df.box_int(i64 %v2181)
  %xcp2185 = load i8, ptr %ctx, align 8
  %xcn2185 = icmp ne i8 %xcp2185, 0
  %xce2185 = call i1 @llvm.expect.i1(i1 %xcn2185, i1 false)
  br i1 %xce2185, label %xc2185.limpar, label %xc2185.fim
xc2185.limpar:
  call void @dartforge_exception_clear()
  br label %xc2185.fim
xc2185.fim:
  %gcvolta1448 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1448, ptr %ctxtopo, align 8
  ret i64 %v2184
b1449:
  %gcvolta1449 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1449, ptr %ctxtopo, align 8
  ret i64 0
b1451:
  %v2194 = add i64 0, 15
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2195 = and i64 %v2191, 1
  %v2196 = icmp ne i64 %v2195, 0
  br i1 %v2196, label %b1453, label %b1454
b1452:
  %gcvolta1452 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1452, ptr %ctxtopo, align 8
  ret i64 0
b1453:
  %v2197 = ashr i64 %v2191, 1
  %v2198 = mul i64 %v2197, %v2194
  store i64 0, ptr %gcs0
  %v2199 = call i64 @df.box_int(i64 %v2198)
  br label %b1455
b1454:
  %v2200 = call i64 @df.box_int(i64 %v2194)
  store i64 %v2200, ptr %gcs1
  %sa2201_0 = getelementptr [2 x i64], ptr %sargs2201, i64 0, i64 0
  store i64 %v2200, ptr %sa2201_0
  %sat2201 = getelementptr [2 x i64], ptr %sargs2201, i64 0, i64 1
  store i64 0, ptr %sat2201
  %sic2201 = getelementptr i64, ptr %area, i64 141814
  %sf2201 = call ptr @df.selector(ptr %sic2201, i64 %v2191, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2201 = call i64 %sf2201(i64 %v2191, ptr %sargs2201, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2202 = load i8, ptr %ctx, align 8
  %c457 = zext i8 %v2202 to i64
  %v2203 = icmp ne i64 %c457, 0
  br i1 %v2203, label %b1457, label %b1456
b1455:
  %v2204 = phi i64 [ %v2199, %b1453 ], [ %v2201, %b1456 ]
  store i64 %v2204, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2205 = getelementptr [1 x i64], ptr %sargs2205, i64 0, i64 0
  store i64 0, ptr %sat2205
  %sic2205 = getelementptr i64, ptr %area, i64 141816
  %sf2205 = call ptr @df.selector(ptr %sic2205, i64 %v2204, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2205 = call i64 %sf2205(i64 %v2204, ptr %sargs2205, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2205, ptr %gcs0
  %v2206 = load i8, ptr %ctx, align 8
  %c458 = zext i8 %v2206 to i64
  %v2207 = icmp ne i64 %c458, 0
  br i1 %v2207, label %b1459, label %b1458
b1456:
  br label %b1455
b1457:
  %gcvolta1457 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1457, ptr %ctxtopo, align 8
  ret i64 0
b1458:
  %v2208 = call i64 @df.unbox_int(i64 %v2205)
  %v2209 = load i8, ptr %ctx, align 8
  %c459 = zext i8 %v2209 to i64
  %v2210 = icmp ne i64 %c459, 0
  br i1 %v2210, label %b1461, label %b1460
b1459:
  %gcvolta1459 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1459, ptr %ctxtopo, align 8
  ret i64 0
b1460:
  store i64 0, ptr %gcs0
  %v2211 = call i64 @df.box_int(i64 %v2208)
  %xcp2212 = load i8, ptr %ctx, align 8
  %xcn2212 = icmp ne i8 %xcp2212, 0
  %xce2212 = call i1 @llvm.expect.i1(i1 %xcn2212, i1 false)
  br i1 %xce2212, label %xc2212.limpar, label %xc2212.fim
xc2212.limpar:
  call void @dartforge_exception_clear()
  br label %xc2212.fim
xc2212.fim:
  %gcvolta1460 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1460, ptr %ctxtopo, align 8
  ret i64 %v2211
b1461:
  %gcvolta1461 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1461, ptr %ctxtopo, align 8
  ret i64 0
b1463:
  %v2221 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2222 = and i64 %v2218, 1
  %v2223 = icmp ne i64 %v2222, 0
  br i1 %v2223, label %b1465, label %b1466
b1464:
  %gcvolta1464 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1464, ptr %ctxtopo, align 8
  ret i64 0
b1465:
  %v2224 = ashr i64 %v2218, 1
  %v2225 = mul i64 %v2224, %v2221
  store i64 0, ptr %gcs0
  %v2226 = call i64 @df.box_int(i64 %v2225)
  br label %b1467
b1466:
  %v2227 = call i64 @df.box_int(i64 %v2221)
  store i64 %v2227, ptr %gcs1
  %sa2228_0 = getelementptr [2 x i64], ptr %sargs2228, i64 0, i64 0
  store i64 %v2227, ptr %sa2228_0
  %sat2228 = getelementptr [2 x i64], ptr %sargs2228, i64 0, i64 1
  store i64 0, ptr %sat2228
  %sic2228 = getelementptr i64, ptr %area, i64 141818
  %sf2228 = call ptr @df.selector(ptr %sic2228, i64 %v2218, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2228 = call i64 %sf2228(i64 %v2218, ptr %sargs2228, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2229 = load i8, ptr %ctx, align 8
  %c460 = zext i8 %v2229 to i64
  %v2230 = icmp ne i64 %c460, 0
  br i1 %v2230, label %b1469, label %b1468
b1467:
  %v2231 = phi i64 [ %v2226, %b1465 ], [ %v2228, %b1468 ]
  store i64 %v2231, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2232 = getelementptr [1 x i64], ptr %sargs2232, i64 0, i64 0
  store i64 0, ptr %sat2232
  %sic2232 = getelementptr i64, ptr %area, i64 141820
  %sf2232 = call ptr @df.selector(ptr %sic2232, i64 %v2231, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2232 = call i64 %sf2232(i64 %v2231, ptr %sargs2232, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2232, ptr %gcs0
  %v2233 = load i8, ptr %ctx, align 8
  %c461 = zext i8 %v2233 to i64
  %v2234 = icmp ne i64 %c461, 0
  br i1 %v2234, label %b1471, label %b1470
b1468:
  br label %b1467
b1469:
  %gcvolta1469 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1469, ptr %ctxtopo, align 8
  ret i64 0
b1470:
  %v2235 = call i64 @df.unbox_int(i64 %v2232)
  %v2236 = load i8, ptr %ctx, align 8
  %c462 = zext i8 %v2236 to i64
  %v2237 = icmp ne i64 %c462, 0
  br i1 %v2237, label %b1473, label %b1472
b1471:
  %gcvolta1471 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1471, ptr %ctxtopo, align 8
  ret i64 0
b1472:
  store i64 0, ptr %gcs0
  %v2238 = call i64 @df.box_int(i64 %v2235)
  %xcp2239 = load i8, ptr %ctx, align 8
  %xcn2239 = icmp ne i8 %xcp2239, 0
  %xce2239 = call i1 @llvm.expect.i1(i1 %xcn2239, i1 false)
  br i1 %xce2239, label %xc2239.limpar, label %xc2239.fim
xc2239.limpar:
  call void @dartforge_exception_clear()
  br label %xc2239.fim
xc2239.fim:
  %gcvolta1472 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1472, ptr %ctxtopo, align 8
  ret i64 %v2238
b1473:
  %gcvolta1473 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1473, ptr %ctxtopo, align 8
  ret i64 0
b1475:
  %v2248 = add i64 0, 65535
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2249 = and i64 %v2245, 1
  %v2250 = icmp ne i64 %v2249, 0
  br i1 %v2250, label %b1477, label %b1478
b1476:
  %gcvolta1476 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1476, ptr %ctxtopo, align 8
  ret i64 0
b1477:
  %v2251 = ashr i64 %v2245, 1
  %v2252 = mul i64 %v2251, %v2248
  store i64 0, ptr %gcs0
  %v2253 = call i64 @df.box_int(i64 %v2252)
  br label %b1479
b1478:
  %v2254 = call i64 @df.box_int(i64 %v2248)
  store i64 %v2254, ptr %gcs1
  %sa2255_0 = getelementptr [2 x i64], ptr %sargs2255, i64 0, i64 0
  store i64 %v2254, ptr %sa2255_0
  %sat2255 = getelementptr [2 x i64], ptr %sargs2255, i64 0, i64 1
  store i64 0, ptr %sat2255
  %sic2255 = getelementptr i64, ptr %area, i64 141822
  %sf2255 = call ptr @df.selector(ptr %sic2255, i64 %v2245, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2255 = call i64 %sf2255(i64 %v2245, ptr %sargs2255, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2256 = load i8, ptr %ctx, align 8
  %c463 = zext i8 %v2256 to i64
  %v2257 = icmp ne i64 %c463, 0
  br i1 %v2257, label %b1481, label %b1480
b1479:
  %v2258 = phi i64 [ %v2253, %b1477 ], [ %v2255, %b1480 ]
  store i64 %v2258, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2259 = getelementptr [1 x i64], ptr %sargs2259, i64 0, i64 0
  store i64 0, ptr %sat2259
  %sic2259 = getelementptr i64, ptr %area, i64 141824
  %sf2259 = call ptr @df.selector(ptr %sic2259, i64 %v2258, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2259 = call i64 %sf2259(i64 %v2258, ptr %sargs2259, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2259, ptr %gcs0
  %v2260 = load i8, ptr %ctx, align 8
  %c464 = zext i8 %v2260 to i64
  %v2261 = icmp ne i64 %c464, 0
  br i1 %v2261, label %b1483, label %b1482
b1480:
  br label %b1479
b1481:
  %gcvolta1481 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1481, ptr %ctxtopo, align 8
  ret i64 0
b1482:
  %v2262 = call i64 @df.unbox_int(i64 %v2259)
  %v2263 = load i8, ptr %ctx, align 8
  %c465 = zext i8 %v2263 to i64
  %v2264 = icmp ne i64 %c465, 0
  br i1 %v2264, label %b1485, label %b1484
b1483:
  %gcvolta1483 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1483, ptr %ctxtopo, align 8
  ret i64 0
b1484:
  store i64 0, ptr %gcs0
  %v2265 = call i64 @df.box_int(i64 %v2262)
  %xcp2266 = load i8, ptr %ctx, align 8
  %xcn2266 = icmp ne i8 %xcp2266, 0
  %xce2266 = call i1 @llvm.expect.i1(i1 %xcn2266, i1 false)
  br i1 %xce2266, label %xc2266.limpar, label %xc2266.fim
xc2266.limpar:
  call void @dartforge_exception_clear()
  br label %xc2266.fim
xc2266.fim:
  %gcvolta1484 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1484, ptr %ctxtopo, align 8
  ret i64 %v2265
b1485:
  %gcvolta1485 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1485, ptr %ctxtopo, align 8
  ret i64 0
b1487:
  %v2275 = add i64 0, 4294967295
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2276 = and i64 %v2272, 1
  %v2277 = icmp ne i64 %v2276, 0
  br i1 %v2277, label %b1489, label %b1490
b1488:
  %gcvolta1488 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1488, ptr %ctxtopo, align 8
  ret i64 0
b1489:
  %v2278 = ashr i64 %v2272, 1
  %v2279 = mul i64 %v2278, %v2275
  store i64 0, ptr %gcs0
  %v2280 = call i64 @df.box_int(i64 %v2279)
  br label %b1491
b1490:
  %v2281 = call i64 @df.box_int(i64 %v2275)
  store i64 %v2281, ptr %gcs1
  %sa2282_0 = getelementptr [2 x i64], ptr %sargs2282, i64 0, i64 0
  store i64 %v2281, ptr %sa2282_0
  %sat2282 = getelementptr [2 x i64], ptr %sargs2282, i64 0, i64 1
  store i64 0, ptr %sat2282
  %sic2282 = getelementptr i64, ptr %area, i64 141826
  %sf2282 = call ptr @df.selector(ptr %sic2282, i64 %v2272, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2282 = call i64 %sf2282(i64 %v2272, ptr %sargs2282, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2283 = load i8, ptr %ctx, align 8
  %c466 = zext i8 %v2283 to i64
  %v2284 = icmp ne i64 %c466, 0
  br i1 %v2284, label %b1493, label %b1492
b1491:
  %v2285 = phi i64 [ %v2280, %b1489 ], [ %v2282, %b1492 ]
  store i64 %v2285, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2286 = getelementptr [1 x i64], ptr %sargs2286, i64 0, i64 0
  store i64 0, ptr %sat2286
  %sic2286 = getelementptr i64, ptr %area, i64 141828
  %sf2286 = call ptr @df.selector(ptr %sic2286, i64 %v2285, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2286 = call i64 %sf2286(i64 %v2285, ptr %sargs2286, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2286, ptr %gcs0
  %v2287 = load i8, ptr %ctx, align 8
  %c467 = zext i8 %v2287 to i64
  %v2288 = icmp ne i64 %c467, 0
  br i1 %v2288, label %b1495, label %b1494
b1492:
  br label %b1491
b1493:
  %gcvolta1493 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1493, ptr %ctxtopo, align 8
  ret i64 0
b1494:
  %v2289 = call i64 @df.unbox_int(i64 %v2286)
  %v2290 = load i8, ptr %ctx, align 8
  %c468 = zext i8 %v2290 to i64
  %v2291 = icmp ne i64 %c468, 0
  br i1 %v2291, label %b1497, label %b1496
b1495:
  %gcvolta1495 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1495, ptr %ctxtopo, align 8
  ret i64 0
b1496:
  store i64 0, ptr %gcs0
  %v2292 = call i64 @df.box_int(i64 %v2289)
  %xcp2293 = load i8, ptr %ctx, align 8
  %xcn2293 = icmp ne i8 %xcp2293, 0
  %xce2293 = call i1 @llvm.expect.i1(i1 %xcn2293, i1 false)
  br i1 %xce2293, label %xc2293.limpar, label %xc2293.fim
xc2293.limpar:
  call void @dartforge_exception_clear()
  br label %xc2293.fim
xc2293.fim:
  %gcvolta1496 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1496, ptr %ctxtopo, align 8
  ret i64 %v2292
b1497:
  %gcvolta1497 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1497, ptr %ctxtopo, align 8
  ret i64 0
b1499:
  %v2298 = ashr i64 %v0, 1
  %v2299 = icmp slt i64 %v2298, %v2295
  br label %b1501
b1500:
  %v2300 = call i64 @df.box_int(i64 %v2295)
  store i64 %v2300, ptr %gcs1
  %sa2301_0 = getelementptr [2 x i64], ptr %sargs2301, i64 0, i64 0
  store i64 %v2300, ptr %sa2301_0
  %sat2301 = getelementptr [2 x i64], ptr %sargs2301, i64 0, i64 1
  store i64 0, ptr %sat2301
  %sic2301 = getelementptr i64, ptr %area, i64 141830
  %sf2301 = call ptr @df.selector(ptr %sic2301, i64 %v0, i64 -4020809642220582218, ptr @df.seln.95, i64 4)
  %v2301 = call i64 %sf2301(i64 %v0, ptr %sargs2301, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2301, ptr %gcs1
  %v2302 = load i8, ptr %ctx, align 8
  %c469 = zext i8 %v2302 to i64
  %v2303 = icmp ne i64 %c469, 0
  br i1 %v2303, label %b1503, label %b1502
b1501:
  %v2307 = phi i1 [ %v2299, %b1499 ], [ %v2304, %b1504 ]
  br i1 %v2307, label %b1506, label %b1507
b1502:
  %v2304 = call i1 @df.unbox_bool(i64 %v2301)
  %v2305 = load i8, ptr %ctx, align 8
  %c470 = zext i8 %v2305 to i64
  %v2306 = icmp ne i64 %c470, 0
  br i1 %v2306, label %b1505, label %b1504
b1503:
  %gcvolta1503 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1503, ptr %ctxtopo, align 8
  ret i64 0
b1504:
  br label %b1501
b1505:
  %gcvolta1505 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1505, ptr %ctxtopo, align 8
  ret i64 0
b1506:
  %v2309 = add i64 0, 1
  %v2310 = sub i64 0, %v2309
  %v2311 = add i64 0, 1
  store i64 0, ptr %gcs1
  %v2312 = call i64 @df.box_int(i64 %v2310)
  store i64 %v2312, ptr %gcs1
  %v2313 = call i64 @df.box_int(i64 %v2311)
  store i64 %v2313, ptr %gcs2
  %sa2314_0 = getelementptr [3 x i64], ptr %sargs2314, i64 0, i64 0
  store i64 %v2312, ptr %sa2314_0
  %sa2314_1 = getelementptr [3 x i64], ptr %sargs2314, i64 0, i64 1
  store i64 %v2313, ptr %sa2314_1
  %sat2314 = getelementptr [3 x i64], ptr %sargs2314, i64 0, i64 2
  store i64 0, ptr %sat2314
  %sic2314 = getelementptr i64, ptr %area, i64 141832
  %sf2314 = call ptr @df.selector(ptr %sic2314, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2314 = call i64 %sf2314(i64 %v0, ptr %sargs2314, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2314, ptr %gcs0
  %v2315 = load i8, ptr %ctx, align 8
  %c471 = zext i8 %v2315 to i64
  %v2316 = icmp ne i64 %c471, 0
  br i1 %v2316, label %b1510, label %b1509
b1507:
  %v2329 = add i64 0, 1
  %v2330 = sub i64 0, %v2329
  %v2331 = add i64 0, 1
  store i64 0, ptr %gcs1
  %v2332 = call i64 @df.box_int(i64 %v2330)
  store i64 %v2332, ptr %gcs1
  %v2333 = call i64 @df.box_int(i64 %v2331)
  store i64 %v2333, ptr %gcs2
  %sa2334_0 = getelementptr [3 x i64], ptr %sargs2334, i64 0, i64 0
  store i64 %v2332, ptr %sa2334_0
  %sa2334_1 = getelementptr [3 x i64], ptr %sargs2334, i64 0, i64 1
  store i64 %v2333, ptr %sa2334_1
  %sat2334 = getelementptr [3 x i64], ptr %sargs2334, i64 0, i64 2
  store i64 0, ptr %sat2334
  %sic2334 = getelementptr i64, ptr %area, i64 141834
  %sf2334 = call ptr @df.selector(ptr %sic2334, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2334 = call i64 %sf2334(i64 %v0, ptr %sargs2334, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2334, ptr %gcs0
  %v2335 = load i8, ptr %ctx, align 8
  %c472 = zext i8 %v2335 to i64
  %v2336 = icmp ne i64 %c472, 0
  br i1 %v2336, label %b1517, label %b1516
b1508:
  %v2348 = phi i64 [ %v2327, %b1513 ], [ %v2347, %b1520 ]
  store i64 %v2348, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2349 = getelementptr [1 x i64], ptr %sargs2349, i64 0, i64 0
  store i64 0, ptr %sat2349
  %sic2349 = getelementptr i64, ptr %area, i64 141836
  %sf2349 = call ptr @df.selector(ptr %sic2349, i64 %v2348, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2349 = call i64 %sf2349(i64 %v2348, ptr %sargs2349, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2349, ptr %gcs0
  %v2350 = load i8, ptr %ctx, align 8
  %c473 = zext i8 %v2350 to i64
  %v2351 = icmp ne i64 %c473, 0
  br i1 %v2351, label %b1524, label %b1523
b1509:
  %v2317 = add i64 0, 128
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2318 = and i64 %v2314, 1
  %v2319 = icmp ne i64 %v2318, 0
  br i1 %v2319, label %b1511, label %b1512
b1510:
  %gcvolta1510 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1510, ptr %ctxtopo, align 8
  ret i64 0
b1511:
  %v2320 = ashr i64 %v2314, 1
  %v2321 = mul i64 %v2320, %v2317
  store i64 0, ptr %gcs0
  %v2322 = call i64 @df.box_int(i64 %v2321)
  br label %b1513
b1512:
  %v2323 = call i64 @df.box_int(i64 %v2317)
  store i64 %v2323, ptr %gcs1
  %sa2324_0 = getelementptr [2 x i64], ptr %sargs2324, i64 0, i64 0
  store i64 %v2323, ptr %sa2324_0
  %sat2324 = getelementptr [2 x i64], ptr %sargs2324, i64 0, i64 1
  store i64 0, ptr %sat2324
  %sic2324 = getelementptr i64, ptr %area, i64 141838
  %sf2324 = call ptr @df.selector(ptr %sic2324, i64 %v2314, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2324 = call i64 %sf2324(i64 %v2314, ptr %sargs2324, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2325 = load i8, ptr %ctx, align 8
  %c474 = zext i8 %v2325 to i64
  %v2326 = icmp ne i64 %c474, 0
  br i1 %v2326, label %b1515, label %b1514
b1513:
  %v2327 = phi i64 [ %v2322, %b1511 ], [ %v2324, %b1514 ]
  br label %b1508
b1514:
  br label %b1513
b1515:
  %gcvolta1515 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1515, ptr %ctxtopo, align 8
  ret i64 0
b1516:
  %v2337 = add i64 0, 127
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2338 = and i64 %v2334, 1
  %v2339 = icmp ne i64 %v2338, 0
  br i1 %v2339, label %b1518, label %b1519
b1517:
  %gcvolta1517 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1517, ptr %ctxtopo, align 8
  ret i64 0
b1518:
  %v2340 = ashr i64 %v2334, 1
  %v2341 = mul i64 %v2340, %v2337
  store i64 0, ptr %gcs0
  %v2342 = call i64 @df.box_int(i64 %v2341)
  br label %b1520
b1519:
  %v2343 = call i64 @df.box_int(i64 %v2337)
  store i64 %v2343, ptr %gcs1
  %sa2344_0 = getelementptr [2 x i64], ptr %sargs2344, i64 0, i64 0
  store i64 %v2343, ptr %sa2344_0
  %sat2344 = getelementptr [2 x i64], ptr %sargs2344, i64 0, i64 1
  store i64 0, ptr %sat2344
  %sic2344 = getelementptr i64, ptr %area, i64 141840
  %sf2344 = call ptr @df.selector(ptr %sic2344, i64 %v2334, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2344 = call i64 %sf2344(i64 %v2334, ptr %sargs2344, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2345 = load i8, ptr %ctx, align 8
  %c475 = zext i8 %v2345 to i64
  %v2346 = icmp ne i64 %c475, 0
  br i1 %v2346, label %b1522, label %b1521
b1520:
  %v2347 = phi i64 [ %v2342, %b1518 ], [ %v2344, %b1521 ]
  br label %b1508
b1521:
  br label %b1520
b1522:
  %gcvolta1522 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1522, ptr %ctxtopo, align 8
  ret i64 0
b1523:
  %v2352 = call i64 @df.unbox_int(i64 %v2349)
  %v2353 = load i8, ptr %ctx, align 8
  %c476 = zext i8 %v2353 to i64
  %v2354 = icmp ne i64 %c476, 0
  br i1 %v2354, label %b1526, label %b1525
b1524:
  %gcvolta1524 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1524, ptr %ctxtopo, align 8
  ret i64 0
b1525:
  store i64 0, ptr %gcs0
  %v2355 = call i64 @df.box_int(i64 %v2352)
  %xcp2356 = load i8, ptr %ctx, align 8
  %xcn2356 = icmp ne i8 %xcp2356, 0
  %xce2356 = call i1 @llvm.expect.i1(i1 %xcn2356, i1 false)
  br i1 %xce2356, label %xc2356.limpar, label %xc2356.fim
xc2356.limpar:
  call void @dartforge_exception_clear()
  br label %xc2356.fim
xc2356.fim:
  %gcvolta1525 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1525, ptr %ctxtopo, align 8
  ret i64 %v2355
b1526:
  %gcvolta1526 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1526, ptr %ctxtopo, align 8
  ret i64 0
b1528:
  %v2361 = ashr i64 %v0, 1
  %v2362 = icmp slt i64 %v2361, %v2358
  br label %b1530
b1529:
  %v2363 = call i64 @df.box_int(i64 %v2358)
  store i64 %v2363, ptr %gcs1
  %sa2364_0 = getelementptr [2 x i64], ptr %sargs2364, i64 0, i64 0
  store i64 %v2363, ptr %sa2364_0
  %sat2364 = getelementptr [2 x i64], ptr %sargs2364, i64 0, i64 1
  store i64 0, ptr %sat2364
  %sic2364 = getelementptr i64, ptr %area, i64 141842
  %sf2364 = call ptr @df.selector(ptr %sic2364, i64 %v0, i64 -4020809642220582218, ptr @df.seln.95, i64 4)
  %v2364 = call i64 %sf2364(i64 %v0, ptr %sargs2364, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2364, ptr %gcs1
  %v2365 = load i8, ptr %ctx, align 8
  %c477 = zext i8 %v2365 to i64
  %v2366 = icmp ne i64 %c477, 0
  br i1 %v2366, label %b1532, label %b1531
b1530:
  %v2370 = phi i1 [ %v2362, %b1528 ], [ %v2367, %b1533 ]
  br i1 %v2370, label %b1535, label %b1536
b1531:
  %v2367 = call i1 @df.unbox_bool(i64 %v2364)
  %v2368 = load i8, ptr %ctx, align 8
  %c478 = zext i8 %v2368 to i64
  %v2369 = icmp ne i64 %c478, 0
  br i1 %v2369, label %b1534, label %b1533
b1532:
  %gcvolta1532 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1532, ptr %ctxtopo, align 8
  ret i64 0
b1533:
  br label %b1530
b1534:
  %gcvolta1534 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1534, ptr %ctxtopo, align 8
  ret i64 0
b1535:
  %v2372 = add i64 0, 1
  %v2373 = sub i64 0, %v2372
  %v2374 = add i64 0, 1
  store i64 0, ptr %gcs1
  %v2375 = call i64 @df.box_int(i64 %v2373)
  store i64 %v2375, ptr %gcs1
  %v2376 = call i64 @df.box_int(i64 %v2374)
  store i64 %v2376, ptr %gcs2
  %sa2377_0 = getelementptr [3 x i64], ptr %sargs2377, i64 0, i64 0
  store i64 %v2375, ptr %sa2377_0
  %sa2377_1 = getelementptr [3 x i64], ptr %sargs2377, i64 0, i64 1
  store i64 %v2376, ptr %sa2377_1
  %sat2377 = getelementptr [3 x i64], ptr %sargs2377, i64 0, i64 2
  store i64 0, ptr %sat2377
  %sic2377 = getelementptr i64, ptr %area, i64 141844
  %sf2377 = call ptr @df.selector(ptr %sic2377, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2377 = call i64 %sf2377(i64 %v0, ptr %sargs2377, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2377, ptr %gcs0
  %v2378 = load i8, ptr %ctx, align 8
  %c479 = zext i8 %v2378 to i64
  %v2379 = icmp ne i64 %c479, 0
  br i1 %v2379, label %b1539, label %b1538
b1536:
  %v2392 = add i64 0, 1
  %v2393 = sub i64 0, %v2392
  %v2394 = add i64 0, 1
  store i64 0, ptr %gcs1
  %v2395 = call i64 @df.box_int(i64 %v2393)
  store i64 %v2395, ptr %gcs1
  %v2396 = call i64 @df.box_int(i64 %v2394)
  store i64 %v2396, ptr %gcs2
  %sa2397_0 = getelementptr [3 x i64], ptr %sargs2397, i64 0, i64 0
  store i64 %v2395, ptr %sa2397_0
  %sa2397_1 = getelementptr [3 x i64], ptr %sargs2397, i64 0, i64 1
  store i64 %v2396, ptr %sa2397_1
  %sat2397 = getelementptr [3 x i64], ptr %sargs2397, i64 0, i64 2
  store i64 0, ptr %sat2397
  %sic2397 = getelementptr i64, ptr %area, i64 141846
  %sf2397 = call ptr @df.selector(ptr %sic2397, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2397 = call i64 %sf2397(i64 %v0, ptr %sargs2397, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2397, ptr %gcs0
  %v2398 = load i8, ptr %ctx, align 8
  %c480 = zext i8 %v2398 to i64
  %v2399 = icmp ne i64 %c480, 0
  br i1 %v2399, label %b1546, label %b1545
b1537:
  %v2411 = phi i64 [ %v2390, %b1542 ], [ %v2410, %b1549 ]
  store i64 %v2411, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2412 = getelementptr [1 x i64], ptr %sargs2412, i64 0, i64 0
  store i64 0, ptr %sat2412
  %sic2412 = getelementptr i64, ptr %area, i64 141848
  %sf2412 = call ptr @df.selector(ptr %sic2412, i64 %v2411, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2412 = call i64 %sf2412(i64 %v2411, ptr %sargs2412, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2412, ptr %gcs0
  %v2413 = load i8, ptr %ctx, align 8
  %c481 = zext i8 %v2413 to i64
  %v2414 = icmp ne i64 %c481, 0
  br i1 %v2414, label %b1553, label %b1552
b1538:
  %v2380 = add i64 0, 32768
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2381 = and i64 %v2377, 1
  %v2382 = icmp ne i64 %v2381, 0
  br i1 %v2382, label %b1540, label %b1541
b1539:
  %gcvolta1539 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1539, ptr %ctxtopo, align 8
  ret i64 0
b1540:
  %v2383 = ashr i64 %v2377, 1
  %v2384 = mul i64 %v2383, %v2380
  store i64 0, ptr %gcs0
  %v2385 = call i64 @df.box_int(i64 %v2384)
  br label %b1542
b1541:
  %v2386 = call i64 @df.box_int(i64 %v2380)
  store i64 %v2386, ptr %gcs1
  %sa2387_0 = getelementptr [2 x i64], ptr %sargs2387, i64 0, i64 0
  store i64 %v2386, ptr %sa2387_0
  %sat2387 = getelementptr [2 x i64], ptr %sargs2387, i64 0, i64 1
  store i64 0, ptr %sat2387
  %sic2387 = getelementptr i64, ptr %area, i64 141850
  %sf2387 = call ptr @df.selector(ptr %sic2387, i64 %v2377, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2387 = call i64 %sf2387(i64 %v2377, ptr %sargs2387, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2388 = load i8, ptr %ctx, align 8
  %c482 = zext i8 %v2388 to i64
  %v2389 = icmp ne i64 %c482, 0
  br i1 %v2389, label %b1544, label %b1543
b1542:
  %v2390 = phi i64 [ %v2385, %b1540 ], [ %v2387, %b1543 ]
  br label %b1537
b1543:
  br label %b1542
b1544:
  %gcvolta1544 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1544, ptr %ctxtopo, align 8
  ret i64 0
b1545:
  %v2400 = add i64 0, 32767
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2401 = and i64 %v2397, 1
  %v2402 = icmp ne i64 %v2401, 0
  br i1 %v2402, label %b1547, label %b1548
b1546:
  %gcvolta1546 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1546, ptr %ctxtopo, align 8
  ret i64 0
b1547:
  %v2403 = ashr i64 %v2397, 1
  %v2404 = mul i64 %v2403, %v2400
  store i64 0, ptr %gcs0
  %v2405 = call i64 @df.box_int(i64 %v2404)
  br label %b1549
b1548:
  %v2406 = call i64 @df.box_int(i64 %v2400)
  store i64 %v2406, ptr %gcs1
  %sa2407_0 = getelementptr [2 x i64], ptr %sargs2407, i64 0, i64 0
  store i64 %v2406, ptr %sa2407_0
  %sat2407 = getelementptr [2 x i64], ptr %sargs2407, i64 0, i64 1
  store i64 0, ptr %sat2407
  %sic2407 = getelementptr i64, ptr %area, i64 141852
  %sf2407 = call ptr @df.selector(ptr %sic2407, i64 %v2397, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2407 = call i64 %sf2407(i64 %v2397, ptr %sargs2407, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2408 = load i8, ptr %ctx, align 8
  %c483 = zext i8 %v2408 to i64
  %v2409 = icmp ne i64 %c483, 0
  br i1 %v2409, label %b1551, label %b1550
b1549:
  %v2410 = phi i64 [ %v2405, %b1547 ], [ %v2407, %b1550 ]
  br label %b1537
b1550:
  br label %b1549
b1551:
  %gcvolta1551 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1551, ptr %ctxtopo, align 8
  ret i64 0
b1552:
  %v2415 = call i64 @df.unbox_int(i64 %v2412)
  %v2416 = load i8, ptr %ctx, align 8
  %c484 = zext i8 %v2416 to i64
  %v2417 = icmp ne i64 %c484, 0
  br i1 %v2417, label %b1555, label %b1554
b1553:
  %gcvolta1553 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1553, ptr %ctxtopo, align 8
  ret i64 0
b1554:
  store i64 0, ptr %gcs0
  %v2418 = call i64 @df.box_int(i64 %v2415)
  %xcp2419 = load i8, ptr %ctx, align 8
  %xcn2419 = icmp ne i8 %xcp2419, 0
  %xce2419 = call i1 @llvm.expect.i1(i1 %xcn2419, i1 false)
  br i1 %xce2419, label %xc2419.limpar, label %xc2419.fim
xc2419.limpar:
  call void @dartforge_exception_clear()
  br label %xc2419.fim
xc2419.fim:
  %gcvolta1554 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1554, ptr %ctxtopo, align 8
  ret i64 %v2418
b1555:
  %gcvolta1555 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1555, ptr %ctxtopo, align 8
  ret i64 0
b1557:
  %v2424 = ashr i64 %v0, 1
  %v2425 = icmp slt i64 %v2424, %v2421
  br label %b1559
b1558:
  %v2426 = call i64 @df.box_int(i64 %v2421)
  store i64 %v2426, ptr %gcs1
  %sa2427_0 = getelementptr [2 x i64], ptr %sargs2427, i64 0, i64 0
  store i64 %v2426, ptr %sa2427_0
  %sat2427 = getelementptr [2 x i64], ptr %sargs2427, i64 0, i64 1
  store i64 0, ptr %sat2427
  %sic2427 = getelementptr i64, ptr %area, i64 141854
  %sf2427 = call ptr @df.selector(ptr %sic2427, i64 %v0, i64 -4020809642220582218, ptr @df.seln.95, i64 4)
  %v2427 = call i64 %sf2427(i64 %v0, ptr %sargs2427, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v2427, ptr %gcs1
  %v2428 = load i8, ptr %ctx, align 8
  %c485 = zext i8 %v2428 to i64
  %v2429 = icmp ne i64 %c485, 0
  br i1 %v2429, label %b1561, label %b1560
b1559:
  %v2433 = phi i1 [ %v2425, %b1557 ], [ %v2430, %b1562 ]
  br i1 %v2433, label %b1564, label %b1565
b1560:
  %v2430 = call i1 @df.unbox_bool(i64 %v2427)
  %v2431 = load i8, ptr %ctx, align 8
  %c486 = zext i8 %v2431 to i64
  %v2432 = icmp ne i64 %c486, 0
  br i1 %v2432, label %b1563, label %b1562
b1561:
  %gcvolta1561 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1561, ptr %ctxtopo, align 8
  ret i64 0
b1562:
  br label %b1559
b1563:
  %gcvolta1563 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1563, ptr %ctxtopo, align 8
  ret i64 0
b1564:
  %v2435 = add i64 0, 1
  %v2436 = sub i64 0, %v2435
  %v2437 = add i64 0, 1
  store i64 0, ptr %gcs1
  %v2438 = call i64 @df.box_int(i64 %v2436)
  store i64 %v2438, ptr %gcs1
  %v2439 = call i64 @df.box_int(i64 %v2437)
  store i64 %v2439, ptr %gcs2
  %sa2440_0 = getelementptr [3 x i64], ptr %sargs2440, i64 0, i64 0
  store i64 %v2438, ptr %sa2440_0
  %sa2440_1 = getelementptr [3 x i64], ptr %sargs2440, i64 0, i64 1
  store i64 %v2439, ptr %sa2440_1
  %sat2440 = getelementptr [3 x i64], ptr %sargs2440, i64 0, i64 2
  store i64 0, ptr %sat2440
  %sic2440 = getelementptr i64, ptr %area, i64 141856
  %sf2440 = call ptr @df.selector(ptr %sic2440, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2440 = call i64 %sf2440(i64 %v0, ptr %sargs2440, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2440, ptr %gcs0
  %v2441 = load i8, ptr %ctx, align 8
  %c487 = zext i8 %v2441 to i64
  %v2442 = icmp ne i64 %c487, 0
  br i1 %v2442, label %b1568, label %b1567
b1565:
  %v2455 = add i64 0, 1
  %v2456 = sub i64 0, %v2455
  %v2457 = add i64 0, 1
  store i64 0, ptr %gcs1
  %v2458 = call i64 @df.box_int(i64 %v2456)
  store i64 %v2458, ptr %gcs1
  %v2459 = call i64 @df.box_int(i64 %v2457)
  store i64 %v2459, ptr %gcs2
  %sa2460_0 = getelementptr [3 x i64], ptr %sargs2460, i64 0, i64 0
  store i64 %v2458, ptr %sa2460_0
  %sa2460_1 = getelementptr [3 x i64], ptr %sargs2460, i64 0, i64 1
  store i64 %v2459, ptr %sa2460_1
  %sat2460 = getelementptr [3 x i64], ptr %sargs2460, i64 0, i64 2
  store i64 0, ptr %sat2460
  %sic2460 = getelementptr i64, ptr %area, i64 141858
  %sf2460 = call ptr @df.selector(ptr %sic2460, i64 %v0, i64 -1664458868869587675, ptr @df.seln.34, i64 8)
  %v2460 = call i64 %sf2460(i64 %v0, ptr %sargs2460, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  store i64 %v2460, ptr %gcs0
  %v2461 = load i8, ptr %ctx, align 8
  %c488 = zext i8 %v2461 to i64
  %v2462 = icmp ne i64 %c488, 0
  br i1 %v2462, label %b1575, label %b1574
b1566:
  %v2474 = phi i64 [ %v2453, %b1571 ], [ %v2473, %b1578 ]
  store i64 %v2474, ptr %gcs0
  store i64 0, ptr %gcs1
  %sat2475 = getelementptr [1 x i64], ptr %sargs2475, i64 0, i64 0
  store i64 0, ptr %sat2475
  %sic2475 = getelementptr i64, ptr %area, i64 141860
  %sf2475 = call ptr @df.selector(ptr %sic2475, i64 %v2474, i64 4857594363590788828, ptr @df.seln.334, i64 8)
  %v2475 = call i64 %sf2475(i64 %v2474, ptr %sargs2475, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2475, ptr %gcs0
  %v2476 = load i8, ptr %ctx, align 8
  %c489 = zext i8 %v2476 to i64
  %v2477 = icmp ne i64 %c489, 0
  br i1 %v2477, label %b1582, label %b1581
b1567:
  %v2443 = add i64 0, 2147483648
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2444 = and i64 %v2440, 1
  %v2445 = icmp ne i64 %v2444, 0
  br i1 %v2445, label %b1569, label %b1570
b1568:
  %gcvolta1568 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1568, ptr %ctxtopo, align 8
  ret i64 0
b1569:
  %v2446 = ashr i64 %v2440, 1
  %v2447 = mul i64 %v2446, %v2443
  store i64 0, ptr %gcs0
  %v2448 = call i64 @df.box_int(i64 %v2447)
  br label %b1571
b1570:
  %v2449 = call i64 @df.box_int(i64 %v2443)
  store i64 %v2449, ptr %gcs1
  %sa2450_0 = getelementptr [2 x i64], ptr %sargs2450, i64 0, i64 0
  store i64 %v2449, ptr %sa2450_0
  %sat2450 = getelementptr [2 x i64], ptr %sargs2450, i64 0, i64 1
  store i64 0, ptr %sat2450
  %sic2450 = getelementptr i64, ptr %area, i64 141862
  %sf2450 = call ptr @df.selector(ptr %sic2450, i64 %v2440, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2450 = call i64 %sf2450(i64 %v2440, ptr %sargs2450, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2451 = load i8, ptr %ctx, align 8
  %c490 = zext i8 %v2451 to i64
  %v2452 = icmp ne i64 %c490, 0
  br i1 %v2452, label %b1573, label %b1572
b1571:
  %v2453 = phi i64 [ %v2448, %b1569 ], [ %v2450, %b1572 ]
  br label %b1566
b1572:
  br label %b1571
b1573:
  %gcvolta1573 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1573, ptr %ctxtopo, align 8
  ret i64 0
b1574:
  %v2463 = add i64 0, 2147483647
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2464 = and i64 %v2460, 1
  %v2465 = icmp ne i64 %v2464, 0
  br i1 %v2465, label %b1576, label %b1577
b1575:
  %gcvolta1575 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1575, ptr %ctxtopo, align 8
  ret i64 0
b1576:
  %v2466 = ashr i64 %v2460, 1
  %v2467 = mul i64 %v2466, %v2463
  store i64 0, ptr %gcs0
  %v2468 = call i64 @df.box_int(i64 %v2467)
  br label %b1578
b1577:
  %v2469 = call i64 @df.box_int(i64 %v2463)
  store i64 %v2469, ptr %gcs1
  %sa2470_0 = getelementptr [2 x i64], ptr %sargs2470, i64 0, i64 0
  store i64 %v2469, ptr %sa2470_0
  %sat2470 = getelementptr [2 x i64], ptr %sargs2470, i64 0, i64 1
  store i64 0, ptr %sat2470
  %sic2470 = getelementptr i64, ptr %area, i64 141864
  %sf2470 = call ptr @df.selector(ptr %sic2470, i64 %v2460, i64 -4020825035383377172, ptr @df.seln.99, i64 4)
  %v2470 = call i64 %sf2470(i64 %v2460, ptr %sargs2470, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2471 = load i8, ptr %ctx, align 8
  %c491 = zext i8 %v2471 to i64
  %v2472 = icmp ne i64 %c491, 0
  br i1 %v2472, label %b1580, label %b1579
b1578:
  %v2473 = phi i64 [ %v2468, %b1576 ], [ %v2470, %b1579 ]
  br label %b1566
b1579:
  br label %b1578
b1580:
  %gcvolta1580 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1580, ptr %ctxtopo, align 8
  ret i64 0
b1581:
  %v2478 = call i64 @df.unbox_int(i64 %v2475)
  %v2479 = load i8, ptr %ctx, align 8
  %c492 = zext i8 %v2479 to i64
  %v2480 = icmp ne i64 %c492, 0
  br i1 %v2480, label %b1584, label %b1583
b1582:
  %gcvolta1582 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1582, ptr %ctxtopo, align 8
  ret i64 0
b1583:
  store i64 0, ptr %gcs0
  %v2481 = call i64 @df.box_int(i64 %v2478)
  %xcp2482 = load i8, ptr %ctx, align 8
  %xcn2482 = icmp ne i8 %xcp2482, 0
  %xce2482 = call i1 @llvm.expect.i1(i1 %xcn2482, i1 false)
  br i1 %xce2482, label %xc2482.limpar, label %xc2482.fim
xc2482.limpar:
  call void @dartforge_exception_clear()
  br label %xc2482.fim
xc2482.fim:
  %gcvolta1583 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1583, ptr %ctxtopo, align 8
  ret i64 %v2481
b1584:
  %gcvolta1584 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1584, ptr %ctxtopo, align 8
  ret i64 0
}
