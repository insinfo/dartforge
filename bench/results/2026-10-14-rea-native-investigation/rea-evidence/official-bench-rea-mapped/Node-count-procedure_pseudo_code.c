
/* WARNING: Unknown calling convention -- yet parameter storage is locked */

void Node_count(void)

{
  long extraout_x0;
  ulong uVar1;
  long extraout_x0_00;
  long lVar2;
  long in_x1;
  long lVar3;
  long in_x15;
  ulong unaff_x22;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  *(long *)(in_x15 + -0x18) = in_x1;
  if (in_x15 - 0x20U <= *(ulong *)(unaff_x26 + 0x38)) {
    in_x1 = stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
  }
  uVar1 = unaff_x22;
  if (*(ulong *)(in_x1 + 7) != unaff_x22) {
    Node_count();
    uVar1 = extraout_x0 * 2;
    if (SCARRY8(extraout_x0,extraout_x0)) {
      lVar3 = extraout_x0;
      uVar1 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub();
      *(long *)(uVar1 + 7) = lVar3;
    }
  }
  if (uVar1 == unaff_x22) {
    lVar3 = 0;
  }
  else {
    lVar3 = (long)uVar1 >> 1;
    if ((uVar1 & 1) != 0) {
      lVar3 = *(long *)(uVar1 + 7);
    }
  }
  *(long *)(in_x15 + -0x20) = lVar3 + 1;
  if ((*(ulong *)(*(long *)(in_x15 + -0x18) + 0xf) != unaff_x22) &&
     (Node_count(), SCARRY8(extraout_x0_00,extraout_x0_00))) {
    lVar3 = extraout_x0_00;
    lVar2 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub();
    *(long *)(lVar2 + 7) = lVar3;
  }
  return;
}

