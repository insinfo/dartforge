
long FUN_1000f4e64(void)

{
  byte bVar1;
  undefined8 *puVar2;
  bool bVar3;
  bool bVar4;
  char cVar5;
  uint uVar6;
  char *pcVar7;
  undefined8 uVar8;
  undefined8 uVar9;
  undefined8 uVar10;
  char *pcVar11;
  long lVar12;
  ulong uVar13;
  long lVar14;
  ulong uVar15;
  ulong uVar16;
  long lVar17;
  long lVar18;
  long lVar19;
  char *pcVar20;
  ulong uVar21;
  ulong uVar22;
  undefined8 uVar23;
  code *pcVar24;
  ulong uVar25;
  long lVar26;
  long lVar27;
  undefined1 uStack_2a9;
  undefined8 local_2a8;
  undefined8 local_2a0;
  ulong local_298;
  ulong uStack_290;
  undefined *local_288;
  undefined *puStack_280;
  undefined *local_278;
  undefined *puStack_270;
  undefined *local_268;
  undefined *puStack_260;
  undefined *local_258;
  undefined *local_250;
  undefined8 uStack_248;
  undefined *local_240;
  undefined8 uStack_238;
  undefined8 local_230;
  undefined *local_228;
  undefined8 uStack_220;
  undefined *local_218;
  undefined8 uStack_210;
  undefined *local_208;
  undefined8 uStack_200;
  undefined8 local_1f8;
  undefined8 local_1f0;
  undefined *local_1e8;
  undefined8 uStack_1e0;
  undefined *local_1d8;
  undefined8 uStack_1d0;
  undefined8 local_1c8;
  undefined *local_1c0;
  undefined8 uStack_1b8;
  undefined *local_1b0;
  undefined8 uStack_1a8;
  undefined *local_1a0;
  undefined8 uStack_198;
  ulong local_190;
  undefined8 uStack_188;
  undefined8 local_180;
  undefined8 local_172;
  undefined2 local_16a;
  undefined *local_168;
  ulong uStack_160;
  undefined *local_158;
  undefined8 uStack_150;
  undefined *local_148;
  undefined8 uStack_140;
  undefined *local_138;
  ulong uStack_130;
  undefined *local_128;
  ulong uStack_120;
  undefined8 local_118;
  undefined8 uStack_110;
  undefined8 local_108;
  undefined *puStack_100;
  undefined8 local_f8;
  ulong uStack_f0;
  undefined8 local_e8;
  undefined *puStack_e0;
  undefined8 local_d8;
  undefined *puStack_d0;
  undefined8 local_c8;
  ulong uStack_c0;
  undefined8 local_b8;
  undefined *puStack_b0;
  undefined8 local_a8;
  undefined *puStack_a0;
  undefined8 local_98;
  ulong uStack_90;
  undefined8 local_88;
  undefined *puStack_80;
  undefined1 auStack_78 [8];
  
  uVar25 = tpidrro_el0;
  pcVar7 = *(char **)((uVar25 & 0xfffffffffffffff8) + DAT_10032c1f8);
  bVar3 = DAT_10032c1f8 == 0 || pcVar7 == (char *)0x0;
  pcVar11 = pcVar7;
  if (bVar3) {
    pcVar11 = (char *)thunk_FUN_10009c0a0();
  }
  if ((*(ulong *)(pcVar11 + 0x18) <= DAT_10032c1b0) ||
     (lVar26 = *(long *)(*(long *)(pcVar11 + 0x10) + DAT_10032c1b0 * 8), lVar26 == 0)) {
    lVar26 = FUN_10009ec90(&DAT_1002bd340,&DAT_10032c1b0);
  }
  pcVar11 = pcVar7;
  if (bVar3) {
    pcVar11 = (char *)thunk_FUN_10009c0a0();
  }
  if (&uStack_2a9 < *(undefined1 **)(pcVar11 + 0x178)) {
    FUN_1000b7c08();
    return 0;
  }
  local_258 = (undefined *)0x0;
  local_2a0 = 9;
  uStack_290 = 0;
  local_298 = 0;
  puStack_280 = (undefined *)0x0;
  local_288 = (undefined *)0x0;
  puStack_270 = (undefined *)0x0;
  local_278 = (undefined *)0x0;
  puStack_260 = (undefined *)0x0;
  local_268 = (undefined *)0x0;
  local_2a8 = *(undefined8 *)(pcVar11 + 8);
  *(undefined8 **)(pcVar11 + 8) = &local_2a8;
  uVar8 = FUN_1000a099c(auStack_78,0,10,0);
  uVar25 = 0;
  local_298 = uVar8;
  do {
    if (pcVar11[0x20] != '\0') {
      uStack_290 = 0;
      FUN_1000b736c();
      if (*pcVar11 != '\0') goto LAB_1000f5428;
    }
    puVar2 = *(undefined8 **)(pcVar11 + 0x90);
    uStack_290 = 0;
    if (((*(undefined8 **)(pcVar11 + 0x98) < puVar2 + 7) || (*(ulong *)(pcVar11 + 0x38) < 0x98)) ||
       (*(char *)(*(long *)(pcVar11 + 0x30) + 0x97) == '\0')) {
      lVar12 = FUN_1000b0d1c(0x97,5,FUN_1000f9740);
    }
    else {
      *(undefined8 **)(pcVar11 + 0x90) = puVar2 + 7;
      lVar12 = (long)puVar2 + 2;
      puVar2[2] = 0;
      *puVar2 = 0x9700050001;
    }
    uStack_290 = lVar12;
    FUN_1001a95c0(lVar12);
    if (*pcVar11 != '\0') goto LAB_1000f5428;
    local_98 = 1;
    local_88 = 0;
    puStack_80 = &DAT_100317a72;
    local_288 = &DAT_10031791a;
    puStack_280 = &DAT_100317a0a;
    local_278 = &DAT_10031777a;
    puStack_270 = &DAT_100317a72;
    local_a8 = 0;
    puStack_a0 = &DAT_10031777a;
    uStack_90 = uVar25;
    uVar9 = FUN_1000b8c90(&local_a8,3);
    puStack_270 = &DAT_1003178b2;
    uStack_c0 = uVar25 % 7;
    local_268 = &DAT_1003177ba;
    puStack_260 = &DAT_100317a72;
    local_d8 = 0;
    puStack_d0 = &DAT_1003177ba;
    local_c8 = 1;
    local_b8 = 0;
    puStack_b0 = &DAT_100317a72;
    local_278 = (undefined *)uVar9;
    uVar10 = FUN_1000b8c90(&local_d8,3);
    puStack_260 = &DAT_10031779a;
    uStack_f0 = uVar25 % 0xd;
    local_258 = &DAT_100317a72;
    local_108 = 0;
    puStack_100 = &DAT_10031779a;
    local_f8 = 1;
    local_e8 = 0;
    puStack_e0 = &DAT_100317a72;
    local_268 = (undefined *)uVar10;
    puStack_260 = (undefined *)FUN_1000b8c90(&local_108,3);
    local_258 = (undefined *)0x0;
    local_118 = uVar10;
    uStack_110 = puStack_260;
    uVar10 = FUN_1000a099c(&local_118,2,10,0);
    puStack_260 = (undefined *)0x0;
    lVar27 = lVar26;
    local_268 = (undefined *)uVar10;
    if (*(char *)(lVar26 + 0x28) == '\0') {
      pcVar20 = pcVar7;
      if (bVar3) {
        pcVar20 = (char *)thunk_FUN_10009c0a0();
      }
      if ((DAT_10032c1b0 < *(ulong *)(pcVar20 + 0x18)) &&
         (lVar27 = *(long *)(*(long *)(pcVar20 + 0x10) + DAT_10032c1b0 * 8), lVar27 != 0)) {
        cVar5 = *(char *)(lVar27 + 0x28);
      }
      else {
        lVar27 = FUN_10009ec90(&DAT_1002bd340,&DAT_10032c1b0);
        cVar5 = *(char *)(lVar27 + 0x28);
      }
      if (cVar5 != '\0') goto LAB_1000f5184;
      uVar23 = FUN_1000b64c8(&DAT_100317a2a);
      *(undefined1 *)(lVar27 + 0x28) = 1;
      *(undefined8 *)(lVar27 + 0x20) = uVar23;
    }
    else {
LAB_1000f5184:
      uVar23 = *(undefined8 *)(lVar27 + 0x20);
    }
    FUN_1000b6b78(uVar10,uVar23);
    puStack_260 = &DAT_1003176aa;
    local_258 = &DAT_10031770a;
    local_172 = 0x203030303030103;
    local_168 = &DAT_10031791a;
    local_158 = &DAT_100317a0a;
    local_148 = &DAT_1003178b2;
    uStack_130 = (ulong)~(uint)uVar25 & 1;
    local_138 = &DAT_1003176aa;
    local_16a = 0x103;
    uStack_120 = uVar25 % 100;
    local_128 = &DAT_10031770a;
    uStack_160 = uVar25;
    uStack_150 = uVar9;
    uStack_140 = uVar10;
    local_288 = (undefined *)FUN_1000a099c(&local_168,10,10,&local_172);
    local_278 = (undefined *)0x0;
    puStack_280 = (undefined *)0x0;
    local_268 = (undefined *)0x0;
    puStack_270 = (undefined *)0x0;
    local_258 = (undefined *)0x0;
    puStack_260 = (undefined *)0x0;
    FUN_1001a70a4(lVar12,local_288);
    if (*pcVar11 != '\0') goto LAB_1000f5428;
    local_288 = (undefined *)0x0;
    if (*(char *)(lVar26 + 0x38) == '\0') {
      pcVar20 = pcVar7;
      if (bVar3) {
        pcVar20 = (char *)thunk_FUN_10009c0a0();
      }
      if ((DAT_10032c1b0 < *(ulong *)(pcVar20 + 0x18)) &&
         (lVar27 = *(long *)(*(long *)(pcVar20 + 0x10) + DAT_10032c1b0 * 8), lVar27 != 0)) {
        cVar5 = *(char *)(lVar27 + 0x38);
      }
      else {
        lVar27 = FUN_10009ec90(&DAT_1002bd340,&DAT_10032c1b0);
        cVar5 = *(char *)(lVar27 + 0x38);
      }
      if (cVar5 == '\0') {
        uVar9 = FUN_1000b64c8(&DAT_10031793a);
        *(undefined1 *)(lVar27 + 0x38) = 1;
        *(undefined8 *)(lVar27 + 0x30) = uVar9;
      }
      else {
        uVar9 = *(undefined8 *)(lVar27 + 0x30);
      }
    }
    else {
      uVar9 = *(undefined8 *)(lVar26 + 0x30);
    }
    FUN_1000b6b78(lVar12,uVar9);
    FUN_1000a08f0(uVar8,lVar12);
    uVar25 = uVar25 + 1;
  } while (uVar25 != 50000);
  uStack_290 = 0;
  if (*(char *)(lVar26 + 0x48) == '\0') {
    pcVar20 = pcVar7;
    if (bVar3) {
      pcVar20 = (char *)thunk_FUN_10009c0a0();
    }
    if ((*(ulong *)(pcVar20 + 0x18) <= DAT_10032c1b0) ||
       (lVar12 = *(long *)(*(long *)(pcVar20 + 0x10) + DAT_10032c1b0 * 8), lVar12 == 0)) {
      lVar12 = FUN_10009ec90(&DAT_1002bd340,&DAT_10032c1b0);
    }
    if (*(char *)(lVar12 + 0x48) == '\0') {
      uVar9 = FUN_1000b64c8(&DAT_100317842);
      *(undefined8 *)(lVar12 + 0x40) = uVar9;
      *(undefined1 *)(lVar12 + 0x48) = 1;
    }
    else {
      uVar9 = *(undefined8 *)(lVar12 + 0x40);
    }
  }
  else {
    uVar9 = *(undefined8 *)(lVar26 + 0x40);
  }
  FUN_1000b6b78(uVar8,uVar9);
  local_298 = FUN_1001ead34(uVar8,0);
  if ((*pcVar11 == '\0') && (uVar25 = FUN_1001eb690(local_298,0), *pcVar11 == '\0')) {
    local_298 = uVar25;
    if (uVar25 == 0) {
      uVar13 = 1;
    }
    else if ((uVar25 & 1) == 0) {
      if ((uVar25 & 0x8000000000000006) == 2) {
        uVar13 = (ulong)*(int *)(uVar25 + 2);
      }
      else {
        uVar13 = FUN_1000a0ad8(uVar25);
      }
    }
    else {
      uVar13 = 2;
    }
    pcVar20 = pcVar7;
    if (bVar3) {
      pcVar20 = (char *)thunk_FUN_10009c0a0();
    }
    if (((uVar13 < *(ulong *)(pcVar20 + 0x160)) && (0x255 < *(ulong *)(pcVar20 + 0x158))) &&
       (*(long *)(*(long *)(pcVar20 + 0x150) + 0x12a8) != 0)) {
      uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x12a8) + (uVar13 >> 6) * 8)
                    >> (uVar13 & 0x3f)) & 1;
    }
    else {
      uVar6 = FUN_10009f014(uVar13,0x255);
    }
    if ((uVar25 == 0) || ((uVar6 & 0xff) == 0)) {
      if (*(char *)(lVar26 + 0x58) == '\0') {
        uVar8 = FUN_1000f7008();
      }
      else {
        uVar8 = *(undefined8 *)(lVar26 + 0x50);
      }
      FUN_1000b38a4(uVar25,uVar8,0,0);
      if (*pcVar11 != '\0') goto LAB_1000f5428;
      if (uVar25 != 0) goto LAB_1000f54f4;
      local_180 = 0;
      lVar12 = 1;
    }
    else {
LAB_1000f54f4:
      if ((uVar25 & 1) == 0) {
        if ((uVar25 & 0x8000000000000006) == 2) {
          lVar12 = (long)*(int *)(uVar25 + 2);
          if (lVar12 - 8U < 3) {
LAB_1000f61c4:
            uVar15 = *(ulong *)(uVar25 + 0xe);
            uVar13 = 0;
            lVar12 = 0;
            while (uVar13 != (uVar15 & ((long)uVar15 >> 0x3f ^ 0xffffffffffffffffU))) {
              if (pcVar11[0x20] != '\0') {
                uStack_290 = 0;
                FUN_1000b736c();
                if (*pcVar11 != '\0') {
                  *(undefined8 *)(pcVar11 + 8) = local_2a8;
                  return 0;
                }
              }
              if (uVar25 == 0) {
                uVar16 = 0;
              }
              else {
                if ((uVar25 & 0x8000000000000006) == 2) {
                  lVar27 = (long)*(int *)(uVar25 + 2);
                }
                else {
                  lVar27 = FUN_1000a0ad8(uVar25);
                }
                uVar16 = uVar25;
                if (lVar27 == 10) {
                  uVar16 = *(ulong *)(uVar25 + 0x16);
                }
              }
              lVar27 = uVar16 + 0x16;
              bVar1 = *(byte *)(uVar16 - 1) & 0x66;
              if (bVar1 < 0x42) {
                if (bVar1 == 4) {
                  uVar16 = *(ulong *)(lVar27 + uVar13 * 8);
                }
                else {
                  if (bVar1 != 0x22) goto LAB_1000f62f4;
                  lVar27 = *(long *)(lVar27 + uVar13 * 8);
                  uStack_290 = 0;
                  if (lVar27 + 0x4000000000000000 < 0) {
                    pcVar20 = pcVar7;
                    if (bVar3) {
                      pcVar20 = (char *)thunk_FUN_10009c0a0();
                    }
                    puVar2 = *(undefined8 **)(pcVar20 + 0x50);
                    if (*(undefined8 **)(pcVar20 + 0x58) < puVar2 + 3) {
                      uVar16 = FUN_10009b470(3,1,0x102);
                      *(long *)(uVar16 + 0xe) = lVar27;
                    }
                    else {
                      *(undefined8 **)(pcVar20 + 0x50) = puVar2 + 3;
                      uVar16 = (long)puVar2 + 2;
                      *puVar2 = 0x300010201;
                      puVar2[2] = lVar27;
                    }
                  }
                  else {
                    uVar16 = lVar27 << 1 | 1;
                  }
                }
              }
              else if (bVar1 == 0x42) {
                uVar8 = *(undefined8 *)(lVar27 + uVar13 * 8);
                uStack_290 = 0;
                pcVar20 = pcVar7;
                if (bVar3) {
                  pcVar20 = (char *)thunk_FUN_10009c0a0();
                }
                puVar2 = *(undefined8 **)(pcVar20 + 0x50);
                if (*(undefined8 **)(pcVar20 + 0x58) < puVar2 + 3) {
                  uVar16 = FUN_10009b470(4,1,0x102);
                  *(undefined8 *)(uVar16 + 0xe) = uVar8;
                }
                else {
                  *(undefined8 **)(pcVar20 + 0x50) = puVar2 + 3;
                  uVar16 = (long)puVar2 + 2;
                  *puVar2 = 0x400010201;
                  puVar2[2] = uVar8;
                }
              }
              else if (bVar1 == 0x62) {
                lVar27 = *(long *)(lVar27 + uVar13 * 8);
                pcVar20 = pcVar7;
                if (bVar3) {
                  pcVar20 = (char *)thunk_FUN_10009c0a0();
                }
                lVar14 = 0x170;
                if (lVar27 != 0) {
                  lVar14 = 0x168;
                }
                uVar16 = *(ulong *)(pcVar20 + lVar14);
              }
              else {
LAB_1000f62f4:
                uStack_290 = 0;
                if ((long)(uVar13 + 0x4000000000000000) < 0) {
                  pcVar20 = pcVar7;
                  if (bVar3) {
                    pcVar20 = (char *)thunk_FUN_10009c0a0();
                  }
                  puVar2 = *(undefined8 **)(pcVar20 + 0x50);
                  if (*(undefined8 **)(pcVar20 + 0x58) < puVar2 + 3) {
                    uStack_290 = FUN_10009b470(3,1,0x102);
                  }
                  else {
                    *(undefined8 **)(pcVar20 + 0x50) = puVar2 + 3;
                    uStack_290 = (long)puVar2 + 2;
                    *puVar2 = 0x300010201;
                  }
                  *(ulong *)(uStack_290 + 0xe) = uVar13;
                }
                else {
                  uStack_290 = uVar13 << 1 | 1;
                }
                uStack_188 = 0;
                local_190 = uStack_290;
                if (uVar25 == 0) {
                  lVar27 = 1;
                }
                else if ((uVar25 & 0x8000000000000006) == 2) {
                  lVar27 = (long)*(int *)(uVar25 + 2);
                }
                else {
                  lVar27 = FUN_1000a0ad8(uVar25);
                }
                if (*(long *)(lVar26 + 0x100) == lVar27 + 1) {
                  pcVar24 = *(code **)(lVar26 + 0x108);
                }
                else {
                  pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x100,uVar25,0x27d543198ed41f36,"tc:[]",5
                                                 );
                }
                uVar16 = (*pcVar24)(uVar25,&local_190,&DAT_1002dab28);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              }
              uStack_290 = uVar16;
              if (*(char *)(lVar26 + 0x68) == '\0') {
                uVar8 = FUN_1000f70b4();
              }
              else {
                uVar8 = *(undefined8 *)(lVar26 + 0x60);
              }
              FUN_1000b38a4(uVar16,uVar8,0,0);
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = &DAT_10031791a;
              local_1a0 = &DAT_10031791a;
              uStack_198 = 0;
              if (uVar16 == 0) {
                lVar27 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar27 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar27 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar27 = 2;
              }
              if (*(long *)(lVar26 + 0x110) == lVar27 + 1) {
                pcVar24 = *(code **)(lVar26 + 0x118);
              }
              else {
                pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x110,uVar16,0x27d543198ed41f36,"tc:[]",5);
              }
              uVar21 = (*pcVar24)(uVar16,&local_1a0,&DAT_1002dab28);
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = (undefined *)uVar21;
              if (uVar21 == 0) {
                uVar22 = 1;
              }
              else if ((uVar21 & 1) == 0) {
                if ((uVar21 & 0x8000000000000006) == 2) {
                  uVar22 = (ulong)*(int *)(uVar21 + 2);
                }
                else {
                  uVar22 = FUN_1000a0ad8(uVar21);
                }
              }
              else {
                uVar22 = 2;
              }
              pcVar20 = pcVar7;
              if (bVar3) {
                pcVar20 = (char *)thunk_FUN_10009c0a0();
              }
              if (((uVar22 < *(ulong *)(pcVar20 + 0x160)) && (0x2a2 < *(ulong *)(pcVar20 + 0x158)))
                 && (*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) != 0)) {
                uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) +
                                         (uVar22 >> 6) * 8) >> (uVar22 & 0x3f)) & 1;
              }
              else {
                uVar6 = FUN_10009f014(uVar22,0x2a2);
              }
              if ((uVar21 == 0) || ((uVar6 & 0xff) == 0)) {
                if (*(char *)(lVar26 + 0x78) == '\0') {
                  uVar8 = FUN_1000f7160();
                }
                else {
                  uVar8 = *(undefined8 *)(lVar26 + 0x70);
                }
                FUN_1000b38a4(uVar21,uVar8,0,0);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              }
              if ((uVar21 & 1) == 0) {
                if (((uVar21 & 0x8000000000000006) == 2) && (*(int *)(uVar21 + 2) == 3)) {
                  lVar27 = *(long *)(uVar21 + 0xe);
                }
                else {
                  lVar27 = FUN_1000b9958(uVar21);
                }
              }
              else {
                lVar27 = (long)uVar21 >> 1;
              }
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = &DAT_100317a0a;
              local_1b0 = &DAT_100317a0a;
              uStack_1a8 = 0;
              if (uVar16 == 0) {
                lVar14 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar14 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar14 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar14 = 2;
              }
              if (*(long *)(lVar26 + 0x120) == lVar14 + 1) {
                pcVar24 = *(code **)(lVar26 + 0x128);
              }
              else {
                pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x120,uVar16,0x27d543198ed41f36,"tc:[]",5);
              }
              uVar21 = (*pcVar24)(uVar16,&local_1b0,&DAT_1002dab28);
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = (undefined *)uVar21;
              if (uVar21 == 0) {
                uVar22 = 1;
              }
              else if ((uVar21 & 1) == 0) {
                if ((uVar21 & 0x8000000000000006) == 2) {
                  uVar22 = (ulong)*(int *)(uVar21 + 2);
                }
                else {
                  uVar22 = FUN_1000a0ad8(uVar21);
                }
              }
              else {
                uVar22 = 2;
              }
              pcVar20 = pcVar7;
              if (bVar3) {
                pcVar20 = (char *)thunk_FUN_10009c0a0();
              }
              if (((uVar22 < *(ulong *)(pcVar20 + 0x160)) && (0x269 < *(ulong *)(pcVar20 + 0x158)))
                 && (*(long *)(*(long *)(pcVar20 + 0x150) + 0x1348) != 0)) {
                uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x1348) +
                                         (uVar22 >> 6) * 8) >> (uVar22 & 0x3f)) & 1;
              }
              else {
                uVar6 = FUN_10009f014(uVar22,0x269);
              }
              if ((uVar21 == 0) || ((uVar6 & 0xff) == 0)) {
                if (*(char *)(lVar26 + 0x88) == '\0') {
                  uVar8 = FUN_1000f720c();
                }
                else {
                  uVar8 = *(undefined8 *)(lVar26 + 0x80);
                }
                FUN_1000b38a4(uVar21,uVar8,0,0);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              }
              lVar14 = *(long *)(uVar21 + 0xe);
              local_288 = &DAT_1003178b2;
              local_1c0 = &DAT_1003178b2;
              uStack_1b8 = 0;
              if (uVar16 == 0) {
                lVar17 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar17 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar17 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar17 = 2;
              }
              if (*(long *)(lVar26 + 0x130) == lVar17 + 1) {
                pcVar24 = *(code **)(lVar26 + 0x138);
              }
              else {
                pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x130,uVar16,0x27d543198ed41f36,"tc:[]",5);
              }
              uVar21 = (*pcVar24)(uVar16,&local_1c0,&DAT_1002dab28);
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = (undefined *)uVar21;
              if (uVar21 == 0) {
                uVar22 = 1;
              }
              else if ((uVar21 & 1) == 0) {
                if ((uVar21 & 0x8000000000000006) == 2) {
                  uVar22 = (ulong)*(int *)(uVar21 + 2);
                }
                else {
                  uVar22 = FUN_1000a0ad8(uVar21);
                }
              }
              else {
                uVar22 = 2;
              }
              pcVar20 = pcVar7;
              if (bVar3) {
                pcVar20 = (char *)thunk_FUN_10009c0a0();
              }
              if (((uVar22 < *(ulong *)(pcVar20 + 0x160)) && (0x255 < *(ulong *)(pcVar20 + 0x158)))
                 && (*(long *)(*(long *)(pcVar20 + 0x150) + 0x12a8) != 0)) {
                uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x12a8) +
                                         (uVar22 >> 6) * 8) >> (uVar22 & 0x3f)) & 1;
              }
              else {
                uVar6 = FUN_10009f014(uVar22,0x255);
              }
              if ((uVar21 == 0) || ((uVar6 & 0xff) == 0)) {
                if (*(char *)(lVar26 + 0x58) == '\0') {
                  uVar8 = FUN_1000f7008();
                }
                else {
                  uVar8 = *(undefined8 *)(lVar26 + 0x50);
                }
                FUN_1000b38a4(uVar21,uVar8,0,0);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
                if (uVar21 != 0) goto LAB_1000f6938;
                local_1c8 = 0;
                lVar17 = 1;
LAB_1000f69a0:
                if (*(long *)(lVar26 + 0x140) == lVar17 + 1) {
                  pcVar24 = *(code **)(lVar26 + 0x148);
                }
                else {
                  pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x140,uVar21,0xa4e23ace76c22b78,
                                                  "g:length",8);
                }
                local_288 = (undefined *)(*pcVar24)(uVar21,&local_1c8,&DAT_1002dab18);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
                if (((ulong)local_288 & 1) == 0) {
                  if ((((ulong)local_288 & 0x8000000000000006) == 2) &&
                     (*(int *)((long)local_288 + 2) == 3)) {
                    lVar17 = *(long *)((long)local_288 + 0xe);
                  }
                  else {
                    lVar17 = FUN_1000b9958();
                    if (*pcVar11 != '\0') goto LAB_1000f6ef0;
                  }
                }
                else {
                  lVar17 = (long)local_288 >> 1;
                }
              }
              else {
LAB_1000f6938:
                if ((uVar21 & 1) != 0) {
                  local_1c8 = 0;
                  lVar17 = 2;
                  goto LAB_1000f69a0;
                }
                if ((uVar21 & 0x8000000000000006) == 2) {
                  lVar17 = (long)*(int *)(uVar21 + 2);
                }
                else {
                  lVar17 = FUN_1000a0ad8(uVar21);
                }
                if ((2 < lVar17 - 8U) || (lVar17 = *(long *)(uVar21 + 0xe), lVar17 == 0)) {
                  local_1c8 = 0;
                  if ((uVar21 & 0x8000000000000006) == 2) {
                    lVar17 = (long)*(int *)(uVar21 + 2);
                  }
                  else {
                    lVar17 = FUN_1000a0ad8(uVar21);
                  }
                  goto LAB_1000f69a0;
                }
              }
              local_288 = &DAT_1003176aa;
              local_1d8 = &DAT_1003176aa;
              uStack_1d0 = 0;
              if (uVar16 == 0) {
                lVar18 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar18 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar18 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar18 = 2;
              }
              if (*(long *)(lVar26 + 0x150) == lVar18 + 1) {
                pcVar24 = *(code **)(lVar26 + 0x158);
              }
              else {
                pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x150,uVar16,0x27d543198ed41f36,"tc:[]",5);
              }
              uVar21 = (*pcVar24)(uVar16,&local_1d8,&DAT_1002dab28);
              if (*pcVar11 != '\0') {
LAB_1000f6ef0:
                *(undefined8 *)(pcVar11 + 8) = local_2a8;
                return 0;
              }
              local_288 = (undefined *)uVar21;
              if (uVar21 == 0) {
                uVar22 = 1;
              }
              else if ((uVar21 & 1) == 0) {
                if ((uVar21 & 0x8000000000000006) == 2) {
                  uVar22 = (ulong)*(int *)(uVar21 + 2);
                }
                else {
                  uVar22 = FUN_1000a0ad8(uVar21);
                }
              }
              else {
                uVar22 = 2;
              }
              pcVar20 = pcVar7;
              if (bVar3) {
                pcVar20 = (char *)thunk_FUN_10009c0a0();
              }
              if (((uVar22 < *(ulong *)(pcVar20 + 0x160)) && (5 < *(ulong *)(pcVar20 + 0x158))) &&
                 (*(long *)(*(long *)(pcVar20 + 0x150) + 0x28) != 0)) {
                uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x28) +
                                         (uVar22 >> 6) * 8) >> (uVar22 & 0x3f)) & 1;
              }
              else {
                uVar6 = FUN_10009f014(uVar22,5);
              }
              if ((uVar21 == 0) || ((uVar6 & 0xff) == 0)) {
                if (*(char *)(lVar26 + 0x98) == '\0') {
                  uVar8 = FUN_1000f72b8();
                }
                else {
                  uVar8 = *(undefined8 *)(lVar26 + 0x90);
                }
                FUN_1000b38a4(uVar21,uVar8,0,0);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              }
              pcVar20 = pcVar7;
              if (bVar3) {
                pcVar20 = (char *)thunk_FUN_10009c0a0();
              }
              if (uVar21 == *(ulong *)(pcVar20 + 0x168)) {
                uVar21 = 1;
              }
              else if (uVar21 == *(ulong *)(pcVar20 + 0x170)) {
                uVar21 = 0;
              }
              else {
                cVar5 = FUN_1000b95ac(uVar21);
                uVar21 = (ulong)(cVar5 != '\0');
              }
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = &DAT_10031770a;
              local_1e8 = &DAT_10031770a;
              uStack_1e0 = 0;
              if (uVar16 == 0) {
                lVar18 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar18 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar18 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar18 = 2;
              }
              if (*(long *)(lVar26 + 0x160) == lVar18 + 1) {
                pcVar24 = *(code **)(lVar26 + 0x168);
              }
              else {
                pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x160,uVar16,0x27d543198ed41f36,"tc:[]",5);
              }
              uVar16 = (*pcVar24)(uVar16,&local_1e8,&DAT_1002dab28);
              if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              local_288 = (undefined *)0x0;
              uStack_290 = uVar16;
              if (uVar16 == 0) {
                uVar22 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  uVar22 = (ulong)*(int *)(uVar16 + 2);
                }
                else {
                  uVar22 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                uVar22 = 2;
              }
              pcVar20 = pcVar7;
              if (bVar3) {
                pcVar20 = (char *)thunk_FUN_10009c0a0();
              }
              if (((uVar22 < *(ulong *)(pcVar20 + 0x160)) && (0x2a2 < *(ulong *)(pcVar20 + 0x158)))
                 && (*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) != 0)) {
                uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) +
                                         (uVar22 >> 6) * 8) >> (uVar22 & 0x3f)) & 1;
              }
              else {
                uVar6 = FUN_10009f014(uVar22,0x2a2);
              }
              if ((uVar16 == 0) || ((uVar6 & 0xff) == 0)) {
                if (*(char *)(lVar26 + 0x78) == '\0') {
                  uVar8 = FUN_1000f7160();
                }
                else {
                  uVar8 = *(undefined8 *)(lVar26 + 0x70);
                }
                FUN_1000b38a4(uVar16,uVar8,0,0);
                if (*pcVar11 != '\0') goto LAB_1000f6ef0;
              }
              if ((uVar16 & 1) == 0) {
                if (((uVar16 & 0x8000000000000006) == 2) && (*(int *)(uVar16 + 2) == 3)) {
                  lVar18 = *(long *)(uVar16 + 0xe);
                }
                else {
                  lVar18 = FUN_1000b9958(uVar16);
                }
              }
              else {
                lVar18 = (long)uVar16 >> 1;
              }
              if (*pcVar11 != '\0') goto LAB_1000f6fc0;
              uVar13 = uVar13 + 1;
              lVar12 = lVar27 + lVar12 + lVar14 + lVar17 + uVar21 + lVar18;
              if (*(ulong *)(uVar25 + 0xe) != uVar15) {
                puVar2 = *(undefined8 **)(pcVar11 + 0x60);
                uStack_290 = 0;
                if (((*(undefined8 **)(pcVar11 + 0x68) < puVar2 + 4) ||
                    (*(ulong *)(pcVar11 + 0x38) < 0x246)) ||
                   (*(char *)(*(long *)(pcVar11 + 0x30) + 0x245) == '\0')) {
                  lVar26 = FUN_1000b0d1c(0x245,2,FUN_1000f95a8);
                }
                else {
                  *(undefined8 **)(pcVar11 + 0x60) = puVar2 + 4;
                  lVar26 = (long)puVar2 + 2;
                  puVar2[2] = 0;
                  *puVar2 = 0x24500020001;
                }
                uStack_290 = lVar26;
                FUN_100115628(lVar26,uVar25);
                if (*pcVar11 == '\0') {
                  local_298 = 0;
                  FUN_10009c5d4(lVar26,3);
                }
LAB_1000f6fc0:
                *(undefined8 *)(pcVar11 + 8) = local_2a8;
                return 0;
              }
            }
            if (*pcVar11 != '\0') {
              FUN_10009c234();
            }
            goto LAB_1000f542c;
          }
          local_180 = 0;
        }
        else {
          lVar12 = FUN_1000a0ad8(uVar25);
          if (lVar12 - 8U < 3) goto LAB_1000f61c4;
          local_180 = 0;
          lVar12 = FUN_1000a0ad8(uVar25);
        }
      }
      else {
        local_180 = 0;
        lVar12 = 2;
      }
    }
    if (*(long *)(lVar26 + 0xf0) == lVar12 + 1) {
      pcVar24 = *(code **)(lVar26 + 0xf8);
    }
    else {
      pcVar24 = (code *)FUN_1000b7430((long *)(lVar26 + 0xf0),uVar25,0xb40061c0859c6f2e,"g:iterator"
                                      ,10);
    }
    uVar25 = (*pcVar24)(uVar25,&local_180,&DAT_1002dab18);
    if (*pcVar11 == '\0') {
      bVar4 = uVar25 != 0;
      lVar27 = 1;
      if (bVar4) {
        lVar27 = 2;
      }
      lVar12 = 0;
      local_298 = uVar25;
      while( true ) {
        uStack_290 = 0;
        local_1f0 = 0;
        lVar14 = lVar27;
        if (bVar4 && (uVar25 & 1) == 0) {
          if ((uVar25 & 0x8000000000000006) == 2) {
            lVar14 = (long)*(int *)(uVar25 + 2);
          }
          else {
            lVar14 = FUN_1000a0ad8(uVar25);
          }
        }
        if (*(long *)(lVar26 + 0x170) == lVar14 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x178);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x170,uVar25,0xba14179663701108,"c:moveNext",10);
        }
        lVar14 = (*pcVar24)(uVar25,&local_1f0,&DAT_1002dab18);
        if (*pcVar11 != '\0') break;
        pcVar20 = pcVar7;
        uStack_290 = lVar14;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (lVar14 != *(long *)(pcVar20 + 0x168)) {
          if (lVar14 == *(long *)(pcVar20 + 0x170)) goto LAB_1000f542c;
          cVar5 = FUN_1000b95ac();
          if (*pcVar11 != '\0') break;
          if (cVar5 == '\0') goto LAB_1000f542c;
        }
        if (pcVar11[0x20] != '\0') {
          uStack_290 = 0;
          FUN_1000b736c();
          if (*pcVar11 != '\0') break;
        }
        uStack_290 = 0;
        local_1f8 = 0;
        lVar14 = lVar27;
        if (bVar4 && (uVar25 & 1) == 0) {
          if ((uVar25 & 0x8000000000000006) == 2) {
            lVar14 = (long)*(int *)(uVar25 + 2);
          }
          else {
            lVar14 = FUN_1000a0ad8(uVar25);
          }
        }
        if (*(long *)(lVar26 + 0x180) == lVar14 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x188);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x180,uVar25,0xe9c0b1d8160bef1,"g:current",9);
        }
        uVar13 = (*pcVar24)(uVar25,&local_1f8,&DAT_1002dab18);
        if (*pcVar11 != '\0') break;
        uStack_290 = uVar13;
        if (*(char *)(lVar26 + 0x68) == '\0') {
          uVar8 = FUN_1000f70b4();
        }
        else {
          uVar8 = *(undefined8 *)(lVar26 + 0x60);
        }
        FUN_1000b38a4(uVar13,uVar8,0,0);
        if (*pcVar11 != '\0') break;
        local_288 = &DAT_10031791a;
        local_208 = &DAT_10031791a;
        uStack_200 = 0;
        if (uVar13 == 0) {
          lVar14 = 1;
        }
        else if ((uVar13 & 1) == 0) {
          if ((uVar13 & 0x8000000000000006) == 2) {
            lVar14 = (long)*(int *)(uVar13 + 2);
          }
          else {
            lVar14 = FUN_1000a0ad8(uVar13);
          }
        }
        else {
          lVar14 = 2;
        }
        if (*(long *)(lVar26 + 400) == lVar14 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x198);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 400,uVar13,0x27d543198ed41f36,"tc:[]",5);
        }
        uVar15 = (*pcVar24)(uVar13,&local_208,&DAT_1002dab28);
        if (*pcVar11 != '\0') break;
        local_288 = (undefined *)uVar15;
        if (uVar15 == 0) {
          uVar16 = 1;
        }
        else if ((uVar15 & 1) == 0) {
          if ((uVar15 & 0x8000000000000006) == 2) {
            uVar16 = (ulong)*(int *)(uVar15 + 2);
          }
          else {
            uVar16 = FUN_1000a0ad8(uVar15);
          }
        }
        else {
          uVar16 = 2;
        }
        pcVar20 = pcVar7;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (((uVar16 < *(ulong *)(pcVar20 + 0x160)) && (0x2a2 < *(ulong *)(pcVar20 + 0x158))) &&
           (*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) != 0)) {
          uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) +
                                   (uVar16 >> 6) * 8) >> (uVar16 & 0x3f)) & 1;
        }
        else {
          uVar6 = FUN_10009f014(uVar16,0x2a2);
        }
        if ((uVar15 == 0) || ((uVar6 & 0xff) == 0)) {
          if (*(char *)(lVar26 + 0x78) == '\0') {
            uVar8 = FUN_1000f7160();
          }
          else {
            uVar8 = *(undefined8 *)(lVar26 + 0x70);
          }
          FUN_1000b38a4(uVar15,uVar8,0,0);
          if (*pcVar11 != '\0') break;
        }
        if ((uVar15 & 1) == 0) {
          if (((uVar15 & 0x8000000000000006) == 2) && (*(int *)(uVar15 + 2) == 3)) {
            lVar14 = *(long *)(uVar15 + 0xe);
          }
          else {
            lVar14 = FUN_1000b9958(uVar15);
          }
        }
        else {
          lVar14 = (long)uVar15 >> 1;
        }
        if (*pcVar11 != '\0') break;
        local_288 = &DAT_100317a0a;
        local_218 = &DAT_100317a0a;
        uStack_210 = 0;
        if (uVar13 == 0) {
          lVar17 = 1;
        }
        else if ((uVar13 & 1) == 0) {
          if ((uVar13 & 0x8000000000000006) == 2) {
            lVar17 = (long)*(int *)(uVar13 + 2);
          }
          else {
            lVar17 = FUN_1000a0ad8(uVar13);
          }
        }
        else {
          lVar17 = 2;
        }
        if (*(long *)(lVar26 + 0x1a0) == lVar17 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x1a8);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x1a0,uVar13,0x27d543198ed41f36,"tc:[]",5);
        }
        uVar15 = (*pcVar24)(uVar13,&local_218,&DAT_1002dab28);
        if (*pcVar11 != '\0') break;
        local_288 = (undefined *)uVar15;
        if (uVar15 == 0) {
          uVar16 = 1;
        }
        else if ((uVar15 & 1) == 0) {
          if ((uVar15 & 0x8000000000000006) == 2) {
            uVar16 = (ulong)*(int *)(uVar15 + 2);
          }
          else {
            uVar16 = FUN_1000a0ad8(uVar15);
          }
        }
        else {
          uVar16 = 2;
        }
        pcVar20 = pcVar7;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (((uVar16 < *(ulong *)(pcVar20 + 0x160)) && (0x269 < *(ulong *)(pcVar20 + 0x158))) &&
           (*(long *)(*(long *)(pcVar20 + 0x150) + 0x1348) != 0)) {
          uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x1348) +
                                   (uVar16 >> 6) * 8) >> (uVar16 & 0x3f)) & 1;
        }
        else {
          uVar6 = FUN_10009f014(uVar16,0x269);
        }
        if ((uVar15 == 0) || ((uVar6 & 0xff) == 0)) {
          if (*(char *)(lVar26 + 0x88) == '\0') {
            uVar8 = FUN_1000f720c();
          }
          else {
            uVar8 = *(undefined8 *)(lVar26 + 0x80);
          }
          FUN_1000b38a4(uVar15,uVar8,0,0);
          if (*pcVar11 != '\0') break;
        }
        lVar17 = *(long *)(uVar15 + 0xe);
        local_288 = &DAT_1003178b2;
        local_228 = &DAT_1003178b2;
        uStack_220 = 0;
        if (uVar13 == 0) {
          lVar18 = 1;
        }
        else if ((uVar13 & 1) == 0) {
          if ((uVar13 & 0x8000000000000006) == 2) {
            lVar18 = (long)*(int *)(uVar13 + 2);
          }
          else {
            lVar18 = FUN_1000a0ad8(uVar13);
          }
        }
        else {
          lVar18 = 2;
        }
        if (*(long *)(lVar26 + 0x1b0) == lVar18 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x1b8);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x1b0,uVar13,0x27d543198ed41f36,"tc:[]",5);
        }
        uVar15 = (*pcVar24)(uVar13,&local_228,&DAT_1002dab28);
        if (*pcVar11 != '\0') break;
        local_288 = (undefined *)uVar15;
        if (uVar15 == 0) {
          uVar16 = 1;
        }
        else if ((uVar15 & 1) == 0) {
          if ((uVar15 & 0x8000000000000006) == 2) {
            uVar16 = (ulong)*(int *)(uVar15 + 2);
          }
          else {
            uVar16 = FUN_1000a0ad8(uVar15);
          }
        }
        else {
          uVar16 = 2;
        }
        pcVar20 = pcVar7;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (((uVar16 < *(ulong *)(pcVar20 + 0x160)) && (0x255 < *(ulong *)(pcVar20 + 0x158))) &&
           (*(long *)(*(long *)(pcVar20 + 0x150) + 0x12a8) != 0)) {
          uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x12a8) +
                                   (uVar16 >> 6) * 8) >> (uVar16 & 0x3f)) & 1;
        }
        else {
          uVar6 = FUN_10009f014(uVar16,0x255);
        }
        if ((uVar15 == 0) || ((uVar6 & 0xff) == 0)) {
          if (*(char *)(lVar26 + 0x58) == '\0') {
            uVar8 = FUN_1000f7008();
          }
          else {
            uVar8 = *(undefined8 *)(lVar26 + 0x50);
          }
          FUN_1000b38a4(uVar15,uVar8,0,0);
          if (*pcVar11 != '\0') break;
          if (uVar15 != 0) goto LAB_1000f5c24;
          local_230 = 0;
          lVar18 = 1;
LAB_1000f5c98:
          if (*(long *)(lVar26 + 0x1c0) == lVar18 + 1) {
            pcVar24 = *(code **)(lVar26 + 0x1c8);
          }
          else {
            pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x1c0,uVar15,0xa4e23ace76c22b78,"g:length",8);
          }
          local_288 = (undefined *)(*pcVar24)(uVar15,&local_230,&DAT_1002dab18);
          if (*pcVar11 != '\0') break;
          if (((ulong)local_288 & 1) == 0) {
            if ((((ulong)local_288 & 0x8000000000000006) == 2) &&
               (*(int *)((long)local_288 + 2) == 3)) {
              lVar18 = *(long *)((long)local_288 + 0xe);
            }
            else {
              lVar18 = FUN_1000b9958();
              if (*pcVar11 != '\0') break;
            }
          }
          else {
            lVar18 = (long)local_288 >> 1;
          }
        }
        else {
LAB_1000f5c24:
          if ((uVar15 & 1) != 0) {
            local_230 = 0;
            lVar18 = 2;
            goto LAB_1000f5c98;
          }
          if ((uVar15 & 0x8000000000000006) == 2) {
            lVar18 = (long)*(int *)(uVar15 + 2);
          }
          else {
            lVar18 = FUN_1000a0ad8(uVar15);
          }
          if ((2 < lVar18 - 8U) || (lVar18 = *(long *)(uVar15 + 0xe), lVar18 == 0)) {
            local_230 = 0;
            if ((uVar15 & 0x8000000000000006) == 2) {
              lVar18 = (long)*(int *)(uVar15 + 2);
            }
            else {
              lVar18 = FUN_1000a0ad8(uVar15);
            }
            goto LAB_1000f5c98;
          }
        }
        local_288 = &DAT_1003176aa;
        local_240 = &DAT_1003176aa;
        uStack_238 = 0;
        if (uVar13 == 0) {
          lVar19 = 1;
        }
        else if ((uVar13 & 1) == 0) {
          if ((uVar13 & 0x8000000000000006) == 2) {
            lVar19 = (long)*(int *)(uVar13 + 2);
          }
          else {
            lVar19 = FUN_1000a0ad8(uVar13);
          }
        }
        else {
          lVar19 = 2;
        }
        if (*(long *)(lVar26 + 0x1d0) == lVar19 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x1d8);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x1d0,uVar13,0x27d543198ed41f36,"tc:[]",5);
        }
        uVar15 = (*pcVar24)(uVar13,&local_240,&DAT_1002dab28);
        if (*pcVar11 != '\0') break;
        local_288 = (undefined *)uVar15;
        if (uVar15 == 0) {
          uVar16 = 1;
        }
        else if ((uVar15 & 1) == 0) {
          if ((uVar15 & 0x8000000000000006) == 2) {
            uVar16 = (ulong)*(int *)(uVar15 + 2);
          }
          else {
            uVar16 = FUN_1000a0ad8(uVar15);
          }
        }
        else {
          uVar16 = 2;
        }
        pcVar20 = pcVar7;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (((uVar16 < *(ulong *)(pcVar20 + 0x160)) && (5 < *(ulong *)(pcVar20 + 0x158))) &&
           (*(long *)(*(long *)(pcVar20 + 0x150) + 0x28) != 0)) {
          uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x28) + (uVar16 >> 6) * 8
                                   ) >> (uVar16 & 0x3f)) & 1;
        }
        else {
          uVar6 = FUN_10009f014(uVar16,5);
        }
        if ((uVar15 == 0) || ((uVar6 & 0xff) == 0)) {
          if (*(char *)(lVar26 + 0x98) == '\0') {
            uVar8 = FUN_1000f72b8();
          }
          else {
            uVar8 = *(undefined8 *)(lVar26 + 0x90);
          }
          FUN_1000b38a4(uVar15,uVar8,0,0);
          if (*pcVar11 != '\0') break;
        }
        pcVar20 = pcVar7;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (uVar15 == *(ulong *)(pcVar20 + 0x168)) {
          uVar15 = 1;
        }
        else if (uVar15 == *(ulong *)(pcVar20 + 0x170)) {
          uVar15 = 0;
        }
        else {
          cVar5 = FUN_1000b95ac(uVar15);
          uVar15 = (ulong)(cVar5 != '\0');
        }
        if (*pcVar11 != '\0') break;
        local_288 = &DAT_10031770a;
        local_250 = &DAT_10031770a;
        uStack_248 = 0;
        if (uVar13 == 0) {
          lVar19 = 1;
        }
        else if ((uVar13 & 1) == 0) {
          if ((uVar13 & 0x8000000000000006) == 2) {
            lVar19 = (long)*(int *)(uVar13 + 2);
          }
          else {
            lVar19 = FUN_1000a0ad8(uVar13);
          }
        }
        else {
          lVar19 = 2;
        }
        if (*(long *)(lVar26 + 0x1e0) == lVar19 + 1) {
          pcVar24 = *(code **)(lVar26 + 0x1e8);
        }
        else {
          pcVar24 = (code *)FUN_1000b7430(lVar26 + 0x1e0,uVar13,0x27d543198ed41f36,"tc:[]",5);
        }
        uVar13 = (*pcVar24)(uVar13,&local_250,&DAT_1002dab28);
        if (*pcVar11 != '\0') break;
        local_288 = (undefined *)0x0;
        uStack_290 = uVar13;
        if (uVar13 == 0) {
          uVar16 = 1;
        }
        else if ((uVar13 & 1) == 0) {
          if ((uVar13 & 0x8000000000000006) == 2) {
            uVar16 = (ulong)*(int *)(uVar13 + 2);
          }
          else {
            uVar16 = FUN_1000a0ad8(uVar13);
          }
        }
        else {
          uVar16 = 2;
        }
        pcVar20 = pcVar7;
        if (bVar3) {
          pcVar20 = (char *)thunk_FUN_10009c0a0();
        }
        if (((uVar16 < *(ulong *)(pcVar20 + 0x160)) && (0x2a2 < *(ulong *)(pcVar20 + 0x158))) &&
           (*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) != 0)) {
          uVar6 = (uint)(*(ulong *)(*(long *)(*(long *)(pcVar20 + 0x150) + 0x1510) +
                                   (uVar16 >> 6) * 8) >> (uVar16 & 0x3f)) & 1;
        }
        else {
          uVar6 = FUN_10009f014(uVar16,0x2a2);
        }
        if ((uVar13 == 0) || ((uVar6 & 0xff) == 0)) {
          if (*(char *)(lVar26 + 0x78) == '\0') {
            uVar8 = FUN_1000f7160();
          }
          else {
            uVar8 = *(undefined8 *)(lVar26 + 0x70);
          }
          FUN_1000b38a4(uVar13,uVar8,0,0);
          if (*pcVar11 != '\0') break;
        }
        if ((uVar13 & 1) == 0) {
          if (((uVar13 & 0x8000000000000006) == 2) && (*(int *)(uVar13 + 2) == 3)) {
            lVar19 = *(long *)(uVar13 + 0xe);
          }
          else {
            lVar19 = FUN_1000b9958(uVar13);
          }
        }
        else {
          lVar19 = (long)uVar13 >> 1;
        }
        if (*pcVar11 != '\0') break;
        lVar12 = lVar14 + lVar12 + lVar17 + lVar18 + uVar15 + lVar19;
      }
    }
  }
LAB_1000f5428:
  lVar12 = 0;
LAB_1000f542c:
  *(undefined8 *)(pcVar11 + 8) = local_2a8;
  return lVar12;
}

