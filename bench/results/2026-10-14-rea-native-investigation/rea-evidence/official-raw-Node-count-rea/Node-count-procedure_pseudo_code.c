
long entry(undefined8 param_1,long param_2)

{
  ulong uVar1;
  long lVar2;
  long in_x15;
  ulong unaff_x22;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  *(long *)(in_x15 + -0x18) = param_2;
  if (in_x15 - 0x20U <= *(ulong *)(unaff_x26 + 0x38)) {
    param_2 = func_0x0008de34();
  }
  uVar1 = unaff_x22;
  if (*(ulong *)(param_2 + 7) != unaff_x22) {
    lVar2 = entry();
    uVar1 = lVar2 * 2;
    if (SCARRY8(lVar2,lVar2)) {
      uVar1 = func_0x0008dfb4();
      *(long *)(uVar1 + 7) = lVar2;
    }
  }
  if (uVar1 == unaff_x22) {
    lVar2 = 0;
  }
  else {
    lVar2 = (long)uVar1 >> 1;
    if ((uVar1 & 1) != 0) {
      lVar2 = *(long *)(uVar1 + 7);
    }
  }
  *(long *)(in_x15 + -0x20) = lVar2 + 1;
  uVar1 = unaff_x22;
  if (*(ulong *)(*(long *)(in_x15 + -0x18) + 0xf) != unaff_x22) {
    lVar2 = entry();
    uVar1 = lVar2 * 2;
    if (SCARRY8(lVar2,lVar2)) {
      uVar1 = func_0x0008dfb4();
      *(long *)(uVar1 + 7) = lVar2;
    }
  }
  if (uVar1 == unaff_x22) {
    lVar2 = 0;
  }
  else {
    lVar2 = (long)uVar1 >> 1;
    if ((uVar1 & 1) != 0) {
      lVar2 = *(long *)(uVar1 + 7);
    }
  }
  return *(long *)(in_x15 + -0x20) + lVar2;
}

