
/* WARNING: Unknown calling convention -- yet parameter storage is locked */

void tree(void)

{
  undefined8 extraout_x0;
  undefined8 extraout_x0_00;
  long lVar1;
  long in_x1;
  long extraout_x1;
  long in_x15;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  if (in_x15 - 0x28U <= *(ulong *)(unaff_x26 + 0x38)) {
    stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
    in_x1 = extraout_x1;
  }
  if (in_x1 == 0) {
    new_Node();
  }
  else {
    *(long *)(in_x15 + -0x18) = in_x1 + -1;
    tree();
    *(undefined8 *)(in_x15 + -0x20) = extraout_x0;
    tree();
    *(undefined8 *)(in_x15 + -0x28) = extraout_x0_00;
    lVar1 = new_Node();
    *(undefined8 *)(lVar1 + 7) = *(undefined8 *)(in_x15 + -0x20);
    *(undefined8 *)(lVar1 + 0xf) = *(undefined8 *)(in_x15 + -0x28);
  }
  return;
}

