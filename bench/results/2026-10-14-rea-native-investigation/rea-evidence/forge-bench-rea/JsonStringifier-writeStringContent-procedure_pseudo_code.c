
void FUN_1001ecdc4(ulong param_1,long param_2)

{
  uint uVar1;
  char cVar2;
  bool bVar3;
  bool bVar4;
  bool bVar5;
  bool bVar6;
  ushort uVar7;
  char *pcVar8;
  long lVar9;
  char *pcVar10;
  undefined8 *puVar11;
  undefined8 uVar12;
  ulong *puVar13;
  long *plVar14;
  undefined *puVar15;
  ulong uVar16;
  code *pcVar17;
  ushort uVar18;
  ulong uVar19;
  long lVar20;
  char *pcVar21;
  ulong uVar22;
  bool bVar23;
  ulong uVar24;
  ulong uVar25;
  ulong uVar26;
  ulong uVar27;
  long local_3f8;
  long local_3e0;
  undefined1 uStack_3d1;
  undefined8 local_3d0;
  undefined8 local_3c8;
  ulong local_3c0;
  long lStack_3b8;
  ulong local_3b0;
  ulong local_3a8;
  long lStack_3a0;
  ulong local_398;
  ulong uStack_390;
  undefined8 local_388;
  long local_380;
  undefined8 uStack_378;
  long local_370;
  undefined8 uStack_368;
  long local_360;
  undefined8 uStack_358;
  ulong local_350;
  undefined8 uStack_348;
  ulong local_340;
  undefined8 uStack_338;
  undefined8 local_330;
  undefined8 uStack_328;
  undefined8 local_320;
  undefined8 uStack_318;
  long local_310;
  ulong uStack_308;
  ulong local_300;
  undefined8 uStack_2f8;
  ulong local_2f0;
  undefined8 uStack_2e8;
  ulong local_2e0;
  undefined8 uStack_2d8;
  ulong local_2d0;
  undefined8 uStack_2c8;
  ulong local_2c0;
  undefined8 local_2b8;
  ulong local_2b0;
  undefined8 uStack_2a8;
  undefined8 local_2a0;
  undefined8 uStack_298;
  undefined8 local_290;
  undefined8 uStack_288;
  undefined8 local_280;
  undefined8 uStack_278;
  undefined8 local_270;
  undefined8 uStack_268;
  undefined8 local_260;
  undefined8 uStack_258;
  undefined8 local_250;
  undefined8 uStack_248;
  ulong local_240;
  undefined8 local_238;
  ulong local_230;
  undefined8 uStack_228;
  ulong local_220;
  undefined8 local_218;
  ulong local_210;
  undefined8 uStack_208;
  ulong local_200;
  undefined8 local_1f8;
  ulong local_1f0;
  undefined8 uStack_1e8;
  ulong local_1e0;
  undefined8 local_1d8;
  ulong local_1d0;
  undefined8 uStack_1c8;
  ulong local_1c0;
  undefined8 local_1b8;
  ulong local_1b0;
  undefined8 uStack_1a8;
  undefined8 local_1a0;
  undefined8 uStack_198;
  undefined8 local_190;
  undefined8 uStack_188;
  long local_180;
  ulong uStack_178;
  ulong local_170;
  undefined8 uStack_168;
  ulong local_160;
  undefined8 uStack_158;
  ulong local_150;
  undefined8 local_148;
  ulong local_140;
  undefined8 local_138;
  ulong local_130;
  undefined8 local_128;
  ulong local_120;
  undefined8 local_118;
  ulong local_110;
  undefined8 local_108;
  ulong local_100;
  undefined8 local_f8;
  undefined8 local_f0;
  undefined8 local_e8;
  undefined8 local_e0;
  undefined8 local_d8;
  undefined8 local_d0;
  undefined8 local_c8;
  undefined8 local_c0;
  undefined8 local_b8;
  undefined8 local_b0;
  undefined8 local_a8;
  undefined8 local_a0;
  undefined8 local_98;
  long local_90;
  ulong local_88;
  ulong local_80;
  undefined8 local_78;
  ulong local_70;
  undefined8 local_68;
  
  uVar27 = tpidrro_el0;
  pcVar21 = *(char **)((uVar27 & 0xfffffffffffffff8) + DAT_10032c1f8);
  bVar4 = DAT_10032c1f8 == 0 || pcVar21 == (char *)0x0;
  pcVar10 = pcVar21;
  if (bVar4) {
    pcVar10 = (char *)thunk_FUN_10009c0a0();
  }
  if ((*(ulong *)(pcVar10 + 0x18) <= DAT_10032c270) ||
     (lVar20 = *(long *)(*(long *)(pcVar10 + 0x10) + DAT_10032c270 * 8), lVar20 == 0)) {
    lVar20 = FUN_10009ec90(&DAT_1002dd318,&DAT_10032c270);
  }
  pcVar10 = pcVar21;
  if (bVar4) {
    pcVar10 = (char *)thunk_FUN_10009c0a0();
  }
  if (&uStack_3d1 < *(undefined1 **)(pcVar10 + 0x178)) {
    FUN_1000b7c08();
    return;
  }
  local_3c8 = 4;
  local_3a8 = 0;
  local_3b0 = 0;
  local_3d0 = *(undefined8 *)(pcVar10 + 8);
  *(undefined8 **)(pcVar10 + 8) = &local_3d0;
  uVar27 = *(ulong *)(param_2 + 0xe);
  local_3c0 = param_1;
  lStack_3b8 = param_2;
  if ((long)uVar27 < 1) {
LAB_1001efe98:
    if (param_1 == 0) {
      lVar9 = 1;
LAB_1001efed0:
      local_3a8 = 0;
      local_3b0 = 0;
      uStack_368 = 0;
      local_370 = param_2;
    }
    else {
      if ((param_1 & 1) != 0) {
        lVar9 = 2;
        goto LAB_1001efed0;
      }
      if ((param_1 & 0x8000000000000006) == 2) {
        lVar9 = (long)*(int *)(param_1 + 2);
      }
      else {
        lVar9 = FUN_1000a0ad8(param_1);
      }
      if (lVar9 - 0x222U < 2) {
        param_1 = *(ulong *)(param_1 + 0x1e);
        uStack_358 = 0;
        local_3b0 = 0;
        local_3a8 = 0;
        local_3c0 = param_1;
        local_360 = param_2;
        if (param_1 == 0) {
          lVar9 = 1;
        }
        else if ((param_1 & 1) == 0) {
          if ((param_1 & 0x8000000000000006) == 2) {
            lVar9 = (long)*(int *)(param_1 + 2);
          }
          else {
            lVar9 = FUN_1000a0ad8(param_1);
          }
        }
        else {
          lVar9 = 2;
        }
        if (*(long *)(lVar20 + 0x2950) == lVar9 + 1) {
          pcVar17 = *(code **)(lVar20 + 0x2958);
        }
        else {
          pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2950,param_1,0xcece15d88a54e5a9,"tc:write",8);
        }
        puVar15 = &DAT_1002dab28;
        plVar14 = &local_360;
        goto LAB_1001eff28;
      }
      if (lVar9 - 0x229U < 2) {
        uVar27 = *(ulong *)(param_2 + 0xe);
        local_3e0 = 0;
        goto LAB_1001eff94;
      }
      uStack_368 = 0;
      local_3b0 = 0;
      local_3a8 = 0;
      if ((param_1 & 0x8000000000000006) == 2) {
        lVar9 = (long)*(int *)(param_1 + 2);
        local_370 = param_2;
      }
      else {
        local_370 = param_2;
        lVar9 = FUN_1000a0ad8(param_1);
      }
    }
    if (*(long *)(lVar20 + 0x2960) == lVar9 + 1) {
      pcVar17 = *(code **)(lVar20 + 0x2968);
    }
    else {
      pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2960,param_1,0x65174a21744564e4,"tc:writeString",
                                      0xe);
    }
    puVar15 = &DAT_1002dab28;
    plVar14 = &local_370;
  }
  else {
    bVar5 = param_1 != 0;
    local_3f8 = 1;
    if (bVar5) {
      local_3f8 = 2;
    }
    bVar6 = (param_1 & 1) == 0;
    bVar3 = bVar6 && bVar5;
    uVar24 = 0;
    uVar22 = 1;
    local_3e0 = 0;
    do {
      if (uVar24 < *(ulong *)(param_2 + 0xe)) {
        if (*(int *)(param_2 + 2) == 6) {
          uVar25 = (ulong)*(byte *)(param_2 + uVar24 + 0x16);
        }
        else {
          uVar25 = (ulong)*(ushort *)(param_2 + uVar24 * 2 + 0x16);
        }
      }
      else {
        local_3b0 = 0;
        local_3a8 = 0;
        uVar25 = FUN_1000a9d2c(param_2,uVar24);
        if (*pcVar10 != '\0') goto LAB_1001eff34;
      }
      uVar26 = param_1;
      if ((long)uVar25 < 0x5d) {
        if ((long)uVar25 < 0x20) {
          if (local_3e0 < (long)uVar24) {
            if (!bVar6 || !bVar5) {
LAB_1001ed068:
              local_3b0 = 0;
              local_3a8 = 0;
              if (local_3e0 + 0x4000000000000000 < 0) {
                pcVar8 = pcVar21;
                if (bVar4) {
                  pcVar8 = (char *)thunk_FUN_10009c0a0();
                }
                puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                  uVar19 = FUN_10009b470(3,1,0x102);
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                  uVar19 = (long)puVar11 + 2;
                  *puVar11 = 0x300010201;
                }
                *(long *)(uVar19 + 0xe) = local_3e0;
              }
              else {
                uVar19 = local_3e0 << 1 | 1;
              }
              uVar16 = uVar22;
              local_3b0 = uVar19;
              if ((long)(uVar24 + 0x4000000000000000) < 0) {
                pcVar8 = pcVar21;
                if (bVar4) {
                  pcVar8 = (char *)thunk_FUN_10009c0a0();
                }
                puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                  uVar16 = FUN_10009b470(3,1,0x102);
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                  uVar16 = (long)puVar11 + 2;
                  *puVar11 = 0x300010201;
                }
                *(ulong *)(uVar16 + 0xe) = uVar24;
              }
              uStack_168 = 0;
              lVar9 = local_3f8;
              local_3a8 = uVar16;
              local_180 = param_2;
              uStack_178 = uVar19;
              local_170 = uVar16;
              if (bVar3) {
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
              }
              if (*(long *)(lVar20 + 0x2780) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2788);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2780,param_1,0x8c8882b1249d8dde,
                                                "tc:writeStringSlice",0x13);
              }
              (*pcVar17)(param_1,&local_180,&DAT_1002db240);
              if (*pcVar10 == '\0') goto LAB_1001ed3ec;
              goto LAB_1001eff34;
            }
            if ((param_1 & 0x8000000000000006) == 2) {
              lVar9 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar9 = FUN_1000a0ad8(param_1);
            }
            if (lVar9 - 0x222U < 2) {
              uVar16 = *(ulong *)(param_1 + 0x1e);
              local_3a8 = 0;
              uVar19 = uVar22;
              local_3b0 = uVar16;
              if ((long)(uVar24 + 0x4000000000000000) < 0) {
                pcVar8 = pcVar21;
                if (bVar4) {
                  pcVar8 = (char *)thunk_FUN_10009c0a0();
                }
                puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                  uVar19 = FUN_10009b470(3,1,0x102);
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                  uVar19 = (long)puVar11 + 2;
                  *puVar11 = 0x300010201;
                }
                *(ulong *)(uVar19 + 0xe) = uVar24;
              }
              local_3a8 = uVar19;
              local_3a8 = FUN_100164988(param_2,local_3e0);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              uStack_158 = 0;
              local_160 = local_3a8;
              if (uVar16 == 0) {
                lVar9 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar9 = 2;
              }
              if (*(long *)(lVar20 + 0x2770) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2778);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2770,uVar16,0xcece15d88a54e5a9,"tc:write"
                                                ,8);
              }
              (*pcVar17)(uVar16,&local_160,&DAT_1002dab28);
            }
            else {
              if (1 < lVar9 - 0x229U) goto LAB_1001ed068;
              local_3b0 = 0;
              local_3a8 = 0;
              FUN_1001fabcc(param_1,param_2,local_3e0,uVar24);
            }
            if (*pcVar10 != '\0') goto LAB_1001eff34;
LAB_1001edac4:
            if ((param_1 & 0x8000000000000006) == 2) {
              lVar9 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar9 = FUN_1000a0ad8(param_1);
            }
            if (lVar9 - 0x222U < 2) {
              uVar19 = *(ulong *)(param_1 + 0x1e);
              local_190 = 0xb9;
              uStack_188 = 0;
              local_3a8 = 0xb9;
              local_3b0 = uVar19;
              if (uVar19 == 0) {
                lVar9 = 1;
              }
              else if ((uVar19 & 1) == 0) {
                if ((uVar19 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(uVar19 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(uVar19);
                }
              }
              else {
                lVar9 = 2;
              }
              if (*(long *)(lVar20 + 0x2790) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2798);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2790,uVar19,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(uVar19,&local_190,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              bVar23 = false;
            }
            else {
              if (1 < lVar9 - 0x229U) {
                local_3b0 = 0xb9;
                local_3a8 = 0;
                local_1a0 = 0xb9;
                uStack_198 = 0;
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                bVar23 = false;
                goto LAB_1001ed438;
              }
              local_3b0 = 0;
              local_3a8 = 0;
              FUN_1001fb478(param_1,0x5c);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              bVar23 = false;
            }
          }
          else {
LAB_1001ed3ec:
            if (param_1 == 0) {
              bVar23 = true;
              lVar9 = 1;
            }
            else {
              if ((param_1 & 1) == 0) goto LAB_1001edac4;
              bVar23 = false;
              lVar9 = 2;
            }
            local_3a8 = 0;
            local_3b0 = 0xb9;
            local_1a0 = 0xb9;
            uStack_198 = 0;
LAB_1001ed438:
            if (*(long *)(lVar20 + 0x27a0) == lVar9 + 1) {
              pcVar17 = *(code **)(lVar20 + 0x27a8);
            }
            else {
              pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x27a0,param_1,0xabd2b0244154f858,
                                              &DAT_1002bd3c0,0x10);
            }
            (*pcVar17)(param_1,&local_1a0,&DAT_1002dab28);
            if (*pcVar10 != '\0') goto LAB_1001eff34;
          }
          if ((long)uVar25 < 10) {
            if (uVar25 != 8) {
              if (uVar25 == 9) {
                if (bVar23) {
                  lVar9 = 1;
LAB_1001edf6c:
                  local_3b0 = 0xe9;
                  local_3a8 = 0;
                  local_1e0 = 0xe9;
                  local_1d8 = 0;
                }
                else {
                  if ((param_1 & 1) != 0) {
                    lVar9 = 2;
                    goto LAB_1001edf6c;
                  }
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  if (lVar9 - 0x222U < 2) {
                    uVar26 = *(ulong *)(param_1 + 0x1e);
                    local_1d0 = 0xe9;
                    uStack_1c8 = 0;
                    local_3a8 = 0xe9;
                    local_3b0 = uVar26;
                    if (uVar26 == 0) {
                      lVar9 = 1;
                    }
                    else if ((uVar26 & 1) == 0) {
                      if ((uVar26 & 0x8000000000000006) == 2) {
                        lVar9 = (long)*(int *)(uVar26 + 2);
                      }
                      else {
                        lVar9 = FUN_1000a0ad8(uVar26);
                      }
                    }
                    else {
                      lVar9 = 2;
                    }
                    if (*(long *)(lVar20 + 0x27d0) == lVar9 + 1) {
                      pcVar17 = *(code **)(lVar20 + 0x27d8);
                    }
                    else {
                      pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x27d0,uVar26,0xabd2b0244154f858,
                                                      &DAT_1002bd3c0,0x10);
                    }
                    puVar13 = &local_1d0;
                    goto LAB_1001efcfc;
                  }
                  if (lVar9 - 0x229U < 2) {
                    uVar12 = 0x74;
                    goto LAB_1001ee35c;
                  }
                  local_3b0 = 0xe9;
                  local_3a8 = 0;
                  local_1e0 = 0xe9;
                  local_1d8 = 0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                }
                if (*(long *)(lVar20 + 0x27e0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x27e8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x27e0,param_1,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                puVar13 = &local_1e0;
                goto LAB_1001efcfc;
              }
LAB_1001edcbc:
              if (bVar23) {
                lVar9 = 1;
LAB_1001edfdc:
                local_3a8 = 0;
                local_3b0 = 0xeb;
                local_260 = 0xeb;
                uStack_258 = 0;
LAB_1001edfe0:
                if (*(long *)(lVar20 + 0x2860) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2868);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2860,param_1,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(param_1,&local_260,&DAT_1002dab28);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
                if (bVar23) {
                  lVar9 = 1;
                }
                else {
                  if ((param_1 & 1) == 0) goto LAB_1001ef7ac;
                  lVar9 = 2;
                }
                local_3b0 = 0x61;
                local_3a8 = 0;
                local_280 = 0x61;
                uStack_278 = 0;
LAB_1001ee070:
                if (*(long *)(lVar20 + 0x2880) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2888);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2880,param_1,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(param_1,&local_280,&DAT_1002dab28);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
                if (bVar23) {
                  lVar9 = 1;
                }
                else {
                  if ((param_1 & 1) == 0) goto LAB_1001ef8f8;
                  lVar9 = 2;
                }
                local_3b0 = 0x61;
                local_3a8 = 0;
                local_2a0 = 0x61;
                uStack_298 = 0;
LAB_1001ee0fc:
                if (*(long *)(lVar20 + 0x28a0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x28a8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x28a0,param_1,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(param_1,&local_2a0,&DAT_1002dab28);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
                uVar16 = uVar25 >> 4 & 0xf;
                uVar19 = uVar25 >> 4 & 0xf | 0x30;
                if (9 < uVar16) {
                  uVar19 = uVar16 + 0x57;
                }
                if (bVar23) {
                  lVar9 = 1;
                }
                else {
                  if ((param_1 & 1) == 0) goto LAB_1001efa5c;
                  lVar9 = 2;
                }
                local_3b0 = uVar19 << 1 | 1;
                local_3a8 = 0;
                local_2b8 = 0;
                local_2c0 = local_3b0;
LAB_1001ee1ac:
                if (*(long *)(lVar20 + 0x28c0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x28c8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x28c0,param_1,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(param_1,&local_2c0,&DAT_1002dab28);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
                uVar19 = uVar25 & 0xf | 0x30;
                if (9 < (uVar25 & 0xf)) {
                  uVar19 = (uVar25 & 0xf) + 0x57;
                }
                if (bVar23) {
                  lVar9 = 1;
                }
                else {
                  uVar25 = uVar19;
                  if ((param_1 & 1) == 0) goto LAB_1001efbc8;
                  lVar9 = 2;
                }
                local_3b0 = uVar19 << 1 | 1;
                local_3a8 = 0;
                uStack_2d8 = 0;
                local_2e0 = local_3b0;
              }
              else {
                if ((param_1 & 1) != 0) {
                  lVar9 = 2;
                  goto LAB_1001edfdc;
                }
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                if (1 < lVar9 - 0x222U) {
                  if (1 < lVar9 - 0x229U) {
                    local_3b0 = 0xeb;
                    local_3a8 = 0;
                    local_260 = 0xeb;
                    uStack_258 = 0;
                    if ((param_1 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(param_1 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(param_1);
                    }
                    goto LAB_1001edfe0;
                  }
                  local_3b0 = 0;
                  local_3a8 = 0;
                  FUN_1001fb478(param_1,0x75);
                  if (*pcVar10 == '\0') goto LAB_1001ef7ac;
                  goto LAB_1001eff34;
                }
                uVar19 = *(ulong *)(param_1 + 0x1e);
                local_250 = 0xeb;
                uStack_248 = 0;
                local_3a8 = 0xeb;
                local_3b0 = uVar19;
                if (uVar19 == 0) {
                  lVar9 = 1;
                }
                else if ((uVar19 & 1) == 0) {
                  if ((uVar19 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(uVar19 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(uVar19);
                  }
                }
                else {
                  lVar9 = 2;
                }
                if (*(long *)(lVar20 + 0x2850) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2858);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2850,uVar19,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(uVar19,&local_250,&DAT_1002dab28);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
LAB_1001ef7ac:
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                if (lVar9 - 0x222U < 2) {
                  uVar19 = *(ulong *)(param_1 + 0x1e);
                  local_270 = 0x61;
                  uStack_268 = 0;
                  local_3a8 = 0x61;
                  local_3b0 = uVar19;
                  if (uVar19 == 0) {
                    lVar9 = 1;
                  }
                  else if ((uVar19 & 1) == 0) {
                    if ((uVar19 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(uVar19 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(uVar19);
                    }
                  }
                  else {
                    lVar9 = 2;
                  }
                  if (*(long *)(lVar20 + 0x2870) == lVar9 + 1) {
                    pcVar17 = *(code **)(lVar20 + 0x2878);
                  }
                  else {
                    pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2870,uVar19,0xabd2b0244154f858,
                                                    &DAT_1002bd3c0,0x10);
                  }
                  (*pcVar17)(uVar19,&local_270,&DAT_1002dab28);
                  cVar2 = *pcVar10;
                }
                else {
                  if (1 < lVar9 - 0x229U) {
                    local_3b0 = 0x61;
                    local_3a8 = 0;
                    local_280 = 0x61;
                    uStack_278 = 0;
                    if ((param_1 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(param_1 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(param_1);
                    }
                    goto LAB_1001ee070;
                  }
                  local_3b0 = 0;
                  local_3a8 = 0;
                  FUN_1001fb478(param_1,0x30);
                  cVar2 = *pcVar10;
                }
                if (cVar2 != '\0') goto LAB_1001eff34;
LAB_1001ef8f8:
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                if (lVar9 - 0x222U < 2) {
                  uVar19 = *(ulong *)(param_1 + 0x1e);
                  local_290 = 0x61;
                  uStack_288 = 0;
                  local_3a8 = 0x61;
                  local_3b0 = uVar19;
                  if (uVar19 == 0) {
                    lVar9 = 1;
                  }
                  else if ((uVar19 & 1) == 0) {
                    if ((uVar19 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(uVar19 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(uVar19);
                    }
                  }
                  else {
                    lVar9 = 2;
                  }
                  if (*(long *)(lVar20 + 0x2890) == lVar9 + 1) {
                    pcVar17 = *(code **)(lVar20 + 0x2898);
                  }
                  else {
                    pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2890,uVar19,0xabd2b0244154f858,
                                                    &DAT_1002bd3c0,0x10);
                  }
                  (*pcVar17)(uVar19,&local_290,&DAT_1002dab28);
                  cVar2 = *pcVar10;
                }
                else {
                  if (1 < lVar9 - 0x229U) {
                    local_3b0 = 0x61;
                    local_3a8 = 0;
                    local_2a0 = 0x61;
                    uStack_298 = 0;
                    if ((param_1 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(param_1 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(param_1);
                    }
                    goto LAB_1001ee0fc;
                  }
                  local_3b0 = 0;
                  local_3a8 = 0;
                  FUN_1001fb478(param_1,0x30);
                  cVar2 = *pcVar10;
                }
                if (cVar2 != '\0') goto LAB_1001eff34;
                uVar16 = uVar25 >> 4 & 0xf;
                uVar19 = uVar25 >> 4 & 0xf | 0x30;
                if (9 < uVar16) {
                  uVar19 = uVar16 + 0x57;
                }
LAB_1001efa5c:
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                if (lVar9 - 0x222U < 2) {
                  uVar16 = *(ulong *)(param_1 + 0x1e);
                  local_3a8 = uVar19 << 1 | 1;
                  lVar9 = 1;
                  uStack_2a8 = 0;
                  local_3b0 = uVar16;
                  local_2b0 = local_3a8;
                  if (uVar16 != 0) {
                    if ((uVar16 & 1) == 0) {
                      if ((uVar16 & 0x8000000000000006) == 2) {
                        lVar9 = (long)*(int *)(uVar16 + 2);
                      }
                      else {
                        lVar9 = FUN_1000a0ad8(uVar16);
                      }
                    }
                    else {
                      lVar9 = 2;
                    }
                  }
                  if (*(long *)(lVar20 + 0x28b0) == lVar9 + 1) {
                    pcVar17 = *(code **)(lVar20 + 0x28b8);
                  }
                  else {
                    pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x28b0,uVar16,0xabd2b0244154f858,
                                                    &DAT_1002bd3c0,0x10);
                  }
                  (*pcVar17)(uVar16,&local_2b0,&DAT_1002dab28);
                  cVar2 = *pcVar10;
                }
                else {
                  if (1 < lVar9 - 0x229U) {
                    local_3b0 = uVar19 << 1 | 1;
                    local_3a8 = 0;
                    local_2b8 = 0;
                    local_2c0 = local_3b0;
                    if ((param_1 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(param_1 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(param_1);
                    }
                    goto LAB_1001ee1ac;
                  }
                  local_3b0 = 0;
                  local_3a8 = 0;
                  FUN_1001fb478(param_1,uVar19);
                  cVar2 = *pcVar10;
                }
                if (cVar2 != '\0') goto LAB_1001eff34;
                uVar19 = uVar25 & 0xf;
                uVar25 = uVar25 & 0xf | 0x30;
                if (9 < uVar19) {
                  uVar25 = uVar19 + 0x57;
                }
LAB_1001efbc8:
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                if (lVar9 - 0x222U < 2) {
                  uVar26 = *(ulong *)(param_1 + 0x1e);
                  local_3a8 = uVar25 << 1 | 1;
                  lVar9 = 1;
                  uStack_2c8 = 0;
                  local_3b0 = uVar26;
                  local_2d0 = local_3a8;
                  if (uVar26 != 0) {
                    if ((uVar26 & 1) == 0) {
                      if ((uVar26 & 0x8000000000000006) == 2) {
                        lVar9 = (long)*(int *)(uVar26 + 2);
                      }
                      else {
                        lVar9 = FUN_1000a0ad8(uVar26);
                      }
                    }
                    else {
                      lVar9 = 2;
                    }
                  }
                  if (*(long *)(lVar20 + 0x28d0) == lVar9 + 1) {
                    pcVar17 = *(code **)(lVar20 + 0x28d8);
                  }
                  else {
                    pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x28d0,uVar26,0xabd2b0244154f858,
                                                    &DAT_1002bd3c0,0x10);
                  }
                  puVar13 = &local_2d0;
                  goto LAB_1001efcfc;
                }
                if (lVar9 - 0x229U < 2) goto LAB_1001efc0c;
                local_3b0 = uVar25 << 1 | 1;
                local_3a8 = 0;
                uStack_2d8 = 0;
                local_2e0 = local_3b0;
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
              }
              if (*(long *)(lVar20 + 0x28e0) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x28e8);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x28e0,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              puVar13 = &local_2e0;
              goto LAB_1001efcfc;
            }
            if (bVar23) {
              lVar9 = 1;
LAB_1001ede94:
              local_3b0 = 0xc5;
              local_3a8 = 0;
              local_1c0 = 0xc5;
              local_1b8 = 0;
            }
            else {
              if ((param_1 & 1) != 0) {
                lVar9 = 2;
                goto LAB_1001ede94;
              }
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar26 = *(ulong *)(param_1 + 0x1e);
                local_1b0 = 0xc5;
                uStack_1a8 = 0;
                local_3a8 = 0xc5;
                local_3b0 = uVar26;
                if (uVar26 == 0) {
                  lVar9 = 1;
                }
                else if ((uVar26 & 1) == 0) {
                  if ((uVar26 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(uVar26 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(uVar26);
                  }
                }
                else {
                  lVar9 = 2;
                }
                if (*(long *)(lVar20 + 0x27b0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x27b8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x27b0,uVar26,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                puVar13 = &local_1b0;
                goto LAB_1001efcfc;
              }
              if (lVar9 - 0x229U < 2) {
                uVar12 = 0x62;
                goto LAB_1001ee35c;
              }
              local_3b0 = 0xc5;
              local_3a8 = 0;
              local_1c0 = 0xc5;
              local_1b8 = 0;
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
            }
            if (*(long *)(lVar20 + 0x27c0) == lVar9 + 1) {
              pcVar17 = *(code **)(lVar20 + 0x27c8);
            }
            else {
              pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x27c0,param_1,0xabd2b0244154f858,
                                              &DAT_1002bd3c0,0x10);
            }
            puVar13 = &local_1c0;
LAB_1001efcfc:
            (*pcVar17)(uVar26,puVar13,&DAT_1002dab28);
          }
          else {
            if (uVar25 != 10) {
              if (uVar25 != 0xc) {
                if (uVar25 != 0xd) goto LAB_1001edcbc;
                if (bVar23) {
                  lVar9 = 1;
LAB_1001edf00:
                  local_3b0 = 0xe5;
                  local_3a8 = 0;
                  local_240 = 0xe5;
                  local_238 = 0;
                }
                else {
                  if ((param_1 & 1) != 0) {
                    lVar9 = 2;
                    goto LAB_1001edf00;
                  }
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  if (lVar9 - 0x222U < 2) {
                    uVar26 = *(ulong *)(param_1 + 0x1e);
                    local_230 = 0xe5;
                    uStack_228 = 0;
                    local_3a8 = 0xe5;
                    local_3b0 = uVar26;
                    if (uVar26 == 0) {
                      lVar9 = 1;
                    }
                    else if ((uVar26 & 1) == 0) {
                      if ((uVar26 & 0x8000000000000006) == 2) {
                        lVar9 = (long)*(int *)(uVar26 + 2);
                      }
                      else {
                        lVar9 = FUN_1000a0ad8(uVar26);
                      }
                    }
                    else {
                      lVar9 = 2;
                    }
                    if (*(long *)(lVar20 + 0x2830) == lVar9 + 1) {
                      pcVar17 = *(code **)(lVar20 + 0x2838);
                    }
                    else {
                      pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2830,uVar26,0xabd2b0244154f858,
                                                      &DAT_1002bd3c0,0x10);
                    }
                    puVar13 = &local_230;
                    goto LAB_1001efcfc;
                  }
                  if (lVar9 - 0x229U < 2) {
                    uVar12 = 0x72;
                    goto LAB_1001ee35c;
                  }
                  local_3b0 = 0xe5;
                  local_3a8 = 0;
                  local_240 = 0xe5;
                  local_238 = 0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                }
                if (*(long *)(lVar20 + 0x2840) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2848);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2840,param_1,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                puVar13 = &local_240;
                goto LAB_1001efcfc;
              }
              if (bVar23) {
                lVar9 = 1;
LAB_1001ede28:
                local_3b0 = 0xcd;
                local_3a8 = 0;
                local_220 = 0xcd;
                local_218 = 0;
              }
              else {
                if ((param_1 & 1) != 0) {
                  lVar9 = 2;
                  goto LAB_1001ede28;
                }
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                if (lVar9 - 0x222U < 2) {
                  uVar26 = *(ulong *)(param_1 + 0x1e);
                  local_210 = 0xcd;
                  uStack_208 = 0;
                  local_3a8 = 0xcd;
                  local_3b0 = uVar26;
                  if (uVar26 == 0) {
                    lVar9 = 1;
                  }
                  else if ((uVar26 & 1) == 0) {
                    if ((uVar26 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(uVar26 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(uVar26);
                    }
                  }
                  else {
                    lVar9 = 2;
                  }
                  if (*(long *)(lVar20 + 0x2810) == lVar9 + 1) {
                    pcVar17 = *(code **)(lVar20 + 0x2818);
                  }
                  else {
                    pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2810,uVar26,0xabd2b0244154f858,
                                                    &DAT_1002bd3c0,0x10);
                  }
                  puVar13 = &local_210;
                  goto LAB_1001efcfc;
                }
                if (lVar9 - 0x229U < 2) {
                  uVar12 = 0x66;
                  goto LAB_1001ee35c;
                }
                local_3b0 = 0xcd;
                local_3a8 = 0;
                local_220 = 0xcd;
                local_218 = 0;
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
              }
              if (*(long *)(lVar20 + 0x2820) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2828);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2820,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              puVar13 = &local_220;
              goto LAB_1001efcfc;
            }
            if (bVar23) {
              lVar9 = 1;
LAB_1001eddbc:
              local_3b0 = 0xdd;
              local_3a8 = 0;
              local_200 = 0xdd;
              local_1f8 = 0;
LAB_1001eddcc:
              if (*(long *)(lVar20 + 0x2800) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2808);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2800,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              puVar13 = &local_200;
              goto LAB_1001efcfc;
            }
            if ((param_1 & 1) != 0) {
              lVar9 = 2;
              goto LAB_1001eddbc;
            }
            if ((param_1 & 0x8000000000000006) == 2) {
              lVar9 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar9 = FUN_1000a0ad8(param_1);
            }
            if (lVar9 - 0x222U < 2) {
              uVar26 = *(ulong *)(param_1 + 0x1e);
              local_1f0 = 0xdd;
              uStack_1e8 = 0;
              local_3a8 = 0xdd;
              local_3b0 = uVar26;
              if (uVar26 == 0) {
                lVar9 = 1;
              }
              else if ((uVar26 & 1) == 0) {
                if ((uVar26 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(uVar26 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(uVar26);
                }
              }
              else {
                lVar9 = 2;
              }
              if (*(long *)(lVar20 + 0x27f0) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x27f8);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x27f0,uVar26,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              puVar13 = &local_1f0;
              goto LAB_1001efcfc;
            }
            if (1 < lVar9 - 0x229U) {
              local_3b0 = 0xdd;
              local_3a8 = 0;
              local_200 = 0xdd;
              local_1f8 = 0;
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              goto LAB_1001eddcc;
            }
            uVar12 = 0x6e;
LAB_1001ee35c:
            local_3a8 = 0;
            local_3b0 = 0;
            FUN_1001fb478(param_1,uVar12);
          }
          cVar2 = *pcVar10;
        }
        else {
          if ((uVar25 != 0x22) && (uVar25 != 0x5c)) goto LAB_1001efd10;
          if (local_3e0 < (long)uVar24) {
            if (!bVar6 || !bVar5) {
LAB_1001ed238:
              local_3b0 = 0;
              local_3a8 = 0;
              if (local_3e0 + 0x4000000000000000 < 0) {
                pcVar8 = pcVar21;
                if (bVar4) {
                  pcVar8 = (char *)thunk_FUN_10009c0a0();
                }
                puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                  uVar19 = FUN_10009b470(3,1,0x102);
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                  uVar19 = (long)puVar11 + 2;
                  *puVar11 = 0x300010201;
                }
                *(long *)(uVar19 + 0xe) = local_3e0;
              }
              else {
                uVar19 = local_3e0 << 1 | 1;
              }
              uVar16 = uVar22;
              local_3b0 = uVar19;
              if ((long)(uVar24 + 0x4000000000000000) < 0) {
                pcVar8 = pcVar21;
                if (bVar4) {
                  pcVar8 = (char *)thunk_FUN_10009c0a0();
                }
                puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                  uVar16 = FUN_10009b470(3,1,0x102);
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                  uVar16 = (long)puVar11 + 2;
                  *puVar11 = 0x300010201;
                }
                *(ulong *)(uVar16 + 0xe) = uVar24;
              }
              uStack_2f8 = 0;
              lVar9 = local_3f8;
              local_3a8 = uVar16;
              local_310 = param_2;
              uStack_308 = uVar19;
              local_300 = uVar16;
              if (bVar3) {
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
              }
              if (*(long *)(lVar20 + 0x2900) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2908);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2900,param_1,0x8c8882b1249d8dde,
                                                "tc:writeStringSlice",0x13);
              }
              (*pcVar17)(param_1,&local_310,&DAT_1002db240);
              if (*pcVar10 == '\0') goto LAB_1001ed764;
              goto LAB_1001eff34;
            }
            if ((param_1 & 0x8000000000000006) == 2) {
              lVar9 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar9 = FUN_1000a0ad8(param_1);
            }
            if (lVar9 - 0x222U < 2) {
              uVar16 = *(ulong *)(param_1 + 0x1e);
              local_3a8 = 0;
              uVar19 = uVar22;
              local_3b0 = uVar16;
              if ((long)(uVar24 + 0x4000000000000000) < 0) {
                pcVar8 = pcVar21;
                if (bVar4) {
                  pcVar8 = (char *)thunk_FUN_10009c0a0();
                }
                puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                  uVar19 = FUN_10009b470(3,1,0x102);
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                  uVar19 = (long)puVar11 + 2;
                  *puVar11 = 0x300010201;
                }
                *(ulong *)(uVar19 + 0xe) = uVar24;
              }
              local_3a8 = uVar19;
              local_3a8 = FUN_100164988(param_2,local_3e0);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              uStack_2e8 = 0;
              local_2f0 = local_3a8;
              if (uVar16 == 0) {
                lVar9 = 1;
              }
              else if ((uVar16 & 1) == 0) {
                if ((uVar16 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(uVar16 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(uVar16);
                }
              }
              else {
                lVar9 = 2;
              }
              if (*(long *)(lVar20 + 0x28f0) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x28f8);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x28f0,uVar16,0xcece15d88a54e5a9,"tc:write"
                                                ,8);
              }
              (*pcVar17)(uVar16,&local_2f0,&DAT_1002dab28);
            }
            else {
              if (1 < lVar9 - 0x229U) goto LAB_1001ed238;
              local_3b0 = 0;
              local_3a8 = 0;
              FUN_1001fabcc(param_1,param_2,local_3e0,uVar24);
            }
            if (*pcVar10 != '\0') goto LAB_1001eff34;
LAB_1001ee568:
            if ((param_1 & 0x8000000000000006) == 2) {
              lVar9 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar9 = FUN_1000a0ad8(param_1);
            }
            if (lVar9 - 0x222U < 2) {
              uVar19 = *(ulong *)(param_1 + 0x1e);
              local_320 = 0xb9;
              uStack_318 = 0;
              local_3a8 = 0xb9;
              local_3b0 = uVar19;
              if (uVar19 == 0) {
                lVar9 = 1;
              }
              else if ((uVar19 & 1) == 0) {
                if ((uVar19 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(uVar19 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(uVar19);
                }
              }
              else {
                lVar9 = 2;
              }
              if (*(long *)(lVar20 + 0x2910) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2918);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2910,uVar19,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(uVar19,&local_320,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              bVar23 = false;
            }
            else {
              if (1 < lVar9 - 0x229U) {
                local_3b0 = 0xb9;
                local_3a8 = 0;
                local_330 = 0xb9;
                uStack_328 = 0;
                if ((param_1 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(param_1 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(param_1);
                }
                bVar23 = false;
                goto LAB_1001ed7b0;
              }
              local_3b0 = 0;
              local_3a8 = 0;
              FUN_1001fb478(param_1,0x5c);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              bVar23 = false;
            }
          }
          else {
LAB_1001ed764:
            if (param_1 == 0) {
              bVar23 = true;
              lVar9 = 1;
            }
            else {
              if ((param_1 & 1) == 0) goto LAB_1001ee568;
              bVar23 = false;
              lVar9 = 2;
            }
            local_3a8 = 0;
            local_3b0 = 0xb9;
            local_330 = 0xb9;
            uStack_328 = 0;
LAB_1001ed7b0:
            if (*(long *)(lVar20 + 0x2920) == lVar9 + 1) {
              pcVar17 = *(code **)(lVar20 + 0x2928);
            }
            else {
              pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2920,param_1,0xabd2b0244154f858,
                                              &DAT_1002bd3c0,0x10);
            }
            (*pcVar17)(param_1,&local_330,&DAT_1002dab28);
            if (*pcVar10 != '\0') goto LAB_1001eff34;
          }
          local_3e0 = uVar24 + 1;
          if (bVar23 || (param_1 & 1) != 0) {
LAB_1001ee6d4:
            local_3b0 = uVar25 << 1 | 1;
            lVar9 = 1;
            if (!bVar23) {
              lVar9 = 2;
            }
            local_3a8 = 0;
            uStack_348 = 0;
            local_350 = local_3b0;
            if (!bVar23 && (param_1 & 1) == 0) {
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
            }
            if (*(long *)(lVar20 + 0x2940) == lVar9 + 1) {
              pcVar17 = *(code **)(lVar20 + 0x2948);
            }
            else {
              pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2940,param_1,0xabd2b0244154f858,
                                              &DAT_1002bd3c0,0x10);
            }
            puVar13 = &local_350;
            goto LAB_1001efcfc;
          }
          if ((param_1 & 0x8000000000000006) == 2) {
            lVar9 = (long)*(int *)(param_1 + 2);
          }
          else {
            lVar9 = FUN_1000a0ad8(param_1);
          }
          if (lVar9 - 0x222U < 2) {
            uVar26 = *(ulong *)(param_1 + 0x1e);
            local_3a8 = uVar25 << 1 | 1;
            lVar9 = 1;
            uStack_338 = 0;
            local_3b0 = uVar26;
            local_340 = local_3a8;
            if (uVar26 != 0) {
              if ((uVar26 & 1) == 0) {
                if ((uVar26 & 0x8000000000000006) == 2) {
                  lVar9 = (long)*(int *)(uVar26 + 2);
                }
                else {
                  lVar9 = FUN_1000a0ad8(uVar26);
                }
              }
              else {
                lVar9 = 2;
              }
            }
            if (*(long *)(lVar20 + 0x2930) == lVar9 + 1) {
              pcVar17 = *(code **)(lVar20 + 0x2938);
            }
            else {
              pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2930,uVar26,0xabd2b0244154f858,
                                              &DAT_1002bd3c0,0x10);
            }
            (*pcVar17)(uVar26,&local_340,&DAT_1002dab28);
            if (*pcVar10 == '\0') goto LAB_1001efd10;
            goto LAB_1001eff34;
          }
          if (1 < lVar9 - 0x229U) goto LAB_1001ee6d4;
LAB_1001efc0c:
          local_3b0 = 0;
          local_3a8 = 0;
          FUN_1001fb478(param_1,uVar25);
          cVar2 = *pcVar10;
        }
        local_3e0 = uVar24 + 1;
        if (cVar2 != '\0') goto LAB_1001eff34;
      }
      else {
        if (0x1a < uVar25 >> 0xb) {
          uVar1 = (uint)uVar25 & 0xfc00;
          if (uVar1 == 0xd800) {
            if ((long)(uVar24 + 1) < (long)uVar27) {
              if (uVar24 + 1 < *(ulong *)(param_2 + 0xe)) {
                if (*(int *)(param_2 + 2) == 6) goto LAB_1001ed290;
                uVar7 = *(ushort *)(param_2 + uVar24 * 2 + 0x18);
              }
              else {
                local_3b0 = 0;
                local_3a8 = 0;
                uVar7 = FUN_1000a9d2c(param_2,uVar24 + 1);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
              }
              uVar18 = 0xdc00;
              goto LAB_1001ed278;
            }
LAB_1001ed290:
            if (local_3e0 < (long)uVar24) {
              if (!bVar6 || !bVar5) {
LAB_1001ed57c:
                local_3b0 = 0;
                local_3a8 = 0;
                if (local_3e0 + 0x4000000000000000 < 0) {
                  pcVar8 = pcVar21;
                  if (bVar4) {
                    pcVar8 = (char *)thunk_FUN_10009c0a0();
                  }
                  puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                  if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                    uVar19 = FUN_10009b470(3,1,0x102);
                  }
                  else {
                    *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                    uVar19 = (long)puVar11 + 2;
                    *puVar11 = 0x300010201;
                  }
                  *(long *)(uVar19 + 0xe) = local_3e0;
                }
                else {
                  uVar19 = local_3e0 << 1 | 1;
                }
                uVar16 = uVar22;
                local_3b0 = uVar19;
                if ((long)(uVar24 + 0x4000000000000000) < 0) {
                  pcVar8 = pcVar21;
                  if (bVar4) {
                    pcVar8 = (char *)thunk_FUN_10009c0a0();
                  }
                  puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                  if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                    uVar16 = FUN_10009b470(3,1,0x102);
                  }
                  else {
                    *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                    uVar16 = (long)puVar11 + 2;
                    *puVar11 = 0x300010201;
                  }
                  *(ulong *)(uVar16 + 0xe) = uVar24;
                }
                local_78 = 0;
                lVar9 = local_3f8;
                local_3a8 = uVar16;
                local_90 = param_2;
                local_88 = uVar19;
                local_80 = uVar16;
                if (bVar3) {
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                }
                if (*(long *)(lVar20 + 0x26a0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x26a8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x26a0,param_1,0x8c8882b1249d8dde,
                                                  "tc:writeStringSlice",0x13);
                }
                (*pcVar17)(param_1,&local_90,&DAT_1002db240);
                if (*pcVar10 == '\0') goto LAB_1001ed880;
                goto LAB_1001eff34;
              }
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar16 = *(ulong *)(param_1 + 0x1e);
                local_3a8 = 0;
                uVar19 = uVar22;
                local_3b0 = uVar16;
                if ((long)(uVar24 + 0x4000000000000000) < 0) {
                  pcVar8 = pcVar21;
                  if (bVar4) {
                    pcVar8 = (char *)thunk_FUN_10009c0a0();
                  }
                  puVar11 = *(undefined8 **)(pcVar8 + 0x50);
                  if (*(undefined8 **)(pcVar8 + 0x58) < puVar11 + 3) {
                    uVar19 = FUN_10009b470(3,1,0x102);
                  }
                  else {
                    *(undefined8 **)(pcVar8 + 0x50) = puVar11 + 3;
                    uVar19 = (long)puVar11 + 2;
                    *puVar11 = 0x300010201;
                  }
                  *(ulong *)(uVar19 + 0xe) = uVar24;
                }
                local_3a8 = uVar19;
                local_3a8 = FUN_100164988(param_2,local_3e0);
                if (*pcVar10 != '\0') goto LAB_1001eff34;
                local_68 = 0;
                local_70 = local_3a8;
                if (uVar16 == 0) {
                  lVar9 = 1;
                }
                else if ((uVar16 & 1) == 0) {
                  if ((uVar16 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(uVar16 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(uVar16);
                  }
                }
                else {
                  lVar9 = 2;
                }
                if (*(long *)(lVar20 + 0x2690) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2698);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2690,uVar16,0xcece15d88a54e5a9,
                                                  "tc:write",8);
                }
                (*pcVar17)(uVar16,&local_70,&DAT_1002dab28);
                cVar2 = *pcVar10;
              }
              else {
                if (1 < lVar9 - 0x229U) goto LAB_1001ed57c;
                local_3b0 = 0;
                local_3a8 = 0;
                FUN_1001fabcc(param_1,param_2,local_3e0,uVar24);
                cVar2 = *pcVar10;
              }
              if (cVar2 != '\0') goto LAB_1001eff34;
LAB_1001ee9f0:
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar19 = *(ulong *)(param_1 + 0x1e);
                local_a0 = 0xb9;
                local_3a8 = 0xb9;
                local_98 = 0;
                local_3b0 = uVar19;
                if (uVar19 == 0) {
                  lVar9 = 1;
                }
                else if ((uVar19 & 1) == 0) {
                  if ((uVar19 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(uVar19 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(uVar19);
                  }
                }
                else {
                  lVar9 = 2;
                }
                if (*(long *)(lVar20 + 0x26b0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x26b8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x26b0,uVar19,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(uVar19,&local_a0,&DAT_1002dab28);
              }
              else {
                if (1 < lVar9 - 0x229U) {
                  local_3b0 = 0xb9;
                  local_3a8 = 0;
                  local_b0 = 0xb9;
                  local_a8 = 0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  bVar23 = false;
                  goto LAB_1001ed8d8;
                }
                local_3b0 = 0;
                local_3a8 = 0;
                FUN_1001fb478(param_1,0x5c);
              }
              if (*pcVar10 != '\0') goto LAB_1001eff34;
LAB_1001eeb48:
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar19 = *(ulong *)(param_1 + 0x1e);
                local_c0 = 0xeb;
                local_3a8 = 0xeb;
                local_b8 = 0;
                local_3b0 = uVar19;
                if (uVar19 == 0) {
                  lVar9 = 1;
                }
                else if ((uVar19 & 1) == 0) {
                  if ((uVar19 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(uVar19 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(uVar19);
                  }
                }
                else {
                  lVar9 = 2;
                }
                if (*(long *)(lVar20 + 0x26d0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x26d8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x26d0,uVar19,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(uVar19,&local_c0,&DAT_1002dab28);
                cVar2 = *pcVar10;
              }
              else {
                if (1 < lVar9 - 0x229U) {
                  local_3b0 = 0xeb;
                  local_3a8 = 0;
                  local_d0 = 0xeb;
                  local_c8 = 0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  bVar23 = false;
                  goto LAB_1001ed97c;
                }
                local_3b0 = 0;
                local_3a8 = 0;
                FUN_1001fb478(param_1,0x75);
                cVar2 = *pcVar10;
              }
              if (cVar2 != '\0') goto LAB_1001eff34;
LAB_1001eec9c:
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar19 = *(ulong *)(param_1 + 0x1e);
                local_e0 = 0xc9;
                local_3a8 = 0xc9;
                local_d8 = 0;
                local_3b0 = uVar19;
                if (uVar19 == 0) {
                  lVar9 = 1;
                }
                else if ((uVar19 & 1) == 0) {
                  if ((uVar19 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(uVar19 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(uVar19);
                  }
                }
                else {
                  lVar9 = 2;
                }
                if (*(long *)(lVar20 + 0x26f0) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x26f8);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x26f0,uVar19,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(uVar19,&local_e0,&DAT_1002dab28);
                cVar2 = *pcVar10;
              }
              else {
                if (1 < lVar9 - 0x229U) {
                  local_3b0 = 0xc9;
                  local_3a8 = 0;
                  local_f0 = 0xc9;
                  local_e8 = 0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  bVar23 = false;
                  goto LAB_1001eed7c;
                }
                local_3b0 = 0;
                local_3a8 = 0;
                FUN_1001fb478(param_1,100);
                cVar2 = *pcVar10;
              }
              if (cVar2 != '\0') goto LAB_1001eff34;
              uVar16 = uVar25 >> 8 & 0xf;
              uVar19 = uVar25 >> 8 & 0xf | 0x30;
              if (9 < uVar16) {
                uVar19 = uVar16 + 0x57;
              }
LAB_1001eeec8:
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar16 = *(ulong *)(param_1 + 0x1e);
                local_3a8 = uVar19 << 1 | 1;
                lVar9 = 1;
                local_f8 = 0;
                local_3b0 = uVar16;
                local_100 = local_3a8;
                if (uVar16 != 0) {
                  if ((uVar16 & 1) == 0) {
                    if ((uVar16 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(uVar16 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(uVar16);
                    }
                  }
                  else {
                    lVar9 = 2;
                  }
                }
                if (*(long *)(lVar20 + 10000) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2718);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 10000,uVar16,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(uVar16,&local_100,&DAT_1002dab28);
                cVar2 = *pcVar10;
              }
              else {
                if (1 < lVar9 - 0x229U) {
                  local_3b0 = uVar19 << 1 | 1;
                  local_3a8 = 0;
                  local_108 = 0;
                  local_110 = local_3b0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  bVar23 = false;
                  goto LAB_1001eefb0;
                }
                local_3b0 = 0;
                local_3a8 = 0;
                FUN_1001fb478(param_1,uVar19);
                cVar2 = *pcVar10;
              }
              if (cVar2 != '\0') goto LAB_1001eff34;
              uVar16 = uVar25 >> 4 & 0xf;
              uVar19 = uVar25 >> 4 & 0xf | 0x30;
              if (9 < uVar16) {
                uVar19 = uVar16 + 0x57;
              }
LAB_1001ef0f8:
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar16 = *(ulong *)(param_1 + 0x1e);
                local_3a8 = uVar19 << 1 | 1;
                lVar9 = 1;
                local_118 = 0;
                local_3b0 = uVar16;
                local_120 = local_3a8;
                if (uVar16 != 0) {
                  if ((uVar16 & 1) == 0) {
                    if ((uVar16 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(uVar16 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(uVar16);
                    }
                  }
                  else {
                    lVar9 = 2;
                  }
                }
                if (*(long *)(lVar20 + 0x2730) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2738);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2730,uVar16,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                (*pcVar17)(uVar16,&local_120,&DAT_1002dab28);
                cVar2 = *pcVar10;
              }
              else {
                if (1 < lVar9 - 0x229U) {
                  local_3b0 = uVar19 << 1 | 1;
                  local_3a8 = 0;
                  local_128 = 0;
                  local_130 = local_3b0;
                  if ((param_1 & 0x8000000000000006) == 2) {
                    lVar9 = (long)*(int *)(param_1 + 2);
                  }
                  else {
                    lVar9 = FUN_1000a0ad8(param_1);
                  }
                  bVar23 = false;
                  goto LAB_1001ef1e0;
                }
                local_3b0 = 0;
                local_3a8 = 0;
                FUN_1001fb478(param_1,uVar19);
                cVar2 = *pcVar10;
              }
              if (cVar2 != '\0') goto LAB_1001eff34;
              uVar19 = uVar25 & 0xf;
              uVar25 = uVar25 & 0xf | 0x30;
              if (9 < uVar19) {
                uVar25 = uVar19 + 0x57;
              }
LAB_1001ef328:
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
              if (lVar9 - 0x222U < 2) {
                uVar26 = *(ulong *)(param_1 + 0x1e);
                local_3a8 = uVar25 << 1 | 1;
                lVar9 = 1;
                local_138 = 0;
                local_3b0 = uVar26;
                local_140 = local_3a8;
                if (uVar26 != 0) {
                  if ((uVar26 & 1) == 0) {
                    if ((uVar26 & 0x8000000000000006) == 2) {
                      lVar9 = (long)*(int *)(uVar26 + 2);
                    }
                    else {
                      lVar9 = FUN_1000a0ad8(uVar26);
                    }
                  }
                  else {
                    lVar9 = 2;
                  }
                }
                if (*(long *)(lVar20 + 0x2750) == lVar9 + 1) {
                  pcVar17 = *(code **)(lVar20 + 0x2758);
                }
                else {
                  pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2750,uVar26,0xabd2b0244154f858,
                                                  &DAT_1002bd3c0,0x10);
                }
                puVar13 = &local_140;
                goto LAB_1001efcfc;
              }
              if (lVar9 - 0x229U < 2) goto LAB_1001efc0c;
              local_3b0 = uVar25 << 1 | 1;
              local_3a8 = 0;
              local_148 = 0;
              local_150 = local_3b0;
              if ((param_1 & 0x8000000000000006) == 2) {
                lVar9 = (long)*(int *)(param_1 + 2);
              }
              else {
                lVar9 = FUN_1000a0ad8(param_1);
              }
            }
            else {
LAB_1001ed880:
              if (param_1 == 0) {
                bVar23 = true;
                lVar9 = 1;
              }
              else {
                if ((param_1 & 1) == 0) goto LAB_1001ee9f0;
                bVar23 = false;
                lVar9 = 2;
              }
              local_3a8 = 0;
              local_3b0 = 0xb9;
              local_b0 = 0xb9;
              local_a8 = 0;
LAB_1001ed8d8:
              if (*(long *)(lVar20 + 0x26c0) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x26c8);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x26c0,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(param_1,&local_b0,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              if (bVar23) {
                bVar23 = true;
                lVar9 = 1;
              }
              else {
                if ((param_1 & 1) == 0) goto LAB_1001eeb48;
                bVar23 = false;
                lVar9 = 2;
              }
              local_3a8 = 0;
              local_3b0 = 0xeb;
              local_d0 = 0xeb;
              local_c8 = 0;
LAB_1001ed97c:
              if (*(long *)(lVar20 + 0x26e0) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x26e8);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x26e0,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(param_1,&local_d0,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              if (bVar23) {
                bVar23 = true;
                lVar9 = 1;
              }
              else {
                if ((param_1 & 1) == 0) goto LAB_1001eec9c;
                bVar23 = false;
                lVar9 = 2;
              }
              local_3a8 = 0;
              local_3b0 = 0xc9;
              local_f0 = 0xc9;
              local_e8 = 0;
LAB_1001eed7c:
              if (*(long *)(lVar20 + 0x2700) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2708);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2700,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(param_1,&local_f0,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              uVar16 = uVar25 >> 8 & 0xf;
              uVar19 = uVar25 >> 8 & 0xf | 0x30;
              if (9 < uVar16) {
                uVar19 = uVar16 + 0x57;
              }
              if (bVar23) {
                bVar23 = true;
                lVar9 = 1;
              }
              else {
                if ((param_1 & 1) == 0) goto LAB_1001eeec8;
                bVar23 = false;
                lVar9 = 2;
              }
              local_3b0 = uVar19 << 1 | 1;
              local_108 = 0;
              local_3a8 = 0;
              local_110 = local_3b0;
LAB_1001eefb0:
              if (*(long *)(lVar20 + 0x2720) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2728);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2720,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(param_1,&local_110,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              uVar16 = uVar25 >> 4 & 0xf;
              uVar19 = uVar25 >> 4 & 0xf | 0x30;
              if (9 < uVar16) {
                uVar19 = uVar16 + 0x57;
              }
              if (bVar23) {
                bVar23 = true;
                lVar9 = 1;
              }
              else {
                if ((param_1 & 1) == 0) goto LAB_1001ef0f8;
                bVar23 = false;
                lVar9 = 2;
              }
              local_3b0 = uVar19 << 1 | 1;
              local_128 = 0;
              local_3a8 = 0;
              local_130 = local_3b0;
LAB_1001ef1e0:
              if (*(long *)(lVar20 + 0x2740) == lVar9 + 1) {
                pcVar17 = *(code **)(lVar20 + 0x2748);
              }
              else {
                pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2740,param_1,0xabd2b0244154f858,
                                                &DAT_1002bd3c0,0x10);
              }
              (*pcVar17)(param_1,&local_130,&DAT_1002dab28);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
              uVar19 = uVar25 & 0xf | 0x30;
              if (9 < (uVar25 & 0xf)) {
                uVar19 = (uVar25 & 0xf) + 0x57;
              }
              if (bVar23) {
                lVar9 = 1;
              }
              else {
                uVar25 = uVar19;
                if ((param_1 & 1) == 0) goto LAB_1001ef328;
                lVar9 = 2;
              }
              local_3b0 = uVar19 << 1 | 1;
              local_3a8 = 0;
              local_148 = 0;
              local_150 = local_3b0;
            }
            if (*(long *)(lVar20 + 0x2760) == lVar9 + 1) {
              pcVar17 = *(code **)(lVar20 + 0x2768);
            }
            else {
              pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2760,param_1,0xabd2b0244154f858,
                                              &DAT_1002bd3c0,0x10);
            }
            puVar13 = &local_150;
            goto LAB_1001efcfc;
          }
          if (uVar1 == 0xdc00) {
            if (uVar24 == 0) goto LAB_1001ed290;
            if (uVar24 - 1 < *(ulong *)(param_2 + 0xe)) {
              if (*(int *)(param_2 + 2) == 6) goto LAB_1001ed290;
              uVar7 = *(ushort *)(param_2 + uVar24 * 2 + 0x14);
            }
            else {
              local_3b0 = 0;
              local_3a8 = 0;
              uVar7 = FUN_1000a9d2c(param_2,uVar24 - 1);
              if (*pcVar10 != '\0') goto LAB_1001eff34;
            }
            uVar18 = 0xd800;
LAB_1001ed278:
            if ((uVar7 & 0xfc00) != uVar18) goto LAB_1001ed290;
          }
        }
        if (*pcVar10 != '\0') {
          FUN_10009c234();
        }
      }
LAB_1001efd10:
      uVar24 = uVar24 + 1;
      uVar22 = uVar22 + 2;
    } while (uVar27 != uVar24);
    if (local_3e0 == 0) goto LAB_1001efe98;
    if ((long)uVar27 <= local_3e0) goto LAB_1001eff34;
    if (bVar6 && bVar5) {
      if ((param_1 & 0x8000000000000006) == 2) {
        lVar9 = (long)*(int *)(param_1 + 2);
      }
      else {
        lVar9 = FUN_1000a0ad8(param_1);
      }
      if (1 < lVar9 - 0x229U) {
        if (1 < lVar9 - 0x222U) goto LAB_1001f0060;
        param_1 = *(ulong *)(param_1 + 0x1e);
        local_3b0 = 0;
        local_3a8 = 0;
        local_3c0 = param_1;
        if ((long)uVar27 < 0x4000000000000000) {
          local_3b0 = uVar27 << 1 | 1;
        }
        else {
          lVar9 = FUN_100223de4();
          puVar11 = *(undefined8 **)(lVar9 + 0x50);
          if (*(undefined8 **)(lVar9 + 0x58) < puVar11 + 3) {
            local_3b0 = FUN_10009b470(3,1,0x102);
          }
          else {
            *(undefined8 **)(lVar9 + 0x50) = puVar11 + 3;
            local_3b0 = (long)puVar11 + 2;
            *puVar11 = 0x300010201;
          }
          *(ulong *)(local_3b0 + 0xe) = uVar27;
        }
        lStack_3b8 = FUN_100164988(param_2,local_3e0);
        if (*pcVar10 != '\0') goto LAB_1001eff34;
        local_3b0 = 0;
        uStack_378 = 0;
        local_380 = lStack_3b8;
        if (param_1 == 0) {
          lVar9 = 1;
        }
        else if ((param_1 & 1) == 0) {
          if ((param_1 & 0x8000000000000006) == 2) {
            lVar9 = (long)*(int *)(param_1 + 2);
          }
          else {
            lVar9 = FUN_1000a0ad8(param_1);
          }
        }
        else {
          lVar9 = 2;
        }
        if (*(long *)(lVar20 + 0x2970) == lVar9 + 1) {
          pcVar17 = *(code **)(lVar20 + 0x2978);
        }
        else {
          pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2970,param_1,0xcece15d88a54e5a9,"tc:write",8);
        }
        puVar15 = &DAT_1002dab28;
        plVar14 = &local_380;
        goto LAB_1001eff28;
      }
LAB_1001eff94:
      local_3a8 = 0;
      local_3b0 = 0;
      FUN_1001fabcc(param_1,param_2,local_3e0,uVar27);
      goto LAB_1001eff34;
    }
LAB_1001f0060:
    local_3b0 = 0;
    local_3a8 = 0;
    if (local_3e0 + 0x4000000000000000 < 0) {
      lVar9 = FUN_100223de4();
      puVar11 = *(undefined8 **)(lVar9 + 0x50);
      if (*(undefined8 **)(lVar9 + 0x58) < puVar11 + 3) {
        uVar24 = FUN_10009b470(3,1,0x102);
      }
      else {
        *(undefined8 **)(lVar9 + 0x50) = puVar11 + 3;
        uVar24 = (long)puVar11 + 2;
        *puVar11 = 0x300010201;
      }
      *(long *)(uVar24 + 0xe) = local_3e0;
    }
    else {
      uVar24 = local_3e0 << 1 | 1;
    }
    local_3b0 = uVar24;
    if ((long)uVar27 < 0x4000000000000000) {
      local_3a8 = uVar27 << 1 | 1;
    }
    else {
      lVar9 = FUN_100223de4();
      puVar11 = *(undefined8 **)(lVar9 + 0x50);
      if (*(undefined8 **)(lVar9 + 0x58) < puVar11 + 3) {
        local_3a8 = FUN_10009b470(3,1,0x102);
      }
      else {
        *(undefined8 **)(lVar9 + 0x50) = puVar11 + 3;
        local_3a8 = (long)puVar11 + 2;
        *puVar11 = 0x300010201;
      }
      *(ulong *)(local_3a8 + 0xe) = uVar27;
    }
    local_388 = 0;
    lStack_3a0 = param_2;
    local_398 = uVar24;
    uStack_390 = local_3a8;
    if (bVar3) {
      if ((param_1 & 0x8000000000000006) == 2) {
        local_3f8 = (long)*(int *)(param_1 + 2);
      }
      else {
        local_3f8 = FUN_1000a0ad8(param_1);
      }
    }
    if (*(long *)(lVar20 + 0x2980) == local_3f8 + 1) {
      pcVar17 = *(code **)(lVar20 + 0x2988);
    }
    else {
      pcVar17 = (code *)FUN_1000b7430(lVar20 + 0x2980,param_1,0x8c8882b1249d8dde,
                                      "tc:writeStringSlice",0x13);
    }
    puVar15 = &DAT_1002db240;
    plVar14 = &lStack_3a0;
  }
LAB_1001eff28:
  (*pcVar17)(param_1,plVar14,puVar15);
LAB_1001eff34:
  *(undefined8 *)(pcVar10 + 8) = local_3d0;
  return;
}

