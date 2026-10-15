define i64 @df.primitive_clamp$2edart..typedUnitClamp(i64 %v0) noinline nounwind {
b0:
  %sargs85 = alloca [3 x i64]
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
  %v4 = add i64 0, 0
  %v5 = add i64 0, 1
  %v6 = call i64 @df.class(i64 %v0)
  switch i64 %v6, label %b3 [ i64 4, label %b1 i64 2, label %b2 i64 3, label %b2 ]
b1:
  %v7 = call i64 @df.box_int(i64 %v4)
  store i64 %v7, ptr %gcs1
  %v8 = call i64 @df.box_int(i64 %v5)
  store i64 %v8, ptr %gcs2
  %v23 = call i64 @df.class(i64 %v0)
  %v24 = icmp eq i64 %v23, 2
  %v25 = icmp eq i64 %v23, 3
  %v26 = icmp eq i64 %v23, 4
  %c0 = zext i1 %v24 to i64
  %c1 = zext i1 %v25 to i64
  %v27 = or i64 %c0, %c1
  %c2 = zext i1 %v26 to i64
  %v28 = or i64 %v27, %c2
  %c3 = trunc i64 %v28 to i1
  br i1 %c3, label %b12, label %b11
b2:
  %v12 = call i64 @df.box_int(i64 %v4)
  store i64 %v12, ptr %gcs1
  %v13 = call i64 @df.box_int(i64 %v5)
  store i64 %v13, ptr %gcs2
  %v44 = call i64 @df.class(i64 %v0)
  %v45 = icmp eq i64 %v44, 2
  %v46 = icmp eq i64 %v44, 3
  %v47 = icmp eq i64 %v44, 4
  %c4 = zext i1 %v45 to i64
  %c5 = zext i1 %v46 to i64
  %v48 = or i64 %c4, %c5
  %c6 = zext i1 %v47 to i64
  %v49 = or i64 %v48, %c6
  %c7 = trunc i64 %v49 to i1
  br i1 %c7, label %b22, label %b21
b3:
  %v17 = call i64 @df.box_int(i64 %v4)
  store i64 %v17, ptr %gcs1
  %v18 = call i64 @df.box_int(i64 %v5)
  store i64 %v18, ptr %gcs2
  %v65 = call i64 @df.class(i64 %v0)
  %v66 = icmp eq i64 %v65, 2
  %v67 = icmp eq i64 %v65, 3
  %v68 = icmp eq i64 %v65, 4
  %c8 = zext i1 %v66 to i64
  %c9 = zext i1 %v67 to i64
  %v69 = or i64 %c8, %c9
  %c10 = zext i1 %v68 to i64
  %v70 = or i64 %v69, %c10
  %c11 = trunc i64 %v70 to i1
  br i1 %c11, label %b32, label %b31
b4:
  %v22 = phi i64 [ %v9, %b5 ], [ %v14, %b7 ], [ %v19, %b9 ]
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret i64 %v22
b5:
  br label %b4
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret i64 0
b7:
  br label %b4
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret i64 0
b9:
  br label %b4
b10:
  %gcvolta10 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta10, ptr %ctxtopo, align 8
  ret i64 0
b12:
  br i1 %v26, label %b14, label %b13
b13:
  %v29 = call i64 @df.unbox_int(i64 %v0)
  %v30 = icmp slt i64 %v29, 0
  br i1 %v30, label %b17, label %b15
b14:
  %v32 = call double @df.unbox_double(i64 %v0)
  %v33 = bitcast double %v32 to i64
  %v34 = icmp eq i64 %v33, -9223372036854775808
  %v35 = fcmp olt double %v32, 0x0000000000000000
  %c12 = zext i1 %v35 to i64
  %c13 = zext i1 %v34 to i64
  %v36 = or i64 %c12, %c13
  %c14 = trunc i64 %v36 to i1
  br i1 %c14, label %b17, label %b16
b15:
  %v31 = icmp sgt i64 %v29, 1
  br i1 %v31, label %b18, label %b19
b16:
  %v37 = and i64 %v33, 9223372036854775807
  %v38 = icmp sgt i64 %v37, 9218868437227405312
  %v39 = fcmp ogt double %v32, 0x3FF0000000000000
  %c15 = zext i1 %v39 to i64
  %c16 = zext i1 %v38 to i64
  %v40 = or i64 %c15, %c16
  %c17 = trunc i64 %v40 to i1
  br i1 %c17, label %b18, label %b19
b17:
  br label %b20
b18:
  br label %b20
b19:
  br label %b20
b11:
  %v41 = phi i64 [ %v7, %b1 ]
  %v42 = phi i64 [ %v8, %b1 ]
  store i64 %v41, ptr %gcs1
  store i64 %v42, ptr %gcs2
  %v43 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v41, i64 %v42)
  br label %b20
b20:
  %v9 = phi i64 [ %v43, %b11 ], [ %v7, %b17 ], [ %v8, %b18 ], [ %v0, %b19 ]
  %v10 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v10 to i64
  %v11 = icmp ne i64 %c18, 0
  br i1 %v11, label %b6, label %b5
b22:
  br i1 %v47, label %b24, label %b23
b23:
  %v50 = call i64 @df.unbox_int(i64 %v0)
  %v51 = icmp slt i64 %v50, 0
  br i1 %v51, label %b27, label %b25
b24:
  %v53 = call double @df.unbox_double(i64 %v0)
  %v54 = bitcast double %v53 to i64
  %v55 = icmp eq i64 %v54, -9223372036854775808
  %v56 = fcmp olt double %v53, 0x0000000000000000
  %c19 = zext i1 %v56 to i64
  %c20 = zext i1 %v55 to i64
  %v57 = or i64 %c19, %c20
  %c21 = trunc i64 %v57 to i1
  br i1 %c21, label %b27, label %b26
b25:
  %v52 = icmp sgt i64 %v50, 1
  br i1 %v52, label %b28, label %b29
b26:
  %v58 = and i64 %v54, 9223372036854775807
  %v59 = icmp sgt i64 %v58, 9218868437227405312
  %v60 = fcmp ogt double %v53, 0x3FF0000000000000
  %c22 = zext i1 %v60 to i64
  %c23 = zext i1 %v59 to i64
  %v61 = or i64 %c22, %c23
  %c24 = trunc i64 %v61 to i1
  br i1 %c24, label %b28, label %b29
b27:
  br label %b30
b28:
  br label %b30
b29:
  br label %b30
b21:
  %v62 = phi i64 [ %v12, %b2 ]
  %v63 = phi i64 [ %v13, %b2 ]
  store i64 %v62, ptr %gcs1
  store i64 %v63, ptr %gcs2
  %v64 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v62, i64 %v63)
  br label %b30
b30:
  %v14 = phi i64 [ %v64, %b21 ], [ %v12, %b27 ], [ %v13, %b28 ], [ %v0, %b29 ]
  %v15 = load i8, ptr %ctx, align 8
  %c25 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c25, 0
  br i1 %v16, label %b8, label %b7
b32:
  br i1 %v68, label %b34, label %b33
b33:
  %v71 = call i64 @df.unbox_int(i64 %v0)
  %v72 = icmp slt i64 %v71, 0
  br i1 %v72, label %b37, label %b35
b34:
  %v74 = call double @df.unbox_double(i64 %v0)
  %v75 = bitcast double %v74 to i64
  %v76 = icmp eq i64 %v75, -9223372036854775808
  %v77 = fcmp olt double %v74, 0x0000000000000000
  %c26 = zext i1 %v77 to i64
  %c27 = zext i1 %v76 to i64
  %v78 = or i64 %c26, %c27
  %c28 = trunc i64 %v78 to i1
  br i1 %c28, label %b37, label %b36
b35:
  %v73 = icmp sgt i64 %v71, 1
  br i1 %v73, label %b38, label %b39
b36:
  %v79 = and i64 %v75, 9223372036854775807
  %v80 = icmp sgt i64 %v79, 9218868437227405312
  %v81 = fcmp ogt double %v74, 0x3FF0000000000000
  %c29 = zext i1 %v81 to i64
  %c30 = zext i1 %v80 to i64
  %v82 = or i64 %c29, %c30
  %c31 = trunc i64 %v82 to i1
  br i1 %c31, label %b38, label %b39
b37:
  br label %b40
b38:
  br label %b40
b39:
  br label %b40
b31:
  %v83 = phi i64 [ %v17, %b3 ]
  %v84 = phi i64 [ %v18, %b3 ]
  store i64 %v83, ptr %gcs1
  store i64 %v84, ptr %gcs2
  %sa85_0 = getelementptr [3 x i64], ptr %sargs85, i64 0, i64 0
  store i64 %v83, ptr %sa85_0
  %sa85_1 = getelementptr [3 x i64], ptr %sargs85, i64 0, i64 1
  store i64 %v84, ptr %sa85_1
  %sat85 = getelementptr [3 x i64], ptr %sargs85, i64 0, i64 2
  store i64 0, ptr %sat85
  %sic85 = getelementptr i64, ptr %area, i64 12
  %sf85 = call ptr @df.selector(ptr %sic85, i64 %v0, i64 -1664458868869587675, ptr @df.seln.1, i64 8)
  %v85 = call i64 %sf85(i64 %v0, ptr %sargs85, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  br label %b40
b40:
  %v19 = phi i64 [ %v85, %b31 ], [ %v17, %b37 ], [ %v18, %b38 ], [ %v0, %b39 ]
  %v20 = load i8, ptr %ctx, align 8
  %c32 = zext i8 %v20 to i64
  %v21 = icmp ne i64 %c32, 0
  br i1 %v21, label %b10, label %b9
}
