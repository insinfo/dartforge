define void @df.mixed_numeric_comparison$2edart..compare(i64 %v0, i64 %v3, i64 %v6) noinline nounwind {
b0:
  %sargs11 = alloca [1 x i64]
  %sargs22 = alloca [2 x i64]
  %sargs41 = alloca [2 x i64]
  %sargs60 = alloca [2 x i64]
  %sargs80 = alloca [2 x i64]
  %sargs99 = alloca [2 x i64]
  %sargs111 = alloca [2 x i64]
  %sargs114 = alloca [1 x i64]
  %jbuf118 = alloca [32 x i64]
  %gcq = alloca { ptr, i64, [16 x i64] }, align 8
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
  store { ptr, i64, [16 x i64] } { ptr null, i64 16, [16 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcs4 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 4
  %gcs5 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 5
  %gcs6 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 6
  %gcs7 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 7
  %gcs8 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 8
  %gcs9 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 9
  %gcs10 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 10
  %gcs11 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 11
  %gcs12 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 12
  %gcs13 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 13
  %gcs14 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 14
  %gcs15 = getelementptr inbounds { ptr, i64, [16 x i64] }, ptr %gcq, i64 0, i32 2, i64 15
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  store i64 %v0, ptr %gcs0
  store i64 %v3, ptr %gcs1
  store i64 %v6, ptr %gcs2
  %v9 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.edaf1bd3d1c2ffcc44df55829c002f26", i64 2) to i64
  store i64 %v9, ptr %gcs3
  %sat11 = getelementptr [1 x i64], ptr %sargs11, i64 0, i64 0
  store i64 0, ptr %sat11
  %sic11 = getelementptr i64, ptr %area, i64 0
  %sf11 = call ptr @df.selector(ptr %sic11, i64 %v0, i64 -3445551396850951156, ptr @df.seln.0, i64 10)
  %v11 = call i64 %sf11(i64 %v0, ptr %sargs11, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v11, ptr %gcs0
  %v12 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c0, 0
  br i1 %v13, label %b2, label %b1
b1:
  %v14 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.ec19394e30723c517d88baa498f680b7", i64 2) to i64
  store i64 %v14, ptr %gcs4
  %v17 = icmp eq i64 %v3, 0
  %v18 = icmp eq i64 %v6, 0
  %c1 = zext i1 %v17 to i64
  %c2 = zext i1 %v18 to i64
  %v19 = or i64 %c1, %c2
  %v20 = icmp ne i64 %v19, 0
  br i1 %v20, label %b3, label %b4
b2:
  %gcvolta2 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta2, ptr %ctxtopo, align 8
  ret void
b3:
  %v21 = icmp eq i64 %v3, %v6
  br label %b5
b4:
  %sa22_0 = getelementptr [2 x i64], ptr %sargs22, i64 0, i64 0
  store i64 %v6, ptr %sa22_0
  %sat22 = getelementptr [2 x i64], ptr %sargs22, i64 0, i64 1
  store i64 0, ptr %sat22
  %sic22 = getelementptr i64, ptr %area, i64 2
  %sf22 = call ptr @df.selector(ptr %sic22, i64 %v3, i64 -9163797671095228218, ptr @df.seln.1, i64 4)
  %v22 = call i64 %sf22(i64 %v3, ptr %sargs22, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v22, ptr %gcs5
  %v23 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v23 to i64
  %v24 = icmp ne i64 %c3, 0
  br i1 %v24, label %b7, label %b6
b5:
  %v28 = phi i1 [ %v21, %b3 ], [ %v25, %b8 ]
  store i64 0, ptr %gcs5
  %v29 = zext i1 %v28 to i8
  %v30 = call i64 @dartforge_to_string_bool(i8 %v29)
  store i64 %v30, ptr %gcs5
  %v31 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.4dadcec4914024c5a9c29ab74369d99a", i64 2) to i64
  store i64 %v31, ptr %gcs6
  %v34 = and i64 %v3, 1
  %v35 = and i64 %v6, 1
  %v36 = and i64 %v34, %v35
  %v37 = icmp ne i64 %v36, 0
  br i1 %v37, label %b10, label %b11
b6:
  %v25 = call i1 @df.unbox_bool(i64 %v22)
  %v26 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v26 to i64
  %v27 = icmp ne i64 %c4, 0
  br i1 %v27, label %b9, label %b8
b7:
  %gcvolta7 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta7, ptr %ctxtopo, align 8
  ret void
b8:
  br label %b5
b9:
  %gcvolta9 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta9, ptr %ctxtopo, align 8
  ret void
b10:
  %v38 = ashr i64 %v3, 1
  %v39 = ashr i64 %v6, 1
  %v40 = icmp slt i64 %v38, %v39
  br label %b12
b11:
  %sa41_0 = getelementptr [2 x i64], ptr %sargs41, i64 0, i64 0
  store i64 %v6, ptr %sa41_0
  %sat41 = getelementptr [2 x i64], ptr %sargs41, i64 0, i64 1
  store i64 0, ptr %sat41
  %sic41 = getelementptr i64, ptr %area, i64 4
  %sf41 = call ptr @df.selector(ptr %sic41, i64 %v3, i64 -678006041086143636, ptr @df.seln.2, i64 3)
  %v41 = call i64 %sf41(i64 %v3, ptr %sargs41, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v41, ptr %gcs7
  %v42 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v42 to i64
  %v43 = icmp ne i64 %c5, 0
  br i1 %v43, label %b14, label %b13
b12:
  %v47 = phi i1 [ %v40, %b10 ], [ %v44, %b15 ]
  store i64 0, ptr %gcs7
  %v48 = zext i1 %v47 to i8
  %v49 = call i64 @dartforge_to_string_bool(i8 %v48)
  store i64 %v49, ptr %gcs7
  %v50 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.4dadcec4914024c5a9c29ab74369d99a", i64 2) to i64
  store i64 %v50, ptr %gcs8
  %v53 = and i64 %v3, 1
  %v54 = and i64 %v6, 1
  %v55 = and i64 %v53, %v54
  %v56 = icmp ne i64 %v55, 0
  br i1 %v56, label %b17, label %b18
b13:
  %v44 = call i1 @df.unbox_bool(i64 %v41)
  %v45 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v45 to i64
  %v46 = icmp ne i64 %c6, 0
  br i1 %v46, label %b16, label %b15
b14:
  %gcvolta14 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta14, ptr %ctxtopo, align 8
  ret void
b15:
  br label %b12
b16:
  %gcvolta16 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta16, ptr %ctxtopo, align 8
  ret void
b17:
  %v57 = ashr i64 %v3, 1
  %v58 = ashr i64 %v6, 1
  %v59 = icmp sle i64 %v57, %v58
  br label %b19
b18:
  %sa60_0 = getelementptr [2 x i64], ptr %sargs60, i64 0, i64 0
  store i64 %v6, ptr %sa60_0
  %sat60 = getelementptr [2 x i64], ptr %sargs60, i64 0, i64 1
  store i64 0, ptr %sat60
  %sic60 = getelementptr i64, ptr %area, i64 6
  %sf60 = call ptr @df.selector(ptr %sic60, i64 %v3, i64 -9164787231560428893, ptr @df.seln.3, i64 4)
  %v60 = call i64 %sf60(i64 %v3, ptr %sargs60, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v60, ptr %gcs9
  %v61 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v61 to i64
  %v62 = icmp ne i64 %c7, 0
  br i1 %v62, label %b21, label %b20
b19:
  %v66 = phi i1 [ %v59, %b17 ], [ %v63, %b22 ]
  store i64 0, ptr %gcs9
  %v67 = zext i1 %v66 to i8
  %v68 = call i64 @dartforge_to_string_bool(i8 %v67)
  store i64 %v68, ptr %gcs9
  %v69 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.4dadcec4914024c5a9c29ab74369d99a", i64 2) to i64
  store i64 %v69, ptr %gcs10
  %v70 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.edaf1bd3d1c2ffcc44df55829c002f26", i64 2) to i64
  store i64 %v70, ptr %gcs11
  %v73 = and i64 %v3, 1
  %v74 = and i64 %v6, 1
  %v75 = and i64 %v73, %v74
  %v76 = icmp ne i64 %v75, 0
  br i1 %v76, label %b24, label %b25
b20:
  %v63 = call i1 @df.unbox_bool(i64 %v60)
  %v64 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v64 to i64
  %v65 = icmp ne i64 %c8, 0
  br i1 %v65, label %b23, label %b22
b21:
  %gcvolta21 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta21, ptr %ctxtopo, align 8
  ret void
b22:
  br label %b19
b23:
  %gcvolta23 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta23, ptr %ctxtopo, align 8
  ret void
b24:
  %v77 = ashr i64 %v3, 1
  %v78 = ashr i64 %v6, 1
  %v79 = icmp sgt i64 %v77, %v78
  br label %b26
b25:
  %sa80_0 = getelementptr [2 x i64], ptr %sargs80, i64 0, i64 0
  store i64 %v6, ptr %sa80_0
  %sat80 = getelementptr [2 x i64], ptr %sargs80, i64 0, i64 1
  store i64 0, ptr %sat80
  %sic80 = getelementptr i64, ptr %area, i64 8
  %sf80 = call ptr @df.selector(ptr %sic80, i64 %v3, i64 -678003842062887214, ptr @df.seln.4, i64 3)
  %v80 = call i64 %sf80(i64 %v3, ptr %sargs80, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v80, ptr %gcs12
  %v81 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v81 to i64
  %v82 = icmp ne i64 %c9, 0
  br i1 %v82, label %b28, label %b27
b26:
  %v86 = phi i1 [ %v79, %b24 ], [ %v83, %b29 ]
  store i64 0, ptr %gcs12
  %v87 = zext i1 %v86 to i8
  %v88 = call i64 @dartforge_to_string_bool(i8 %v87)
  store i64 %v88, ptr %gcs12
  %v89 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.4dadcec4914024c5a9c29ab74369d99a", i64 2) to i64
  store i64 %v89, ptr %gcs13
  %v92 = and i64 %v3, 1
  %v93 = and i64 %v6, 1
  %v94 = and i64 %v92, %v93
  %v95 = icmp ne i64 %v94, 0
  br i1 %v95, label %b31, label %b32
b27:
  %v83 = call i1 @df.unbox_bool(i64 %v80)
  %v84 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v84 to i64
  %v85 = icmp ne i64 %c10, 0
  br i1 %v85, label %b30, label %b29
b28:
  %gcvolta28 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta28, ptr %ctxtopo, align 8
  ret void
b29:
  br label %b26
b30:
  %gcvolta30 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta30, ptr %ctxtopo, align 8
  ret void
b31:
  %v96 = ashr i64 %v3, 1
  %v97 = ashr i64 %v6, 1
  %v98 = icmp sge i64 %v96, %v97
  br label %b33
b32:
  %sa99_0 = getelementptr [2 x i64], ptr %sargs99, i64 0, i64 0
  store i64 %v6, ptr %sa99_0
  %sat99 = getelementptr [2 x i64], ptr %sargs99, i64 0, i64 1
  store i64 0, ptr %sat99
  %sic99 = getelementptr i64, ptr %area, i64 10
  %sf99 = call ptr @df.selector(ptr %sic99, i64 %v3, i64 -9162812508676540387, ptr @df.seln.5, i64 4)
  %v99 = call i64 %sf99(i64 %v3, ptr %sargs99, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v99, ptr %gcs14
  %v100 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v100 to i64
  %v101 = icmp ne i64 %c11, 0
  br i1 %v101, label %b35, label %b34
b33:
  %v105 = phi i1 [ %v98, %b31 ], [ %v102, %b36 ]
  store i64 0, ptr %gcs14
  %v106 = zext i1 %v105 to i8
  %v107 = call i64 @dartforge_to_string_bool(i8 %v106)
  store i64 %v107, ptr %gcs14
  %v108 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.4dadcec4914024c5a9c29ab74369d99a", i64 2) to i64
  store i64 %v108, ptr %gcs15
  %sa111_0 = getelementptr [2 x i64], ptr %sargs111, i64 0, i64 0
  store i64 %v6, ptr %sa111_0
  %sat111 = getelementptr [2 x i64], ptr %sargs111, i64 0, i64 1
  store i64 0, ptr %sat111
  %sic111 = getelementptr i64, ptr %area, i64 12
  %sf111 = call ptr @df.selector(ptr %sic111, i64 %v3, i64 -1318469987607712126, ptr @df.seln.6, i64 11)
  %v111 = call i64 %sf111(i64 %v3, ptr %sargs111, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v111, ptr %gcs1
  %v112 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v112 to i64
  %v113 = icmp ne i64 %c12, 0
  br i1 %v113, label %b39, label %b38
b34:
  %v102 = call i1 @df.unbox_bool(i64 %v99)
  %v103 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v103 to i64
  %v104 = icmp ne i64 %c13, 0
  br i1 %v104, label %b37, label %b36
b35:
  %gcvolta35 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta35, ptr %ctxtopo, align 8
  ret void
b36:
  br label %b33
b37:
  %gcvolta37 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta37, ptr %ctxtopo, align 8
  ret void
b38:
  store i64 0, ptr %gcs2
  %sat114 = getelementptr [1 x i64], ptr %sargs114, i64 0, i64 0
  store i64 0, ptr %sat114
  %sic114 = getelementptr i64, ptr %area, i64 14
  %sf114 = call ptr @df.selector(ptr %sic114, i64 %v111, i64 -3445551396850951156, ptr @df.seln.0, i64 10)
  %v114 = call i64 %sf114(i64 %v111, ptr %sargs114, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v114, ptr %gcs1
  %v115 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v115 to i64
  %v116 = icmp ne i64 %c14, 0
  br i1 %v116, label %b41, label %b40
b39:
  %gcvolta39 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta39, ptr %ctxtopo, align 8
  ret void
b40:
  %v117 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.edaf1bd3d1c2ffcc44df55829c002f26", i64 2) to i64
  store i64 %v117, ptr %gcs2
  %jk118_0 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 0
  store i64 0, ptr %jk118_0
  %jp118_0 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 1
  store i64 %v9, ptr %jp118_0
  %jk118_1 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 2
  store i64 0, ptr %jk118_1
  %jp118_1 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 3
  store i64 %v11, ptr %jp118_1
  %jk118_2 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 4
  store i64 0, ptr %jk118_2
  %jp118_2 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 5
  store i64 %v14, ptr %jp118_2
  %jk118_3 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 6
  store i64 0, ptr %jk118_3
  %jp118_3 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 7
  store i64 %v30, ptr %jp118_3
  %jk118_4 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 8
  store i64 0, ptr %jk118_4
  %jp118_4 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 9
  store i64 %v31, ptr %jp118_4
  %jk118_5 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 10
  store i64 0, ptr %jk118_5
  %jp118_5 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 11
  store i64 %v49, ptr %jp118_5
  %jk118_6 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 12
  store i64 0, ptr %jk118_6
  %jp118_6 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 13
  store i64 %v50, ptr %jp118_6
  %jk118_7 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 14
  store i64 0, ptr %jk118_7
  %jp118_7 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 15
  store i64 %v68, ptr %jp118_7
  %jk118_8 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 16
  store i64 0, ptr %jk118_8
  %jp118_8 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 17
  store i64 %v69, ptr %jp118_8
  %jk118_9 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 18
  store i64 0, ptr %jk118_9
  %jp118_9 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 19
  store i64 %v70, ptr %jp118_9
  %jk118_10 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 20
  store i64 0, ptr %jk118_10
  %jp118_10 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 21
  store i64 %v88, ptr %jp118_10
  %jk118_11 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 22
  store i64 0, ptr %jk118_11
  %jp118_11 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 23
  store i64 %v89, ptr %jp118_11
  %jk118_12 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 24
  store i64 0, ptr %jk118_12
  %jp118_12 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 25
  store i64 %v107, ptr %jp118_12
  %jk118_13 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 26
  store i64 0, ptr %jk118_13
  %jp118_13 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 27
  store i64 %v108, ptr %jp118_13
  %jk118_14 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 28
  store i64 0, ptr %jk118_14
  %jp118_14 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 29
  store i64 %v114, ptr %jp118_14
  %jk118_15 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 30
  store i64 0, ptr %jk118_15
  %jp118_15 = getelementptr [32 x i64], ptr %jbuf118, i64 0, i64 31
  store i64 %v117, ptr %jp118_15
  %v118 = call i64 @dartforge_string_join_typed(ptr %jbuf118, i64 16)
  store i64 %v118, ptr %gcs0
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  store i64 0, ptr %gcs3
  store i64 0, ptr %gcs4
  store i64 0, ptr %gcs5
  store i64 0, ptr %gcs6
  store i64 0, ptr %gcs7
  store i64 0, ptr %gcs8
  store i64 0, ptr %gcs9
  store i64 0, ptr %gcs10
  store i64 0, ptr %gcs11
  store i64 0, ptr %gcs12
  store i64 0, ptr %gcs13
  store i64 0, ptr %gcs14
  store i64 0, ptr %gcs15
  call void @df.dart$3acore..print(i64 %v118)
  %v120 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v120 to i64
  %v121 = icmp ne i64 %c15, 0
  br i1 %v121, label %b43, label %b42
b41:
  %gcvolta41 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta41, ptr %ctxtopo, align 8
  ret void
b42:
  %gcvolta42 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta42, ptr %ctxtopo, align 8
  ret void
b43:
  %gcvolta43 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta43, ptr %ctxtopo, align 8
  ret void
}
