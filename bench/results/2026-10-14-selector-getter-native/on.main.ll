define void @dart_main() nounwind {
b0:
  %sargs7 = alloca [1 x i64]
  %sargs11 = alloca [2 x i64]
  %list_buf_19 = alloca [2 x i64], align 8
  %list_tags_19 = alloca [2 x i8], align 1
  %sargs94 = alloca [1 x i64]
  %sargs84 = alloca [1 x i64]
  %sargs58 = alloca [2 x i64]
  %sargs88 = alloca [2 x i64]
  %sargs97 = alloca [1 x i64]
  %sargs108 = alloca [1 x i64]
  %sargs131 = alloca [1 x i64]
  %sargs135 = alloca [2 x i64]
  %jbuf168 = alloca [6 x i64]
  %sargs188 = alloca [1 x i64]
  %sargs192 = alloca [2 x i64]
  %sargs199 = alloca [1 x i64]
  %sargs203 = alloca [2 x i64]
  %sargs210 = alloca [1 x i64]
  %list_buf_215 = alloca [2 x i64], align 8
  %list_tags_215 = alloca [2 x i8], align 1
  %sargs222 = alloca [2 x i64]
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
  ret void
pilha.ok:
  store { ptr, i64, [4 x i64] } { ptr null, i64 4, [4 x i64] zeroinitializer }, ptr %gcq
  %gcs0 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 0
  %gcs1 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 1
  %gcs2 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 2
  %gcs3 = getelementptr inbounds { ptr, i64, [4 x i64] }, ptr %gcq, i64 0, i32 2, i64 3
  %gcant = load ptr, ptr %ctxtopo, align 8
  store ptr %gcant, ptr %gcq, align 8
  store ptr %gcq, ptr %ctxtopo, align 8
  %ta0 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb0 = load ptr, ptr %ta0, align 8
  %tfa0 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf0 = load ptr, ptr %tfa0, align 8
  %tnx0 = getelementptr i8, ptr %tb0, i64 24
  %tz0 = icmp ugt ptr %tnx0, %tf0
  %tnp0 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn0 = load i64, ptr %tnp0, align 8
  %tk0 = icmp ule i64 %tn0, 1029
  %tq0 = or i1 %tz0, %tk0
  %tx0 = call i1 @llvm.expect.i1(i1 %tq0, i1 false)
  br i1 %tx0, label %ao0.lento, label %ao0.reg
ao0.reg:
  %trp0 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr0 = load ptr, ptr %trp0, align 8
  %trb0 = getelementptr inbounds i8, ptr %tr0, i64 1029
  %trv0 = load i8, ptr %trb0, align 1
  %trz0 = icmp eq i8 %trv0, 0
  %try0 = call i1 @llvm.expect.i1(i1 %trz0, i1 false)
  br i1 %try0, label %ao0.lento, label %ao0.rapido
ao0.rapido:
  store ptr %tnx0, ptr %ta0, align 8
  %tpp0 = getelementptr inbounds i8, ptr %tb0, i64 16
  store i64 0, ptr %tpp0, align 8
  store i64 4419521347585, ptr %tb0, align 8
  %thb0 = ptrtoint ptr %tb0 to i64
  %th0 = add i64 %thb0, 2
  br label %ao0.fim
ao0.lento:
  %tl0 = call i64 @dartforge_object_new_t(i64 1029, i64 0, ptr @df.mt.getter_return_provenance$2edart.Owner)
  br label %ao0.fim
ao0.fim:
  %v0 = phi i64 [ %th0, %ao0.rapido ], [ %tl0, %ao0.lento ]
  store i64 %v0, ptr %gcs0
  %sat7 = getelementptr [1 x i64], ptr %sargs7, i64 0, i64 0
  store i64 0, ptr %sat7
  %sic7 = getelementptr i64, ptr %area, i64 30
  ; df.selector-exact 1029
  %sf7 = call ptr @df.selector(ptr %sic7, i64 %v0, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v7 = call i64 %sf7(i64 %v0, ptr %sargs7, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v7, ptr %gcs0
  %v8 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v8 to i64
  %v9 = icmp ne i64 %c0, 0
  br i1 %v9, label %b4, label %b3
b3:
  %v10 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.fd675c8bb0bc6c26db040982b16764a2", i64 2) to i64
  store i64 %v10, ptr %gcs1
  %sa11_0 = getelementptr [2 x i64], ptr %sargs11, i64 0, i64 0
  store i64 %v10, ptr %sa11_0
  %sat11 = getelementptr [2 x i64], ptr %sargs11, i64 0, i64 1
  store i64 0, ptr %sat11
  %sic11 = getelementptr i64, ptr %area, i64 32
  ; df.selector-exact 1,1025
  %sf11 = call ptr @df.selector(ptr %sic11, i64 %v7, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v11 = call i64 %sf11(i64 %v7, ptr %sargs11, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v11, ptr %gcs0
  %v12 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v12 to i64
  %v13 = icmp ne i64 %c1, 0
  br i1 %v13, label %b6, label %b5
b4:
  %gcvolta4 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta4, ptr %ctxtopo, align 8
  ret void
b5:
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v11)
  %v15 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v15 to i64
  %v16 = icmp ne i64 %c2, 0
  br i1 %v16, label %b8, label %b7
b6:
  %gcvolta6 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta6, ptr %ctxtopo, align 8
  ret void
b7:
  store i64 0, ptr %gcs0
  %lp19_0 = getelementptr [2 x i64], ptr %list_buf_19, i64 0, i64 0
  store i64 1, ptr %lp19_0, align 8
  %lp19_1 = getelementptr [2 x i64], ptr %list_buf_19, i64 0, i64 1
  store i64 0, ptr %lp19_1, align 8
  %v19 = call i64 @dartforge_list_new(ptr %list_buf_19, i64 2, i64 10, i64 3)
  store i64 %v19, ptr %gcs0
  %ga20 = getelementptr i64, ptr %area, i64 1
  %v20 = load i8, ptr %ga20
  %c3 = zext i8 %v20 to i64
  %v21 = icmp ne i64 %c3, 0
  br i1 %v21, label %b9, label %b10
b8:
  %gcvolta8 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta8, ptr %ctxtopo, align 8
  ret void
b9:
  %ga22 = getelementptr i64, ptr %area, i64 0
  %v22 = load i64, ptr %ga22
  br label %b11
b10:
  %v23 = call i64 @df.rti.3d5d570afdeccc37.getter_return_provenance$2edart()
  br label %b11
b11:
  %v24 = phi i64 [ %v22, %b9 ], [ %v23, %b10 ]
  call void @dartforge_rti_set(i64 %v19, i64 %v24)
  %v26 = call i64 @df.class(i64 %v19)
  %v27 = sub i64 %v26, 8
  %v28 = icmp ult i64 %v27, 3
  br i1 %v28, label %b12, label %b13
b12:
  %v29 = call i64 @df.list_len(i64 %v19)
  br label %b15
b13:
  %sat94 = getelementptr [1 x i64], ptr %sargs94, i64 0, i64 0
  store i64 0, ptr %sat94
  %sic94 = getelementptr i64, ptr %area, i64 34
  ; df.selector-exact 10
  %sf94 = call ptr @df.selector(ptr %sic94, i64 %v19, i64 -5476269667379286226, ptr @df.seln.3, i64 10)
  %v94 = call i64 %sf94(i64 %v19, ptr %sargs94, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v94, ptr %gcs0
  %v95 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v95 to i64
  %v96 = icmp ne i64 %c4, 0
  br i1 %v96, label %b53, label %b52
b14:
  store i64 0, ptr %gcs0
  store i64 0, ptr %gcs1
  %ta141 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb141 = load ptr, ptr %ta141, align 8
  %tfa141 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf141 = load ptr, ptr %tfa141, align 8
  %tnx141 = getelementptr i8, ptr %tb141, i64 24
  %tz141 = icmp ugt ptr %tnx141, %tf141
  %tnp141 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn141 = load i64, ptr %tnp141, align 8
  %tk141 = icmp ule i64 %tn141, 1024
  %tq141 = or i1 %tz141, %tk141
  %tx141 = call i1 @llvm.expect.i1(i1 %tq141, i1 false)
  br i1 %tx141, label %ao141.lento, label %ao141.reg
ao141.reg:
  %trp141 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr141 = load ptr, ptr %trp141, align 8
  %trb141 = getelementptr inbounds i8, ptr %tr141, i64 1024
  %trv141 = load i8, ptr %trb141, align 1
  %trz141 = icmp eq i8 %trv141, 0
  %try141 = call i1 @llvm.expect.i1(i1 %trz141, i1 false)
  br i1 %try141, label %ao141.lento, label %ao141.rapido
ao141.rapido:
  store ptr %tnx141, ptr %ta141, align 8
  %tpp141 = getelementptr inbounds i8, ptr %tb141, i64 16
  store i64 0, ptr %tpp141, align 8
  store i64 4398046511105, ptr %tb141, align 8
  %thb141 = ptrtoint ptr %tb141 to i64
  %th141 = add i64 %thb141, 2
  br label %ao141.fim
ao141.lento:
  %tl141 = call i64 @dartforge_object_new_t(i64 1024, i64 0, ptr @df.mt.getter_return_provenance$2edart.CustomCodec)
  br label %ao141.fim
ao141.fim:
  %v141 = phi i64 [ %th141, %ao141.rapido ], [ %tl141, %ao141.lento ]
  store i64 %v141, ptr %gcs0
  %v145 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.82cdedf90b670855d9426cb8c98e2c21", i64 2) to i64
  store i64 %v145, ptr %gcs1
  %v146 = call i64 @df.getter_return_provenance$2edart..convertUnknown(i64 %v141, i64 %v145)
  store i64 %v146, ptr %gcs0
  %v147 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v147 to i64
  %v148 = icmp ne i64 %c5, 0
  br i1 %v148, label %b87, label %b86
b15:
  %v228 = phi i64 [ 0, %b12 ], [ %v65, %b50 ]
  %v32 = call i64 @df.list_len(i64 %v19)
  %v33 = icmp eq i64 %v32, %v29
  br i1 %v33, label %b17, label %b16
b16:
  store i64 0, ptr %gcs1
  %ta34 = getelementptr inbounds i8, ptr %ctx, i64 96
  %tb34 = load ptr, ptr %ta34, align 8
  %tfa34 = getelementptr inbounds i8, ptr %ctx, i64 104
  %tf34 = load ptr, ptr %tfa34, align 8
  %tnx34 = getelementptr i8, ptr %tb34, i64 32
  %tz34 = icmp ugt ptr %tnx34, %tf34
  %tnp34 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn34 = load i64, ptr %tnp34, align 8
  %tk34 = icmp ule i64 %tn34, 581
  %tq34 = or i1 %tz34, %tk34
  %tx34 = call i1 @llvm.expect.i1(i1 %tq34, i1 false)
  br i1 %tx34, label %ao34.lento, label %ao34.reg
ao34.reg:
  %trp34 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr34 = load ptr, ptr %trp34, align 8
  %trb34 = getelementptr inbounds i8, ptr %tr34, i64 581
  %trv34 = load i8, ptr %trb34, align 1
  %trz34 = icmp eq i8 %trv34, 0
  %try34 = call i1 @llvm.expect.i1(i1 %trz34, i1 false)
  br i1 %try34, label %ao34.lento, label %ao34.rapido
ao34.rapido:
  store ptr %tnx34, ptr %ta34, align 8
  %tpp34 = getelementptr inbounds i8, ptr %tb34, i64 16
  store i64 0, ptr %tpp34, align 8
  store i64 2495376130049, ptr %tb34, align 8
  %thb34 = ptrtoint ptr %tb34 to i64
  %th34 = add i64 %thb34, 2
  br label %ao34.fim
ao34.lento:
  %tl34 = call i64 @dartforge_object_new_t(i64 581, i64 2, ptr @df.mt.dart$3acore.ConcurrentModificationError)
  br label %ao34.fim
ao34.fim:
  %v34 = phi i64 [ %th34, %ao34.rapido ], [ %tl34, %ao34.lento ]
  store i64 %v34, ptr %gcs1
  call void @df.dart$3acore.ConcurrentModificationError.new(i64 %v34, i64 %v19)
  %v36 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v36 to i64
  %v37 = icmp ne i64 %c6, 0
  br i1 %v37, label %b20, label %b19
b17:
  %v40 = icmp slt i64 %v228, %v32
  br i1 %v40, label %b18, label %b14
b18:
  %ip41 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv41 = load atomic i8, ptr %ip41 monotonic, align 8
  %v41 = call i8 @llvm.expect.i8(i8 %iv41, i8 0)
  %c7 = zext i8 %v41 to i64
  %v42 = icmp ne i64 %c7, 0
  br i1 %v42, label %b22, label %b23
b19:
  store i64 0, ptr %gcs0
  call void @dartforge_exception_throw(i64 %v34, i8 3)
  %gcvolta19 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta19, ptr %ctxtopo, align 8
  ret void
b20:
  %gcvolta20 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta20, ptr %ctxtopo, align 8
  ret void
b22:
  store i64 0, ptr %gcs1
  call void @dartforge_safepoint()
  %v44 = load i8, ptr %ctx, align 8
  %c8 = zext i8 %v44 to i64
  %v45 = icmp ne i64 %c8, 0
  br i1 %v45, label %b25, label %b24
b23:
  %v46 = call i64 @df.core_storage(i64 %v19)
  %v47 = call i64 @df.core_shape(i64 %v46)
  %v48 = add i64 %v46, 22
  %v49 = icmp eq i64 %v47, 98
  br i1 %v49, label %b27, label %b28
b24:
  br label %b23
b25:
  %gcvolta25 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta25, ptr %ctxtopo, align 8
  ret void
b26:
  %v64 = phi i1 [ %v51, %b27 ], [ %v54, %b31 ], [ %v61, %b35 ]
  %v65 = add i64 %v228, 1
  br i1 %v64, label %b132, label %b133
b132:
  store i64 0, ptr %gcs1
  %ta229 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb229 = load ptr, ptr %ta229, align 8
  %tfa229 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf229 = load ptr, ptr %tfa229, align 8
  %tnx229 = getelementptr i8, ptr %tb229, i64 24
  %tz229 = icmp ugt ptr %tnx229, %tf229
  %tnp229 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn229 = load i64, ptr %tnp229, align 8
  %tk229 = icmp ule i64 %tn229, 1029
  %tq229 = or i1 %tz229, %tk229
  %tx229 = call i1 @llvm.expect.i1(i1 %tq229, i1 false)
  br i1 %tx229, label %ao229.lento, label %ao229.reg
ao229.reg:
  %trp229 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr229 = load ptr, ptr %trp229, align 8
  %trb229 = getelementptr inbounds i8, ptr %tr229, i64 1029
  %trv229 = load i8, ptr %trb229, align 1
  %trz229 = icmp eq i8 %trv229, 0
  %try229 = call i1 @llvm.expect.i1(i1 %trz229, i1 false)
  br i1 %try229, label %ao229.lento, label %ao229.rapido
ao229.rapido:
  store ptr %tnx229, ptr %ta229, align 8
  %tpp229 = getelementptr inbounds i8, ptr %tb229, i64 16
  store i64 0, ptr %tpp229, align 8
  store i64 4419521347585, ptr %tb229, align 8
  %thb229 = ptrtoint ptr %tb229 to i64
  %th229 = add i64 %thb229, 2
  br label %ao229.fim
ao229.lento:
  %tl229 = call i64 @dartforge_object_new_t(i64 1029, i64 0, ptr @df.mt.getter_return_provenance$2edart.Owner)
  br label %ao229.fim
ao229.fim:
  %v229 = phi i64 [ %th229, %ao229.rapido ], [ %tl229, %ao229.lento ]
  br label %b134
b133:
  store i64 0, ptr %gcs1
  %ta231 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb231 = load ptr, ptr %ta231, align 8
  %tfa231 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf231 = load ptr, ptr %tfa231, align 8
  %tnx231 = getelementptr i8, ptr %tb231, i64 24
  %tz231 = icmp ugt ptr %tnx231, %tf231
  %tnp231 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn231 = load i64, ptr %tnp231, align 8
  %tk231 = icmp ule i64 %tn231, 1028
  %tq231 = or i1 %tz231, %tk231
  %tx231 = call i1 @llvm.expect.i1(i1 %tq231, i1 false)
  br i1 %tx231, label %ao231.lento, label %ao231.reg
ao231.reg:
  %trp231 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr231 = load ptr, ptr %trp231, align 8
  %trb231 = getelementptr inbounds i8, ptr %tr231, i64 1028
  %trv231 = load i8, ptr %trb231, align 1
  %trz231 = icmp eq i8 %trv231, 0
  %try231 = call i1 @llvm.expect.i1(i1 %trz231, i1 false)
  br i1 %try231, label %ao231.lento, label %ao231.rapido
ao231.rapido:
  store ptr %tnx231, ptr %ta231, align 8
  %tpp231 = getelementptr inbounds i8, ptr %tb231, i64 16
  store i64 0, ptr %tpp231, align 8
  store i64 4415226380289, ptr %tb231, align 8
  %thb231 = ptrtoint ptr %tb231 to i64
  %th231 = add i64 %thb231, 2
  br label %ao231.fim
ao231.lento:
  %tl231 = call i64 @dartforge_object_new_t(i64 1028, i64 0, ptr @df.mt.getter_return_provenance$2edart.OtherOwner)
  br label %ao231.fim
ao231.fim:
  %v231 = phi i64 [ %th231, %ao231.rapido ], [ %tl231, %ao231.lento ]
  br label %b134
b134:
  %v233 = phi i64 [ %v229, %ao229.fim ], [ %v231, %ao231.fim ]
  store i64 %v233, ptr %gcs1
  %sat84 = getelementptr [1 x i64], ptr %sargs84, i64 0, i64 0
  store i64 0, ptr %sat84
  %sic84 = getelementptr i64, ptr %area, i64 36
  ; df.selector-exact 1028,1029
  %sf84 = call ptr @df.selector(ptr %sic84, i64 %v233, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v84 = call i64 %sf84(i64 %v233, ptr %sargs84, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v84, ptr %gcs1
  %v85 = load i8, ptr %ctx, align 8
  %c9 = zext i8 %v85 to i64
  %v86 = icmp ne i64 %c9, 0
  br i1 %v86, label %b47, label %b46
b27:
  %cp50 = inttoptr i64 %v48 to ptr
  %cg50 = getelementptr i64, ptr %cp50, i64 %v228
  %v50 = load i64, ptr %cg50, align 1
  %v51 = icmp ne i64 %v50, 0
  br label %b26
b28:
  %v52 = icmp eq i64 %v47, 4
  br i1 %v52, label %b29, label %b30
b29:
  %v53 = call i64 @df.word_ref(i64 %v48, i64 %v228)
  store i64 %v53, ptr %gcs1
  %v54 = call i1 @df.unbox_bool(i64 %v53)
  %v55 = load i8, ptr %ctx, align 8
  %c10 = zext i8 %v55 to i64
  %v56 = icmp ne i64 %c10, 0
  br i1 %v56, label %b32, label %b31
b30:
  store i64 0, ptr %gcs1
  %v57 = call i64 @df.box_int(i64 %v228)
  store i64 %v57, ptr %gcs1
  %sa58_0 = getelementptr [2 x i64], ptr %sargs58, i64 0, i64 0
  store i64 %v57, ptr %sa58_0
  %sat58 = getelementptr [2 x i64], ptr %sargs58, i64 0, i64 1
  store i64 0, ptr %sat58
  %sic58 = getelementptr i64, ptr %area, i64 38
  ; df.selector-exact 10
  %sf58 = call ptr @df.selector(ptr %sic58, i64 %v19, i64 2870274114568068918, ptr @df.seln.4, i64 5)
  %v58 = call i64 %sf58(i64 %v19, ptr %sargs58, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v58, ptr %gcs1
  %v59 = load i8, ptr %ctx, align 8
  %c11 = zext i8 %v59 to i64
  %v60 = icmp ne i64 %c11, 0
  br i1 %v60, label %b34, label %b33
b31:
  br label %b26
b32:
  %gcvolta32 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta32, ptr %ctxtopo, align 8
  ret void
b33:
  %v61 = call i1 @df.unbox_bool(i64 %v58)
  %v62 = load i8, ptr %ctx, align 8
  %c12 = zext i8 %v62 to i64
  %v63 = icmp ne i64 %c12, 0
  br i1 %v63, label %b36, label %b35
b34:
  %gcvolta34 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta34, ptr %ctxtopo, align 8
  ret void
b35:
  br label %b26
b36:
  %gcvolta36 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta36, ptr %ctxtopo, align 8
  ret void
b46:
  %v87 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.8e13c211ab38ef9000028cbc061a83c5", i64 2) to i64
  store i64 %v87, ptr %gcs2
  %sa88_0 = getelementptr [2 x i64], ptr %sargs88, i64 0, i64 0
  store i64 %v87, ptr %sa88_0
  %sat88 = getelementptr [2 x i64], ptr %sargs88, i64 0, i64 1
  store i64 0, ptr %sat88
  %sic88 = getelementptr i64, ptr %area, i64 40
  ; df.selector-exact 1,1025,1026
  %sf88 = call ptr @df.selector(ptr %sic88, i64 %v84, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v88 = call i64 %sf88(i64 %v84, ptr %sargs88, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v88, ptr %gcs1
  %v89 = load i8, ptr %ctx, align 8
  %c13 = zext i8 %v89 to i64
  %v90 = icmp ne i64 %c13, 0
  br i1 %v90, label %b49, label %b48
b47:
  %gcvolta47 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta47, ptr %ctxtopo, align 8
  ret void
b48:
  store i64 0, ptr %gcs2
  call void @df.dart$3acore..print(i64 %v88)
  %v92 = load i8, ptr %ctx, align 8
  %c14 = zext i8 %v92 to i64
  %v93 = icmp ne i64 %c14, 0
  br i1 %v93, label %b51, label %b50
b49:
  %gcvolta49 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta49, ptr %ctxtopo, align 8
  ret void
b50:
  br label %b15
b51:
  %gcvolta51 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta51, ptr %ctxtopo, align 8
  ret void
b52:
  br label %b54
b53:
  %gcvolta53 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta53, ptr %ctxtopo, align 8
  ret void
b54:
  store i64 0, ptr %gcs1
  %sat97 = getelementptr [1 x i64], ptr %sargs97, i64 0, i64 0
  store i64 0, ptr %sat97
  %sic97 = getelementptr i64, ptr %area, i64 42
  %sf97 = call ptr @df.selector(ptr %sic97, i64 %v94, i64 -5038376148439920376, ptr @df.seln.5, i64 10)
  %v97 = call i64 %sf97(i64 %v94, ptr %sargs97, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v97, ptr %gcs1
  %v98 = load i8, ptr %ctx, align 8
  %c15 = zext i8 %v98 to i64
  %v99 = icmp ne i64 %c15, 0
  br i1 %v99, label %b58, label %b57
b55:
  %ip103 = getelementptr inbounds i8, ptr %ctx, i64 32
  %iv103 = load atomic i8, ptr %ip103 monotonic, align 8
  %v103 = call i8 @llvm.expect.i8(i8 %iv103, i8 0)
  %c16 = zext i8 %v103 to i64
  %v104 = icmp ne i64 %c16, 0
  br i1 %v104, label %b61, label %b62
b56:
  br label %b14
b57:
  %v100 = call i1 @df.unbox_bool(i64 %v97)
  %v101 = load i8, ptr %ctx, align 8
  %c17 = zext i8 %v101 to i64
  %v102 = icmp ne i64 %c17, 0
  br i1 %v102, label %b60, label %b59
b58:
  %gcvolta58 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta58, ptr %ctxtopo, align 8
  ret void
b59:
  br i1 %v100, label %b55, label %b56
b60:
  %gcvolta60 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta60, ptr %ctxtopo, align 8
  ret void
b61:
  store i64 0, ptr %gcs1
  call void @dartforge_safepoint()
  %v106 = load i8, ptr %ctx, align 8
  %c18 = zext i8 %v106 to i64
  %v107 = icmp ne i64 %c18, 0
  br i1 %v107, label %b64, label %b63
b62:
  store i64 0, ptr %gcs1
  %sat108 = getelementptr [1 x i64], ptr %sargs108, i64 0, i64 0
  store i64 0, ptr %sat108
  %sic108 = getelementptr i64, ptr %area, i64 44
  %sf108 = call ptr @df.selector(ptr %sic108, i64 %v94, i64 1052728634250411761, ptr @df.seln.6, i64 9)
  %v108 = call i64 %sf108(i64 %v94, ptr %sargs108, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v108, ptr %gcs1
  %v109 = load i8, ptr %ctx, align 8
  %c19 = zext i8 %v109 to i64
  %v110 = icmp ne i64 %c19, 0
  br i1 %v110, label %b66, label %b65
b63:
  br label %b62
b64:
  %gcvolta64 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta64, ptr %ctxtopo, align 8
  ret void
b65:
  %v111 = call i1 @df.unbox_bool(i64 %v108)
  %v112 = load i8, ptr %ctx, align 8
  %c20 = zext i8 %v112 to i64
  %v113 = icmp ne i64 %c20, 0
  br i1 %v113, label %b68, label %b67
b66:
  %gcvolta66 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta66, ptr %ctxtopo, align 8
  ret void
b67:
  br i1 %v111, label %b140, label %b141
b140:
  store i64 0, ptr %gcs1
  %ta234 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb234 = load ptr, ptr %ta234, align 8
  %tfa234 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf234 = load ptr, ptr %tfa234, align 8
  %tnx234 = getelementptr i8, ptr %tb234, i64 24
  %tz234 = icmp ugt ptr %tnx234, %tf234
  %tnp234 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn234 = load i64, ptr %tnp234, align 8
  %tk234 = icmp ule i64 %tn234, 1029
  %tq234 = or i1 %tz234, %tk234
  %tx234 = call i1 @llvm.expect.i1(i1 %tq234, i1 false)
  br i1 %tx234, label %ao234.lento, label %ao234.reg
ao234.reg:
  %trp234 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr234 = load ptr, ptr %trp234, align 8
  %trb234 = getelementptr inbounds i8, ptr %tr234, i64 1029
  %trv234 = load i8, ptr %trb234, align 1
  %trz234 = icmp eq i8 %trv234, 0
  %try234 = call i1 @llvm.expect.i1(i1 %trz234, i1 false)
  br i1 %try234, label %ao234.lento, label %ao234.rapido
ao234.rapido:
  store ptr %tnx234, ptr %ta234, align 8
  %tpp234 = getelementptr inbounds i8, ptr %tb234, i64 16
  store i64 0, ptr %tpp234, align 8
  store i64 4419521347585, ptr %tb234, align 8
  %thb234 = ptrtoint ptr %tb234 to i64
  %th234 = add i64 %thb234, 2
  br label %ao234.fim
ao234.lento:
  %tl234 = call i64 @dartforge_object_new_t(i64 1029, i64 0, ptr @df.mt.getter_return_provenance$2edart.Owner)
  br label %ao234.fim
ao234.fim:
  %v234 = phi i64 [ %th234, %ao234.rapido ], [ %tl234, %ao234.lento ]
  br label %b142
b141:
  store i64 0, ptr %gcs1
  %ta236 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb236 = load ptr, ptr %ta236, align 8
  %tfa236 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf236 = load ptr, ptr %tfa236, align 8
  %tnx236 = getelementptr i8, ptr %tb236, i64 24
  %tz236 = icmp ugt ptr %tnx236, %tf236
  %tnp236 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn236 = load i64, ptr %tnp236, align 8
  %tk236 = icmp ule i64 %tn236, 1028
  %tq236 = or i1 %tz236, %tk236
  %tx236 = call i1 @llvm.expect.i1(i1 %tq236, i1 false)
  br i1 %tx236, label %ao236.lento, label %ao236.reg
ao236.reg:
  %trp236 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr236 = load ptr, ptr %trp236, align 8
  %trb236 = getelementptr inbounds i8, ptr %tr236, i64 1028
  %trv236 = load i8, ptr %trb236, align 1
  %trz236 = icmp eq i8 %trv236, 0
  %try236 = call i1 @llvm.expect.i1(i1 %trz236, i1 false)
  br i1 %try236, label %ao236.lento, label %ao236.rapido
ao236.rapido:
  store ptr %tnx236, ptr %ta236, align 8
  %tpp236 = getelementptr inbounds i8, ptr %tb236, i64 16
  store i64 0, ptr %tpp236, align 8
  store i64 4415226380289, ptr %tb236, align 8
  %thb236 = ptrtoint ptr %tb236 to i64
  %th236 = add i64 %thb236, 2
  br label %ao236.fim
ao236.lento:
  %tl236 = call i64 @dartforge_object_new_t(i64 1028, i64 0, ptr @df.mt.getter_return_provenance$2edart.OtherOwner)
  br label %ao236.fim
ao236.fim:
  %v236 = phi i64 [ %th236, %ao236.rapido ], [ %tl236, %ao236.lento ]
  br label %b142
b142:
  %v238 = phi i64 [ %v234, %ao234.fim ], [ %v236, %ao236.fim ]
  store i64 %v238, ptr %gcs1
  %sat131 = getelementptr [1 x i64], ptr %sargs131, i64 0, i64 0
  store i64 0, ptr %sat131
  %sic131 = getelementptr i64, ptr %area, i64 46
  ; df.selector-exact 1028,1029
  %sf131 = call ptr @df.selector(ptr %sic131, i64 %v238, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v131 = call i64 %sf131(i64 %v238, ptr %sargs131, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v131, ptr %gcs1
  %v132 = load i8, ptr %ctx, align 8
  %c21 = zext i8 %v132 to i64
  %v133 = icmp ne i64 %c21, 0
  br i1 %v133, label %b79, label %b78
b68:
  %gcvolta68 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta68, ptr %ctxtopo, align 8
  ret void
b78:
  %v134 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.8e13c211ab38ef9000028cbc061a83c5", i64 2) to i64
  store i64 %v134, ptr %gcs2
  %sa135_0 = getelementptr [2 x i64], ptr %sargs135, i64 0, i64 0
  store i64 %v134, ptr %sa135_0
  %sat135 = getelementptr [2 x i64], ptr %sargs135, i64 0, i64 1
  store i64 0, ptr %sat135
  %sic135 = getelementptr i64, ptr %area, i64 48
  ; df.selector-exact 1,1025,1026
  %sf135 = call ptr @df.selector(ptr %sic135, i64 %v131, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v135 = call i64 %sf135(i64 %v131, ptr %sargs135, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v135, ptr %gcs1
  %v136 = load i8, ptr %ctx, align 8
  %c22 = zext i8 %v136 to i64
  %v137 = icmp ne i64 %c22, 0
  br i1 %v137, label %b81, label %b80
b79:
  %gcvolta79 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta79, ptr %ctxtopo, align 8
  ret void
b80:
  store i64 0, ptr %gcs2
  call void @df.dart$3acore..print(i64 %v135)
  %v139 = load i8, ptr %ctx, align 8
  %c23 = zext i8 %v139 to i64
  %v140 = icmp ne i64 %c23, 0
  br i1 %v140, label %b83, label %b82
b81:
  %gcvolta81 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta81, ptr %ctxtopo, align 8
  ret void
b82:
  br label %b54
b83:
  %gcvolta83 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta83, ptr %ctxtopo, align 8
  ret void
b86:
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v146)
  %v150 = load i8, ptr %ctx, align 8
  %c24 = zext i8 %v150 to i64
  %v151 = icmp ne i64 %c24, 0
  br i1 %v151, label %b89, label %b88
b87:
  %gcvolta87 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta87, ptr %ctxtopo, align 8
  ret void
b88:
  store i64 0, ptr %gcs0
  %ta152 = getelementptr inbounds i8, ptr %ctx, i64 80
  %tb152 = load ptr, ptr %ta152, align 8
  %tfa152 = getelementptr inbounds i8, ptr %ctx, i64 88
  %tf152 = load ptr, ptr %tfa152, align 8
  %tnx152 = getelementptr i8, ptr %tb152, i64 24
  %tz152 = icmp ugt ptr %tnx152, %tf152
  %tnp152 = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn152 = load i64, ptr %tnp152, align 8
  %tk152 = icmp ule i64 %tn152, 1027
  %tq152 = or i1 %tz152, %tk152
  %tx152 = call i1 @llvm.expect.i1(i1 %tq152, i1 false)
  br i1 %tx152, label %ao152.lento, label %ao152.reg
ao152.reg:
  %trp152 = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr152 = load ptr, ptr %trp152, align 8
  %trb152 = getelementptr inbounds i8, ptr %tr152, i64 1027
  %trv152 = load i8, ptr %trb152, align 1
  %trz152 = icmp eq i8 %trv152, 0
  %try152 = call i1 @llvm.expect.i1(i1 %trz152, i1 false)
  br i1 %try152, label %ao152.lento, label %ao152.rapido
ao152.rapido:
  store ptr %tnx152, ptr %ta152, align 8
  %tpp152 = getelementptr inbounds i8, ptr %tb152, i64 16
  store i64 0, ptr %tpp152, align 8
  store i64 4410931412993, ptr %tb152, align 8
  %thb152 = ptrtoint ptr %tb152 to i64
  %th152 = add i64 %thb152, 2
  br label %ao152.fim
ao152.lento:
  %tl152 = call i64 @dartforge_object_new_t(i64 1027, i64 0, ptr @df.mt.getter_return_provenance$2edart.MissingGetter)
  br label %ao152.fim
ao152.fim:
  %v152 = phi i64 [ %th152, %ao152.rapido ], [ %tl152, %ao152.lento ]
  store i64 %v152, ptr %gcs0
  %v156 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.525e451e398f19bdbeb6571006e20b55", i64 2) to i64
  store i64 %v156, ptr %gcs1
  %v157 = call i64 @df.getter_return_provenance$2edart..convertUnknown(i64 %v152, i64 %v156)
  store i64 %v157, ptr %gcs0
  %v158 = load i8, ptr %ctx, align 8
  %c25 = zext i8 %v158 to i64
  %v159 = icmp ne i64 %c25, 0
  br i1 %v159, label %b93, label %b92
b89:
  %gcvolta89 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta89, ptr %ctxtopo, align 8
  ret void
b92:
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v157)
  %v161 = load i8, ptr %ctx, align 8
  %c26 = zext i8 %v161 to i64
  %v162 = icmp ne i64 %c26, 0
  br i1 %v162, label %b95, label %b94
b93:
  %gcvolta93 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta93, ptr %ctxtopo, align 8
  ret void
b94:
  %v163 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.ab78fcd38f54917ebc89c8edec3ad2f4", i64 2) to i64
  store i64 %v163, ptr %gcs0
  %v164 = call i64 @df.getter_return_provenance$2edart..getterCalls()
  %v165 = load i8, ptr %ctx, align 8
  %c27 = zext i8 %v165 to i64
  %v166 = icmp ne i64 %c27, 0
  br i1 %v166, label %b97, label %b96
b95:
  %gcvolta95 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta95, ptr %ctxtopo, align 8
  ret void
b96:
  %v167 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.edaf1bd3d1c2ffcc44df55829c002f26", i64 2) to i64
  store i64 %v167, ptr %gcs1
  %jk168_0 = getelementptr [6 x i64], ptr %jbuf168, i64 0, i64 0
  store i64 0, ptr %jk168_0
  %jp168_0 = getelementptr [6 x i64], ptr %jbuf168, i64 0, i64 1
  store i64 %v163, ptr %jp168_0
  %jk168_1 = getelementptr [6 x i64], ptr %jbuf168, i64 0, i64 2
  store i64 1, ptr %jk168_1
  %jp168_1 = getelementptr [6 x i64], ptr %jbuf168, i64 0, i64 3
  store i64 %v164, ptr %jp168_1
  %jk168_2 = getelementptr [6 x i64], ptr %jbuf168, i64 0, i64 4
  store i64 0, ptr %jk168_2
  %jp168_2 = getelementptr [6 x i64], ptr %jbuf168, i64 0, i64 5
  store i64 %v167, ptr %jp168_2
  %v168 = call i64 @dartforge_string_join_typed(ptr %jbuf168, i64 3)
  store i64 %v168, ptr %gcs0
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v168)
  %v170 = load i8, ptr %ctx, align 8
  %c28 = zext i8 %v170 to i64
  %v171 = icmp ne i64 %c28, 0
  br i1 %v171, label %b99, label %b98
b97:
  %gcvolta97 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta97, ptr %ctxtopo, align 8
  ret void
b98:
  store i64 0, ptr %gcs0
  %v172 = call i64 @dfc.78b0743a69873ea4.get()
  store i64 %v172, ptr %gcs0
  %v173 = load i8, ptr %ctx, align 8
  %c29 = zext i8 %v173 to i64
  %v174 = icmp ne i64 %c29, 0
  br i1 %v174, label %b101, label %b100
b99:
  %gcvolta99 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta99, ptr %ctxtopo, align 8
  ret void
b100:
  %v177 = call i64 @dfc.e9a84756bbdc9480.get()
  store i64 %v177, ptr %gcs1
  %v178 = load i8, ptr %ctx, align 8
  %c30 = zext i8 %v178 to i64
  %v179 = icmp ne i64 %c30, 0
  br i1 %v179, label %b103, label %b102
b101:
  %gcvolta101 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta101, ptr %ctxtopo, align 8
  ret void
b102:
  %v182 = call i64 @dfc.3fcc3763a4fbed30.get()
  store i64 %v182, ptr %gcs2
  %v183 = load i8, ptr %ctx, align 8
  %c31 = zext i8 %v183 to i64
  %v184 = icmp ne i64 %c31, 0
  br i1 %v184, label %b105, label %b104
b103:
  %gcvolta103 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta103, ptr %ctxtopo, align 8
  ret void
b104:
  %sat188 = getelementptr [1 x i64], ptr %sargs188, i64 0, i64 0
  store i64 0, ptr %sat188
  %sic188 = getelementptr i64, ptr %area, i64 50
  %sf188 = call ptr @df.selector(ptr %sic188, i64 %v172, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v188 = call i64 %sf188(i64 %v172, ptr %sargs188, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v188, ptr %gcs0
  %v189 = load i8, ptr %ctx, align 8
  %c32 = zext i8 %v189 to i64
  %v190 = icmp ne i64 %c32, 0
  br i1 %v190, label %b107, label %b106
b105:
  %gcvolta105 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta105, ptr %ctxtopo, align 8
  ret void
b106:
  %v191 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.4e9d410e0b1f5162b8eee78ad4660bd5", i64 2) to i64
  store i64 %v191, ptr %gcs3
  %sa192_0 = getelementptr [2 x i64], ptr %sargs192, i64 0, i64 0
  store i64 %v191, ptr %sa192_0
  %sat192 = getelementptr [2 x i64], ptr %sargs192, i64 0, i64 1
  store i64 0, ptr %sat192
  %sic192 = getelementptr i64, ptr %area, i64 52
  %sf192 = call ptr @df.selector(ptr %sic192, i64 %v188, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v192 = call i64 %sf192(i64 %v188, ptr %sargs192, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v192, ptr %gcs0
  %v193 = load i8, ptr %ctx, align 8
  %c33 = zext i8 %v193 to i64
  %v194 = icmp ne i64 %c33, 0
  br i1 %v194, label %b109, label %b108
b107:
  %gcvolta107 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta107, ptr %ctxtopo, align 8
  ret void
b108:
  store i64 0, ptr %gcs3
  call void @df.dart$3acore..print(i64 %v192)
  %v196 = load i8, ptr %ctx, align 8
  %c34 = zext i8 %v196 to i64
  %v197 = icmp ne i64 %c34, 0
  br i1 %v197, label %b111, label %b110
b109:
  %gcvolta109 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta109, ptr %ctxtopo, align 8
  ret void
b110:
  store i64 0, ptr %gcs0
  %sat199 = getelementptr [1 x i64], ptr %sargs199, i64 0, i64 0
  store i64 0, ptr %sat199
  %sic199 = getelementptr i64, ptr %area, i64 54
  %sf199 = call ptr @df.selector(ptr %sic199, i64 %v177, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v199 = call i64 %sf199(i64 %v177, ptr %sargs199, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v199, ptr %gcs0
  %v200 = load i8, ptr %ctx, align 8
  %c35 = zext i8 %v200 to i64
  %v201 = icmp ne i64 %c35, 0
  br i1 %v201, label %b113, label %b112
b111:
  %gcvolta111 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta111, ptr %ctxtopo, align 8
  ret void
b112:
  %v202 = ptrtoint ptr getelementptr inbounds (i8, ptr @"df.s.8ad4deab2bab907c7a4d333afde6483c", i64 2) to i64
  store i64 %v202, ptr %gcs1
  %sa203_0 = getelementptr [2 x i64], ptr %sargs203, i64 0, i64 0
  store i64 %v202, ptr %sa203_0
  %sat203 = getelementptr [2 x i64], ptr %sargs203, i64 0, i64 1
  store i64 0, ptr %sat203
  %sic203 = getelementptr i64, ptr %area, i64 56
  %sf203 = call ptr @df.selector(ptr %sic203, i64 %v199, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v203 = call i64 %sf203(i64 %v199, ptr %sargs203, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v203, ptr %gcs0
  %v204 = load i8, ptr %ctx, align 8
  %c36 = zext i8 %v204 to i64
  %v205 = icmp ne i64 %c36, 0
  br i1 %v205, label %b115, label %b114
b113:
  %gcvolta113 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta113, ptr %ctxtopo, align 8
  ret void
b114:
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v203)
  %v207 = load i8, ptr %ctx, align 8
  %c37 = zext i8 %v207 to i64
  %v208 = icmp ne i64 %c37, 0
  br i1 %v208, label %b117, label %b116
b115:
  %gcvolta115 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta115, ptr %ctxtopo, align 8
  ret void
b116:
  store i64 0, ptr %gcs0
  %sat210 = getelementptr [1 x i64], ptr %sargs210, i64 0, i64 0
  store i64 0, ptr %sat210
  %sic210 = getelementptr i64, ptr %area, i64 58
  %sf210 = call ptr @df.selector(ptr %sic210, i64 %v182, i64 -7321284671083058388, ptr @df.seln.1, i64 9)
  %v210 = call i64 %sf210(i64 %v182, ptr %sargs210, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v210, ptr %gcs0
  %v211 = load i8, ptr %ctx, align 8
  %c38 = zext i8 %v211 to i64
  %v212 = icmp ne i64 %c38, 0
  br i1 %v212, label %b119, label %b118
b117:
  %gcvolta117 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta117, ptr %ctxtopo, align 8
  ret void
b118:
  %v213 = add i64 0, 65
  %v214 = add i64 0, 66
  store i64 0, ptr %gcs2
  %lp215_0 = getelementptr [2 x i64], ptr %list_buf_215, i64 0, i64 0
  store i64 %v213, ptr %lp215_0, align 8
  %lp215_1 = getelementptr [2 x i64], ptr %list_buf_215, i64 0, i64 1
  store i64 %v214, ptr %lp215_1, align 8
  %v215 = call i64 @dartforge_list_new(ptr %list_buf_215, i64 2, i64 10, i64 1)
  store i64 %v215, ptr %gcs1
  %ga216 = getelementptr i64, ptr %area, i64 11
  %v216 = load i8, ptr %ga216
  %c39 = zext i8 %v216 to i64
  %v217 = icmp ne i64 %c39, 0
  br i1 %v217, label %b120, label %b121
b119:
  %gcvolta119 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta119, ptr %ctxtopo, align 8
  ret void
b120:
  %ga218 = getelementptr i64, ptr %area, i64 10
  %v218 = load i64, ptr %ga218
  br label %b122
b121:
  %v219 = call i64 @df.rti.3826facde6a2fcb1.getter_return_provenance$2edart()
  br label %b122
b122:
  %v220 = phi i64 [ %v218, %b120 ], [ %v219, %b121 ]
  call void @dartforge_rti_set(i64 %v215, i64 %v220)
  %sa222_0 = getelementptr [2 x i64], ptr %sargs222, i64 0, i64 0
  store i64 %v215, ptr %sa222_0
  %sat222 = getelementptr [2 x i64], ptr %sargs222, i64 0, i64 1
  store i64 0, ptr %sat222
  %sic222 = getelementptr i64, ptr %area, i64 60
  %sf222 = call ptr @df.selector(ptr %sic222, i64 %v210, i64 -1047269154501687337, ptr @df.seln.2, i64 9)
  %v222 = call i64 %sf222(i64 %v210, ptr %sargs222, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  store i64 %v222, ptr %gcs0
  %v223 = load i8, ptr %ctx, align 8
  %c40 = zext i8 %v223 to i64
  %v224 = icmp ne i64 %c40, 0
  br i1 %v224, label %b124, label %b123
b123:
  store i64 0, ptr %gcs1
  call void @df.dart$3acore..print(i64 %v222)
  %v226 = load i8, ptr %ctx, align 8
  %c41 = zext i8 %v226 to i64
  %v227 = icmp ne i64 %c41, 0
  br i1 %v227, label %b126, label %b125
b124:
  %gcvolta124 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta124, ptr %ctxtopo, align 8
  ret void
b125:
  %gcvolta125 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta125, ptr %ctxtopo, align 8
  ret void
b126:
  %gcvolta126 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta126, ptr %ctxtopo, align 8
  ret void
}
