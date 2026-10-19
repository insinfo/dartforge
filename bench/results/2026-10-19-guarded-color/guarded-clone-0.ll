define i64 @df.specialized.8a055f5f6e8822e6b7c560145408f601667f6da89c115e59b8539ddd4d29d68b.e00cd59a2f96645b24ed203264c390f15b73f8c88b56ad5bdd128f3df08e6c63(i64 %v0, i64 %v3, i64 %v6) "disable-tail-calls"="true" nounwind {
b0:
  %sargs2002 = alloca [3 x i64]
  %sargs2013 = alloca [2 x i64]
  %sargs2029 = alloca [1 x i64]
  %gcq = alloca { ptr, i64, [3 x i64] }, align 8
  %area = call ptr @df.get_area()
  %ctx = call ptr @df.context()
  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8
  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 376
  %pilhal = load i64, ptr %pilhalp, align 8
  %pilhasp = call i64 @llvm.read_register.i64(metadata !{!"sp"})
  %pilhab = icmp ult i64 %pilhasp, %pilhal
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
  %fxk15 = and i64 %v3, -9223372036854775801
  %fxo15 = icmp eq i64 %fxk15, 2
  %fxa15 = add i64 %v3, -2
  %fxq15 = inttoptr i64 %fxa15 to ptr
  %fxvp15 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv15 = load ptr, ptr %fxvp15, align 8
  %fcb15 = select i1 %fxo15, ptr %fxq15, ptr %fxv15
  %fp15 = getelementptr inbounds i8, ptr %fcb15, i64 16
  %fg15 = getelementptr inbounds i64, ptr %fp15, i64 0
  %v15 = load i64, ptr %fg15, align 8
  %v2468 = icmp eq i64 %v15, 10
  br i1 %v2468, label %b17, label %b1431
b17:
  %fxk1841 = and i64 %v6, -9223372036854775801
  %fxo1841 = icmp eq i64 %fxk1841, 2
  %fxa1841 = add i64 %v6, -2
  %fxq1841 = inttoptr i64 %fxa1841 to ptr
  %fxvp1841 = getelementptr inbounds i8, ptr %ctx, i64 40
  %fxv1841 = load ptr, ptr %fxvp1841, align 8
  %fcb1841 = select i1 %fxo1841, ptr %fxq1841, ptr %fxv1841
  %fp1841 = getelementptr inbounds i8, ptr %fcb1841, i64 16
  %fg1841 = getelementptr inbounds i64, ptr %fp1841, i64 0
  %v1841 = load i64, ptr %fg1841, align 8
  %v2477 = icmp eq i64 %v1841, 3
  br i1 %v2477, label %b1086, label %b1431
b1086:
  %v1987 = add i64 0, 0
  %v1988 = add i64 0, 1
  %v1989 = call i64 @df.class(i64 %v0)
  switch i64 %v1989, label %b1179 [ i64 4, label %b1177 i64 2, label %b1178 i64 3, label %b1178 ]
b1177:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1990 = call i64 @df.box_int(i64 %v1987)
  store i64 %v1990, ptr %gcs1
  %v1991 = call i64 @df.box_int(i64 %v1988)
  store i64 %v1991, ptr %gcs2
  %v1992 = call i64 @df.dart$3acore._Double.clamp(i64 %v0, i64 %v1990, i64 %v1991)
  %v1993 = load i8, ptr %ctx, align 8
  %c0 = zext i8 %v1993 to i64
  %v1994 = icmp ne i64 %c0, 0
  br i1 %v1994, label %b1182, label %b1181
b1178:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v1995 = call i64 @df.box_int(i64 %v1987)
  store i64 %v1995, ptr %gcs1
  %v1996 = call i64 @df.box_int(i64 %v1988)
  store i64 %v1996, ptr %gcs2
  %v1997 = call i64 @df.dart$3acore._IntegerImplementation.clamp(i64 %v0, i64 %v1995, i64 %v1996)
  %v1998 = load i8, ptr %ctx, align 8
  %c1 = zext i8 %v1998 to i64
  %v1999 = icmp ne i64 %c1, 0
  br i1 %v1999, label %b1184, label %b1183
b1179:
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2000 = call i64 @df.box_int(i64 %v1987)
  store i64 %v2000, ptr %gcs1
  %v2001 = call i64 @df.box_int(i64 %v1988)
  store i64 %v2001, ptr %gcs2
  %sa2002_0 = getelementptr [3 x i64], ptr %sargs2002, i64 0, i64 0
  store i64 %v2000, ptr %sa2002_0
  %sa2002_1 = getelementptr [3 x i64], ptr %sargs2002, i64 0, i64 1
  store i64 %v2001, ptr %sa2002_1
  %sat2002 = getelementptr [3 x i64], ptr %sargs2002, i64 0, i64 2
  store i64 0, ptr %sat2002
  %sic2002 = getelementptr i64, ptr %area, i64 350
  %sf2002 = call ptr @df.selector(ptr %sic2002, i64 %v0, i64 -1664458868869587675, ptr @df.seln.13, i64 8)
  %v2002 = call i64 %sf2002(i64 %v0, ptr %sargs2002, ptr @df.const.i64.2.d698ddd8a0bfb0ecef0225536b2b51b5665422a52c6f5e380efc218a61082cec)
  %v2003 = load i8, ptr %ctx, align 8
  %c2 = zext i8 %v2003 to i64
  %v2004 = icmp ne i64 %c2, 0
  br i1 %v2004, label %b1186, label %b1185
b1180:
  %v2005 = phi i64 [ %v1992, %b1181 ], [ %v1997, %b1183 ], [ %v2002, %b1185 ]
  store i64 %v2005, ptr %gcs0
  %v2006 = add i64 0, 255
  store i64 0, ptr %gcs1
  store i64 0, ptr %gcs2
  %v2007 = and i64 %v2005, 1
  %v2008 = icmp ne i64 %v2007, 0
  br i1 %v2008, label %b1187, label %b1188
b1181:
  br label %b1180
b1182:
  %gcvolta1182 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1182, ptr %ctxtopo, align 8
  ret i64 0
b1183:
  br label %b1180
b1184:
  %gcvolta1184 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1184, ptr %ctxtopo, align 8
  ret i64 0
b1185:
  br label %b1180
b1186:
  %gcvolta1186 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1186, ptr %ctxtopo, align 8
  ret i64 0
b1187:
  %v2009 = ashr i64 %v2005, 1
  %v2010 = mul i64 %v2009, %v2006
  store i64 0, ptr %gcs0
  %v2011 = call i64 @df.box_int(i64 %v2010)
  br label %b1189
b1188:
  %v2012 = call i64 @df.box_int(i64 %v2006)
  store i64 %v2012, ptr %gcs1
  %sa2013_0 = getelementptr [2 x i64], ptr %sargs2013, i64 0, i64 0
  store i64 %v2012, ptr %sa2013_0
  %sat2013 = getelementptr [2 x i64], ptr %sargs2013, i64 0, i64 1
  store i64 0, ptr %sat2013
  %sic2013 = getelementptr i64, ptr %area, i64 352
  %sf2013 = call ptr @df.selector(ptr %sic2013, i64 %v2005, i64 -4020825035383377172, ptr @df.seln.11, i64 4)
  %v2013 = call i64 %sf2013(i64 %v2005, ptr %sargs2013, ptr @df.const.i64.2.5add879d03074d0745fe088492026a7fe5e24eae5505fb5feeaa2dea820028da)
  %v2014 = load i8, ptr %ctx, align 8
  %c3 = zext i8 %v2014 to i64
  %v2015 = icmp ne i64 %c3, 0
  br i1 %v2015, label %b1191, label %b1190
b1189:
  %v2016 = phi i64 [ %v2011, %b1187 ], [ %v2013, %b1190 ]
  store i64 %v2016, ptr %gcs0
  %v2017 = call i64 @df.class(i64 %v2016)
  %v2018 = icmp eq i64 %v2017, 4
  br i1 %v2018, label %b1192, label %b1193
b1190:
  br label %b1189
b1191:
  %gcvolta1191 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1191, ptr %ctxtopo, align 8
  ret i64 0
b1192:
  store i64 0, ptr %gcs1
  %v2019 = call double @df.unbox_double(i64 %v2016)
  %v2020 = load i8, ptr %ctx, align 8
  %c4 = zext i8 %v2020 to i64
  %v2021 = icmp ne i64 %c4, 0
  br i1 %v2021, label %b1196, label %b1195
b1193:
  store i64 0, ptr %gcs1
  %v2036 = call i64 @df.unbox_int(i64 %v2016)
  %v2037 = load i8, ptr %ctx, align 8
  %c5 = zext i8 %v2037 to i64
  %v2038 = icmp ne i64 %c5, 0
  br i1 %v2038, label %b1205, label %b1204
b1194:
  %v2039 = phi i64 [ %v2035, %b1199 ], [ %v2036, %b1204 ]
  store i64 0, ptr %gcs0
  %v2040 = call i64 @df.box_int(i64 %v2039)
  %xcp2041 = load i8, ptr %ctx, align 8
  %xcn2041 = icmp ne i8 %xcp2041, 0
  %xce2041 = call i1 @llvm.expect.i1(i1 %xcn2041, i1 false)
  br i1 %xce2041, label %xc2041.limpar, label %xc2041.fim
xc2041.limpar:
  call void @dartforge_exception_clear()
  br label %xc2041.fim
xc2041.fim:
  %gcvolta1194 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1194, ptr %ctxtopo, align 8
  ret i64 %v2040
b1195:
  %v2022 = fcmp oge double %v2019, 0xC3E0000000000000
  %v2023 = fcmp olt double %v2019, 0x43E0000000000000
  %v2024 = zext i1 %v2022 to i64
  %v2025 = zext i1 %v2023 to i64
  %v2026 = and i64 %v2024, %v2025
  %v2027 = icmp ne i64 %v2026, 0
  br i1 %v2027, label %b1197, label %b1198
b1196:
  %gcvolta1196 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1196, ptr %ctxtopo, align 8
  ret i64 0
b1197:
  %v2028 = fptosi double %v2019 to i64
  br label %b1199
b1198:
  %sat2029 = getelementptr [1 x i64], ptr %sargs2029, i64 0, i64 0
  store i64 0, ptr %sat2029
  %sic2029 = getelementptr i64, ptr %area, i64 354
  %sf2029 = call ptr @df.selector(ptr %sic2029, i64 %v2016, i64 5852185211741937682, ptr @df.seln.14, i64 7)
  %v2029 = call i64 %sf2029(i64 %v2016, ptr %sargs2029, ptr @df.const.i64.2.c314eb35009b14025e5ef5c352349ffcb0ff36f19602db3b02250d0153c9f892)
  store i64 %v2029, ptr %gcs0
  %v2030 = load i8, ptr %ctx, align 8
  %c6 = zext i8 %v2030 to i64
  %v2031 = icmp ne i64 %c6, 0
  br i1 %v2031, label %b1201, label %b1200
b1199:
  %v2035 = phi i64 [ %v2028, %b1197 ], [ %v2032, %b1202 ]
  br label %b1194
b1200:
  %v2032 = call i64 @df.unbox_int(i64 %v2029)
  %v2033 = load i8, ptr %ctx, align 8
  %c7 = zext i8 %v2033 to i64
  %v2034 = icmp ne i64 %c7, 0
  br i1 %v2034, label %b1203, label %b1202
b1201:
  %gcvolta1201 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1201, ptr %ctxtopo, align 8
  ret i64 0
b1202:
  br label %b1199
b1203:
  %gcvolta1203 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1203, ptr %ctxtopo, align 8
  ret i64 0
b1204:
  br label %b1194
b1205:
  %gcvolta1205 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1205, ptr %ctxtopo, align 8
  ret i64 0
b1431:
  %v2478 = call i64 @df.package$3adart_ui$2fsrc$2fgraphics$2fimage$2fcodecs$2fcolor$2fformat$2edart..convertFormatValue(i64 %v0, i64 %v3, i64 %v6)
  %gcvolta1431 = load ptr, ptr %gcq, align 8
  store ptr %gcvolta1431, ptr %ctxtopo, align 8
  ret i64 %v2478
}
