
long FUN_1000f40b4(long param_1)

{
  ulong uVar1;
  long lVar2;
  long lVar3;
  char *pcVar4;
  undefined1 uStack_51;
  undefined8 local_50;
  undefined8 local_48;
  long local_40;
  long local_38;
  
  uVar1 = tpidrro_el0;
  pcVar4 = *(char **)((uVar1 & 0xfffffffffffffff8) + DAT_10032c1f8);
  if (DAT_10032c1f8 == 0 || pcVar4 == (char *)0x0) {
    pcVar4 = (char *)thunk_FUN_10009c36c();
  }
  if (&uStack_51 < *(undefined1 **)(pcVar4 + 0x178)) {
    FUN_1000b7dd4();
    return 0;
  }
  local_48 = 2;
  local_50 = *(undefined8 *)(pcVar4 + 8);
  *(undefined8 **)(pcVar4 + 8) = &local_50;
  local_38 = *(long *)(param_1 + 0xe);
  if (local_38 == 0) {
    lVar2 = 1;
  }
  else {
    local_40 = param_1;
    lVar2 = FUN_1000f40b4();
    if (*pcVar4 != '\0') {
      lVar3 = 0;
      goto LAB_1000f4178;
    }
    lVar2 = lVar2 + 1;
  }
  local_40 = *(long *)(param_1 + 0x16);
  local_38 = 0;
  if (local_40 == 0) {
    lVar3 = 0;
  }
  else {
    lVar3 = FUN_1000f40b4();
    if (*pcVar4 != '\0') {
      lVar3 = 0;
      goto LAB_1000f4178;
    }
  }
  lVar3 = lVar3 + lVar2;
LAB_1000f4178:
  *(undefined8 *)(pcVar4 + 8) = local_50;
  return lVar3;
}


