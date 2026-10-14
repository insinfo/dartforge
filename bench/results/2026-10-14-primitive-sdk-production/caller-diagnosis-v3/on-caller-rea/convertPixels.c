
/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

long entry(ulong param_1,ulong param_2)

{
  byte bVar1;
  long lVar2;
  long lVar3;
  undefined8 *puVar4;
  bool bVar5;
  long lVar6;
  undefined8 uVar7;
  char *pcVar8;
  char *pcVar9;
  ulong *puVar10;
  ulong uVar11;
  code *pcVar12;
  ulong uVar13;
  char *pcVar14;
  long lVar15;
  long lVar16;
  ulong uVar17;
  ulong uVar18;
  undefined1 uStack_c9;
  undefined8 local_c8;
  undefined8 local_c0;
  ulong local_b8;
  ulong local_b0;
  undefined8 local_a8;
  undefined8 local_a0;
  undefined8 local_98;
  ulong local_90 [3];
  undefined8 uStack_78;
  
  uVar18 = tpidrro_el0;
  pcVar14 = *(char **)((uVar18 & 0xfffffffffffffff8) + _DAT_100318228);
  bVar5 = _DAT_100318228 == 0 || pcVar14 == (char *)0x0;
  pcVar9 = pcVar14;
  if (bVar5) {
    pcVar9 = (char *)func_0x00010010296c();
  }
  if ((*(ulong *)(pcVar9 + 0x18) <= _DAT_1003181e0) ||
     (lVar16 = *(long *)(*(long *)(pcVar9 + 0x10) + _DAT_1003181e0 * 8), lVar16 == 0)) {
    lVar16 = func_0x00010009ed44(0x1002af9d0,&DAT_1003181e0);
  }
  pcVar9 = pcVar14;
  if (bVar5) {
    pcVar9 = (char *)func_0x00010010296c();
  }
  if (&uStack_c9 < *(undefined1 **)(pcVar9 + 0x178)) {
    func_0x0001000b7930();
    lVar15 = 0;
  }
  else {
    local_b0 = 0;
    local_a8 = 0;
    local_c0 = 4;
    local_a0 = 0;
    local_c8 = *(undefined8 *)(pcVar9 + 8);
    *(undefined8 **)(pcVar9 + 8) = &local_c8;
    local_b8 = param_1;
    if ((long)param_2 < 1) {
      if (*pcVar9 != '\0') {
        func_0x00010009c2e8();
      }
LAB_1000f41b8:
      lVar15 = 0;
    }
    else {
      lVar2 = 1;
      if (param_1 != 0) {
        lVar2 = 2;
      }
      uVar18 = 0;
      lVar15 = 0;
      uVar11 = param_1 & 0x8000000000000006;
      do {
        uVar17 = uVar18 & 0xff;
        if ((param_1 & 1) != 0 || param_1 == 0) {
          local_b0 = uVar17 << 1 | 1;
          uStack_78 = 0;
          lVar6 = lVar2;
          local_90[2] = local_b0;
LAB_1000f3dbc:
          if (*(long *)(lVar16 + 0x240) == lVar6 + 1) {
            pcVar12 = *(code **)(lVar16 + 0x248);
          }
          else {
            pcVar12 = (code *)func_0x0001000b7158(lVar16 + 0x240,param_1,0x27d543198ed41f36,
                                                  0x1002ca25f,5);
          }
          puVar10 = local_90 + 2;
LAB_1000f3e08:
          uVar17 = (*pcVar12)(param_1,puVar10,0x1002ca2c8);
          if (*pcVar9 != '\0') goto LAB_1000f41b8;
        }
        else {
          if (uVar11 == 2) {
            lVar6 = (long)*(int *)(param_1 + 2);
          }
          else {
            lVar6 = func_0x0001000a18e8(param_1);
          }
          if ((2 < lVar6 - 8U) || (*(ulong *)(param_1 + 0xe) <= uVar17)) {
            local_b0 = uVar17 << 1 | 1;
            uStack_78 = 0;
            local_90[2] = local_b0;
            if (uVar11 == 2) {
              lVar6 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar6 = func_0x0001000a18e8(param_1);
            }
            goto LAB_1000f3dbc;
          }
          if (uVar11 == 2) {
            lVar6 = (long)*(int *)(param_1 + 2);
          }
          else {
            lVar6 = func_0x0001000a18e8(param_1);
          }
          uVar13 = param_1;
          if (lVar6 == 10) {
            uVar13 = *(ulong *)(param_1 + 0x16);
          }
          lVar6 = uVar13 + 0x16;
          bVar1 = *(byte *)(uVar13 - 1) & 0x66;
          if (bVar1 < 0x42) {
            if (bVar1 == 4) {
              uVar17 = *(ulong *)(lVar6 + uVar17 * 8);
              goto LAB_1000f3e24;
            }
            if (bVar1 == 0x22) {
              lVar6 = *(long *)(lVar6 + uVar17 * 8);
              local_b0 = 0;
              if (lVar6 + 0x4000000000000000 < 0) {
                pcVar8 = pcVar14;
                if (bVar5) {
                  pcVar8 = (char *)func_0x00010010296c();
                }
                puVar4 = *(undefined8 **)(pcVar8 + 0x50);
                if (*(undefined8 **)(pcVar8 + 0x58) < puVar4 + 3) {
                  uVar17 = func_0x00010009b524(3,1,0x102);
                  *(long *)(uVar17 + 0xe) = lVar6;
                }
                else {
                  *(undefined8 **)(pcVar8 + 0x50) = puVar4 + 3;
                  uVar17 = (long)puVar4 + 2;
                  *puVar4 = 0x300010201;
                  puVar4[2] = lVar6;
                }
              }
              else {
                uVar17 = lVar6 << 1 | 1;
              }
              goto LAB_1000f3e24;
            }
LAB_1000f4058:
            local_b0 = uVar17 << 1 | 1;
            local_90[1] = 0;
            local_90[0] = local_b0;
            if (uVar11 == 2) {
              lVar6 = (long)*(int *)(param_1 + 2);
            }
            else {
              lVar6 = func_0x0001000a18e8(param_1);
            }
            if (*(long *)(lVar16 + 0x250) == lVar6 + 1) {
              pcVar12 = *(code **)(lVar16 + 600);
              puVar10 = local_90;
            }
            else {
              pcVar12 = (code *)func_0x0001000b7158(lVar16 + 0x250,param_1,0x27d543198ed41f36,
                                                    0x1002ca25f,5);
              puVar10 = local_90;
            }
            goto LAB_1000f3e08;
          }
          if (bVar1 == 0x42) {
            uVar7 = *(undefined8 *)(lVar6 + uVar17 * 8);
            local_b0 = 0;
            pcVar8 = pcVar14;
            if (bVar5) {
              pcVar8 = (char *)func_0x00010010296c();
            }
            puVar4 = *(undefined8 **)(pcVar8 + 0x50);
            if (*(undefined8 **)(pcVar8 + 0x58) < puVar4 + 3) {
              uVar17 = func_0x00010009b524(4,1,0x102);
              *(undefined8 *)(uVar17 + 0xe) = uVar7;
            }
            else {
              *(undefined8 **)(pcVar8 + 0x50) = puVar4 + 3;
              uVar17 = (long)puVar4 + 2;
              *puVar4 = 0x400010201;
              puVar4[2] = uVar7;
            }
          }
          else {
            if (bVar1 != 0x62) goto LAB_1000f4058;
            lVar6 = *(long *)(lVar6 + uVar17 * 8);
            pcVar8 = pcVar14;
            if (bVar5) {
              pcVar8 = (char *)func_0x00010010296c();
            }
            lVar3 = 0x170;
            if (lVar6 != 0) {
              lVar3 = 0x168;
            }
            uVar17 = *(ulong *)(pcVar8 + lVar3);
          }
        }
LAB_1000f3e24:
        local_b0 = uVar17;
        uVar7 = func_0x000100100058();
        local_a8 = uVar7;
        local_a0 = func_0x0001000ff2c8();
        uVar17 = func_0x0001000f43bc(uVar17,uVar7,local_a0);
        if (*pcVar9 != '\0') goto LAB_1000f41b8;
        local_b0 = uVar17;
        if (uVar17 == 0) {
          local_a8 = 0;
          local_a0 = 0;
          lVar6 = 1;
          local_98 = 0;
LAB_1000f3e8c:
          if (*(long *)(lVar16 + 0x260) == lVar6 + 1) {
            pcVar12 = *(code **)(lVar16 + 0x268);
          }
          else {
            pcVar12 = (code *)func_0x0001000b7158(lVar16 + 0x260,uVar17,0x4369a45648c4c6dc,
                                                  0x1002afee0,8);
          }
          local_b0 = (*pcVar12)(uVar17,&local_98,0x1002ca2b8);
          if (*pcVar9 != '\0') goto LAB_1000f41b8;
          if ((local_b0 & 1) == 0) {
            if (((local_b0 & 0x8000000000000006) != 2) || (*(int *)(local_b0 + 2) != 3)) {
              lVar6 = func_0x0001000b9680();
              goto LAB_1000f3f48;
            }
            lVar6 = *(long *)(local_b0 + 0xe);
          }
          else {
            lVar6 = (long)local_b0 >> 1;
          }
        }
        else {
          if ((uVar17 & 1) == 0) {
            uVar13 = uVar17 & 0x8000000000000006;
            if (uVar13 == 2) {
              lVar6 = (long)*(int *)(uVar17 + 2);
            }
            else {
              lVar6 = func_0x0001000a18e8(uVar17);
            }
            if (1 < lVar6 - 2U) {
              if (lVar6 != 4) {
                local_a8 = 0;
                local_a0 = 0;
                local_98 = 0;
                if (uVar13 == 2) {
                  lVar6 = (long)*(int *)(uVar17 + 2);
                }
                else {
                  lVar6 = func_0x0001000a18e8(uVar17);
                }
                goto LAB_1000f3e8c;
              }
              local_a8 = 0;
              local_a0 = 0;
              if ((uVar13 == 2) && (*(int *)(uVar17 + 2) == 4)) {
                uVar7 = *(undefined8 *)(uVar17 + 0xe);
              }
              else {
                uVar7 = func_0x0001000b94cc(uVar17);
              }
              if (*pcVar9 == '\0') {
                local_b0 = 0;
                lVar6 = func_0x0001000a2538(uVar7);
                goto LAB_1000f3f48;
              }
              goto LAB_1000f41b8;
            }
          }
          local_a8 = 0;
          local_a0 = 0;
          lVar6 = func_0x000100165fdc(uVar17);
LAB_1000f3f48:
          if (*pcVar9 != '\0') goto LAB_1000f41b8;
        }
        uVar18 = uVar18 + 1;
        lVar15 = lVar6 + lVar15;
      } while (param_2 != uVar18);
    }
    *(undefined8 *)(pcVar9 + 8) = local_c8;
  }
  return lVar15;
}


