define i64 @df.specialized.8a055f5f6e8822e6b7c560145408f601667f6da89c115e59b8539ddd4d29d68b.e00cd59a2f96645b24ed203264c390f15b73f8c88b56ad5bdd128f3df08e6c63(i64 %v0, i64 %v3, i64 %v6) nounwind {
b0:
  %sargs16 = alloca [2 x i64]
  %sargs2395 = alloca [3 x i64]
  %sargs2406 = alloca [2 x i64]
  %sargs2420 = alloca [1 x i64]
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
  %v11 = icmp eq i64 %v3, 0
  %v12 = icmp eq i64 %v6, 0
  %c0 = zext i1 %v11 to i64
  %c1 = zext i1 %v12 to i64
  %v13 = or i64 %c0, %c1
  %v14 = icmp ne i64 %v13, 0
  br i1 %v14, label %b1, label %b2
b1:
  br label %b3
b2:
  %sa16_0 = getelementptr [2 x i64], ptr %sargs16, i64 0, i64 0
  store i64 %v6, ptr %sa16_0
  %sat16 = getelementptr [2 x i64], ptr %sargs16, i64 0, i64 1
  store i64 0, ptr %sat16
  %sic16 = getelementptr i64, ptr %area, i64 90
  %sf16 = call ptr @df.selector(ptr %sic16, i64 %v3, i64 -9163797671095228218, ptr @df.seln.6, i64 4)
  %v16 = call i64 %sf16(i64 %v3, ptr %sargs16, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v16, ptr %gcs1
  %v17 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v17 to i64
  %v18 = icmp ne i64 %c2, 0
  br i1 %v18, label %b5, label %b4
b3:
  %v22 = phi i1 [ false, %b1 ], [ %v19, %b6 ]
  br i1 %v22, label %b8, label %b10
b4:
  store i64 0, ptr %gcs2
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
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v26 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  %v27 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v27 to i64
  %v28 = icmp ne i64 %c4, 0
  br i1 %v28, label %b27, label %b26
b26:
  %v30 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  %v31 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v31 to i64
  %v32 = icmp ne i64 %c5, 0
  br i1 %v32, label %b31, label %b30
b27:
  %gcvolta27 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta27, ptr %ctxtopo, align 8
  ret i64 0
b30:
  %v34 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  %v35 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v35 to i64
  %v36 = icmp ne i64 %c6, 0
  br i1 %v36, label %b35, label %b34
b31:
  %gcvolta31 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta31, ptr %ctxtopo, align 8
  ret i64 0
b34:
  %v38 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  %v39 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v39 to i64
  %v40 = icmp ne i64 %c7, 0
  br i1 %v40, label %b39, label %b38
b35:
  %gcvolta35 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta35, ptr %ctxtopo, align 8
  ret i64 0
b38:
  %v42 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint16()
  %v43 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v43 to i64
  %v44 = icmp ne i64 %c8, 0
  br i1 %v44, label %b43, label %b42
b39:
  %gcvolta39 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta39, ptr %ctxtopo, align 8
  ret i64 0
b42:
  %v46 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint32()
  %v47 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v47 to i64
  %v48 = icmp ne i64 %c9, 0
  br i1 %v48, label %b47, label %b46
b43:
  %gcvolta43 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta43, ptr %ctxtopo, align 8
  ret i64 0
b46:
  %v50 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int8()
  %v51 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v51 to i64
  %v52 = icmp ne i64 %c10, 0
  br i1 %v52, label %b51, label %b50
b47:
  %gcvolta47 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta47, ptr %ctxtopo, align 8
  ret i64 0
b50:
  %v54 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int16()
  %v55 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v55 to i64
  %v56 = icmp ne i64 %c11, 0
  br i1 %v56, label %b55, label %b54
b51:
  %gcvolta51 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta51, ptr %ctxtopo, align 8
  ret i64 0
b54:
  %v58 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.int32()
  %v59 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v59 to i64
  %v60 = icmp ne i64 %c12, 0
  br i1 %v60, label %b59, label %b58
b55:
  %gcvolta55 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta55, ptr %ctxtopo, align 8
  ret i64 0
b58:
  %v62 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float16()
  %v63 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v63 to i64
  %v64 = icmp ne i64 %c13, 0
  br i1 %v64, label %b63, label %b62
b59:
  %gcvolta59 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta59, ptr %ctxtopo, align 8
  ret i64 0
b62:
  %v66 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.float32()
  %v67 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v67 to i64
  %v68 = icmp ne i64 %c14, 0
  br i1 %v68, label %b68, label %b67
b63:
  %gcvolta63 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta63, ptr %ctxtopo, align 8
  ret i64 0
b67:
  %v2199 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint1()
  %v2200 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v2200 to i64
  %v2201 = icmp ne i64 %c15, 0
  br i1 %v2201, label %b1479, label %b1478
b68:
  %gcvolta68 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta68, ptr %ctxtopo, align 8
  ret i64 0
b1478:
  %v2203 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint2()
  %v2204 = load i8, ptr %ctx, align 8
  %c16 = zext i8 %v2204 to i64
  %v2205 = icmp ne i64 %c16, 0
  br i1 %v2205, label %b1483, label %b1482
b1479:
  %gcvolta1479 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1479, ptr %ctxtopo, align 8
  ret i64 0
b1482:
  %v2207 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint4()
  %v2208 = load i8, ptr %ctx, align 8
  %c17 = zext i8 %v2208 to i64
  %v2209 = icmp ne i64 %c17, 0
  br i1 %v2209, label %b1487, label %b1486
b1483:
  %gcvolta1483 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1483, ptr %ctxtopo, align 8
  ret i64 0
b1486:
  %v2211 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart.Format.uint8()
  %v2212 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v2212 to i64
  %v2213 = icmp ne i64 %c18, 0
  br i1 %v2213, label %b1491, label %b1490
b1487:
  %gcvolta1487 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1487, ptr %ctxtopo, align 8
  ret i64 0
b1490:
  %v2380 = add i64 0, 0
  %v2381 = add i64 0, 1
  %v2382 = call i64 @df.class(i64 %v0)
  switch i64 %v2382, label %b1611 [ i64 4, label %b1609 i64 2, label %b1610 i64 3, label %b1610 ]
b1491:
  %gcvolta1491 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1491, ptr %ctxtopo, align 8
  ret i64 0
b1609:
  %v2383 = call i64 @df.box_int(i64 %v2380)
  store i64 %v2383, ptr %gcs1
  %v2384 = call i64 @df.box_int(i64 %v2381)
  store i64 %v2384, ptr %gcs2
  %v2385 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v2383, i64 %v2384)
  %v2386 = load i8, ptr %ctx, align 8
  %c19 = zext i8 %v2386 to i64
  %v2387 = icmp ne i64 %c19, 0
  br i1 %v2387, label %b1614, label %b1613
b1610:
  %v2388 = call i64 @df.box_int(i64 %v2380)
  store i64 %v2388, ptr %gcs1
  %v2389 = call i64 @df.box_int(i64 %v2381)
  store i64 %v2389, ptr %gcs2
  %v2390 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v2388, i64 %v2389)
  %v2391 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v2391 to i64
  %v2392 = icmp ne i64 %c20, 0
  br i1 %v2392, label %b1616, label %b1615
b1611:
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
  %sic2395 = getelementptr i64, ptr %area, i64 92
  %sf2395 = call ptr @df.selector(ptr %sic2395, i64 %v0, i64 -1664458868869587675, ptr @df.seln.7, i64 8)
  %v2395 = call i64 %sf2395(i64 %v0, ptr %sargs2395, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2396 = load i8, ptr %ctx, align 8
  %c21 = zext i8 %v2396 to i64
  %v2397 = icmp ne i64 %c21, 0
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
  %sic2406 = getelementptr i64, ptr %area, i64 94
  %sf2406 = call ptr @df.selector(ptr %sic2406, i64 %v2398, i64 -4020825035383377172, ptr @df.seln.8, i64 4)
  %v2406 = call i64 %sf2406(i64 %v2398, ptr %sargs2406, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2407 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v2407 to i64
  %v2408 = icmp ne i64 %c22, 0
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
  %c23 = zext i8 %v2412 to i64
  %v2413 = icmp ne i64 %c23, 0
  br i1 %v2413, label %b1629, label %b1628
b1625:
  store i64 0, ptr %gcs1
  %v2417 = call i64 @df.dart$3acore._IntegerImplementation.toInt(i64 %v2409)
  %v2418 = load i8, ptr %ctx, align 8
  %c24 = zext i8 %v2418 to i64
  %v2419 = icmp ne i64 %c24, 0
  br i1 %v2419, label %b1633, label %b1632
b1626:
  store i64 0, ptr %gcs1
  %sat2420 = getelementptr [1 x i64], ptr %sargs2420, i64 0, i64 0
  store i64 0, ptr %sat2420
  %sic2420 = getelementptr i64, ptr %area, i64 96
  %sf2420 = call ptr @df.selector(ptr %sic2420, i64 %v2409, i64 4857594363590788828, ptr @df.seln.1, i64 8)
  %v2420 = call i64 %sf2420(i64 %v2409, ptr %sargs2420, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2420, ptr %gcs0
  %v2421 = load i8, ptr %ctx, align 8
  %c25 = zext i8 %v2421 to i64
  %v2422 = icmp ne i64 %c25, 0
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
  %c26 = zext i8 %v2415 to i64
  %v2416 = icmp ne i64 %c26, 0
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
  %c27 = zext i8 %v2424 to i64
  %v2425 = icmp ne i64 %c27, 0
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
}
