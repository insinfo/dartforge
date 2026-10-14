
long FUN_1000f3ee8(long param_1)

{
  undefined8 *puVar1;
  char cVar2;
  ulong uVar3;
  bool bVar4;
  long lVar5;
  long lVar6;
  char *pcVar7;
  char *pcVar8;
  char *pcVar9;
  undefined1 uStack_61;
  undefined8 local_60;
  undefined8 local_58;
  ulong local_50;
  ulong local_48;
  
  uVar3 = tpidrro_el0;
  pcVar9 = *(char **)((uVar3 & 0xfffffffffffffff8) + DAT_10032c1f8);
  bVar4 = DAT_10032c1f8 == 0 || pcVar9 == (char *)0x0;
  pcVar7 = pcVar9;
  if (bVar4) {
    pcVar7 = (char *)thunk_FUN_10009c0a0();
  }
  if (&uStack_61 < *(undefined1 **)(pcVar7 + 0x178)) {
    FUN_1000b7c08();
    return 0;
  }
  local_58 = 2;
  local_60 = *(undefined8 *)(pcVar7 + 8);
  *(undefined8 **)(pcVar7 + 8) = &local_60;
  local_48 = *(long *)(param_1 + 0xe);
  if (local_48 == 0) {
    lVar5 = 0;
LAB_1000f3fa0:
    local_50 = *(long *)(param_1 + 0x16);
    local_48 = 0;
    lVar6 = 0;
    if (local_50 == 0) {
LAB_1000f40fc:
      lVar6 = lVar5 + 1 + lVar6;
      goto LAB_1000f4108;
    }
    lVar6 = FUN_1000f3ee8();
    if (*pcVar7 == '\0') {
      local_50 = 0;
      if (lVar6 + 0x4000000000000000 < 0) {
        if (bVar4) {
          pcVar9 = (char *)thunk_FUN_10009c0a0();
        }
        puVar1 = *(undefined8 **)(pcVar9 + 0x50);
        if (*(undefined8 **)(pcVar9 + 0x58) < puVar1 + 3) {
          local_50 = FUN_10009b470(3,1,0x102);
        }
        else {
          *(undefined8 **)(pcVar9 + 0x50) = puVar1 + 3;
          local_50 = (long)puVar1 + 2;
          *puVar1 = 0x300010201;
        }
        *(long *)(local_50 + 0xe) = lVar6;
        lVar6 = 0;
        if (local_50 == 0) goto LAB_1000f40fc;
      }
      else {
        local_50 = lVar6 << 1 | 1;
      }
      if ((local_50 & 1) == 0) {
        if (((local_50 & 0x8000000000000006) == 2) && (*(int *)(local_50 + 2) == 3)) {
          lVar6 = *(long *)(local_50 + 0xe);
          cVar2 = *pcVar7;
        }
        else {
          lVar6 = FUN_1000b9958();
          cVar2 = *pcVar7;
        }
      }
      else {
        lVar6 = (long)local_50 >> 1;
        cVar2 = *pcVar7;
      }
      if (cVar2 == '\0') goto LAB_1000f40fc;
    }
  }
  else {
    local_50 = param_1;
    lVar5 = FUN_1000f3ee8();
    if (*pcVar7 == '\0') {
      local_48 = 0;
      if (lVar5 + 0x4000000000000000 < 0) {
        pcVar8 = pcVar9;
        if (bVar4) {
          pcVar8 = (char *)thunk_FUN_10009c0a0();
        }
        puVar1 = *(undefined8 **)(pcVar8 + 0x50);
        if (*(undefined8 **)(pcVar8 + 0x58) < puVar1 + 3) {
          local_48 = FUN_10009b470(3,1,0x102);
        }
        else {
          *(undefined8 **)(pcVar8 + 0x50) = puVar1 + 3;
          local_48 = (long)puVar1 + 2;
          *puVar1 = 0x300010201;
        }
        *(long *)(local_48 + 0xe) = lVar5;
        if (local_48 == 0) {
          lVar5 = 0;
          goto LAB_1000f3fa0;
        }
      }
      else {
        local_48 = lVar5 << 1 | 1;
      }
      if ((local_48 & 1) == 0) {
        if (((local_48 & 0x8000000000000006) == 2) && (*(int *)(local_48 + 2) == 3)) {
          lVar5 = *(long *)(local_48 + 0xe);
          cVar2 = *pcVar7;
        }
        else {
          lVar5 = FUN_1000b9958();
          cVar2 = *pcVar7;
        }
      }
      else {
        lVar5 = (long)local_48 >> 1;
        cVar2 = *pcVar7;
      }
      if (cVar2 == '\0') goto LAB_1000f3fa0;
    }
  }
  lVar6 = 0;
LAB_1000f4108:
  *(undefined8 *)(pcVar7 + 8) = local_60;
  return lVar6;
}

