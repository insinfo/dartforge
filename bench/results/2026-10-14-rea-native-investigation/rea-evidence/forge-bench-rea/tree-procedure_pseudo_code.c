
long FUN_1000f4198(long param_1)

{
  ulong uVar1;
  undefined8 uVar2;
  undefined8 uVar3;
  long lVar4;
  undefined8 *puVar5;
  undefined8 *puVar6;
  char *pcVar7;
  undefined1 uStack_51;
  undefined8 local_50;
  undefined8 local_48;
  undefined8 local_40;
  undefined8 local_38;
  
  uVar1 = tpidrro_el0;
  pcVar7 = *(char **)((uVar1 & 0xfffffffffffffff8) + DAT_10032c1f8);
  if (DAT_10032c1f8 == 0 || pcVar7 == (char *)0x0) {
    pcVar7 = (char *)thunk_FUN_10009c0a0();
  }
  if (&uStack_51 < *(undefined1 **)(pcVar7 + 0x178)) {
    FUN_1000b7c08();
    return 0;
  }
  local_40 = 0;
  local_38 = 0;
  local_48 = 2;
  local_50 = *(undefined8 *)(pcVar7 + 8);
  *(undefined8 **)(pcVar7 + 8) = &local_50;
  if (param_1 == 0) {
    puVar6 = *(undefined8 **)(pcVar7 + 0x60);
    puVar5 = puVar6 + 4;
    if (((*(undefined8 **)(pcVar7 + 0x68) < puVar5) || (*(ulong *)(pcVar7 + 0x38) < 0x401)) ||
       (*(char *)(*(long *)(pcVar7 + 0x30) + 0x400) == '\0')) {
      lVar4 = FUN_1000b0d1c(0x400,2,FUN_1000f89f0);
      uVar2 = 0;
      uVar3 = 0;
      goto LAB_1000f4288;
    }
    uVar2 = 0;
    uVar3 = 0;
  }
  else {
    uVar2 = FUN_1000f4198(param_1 + -1);
    if ((*pcVar7 != '\0') ||
       (local_40 = uVar2, uVar3 = FUN_1000f4198(param_1 + -1), *pcVar7 != '\0')) {
      *(undefined8 *)(pcVar7 + 8) = local_50;
      return 0;
    }
    puVar6 = *(undefined8 **)(pcVar7 + 0x60);
    puVar5 = puVar6 + 4;
    if (((*(undefined8 **)(pcVar7 + 0x68) < puVar5) || (*(ulong *)(pcVar7 + 0x38) < 0x401)) ||
       (*(char *)(*(long *)(pcVar7 + 0x30) + 0x400) == '\0')) {
      local_38 = uVar3;
      lVar4 = FUN_1000b0d1c(0x400,2,FUN_1000f89f0);
      goto LAB_1000f4288;
    }
  }
  *(undefined8 **)(pcVar7 + 0x60) = puVar5;
  lVar4 = (long)puVar6 + 2;
  puVar6[2] = 0;
  *puVar6 = 0x40000020001;
LAB_1000f4288:
  *(undefined8 *)(lVar4 + 0xe) = uVar2;
  *(undefined8 *)(lVar4 + 0x16) = uVar3;
  *(uint *)(lVar4 + 6) = *(uint *)(lVar4 + 6) | 3;
  *(undefined8 *)(pcVar7 + 8) = local_50;
  return lVar4;
}

