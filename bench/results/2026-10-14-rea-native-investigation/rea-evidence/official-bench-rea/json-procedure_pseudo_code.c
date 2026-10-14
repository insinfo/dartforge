
/* WARNING: Unknown calling convention -- yet parameter storage is locked */

void json(void)

{
  uint uVar1;
  undefined8 extraout_x0;
  ulong uVar2;
  ulong extraout_x0_00;
  undefined8 extraout_x0_01;
  undefined8 extraout_x0_02;
  undefined8 uVar3;
  undefined8 extraout_x0_03;
  ulong uVar4;
  ulong extraout_x0_04;
  long lVar5;
  long lVar6;
  long lVar7;
  long in_x15;
  ulong extraout_x15;
  long *extraout_x15_00;
  long *extraout_x15_01;
  long *extraout_x15_02;
  long *extraout_x15_03;
  long *extraout_x15_04;
  ulong extraout_x15_05;
  ulong extraout_x15_06;
  ulong extraout_x15_07;
  ulong extraout_x15_08;
  long *extraout_x15_09;
  long *extraout_x15_10;
  long *plVar8;
  ulong extraout_x15_11;
  ulong extraout_x15_12;
  long unaff_x21;
  long unaff_x22;
  long unaff_x26;
  long unaff_x27;
  ulong unaff_x28;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  if (in_x15 - 0x60U <= *(ulong *)(unaff_x26 + 0x38)) {
    stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
  }
  new__GrowableList();
  *(undefined8 *)(in_x15 + -0x20) = extraout_x0;
  lVar5 = 0;
  uVar2 = extraout_x15;
  while( true ) {
    *(long *)(in_x15 + -0x18) = lVar5;
    if (uVar2 <= *(ulong *)(unaff_x26 + 0x38)) {
      stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
    }
    if (49999 < lVar5) break;
    lVar5 = stub__iso_stub_AllocateArrayStub();
    *(long *)(in_x15 + -0x30) = lVar5;
    *(undefined8 *)(lVar5 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17d0);
    lVar7 = *(long *)(in_x15 + -0x18);
    uVar2 = lVar7 * 2;
    if (SCARRY8(lVar7,lVar7)) {
      uVar2 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub();
      *(long *)(uVar2 + 7) = lVar7;
    }
    *(ulong *)(in_x15 + -0x28) = uVar2;
    *(ulong *)(lVar5 + 0x1f) = uVar2;
    if (((uVar2 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar2 - 1) & *(byte *)(lVar5 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      stub__iso_stub_ArrayWriteBarrierStub();
    }
    *(undefined8 *)(lVar5 + 0x27) = *(undefined8 *)(unaff_x27 + 0x17d8);
    lVar5 = stub__iso_stub_AllocateArrayStub();
    *(undefined8 *)(lVar5 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17e0);
    *(undefined8 *)(lVar5 + 0x1f) = *(undefined8 *)(in_x15 + -0x28);
    *extraout_x15_00 = lVar5;
    _StringBase__interpolate();
    lVar5 = *(long *)(in_x15 + -0x30);
    *(ulong *)(lVar5 + 0x2f) = extraout_x0_00;
    if (((extraout_x0_00 & 1) != 0) &&
       (((ulong)(*(byte *)(extraout_x0_00 - 1) & *(byte *)(lVar5 + -1) >> 2) & unaff_x28 >> 0x20) !=
        0)) {
      stub__iso_stub_ArrayWriteBarrierStub();
    }
    *(undefined8 *)(*(long *)(in_x15 + -0x30) + 0x37) = *(undefined8 *)(unaff_x27 + 0x17e8);
    lVar7 = stub__iso_stub_AllocateArrayStub();
    *(undefined8 *)(lVar7 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17f0);
    lVar5 = *(long *)(in_x15 + -0x18) % 7;
    if (lVar5 < 0) {
      lVar5 = lVar5 + 7;
    }
    *(long *)(lVar7 + 0x1f) = lVar5 << 1;
    *extraout_x15_01 = lVar7;
    _StringBase__interpolate();
    *(undefined8 *)(in_x15 + -0x28) = extraout_x0_01;
    lVar7 = stub__iso_stub_AllocateArrayStub();
    *(undefined8 *)(lVar7 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17f8);
    lVar5 = *(long *)(in_x15 + -0x18) % 0xd;
    if (lVar5 < 0) {
      lVar5 = lVar5 + 0xd;
    }
    *(long *)(lVar7 + 0x1f) = lVar5 << 1;
    *extraout_x15_02 = lVar7;
    _StringBase__interpolate();
    *(undefined8 *)(in_x15 + -0x38) = extraout_x0_02;
    lVar5 = stub__iso_stub_AllocateArrayStub();
    *(long *)(in_x15 + -0x40) = lVar5;
    *(undefined8 *)(lVar5 + 0x17) = *(undefined8 *)(in_x15 + -0x28);
    uVar3 = *(undefined8 *)(in_x15 + -0x38);
    *(undefined8 *)(lVar5 + 0x1f) = uVar3;
    uVar2 = stub__iso_stub_AllocateGrowableArrayStub(uVar3,*(undefined8 *)(unaff_x27 + 0xa0));
    *(undefined8 *)(uVar2 + 0x17) = *(undefined8 *)(in_x15 + -0x40);
    *(undefined8 *)(uVar2 + 0xf) = 4;
    lVar5 = *(long *)(in_x15 + -0x30);
    *(ulong *)(lVar5 + 0x3f) = uVar2;
    plVar8 = extraout_x15_03;
    if (((uVar2 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar2 - 1) & *(byte *)(lVar5 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      stub__iso_stub_ArrayWriteBarrierStub();
      plVar8 = extraout_x15_04;
    }
    lVar5 = *(long *)(in_x15 + -0x30);
    *(undefined8 *)(lVar5 + 0x47) = *(undefined8 *)(unaff_x27 + 0x1800);
    if ((*(ulong *)(in_x15 + -0x18) & 1) == 0) {
      lVar7 = unaff_x22 + 0x20;
    }
    else {
      lVar7 = unaff_x22 + 0x30;
    }
    *(long *)(lVar5 + 0x4f) = lVar7;
    *(undefined8 *)(lVar5 + 0x57) = *(undefined8 *)(unaff_x27 + 0x1808);
    lVar7 = *(long *)(in_x15 + -0x18) % 100;
    if (lVar7 < 0) {
      lVar7 = lVar7 + 100;
    }
    *(long *)(lVar5 + 0x5f) = lVar7 << 1;
    lVar7 = *(long *)(unaff_x27 + 0x1810);
    *plVar8 = lVar5;
    plVar8[1] = lVar7;
    new_Map__fromLiteral();
    *(undefined8 *)(in_x15 + -0x28) = extraout_x0_03;
    lVar7 = *(long *)(*(long *)(*(long *)(in_x15 + -0x20) + 0x17) + 0xf);
    lVar5 = *(long *)(*(long *)(in_x15 + -0x20) + 0xf) >> 1;
    *(long *)(in_x15 + -0x48) = lVar5;
    uVar2 = extraout_x15_05;
    if (lVar5 == lVar7 >> 1) {
      List__growToNextCapacity();
      uVar2 = extraout_x15_06;
    }
    lVar7 = *(long *)(in_x15 + -0x20);
    lVar5 = *(long *)(in_x15 + -0x18);
    lVar6 = *(long *)(in_x15 + -0x48);
    *(long *)(lVar7 + 0xf) = (lVar6 + 1) * 2;
    lVar7 = *(long *)(lVar7 + 0x17);
    uVar4 = *(ulong *)(in_x15 + -0x28);
    *(ulong *)(lVar7 + lVar6 * 8 + 0x17) = uVar4;
    if (((uVar4 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar4 - 1) & *(byte *)(lVar7 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      stub__iso_stub_ArrayWriteBarrierStub();
      uVar2 = extraout_x15_07;
    }
    lVar5 = lVar5 + 1;
  }
  jsonEncode();
  jsonDecode();
  *(ulong *)(in_x15 + -0x20) = extraout_x0_04;
  uVar2 = 0x3c;
  if ((extraout_x0_04 & 1) != 0) {
    uVar2 = *(ulong *)(extraout_x0_04 - 1) >> 0xc & 0xfffff;
  }
  if (((2 < uVar2 - 0x5a) && (0x37 < uVar2 - 0x70)) && (uVar2 - 0x70 != 0x1b7)) {
    stub__iso_stub_DefaultTypeTestStub(*(undefined8 *)(unaff_x27 + 0x1818));
  }
  lVar7 = (**(code **)(unaff_x21 +
                      ((*(ulong *)(*(long *)(in_x15 + -0x20) + -1) >> 0xc & 0xfffff) - 0xbf7) * 8))
                    ();
  *(long *)(in_x15 + -0x20) = lVar7;
  lVar5 = 0;
  uVar2 = extraout_x15_08;
  while( true ) {
    *(long *)(in_x15 + -0x18) = lVar5;
    if (uVar2 <= *(ulong *)(unaff_x26 + 0x38)) {
      stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
    }
    uVar2 = *(ulong *)(lVar7 + -1) >> 0xc & 0xfffff;
    uVar1 = (**(code **)(unaff_x21 + uVar2 * 8))(uVar2,lVar7);
    if ((uVar1 >> 4 & 1) != 0) break;
    lVar5 = *(long *)(in_x15 + -0x20);
    uVar2 = *(ulong *)(lVar5 + -1) >> 0xc & 0xfffff;
    uVar3 = (**(code **)(unaff_x21 + (uVar2 - 0x1000) * 8))
                      (uVar2,lVar5,lVar5,*(undefined8 *)(in_x15 + -0x18));
    *(undefined8 *)(in_x15 + -0x28) = uVar3;
    assert_type_is_Map<String,_dynamic>(*(undefined8 *)(unaff_x27 + 0x1830));
    uVar2 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar2 = (**(code **)(unaff_x21 + uVar2 * 8))
                      (uVar2,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x17d0));
    *(ulong *)(in_x15 + -0x30) = uVar2;
    if (((uVar2 & 1) != 0) && (1 < (*(ulong *)(uVar2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
      assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x5f8));
    }
    uVar2 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar2 = (**(code **)(unaff_x21 + uVar2 * 8))
                      (uVar2,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x17d8));
    *(ulong *)(in_x15 + -0x38) = uVar2;
    uVar4 = 0x3c;
    if ((uVar2 & 1) != 0) {
      uVar4 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
    }
    if (1 < uVar4 - 0x5e) {
      assert_type_is_String(*(undefined8 *)(unaff_x27 + 0xf8));
    }
    uVar2 = *(ulong *)(in_x15 + -0x30);
    lVar5 = (long)uVar2 >> 1;
    if ((uVar2 & 1) != 0) {
      lVar5 = *(long *)(uVar2 + 7);
    }
    *(long *)(in_x15 + -0x48) = lVar5 + (*(long *)(*(long *)(in_x15 + -0x38) + 7) >> 1);
    uVar2 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar2 = (**(code **)(unaff_x21 + uVar2 * 8))
                      (uVar2,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x17e8));
    *(ulong *)(in_x15 + -0x30) = uVar2;
    uVar4 = 0x3c;
    if ((uVar2 & 1) != 0) {
      uVar4 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
    }
    plVar8 = extraout_x15_09;
    if (((2 < uVar4 - 0x5a) && (0x37 < uVar4 - 0x70)) && (uVar4 - 0x70 != 0x1b7)) {
      stub__iso_stub_DefaultTypeTestStub(*(undefined8 *)(unaff_x27 + 0x1818));
      plVar8 = extraout_x15_10;
    }
    uVar2 = *(ulong *)(*(long *)(in_x15 + -0x30) + -1);
    *plVar8 = *(long *)(in_x15 + -0x30);
    lVar5 = (**(code **)(unaff_x21 + ((uVar2 >> 0xc & 0xfffff) - 0x1000) * 8))();
    *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x48) + (lVar5 >> 1);
    uVar2 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar2 = (**(code **)(unaff_x21 + uVar2 * 8))
                      (uVar2,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x1800));
    *(ulong *)(in_x15 + -0x30) = uVar2;
    uVar4 = 0x3c;
    if ((uVar2 & 1) != 0) {
      uVar4 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
    }
    if (uVar4 != 0x3f) {
      stub__iso_stub_DefaultTypeTestStub(*(undefined8 *)(unaff_x27 + 0x720));
    }
    *(long *)(in_x15 + -0x48) =
         *(long *)(in_x15 + -0x50) +
         ((long)((ulong)((*(ulong *)(in_x15 + -0x30) & 0x10) == 0) << 1) >> 1);
    uVar2 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar4 = (**(code **)(unaff_x21 + uVar2 * 8))
                      (uVar2,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x1808));
    *(ulong *)(in_x15 + -0x28) = uVar4;
    uVar2 = extraout_x15_11;
    if (((uVar4 & 1) != 0) && (1 < (*(ulong *)(uVar4 - 1) >> 0xc & 0xfffff) - 0x3c)) {
      assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x5f8));
      uVar2 = extraout_x15_12;
    }
    uVar4 = *(ulong *)(in_x15 + -0x28);
    lVar5 = (long)uVar4 >> 1;
    if ((uVar4 & 1) != 0) {
      lVar5 = *(long *)(uVar4 + 7);
    }
    lVar5 = *(long *)(in_x15 + -0x18) + *(long *)(in_x15 + -0x48) + lVar5;
    lVar7 = *(long *)(in_x15 + -0x20);
  }
  return;
}

