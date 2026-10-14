
undefined8 entry(undefined8 param_1)

{
  uint uVar1;
  long lVar2;
  ulong uVar3;
  undefined8 uVar4;
  ulong uVar5;
  long lVar6;
  long lVar7;
  long lVar8;
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
  long *plVar9;
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
    param_1 = func_0x0008de34();
  }
  lVar2 = func_0x0005a1e4(param_1,*(undefined8 *)(unaff_x27 + 0x17c8),0);
  *(long *)(in_x15 + -0x20) = lVar2;
  lVar6 = 0;
  uVar3 = extraout_x15;
  while( true ) {
    *(long *)(in_x15 + -0x18) = lVar6;
    if (uVar3 <= *(ulong *)(unaff_x26 + 0x38)) {
      lVar2 = func_0x0008de34();
    }
    if (49999 < lVar6) break;
    lVar6 = func_0x0008dd34();
    *(long *)(in_x15 + -0x30) = lVar6;
    *(undefined8 *)(lVar6 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17d0);
    lVar2 = *(long *)(in_x15 + -0x18);
    uVar3 = lVar2 * 2;
    if (SCARRY8(lVar2,lVar2)) {
      uVar3 = func_0x0008dfb4();
      *(long *)(uVar3 + 7) = lVar2;
    }
    *(ulong *)(in_x15 + -0x28) = uVar3;
    *(ulong *)(lVar6 + 0x1f) = uVar3;
    if (((uVar3 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar3 - 1) & *(byte *)(lVar6 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      func_0x0008bf5c();
    }
    *(undefined8 *)(lVar6 + 0x27) = *(undefined8 *)(unaff_x27 + 0x17d8);
    lVar6 = func_0x0008dd34();
    *(undefined8 *)(lVar6 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17e0);
    *(undefined8 *)(lVar6 + 0x1f) = *(undefined8 *)(in_x15 + -0x28);
    *extraout_x15_00 = lVar6;
    uVar3 = func_0x00058ba0();
    lVar6 = *(long *)(in_x15 + -0x30);
    *(ulong *)(lVar6 + 0x2f) = uVar3;
    if (((uVar3 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar3 - 1) & *(byte *)(lVar6 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      func_0x0008bf5c();
    }
    *(undefined8 *)(*(long *)(in_x15 + -0x30) + 0x37) = *(undefined8 *)(unaff_x27 + 0x17e8);
    lVar2 = func_0x0008dd34();
    *(undefined8 *)(lVar2 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17f0);
    lVar6 = *(long *)(in_x15 + -0x18) % 7;
    if (lVar6 < 0) {
      lVar6 = lVar6 + 7;
    }
    *(long *)(lVar2 + 0x1f) = lVar6 << 1;
    *extraout_x15_01 = lVar2;
    uVar4 = func_0x00058ba0();
    *(undefined8 *)(in_x15 + -0x28) = uVar4;
    lVar2 = func_0x0008dd34();
    *(undefined8 *)(lVar2 + 0x17) = *(undefined8 *)(unaff_x27 + 0x17f8);
    lVar6 = *(long *)(in_x15 + -0x18) % 0xd;
    if (lVar6 < 0) {
      lVar6 = lVar6 + 0xd;
    }
    *(long *)(lVar2 + 0x1f) = lVar6 << 1;
    *extraout_x15_02 = lVar2;
    uVar4 = func_0x00058ba0();
    *(undefined8 *)(in_x15 + -0x38) = uVar4;
    lVar6 = func_0x0008dd34();
    *(long *)(in_x15 + -0x40) = lVar6;
    *(undefined8 *)(lVar6 + 0x17) = *(undefined8 *)(in_x15 + -0x28);
    uVar4 = *(undefined8 *)(in_x15 + -0x38);
    *(undefined8 *)(lVar6 + 0x1f) = uVar4;
    uVar3 = func_0x0008cc70(uVar4,*(undefined8 *)(unaff_x27 + 0xa0));
    *(undefined8 *)(uVar3 + 0x17) = *(undefined8 *)(in_x15 + -0x40);
    *(undefined8 *)(uVar3 + 0xf) = 4;
    lVar6 = *(long *)(in_x15 + -0x30);
    *(ulong *)(lVar6 + 0x3f) = uVar3;
    plVar9 = extraout_x15_03;
    if (((uVar3 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar3 - 1) & *(byte *)(lVar6 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      func_0x0008bf5c();
      plVar9 = extraout_x15_04;
    }
    lVar6 = *(long *)(in_x15 + -0x30);
    *(undefined8 *)(lVar6 + 0x47) = *(undefined8 *)(unaff_x27 + 0x1800);
    if ((*(ulong *)(in_x15 + -0x18) & 1) == 0) {
      lVar2 = unaff_x22 + 0x20;
    }
    else {
      lVar2 = unaff_x22 + 0x30;
    }
    *(long *)(lVar6 + 0x4f) = lVar2;
    *(undefined8 *)(lVar6 + 0x57) = *(undefined8 *)(unaff_x27 + 0x1808);
    lVar2 = *(long *)(in_x15 + -0x18) % 100;
    if (lVar2 < 0) {
      lVar2 = lVar2 + 100;
    }
    *(long *)(lVar6 + 0x5f) = lVar2 << 1;
    lVar2 = *(long *)(unaff_x27 + 0x1810);
    *plVar9 = lVar6;
    plVar9[1] = lVar2;
    uVar4 = func_0x0006d470();
    lVar6 = *(long *)(in_x15 + -0x20);
    *(undefined8 *)(in_x15 + -0x28) = uVar4;
    lVar8 = *(long *)(*(long *)(lVar6 + 0x17) + 0xf);
    lVar2 = *(long *)(lVar6 + 0xf) >> 1;
    *(long *)(in_x15 + -0x48) = lVar2;
    uVar3 = extraout_x15_05;
    if (lVar2 == lVar8 >> 1) {
      func_0x0005a00c(lVar6,lVar6);
      uVar3 = extraout_x15_06;
    }
    lVar2 = *(long *)(in_x15 + -0x20);
    lVar6 = *(long *)(in_x15 + -0x18);
    lVar7 = *(long *)(in_x15 + -0x48);
    *(long *)(lVar2 + 0xf) = (lVar7 + 1) * 2;
    lVar8 = *(long *)(lVar2 + 0x17);
    uVar5 = *(ulong *)(in_x15 + -0x28);
    *(ulong *)(lVar8 + lVar7 * 8 + 0x17) = uVar5;
    if (((uVar5 & 1) != 0) &&
       (((ulong)(*(byte *)(uVar5 - 1) & *(byte *)(lVar8 + -1) >> 2) & unaff_x28 >> 0x20) != 0)) {
      func_0x0008bf5c();
      uVar3 = extraout_x15_07;
    }
    lVar6 = lVar6 + 1;
  }
  uVar4 = func_0x0009cbe4(lVar2,lVar2);
  uVar3 = func_0x00098488(uVar4,uVar4);
  *(ulong *)(in_x15 + -0x20) = uVar3;
  uVar5 = 0x3c;
  if ((uVar3 & 1) != 0) {
    uVar5 = *(ulong *)(uVar3 - 1) >> 0xc & 0xfffff;
  }
  if (((2 < uVar5 - 0x5a) && (0x37 < uVar5 - 0x70)) && (uVar5 - 0x70 != 0x1b7)) {
    func_0x0008bb78(*(undefined8 *)(unaff_x27 + 0x1818));
  }
  lVar2 = (**(code **)(unaff_x21 +
                      ((*(ulong *)(*(long *)(in_x15 + -0x20) + -1) >> 0xc & 0xfffff) - 0xbf7) * 8))
                    ();
  *(long *)(in_x15 + -0x20) = lVar2;
  lVar6 = 0;
  uVar3 = extraout_x15_08;
  while( true ) {
    *(long *)(in_x15 + -0x18) = lVar6;
    if (uVar3 <= *(ulong *)(unaff_x26 + 0x38)) {
      func_0x0008de34();
    }
    uVar3 = *(ulong *)(lVar2 + -1) >> 0xc & 0xfffff;
    uVar1 = (**(code **)(unaff_x21 + uVar3 * 8))(uVar3,lVar2);
    if ((uVar1 >> 4 & 1) != 0) break;
    lVar6 = *(long *)(in_x15 + -0x20);
    uVar3 = *(ulong *)(lVar6 + -1) >> 0xc & 0xfffff;
    uVar4 = (**(code **)(unaff_x21 + (uVar3 - 0x1000) * 8))
                      (uVar3,lVar6,lVar6,*(undefined8 *)(in_x15 + -0x18));
    *(undefined8 *)(in_x15 + -0x28) = uVar4;
    func_0x0009cc24(*(undefined8 *)(unaff_x27 + 0x1830));
    uVar3 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar3 = (**(code **)(unaff_x21 + uVar3 * 8))
                      (uVar3,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x17d0));
    *(ulong *)(in_x15 + -0x30) = uVar3;
    if (((uVar3 & 1) != 0) && (1 < (*(ulong *)(uVar3 - 1) >> 0xc & 0xfffff) - 0x3c)) {
      func_0x0009dd70(*(undefined8 *)(unaff_x27 + 0x5f8));
    }
    uVar3 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar3 = (**(code **)(unaff_x21 + uVar3 * 8))
                      (uVar3,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x17d8));
    *(ulong *)(in_x15 + -0x38) = uVar3;
    uVar5 = 0x3c;
    if ((uVar3 & 1) != 0) {
      uVar5 = *(ulong *)(uVar3 - 1) >> 0xc & 0xfffff;
    }
    if (1 < uVar5 - 0x5e) {
      func_0x0009dccc(*(undefined8 *)(unaff_x27 + 0xf8));
    }
    uVar3 = *(ulong *)(in_x15 + -0x30);
    lVar6 = (long)uVar3 >> 1;
    if ((uVar3 & 1) != 0) {
      lVar6 = *(long *)(uVar3 + 7);
    }
    *(long *)(in_x15 + -0x48) = lVar6 + (*(long *)(*(long *)(in_x15 + -0x38) + 7) >> 1);
    uVar3 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar3 = (**(code **)(unaff_x21 + uVar3 * 8))
                      (uVar3,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x17e8));
    *(ulong *)(in_x15 + -0x30) = uVar3;
    uVar5 = 0x3c;
    if ((uVar3 & 1) != 0) {
      uVar5 = *(ulong *)(uVar3 - 1) >> 0xc & 0xfffff;
    }
    plVar9 = extraout_x15_09;
    if (((2 < uVar5 - 0x5a) && (0x37 < uVar5 - 0x70)) && (uVar5 - 0x70 != 0x1b7)) {
      func_0x0008bb78(*(undefined8 *)(unaff_x27 + 0x1818));
      plVar9 = extraout_x15_10;
    }
    uVar3 = *(ulong *)(*(long *)(in_x15 + -0x30) + -1);
    *plVar9 = *(long *)(in_x15 + -0x30);
    lVar6 = (**(code **)(unaff_x21 + ((uVar3 >> 0xc & 0xfffff) - 0x1000) * 8))();
    *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x48) + (lVar6 >> 1);
    uVar3 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar3 = (**(code **)(unaff_x21 + uVar3 * 8))
                      (uVar3,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x1800));
    *(ulong *)(in_x15 + -0x30) = uVar3;
    uVar5 = 0x3c;
    if ((uVar3 & 1) != 0) {
      uVar5 = *(ulong *)(uVar3 - 1) >> 0xc & 0xfffff;
    }
    if (uVar5 != 0x3f) {
      func_0x0008bb78(*(undefined8 *)(unaff_x27 + 0x720));
    }
    *(long *)(in_x15 + -0x48) =
         *(long *)(in_x15 + -0x50) +
         ((long)((ulong)((*(ulong *)(in_x15 + -0x30) & 0x10) == 0) << 1) >> 1);
    uVar3 = *(ulong *)(*(long *)(in_x15 + -0x28) + -1) >> 0xc & 0xfffff;
    uVar5 = (**(code **)(unaff_x21 + uVar3 * 8))
                      (uVar3,*(long *)(in_x15 + -0x28),*(undefined8 *)(unaff_x27 + 0x1808));
    *(ulong *)(in_x15 + -0x28) = uVar5;
    uVar3 = extraout_x15_11;
    if (((uVar5 & 1) != 0) && (1 < (*(ulong *)(uVar5 - 1) >> 0xc & 0xfffff) - 0x3c)) {
      func_0x0009dd70(*(undefined8 *)(unaff_x27 + 0x5f8));
      uVar3 = extraout_x15_12;
    }
    uVar5 = *(ulong *)(in_x15 + -0x28);
    lVar6 = (long)uVar5 >> 1;
    if ((uVar5 & 1) != 0) {
      lVar6 = *(long *)(uVar5 + 7);
    }
    lVar6 = *(long *)(in_x15 + -0x18) + *(long *)(in_x15 + -0x48) + lVar6;
    lVar2 = *(long *)(in_x15 + -0x20);
  }
  return *(undefined8 *)(in_x15 + -0x18);
}

