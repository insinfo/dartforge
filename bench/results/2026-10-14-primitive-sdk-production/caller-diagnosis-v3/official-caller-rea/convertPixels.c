
/* WARNING: Control flow encountered bad instruction data */

long entry(undefined8 param_1,long param_2,long param_3)

{
  ulong uVar1;
  ulong uVar2;
  long lVar3;
  long extraout_x8;
  long in_x15;
  ulong uVar4;
  undefined8 *extraout_x15;
  ulong *extraout_x15_00;
  ulong *extraout_x15_01;
  ulong extraout_x15_02;
  ulong extraout_x15_03;
  long unaff_x21;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  uVar4 = in_x15 - 0x40;
  *(long *)(in_x15 + -0x28) = param_2;
  *(long *)(in_x15 + -0x30) = param_3;
  if (uVar4 <= *(ulong *)(unaff_x26 + 0x38)) {
    func_0x000894e8();
    uVar4 = extraout_x15_03;
  }
  lVar3 = 0;
  uVar2 = 0;
  while( true ) {
    uVar1 = 0xff;
    *(long *)(in_x15 + -0x18) = lVar3;
    *(ulong *)(in_x15 + -0x20) = uVar2;
    if (uVar4 <= *(ulong *)(unaff_x26 + 0x38)) {
      func_0x000894e8();
      lVar3 = extraout_x8;
    }
    if (param_3 <= (long)uVar2) break;
    uVar1 = uVar2 & 0xffffffff & uVar1;
    if ((ulong)(*(long *)(param_2 + 0xf) >> 1) <= uVar1) {
      func_0x00089930();
                    /* WARNING: Bad instruction - Truncating control flow here */
      halt_baddata();
    }
    uVar2 = *(ulong *)(*(long *)(param_2 + 0x17) + uVar1 * 8 + 0x17);
    uVar4 = 0x3c;
    if ((uVar2 & 1) != 0) {
      uVar4 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
    }
    uVar4 = (**(code **)(unaff_x21 + (uVar4 - 0xffa) * 8))(uVar4,uVar2,0,1);
    uVar2 = 0x3c;
    if ((uVar4 & 1) != 0) {
      uVar2 = *(ulong *)(uVar4 - 1) >> 0xc & 0xfffff;
    }
    *extraout_x15 = 0x1fe;
    extraout_x15[1] = uVar4;
    uVar4 = (**(code **)(unaff_x21 + (uVar2 - 0x1000) * 8))();
    uVar2 = 0x3c;
    if ((uVar4 & 1) != 0) {
      uVar2 = *(ulong *)(uVar4 - 1) >> 0xc & 0xfffff;
    }
    *extraout_x15_00 = uVar4;
    uVar4 = (**(code **)(unaff_x21 + (uVar2 - 0xff7) * 8))();
    uVar2 = 0x3c;
    if ((uVar4 & 1) != 0) {
      uVar2 = *(ulong *)(uVar4 - 1) >> 0xc & 0xfffff;
    }
    *extraout_x15_01 = uVar4;
    uVar4 = (**(code **)(unaff_x21 + (uVar2 - 0xff7) * 8))();
    lVar3 = (long)uVar4 >> 1;
    if ((uVar4 & 1) != 0) {
      lVar3 = *(long *)(uVar4 + 7);
    }
    lVar3 = *(long *)(in_x15 + -0x18) + lVar3;
    uVar2 = *(long *)(in_x15 + -0x20) + 1;
    param_2 = *(long *)(in_x15 + -0x28);
    param_3 = *(long *)(in_x15 + -0x30);
    uVar4 = extraout_x15_02;
  }
  return lVar3;
}


