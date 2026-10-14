
/* WARNING: Control flow encountered bad instruction data */
/* WARNING: Possible PIC construction at 0x000799c0: Changing call to branch */

void entry(undefined8 param_1,long param_2,long param_3)

{
  long lVar1;
  ulong uVar2;
  undefined8 uVar3;
  long lVar4;
  ulong uVar5;
  long lVar6;
  ulong uVar7;
  ulong extraout_x8;
  long lVar8;
  long extraout_x9;
  ulong uVar9;
  long in_x15;
  long *plVar10;
  long *extraout_x15;
  long *extraout_x15_00;
  long *extraout_x15_01;
  long *extraout_x15_02;
  long *extraout_x15_03;
  long *extraout_x15_04;
  long *extraout_x15_05;
  long *extraout_x15_06;
  long *extraout_x15_07;
  long *extraout_x15_08;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  plVar10 = (long *)(in_x15 + -0x58);
  *(long *)(in_x15 + -0x40) = param_2;
  *(long *)(in_x15 + -0x48) = param_3;
  if (plVar10 <= *(long **)(unaff_x26 + 0x38)) {
    func_0x0008de34();
    plVar10 = extraout_x15_07;
  }
  lVar1 = *(long *)(param_3 + 7);
  uVar5 = lVar1 >> 1;
  *(ulong *)(in_x15 + -0x38) = uVar5;
  lVar6 = (*(ulong *)(param_3 + -1) >> 0xc & 0xfffff) << 1;
  *(long *)(in_x15 + -0x30) = lVar6;
  *(undefined8 *)(in_x15 + -0x28) = *(undefined8 *)(param_2 + 0x17);
  lVar4 = 0;
  lVar8 = 0;
  do {
    uVar7 = 0xfc00;
    *(long *)(in_x15 + -0x20) = lVar8;
    if (plVar10 <= *(long **)(unaff_x26 + 0x38)) {
      lVar1 = func_0x0008de34();
      uVar7 = extraout_x8;
      lVar8 = extraout_x9;
      plVar10 = extraout_x15_08;
    }
    if ((long)uVar5 <= lVar8) {
      if (lVar4 == 0) {
        func_0x00079a58(lVar1,*(undefined8 *)(in_x15 + -0x40),*(undefined8 *)(in_x15 + -0x48));
      }
      else if (lVar4 < *(long *)(in_x15 + -0x38)) {
        halt_baddata();
      }
      return;
    }
    if (lVar6 == 0xbc) {
      uVar2 = (ulong)*(byte *)(param_3 + lVar8 + 0xf);
    }
    else {
      uVar2 = (ulong)*(ushort *)(param_3 + lVar8 * 2 + 0xf);
    }
    *(ulong *)(in_x15 + -0x18) = uVar2;
    if (uVar2 < 0x5d) {
      if (uVar2 < 0x20) {
        if (lVar4 < *(long *)(in_x15 + -0x20)) {
          *plVar10 = *(long *)(in_x15 + -0x20) << 1;
          uVar3 = func_0x00058960(uVar2,*(undefined8 *)(in_x15 + -0x48));
          func_0x0005f8a8(uVar3,*(undefined8 *)(in_x15 + -0x28),uVar3);
        }
        *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x20) + 1;
        func_0x00062624(*(undefined8 *)(in_x15 + -0x18),*(undefined8 *)(in_x15 + -0x28),0x5c);
        uVar5 = *(ulong *)(in_x15 + -0x18);
        if ((long)uVar5 < 0xb) {
          if ((long)uVar5 < 10) {
            if ((long)uVar5 < 9) {
              if ((uVar5 & 0x7fffffffffffffff) != 8) goto LAB_00079844;
              func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),0x62);
              plVar10 = extraout_x15_00;
            }
            else {
              func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),0x74);
              plVar10 = extraout_x15_01;
            }
          }
          else {
            func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),0x6e);
            plVar10 = extraout_x15_02;
          }
        }
        else if ((long)uVar5 < 0xc) {
LAB_00079844:
          uVar3 = func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),0x75);
          uVar3 = func_0x00062624(uVar3,*(undefined8 *)(in_x15 + -0x28),0x30);
          func_0x00062624(uVar3,*(undefined8 *)(in_x15 + -0x28),0x30);
          uVar5 = (*(ulong *)(in_x15 + -0x18) & 0xffffffff) >> 4 & 0xf;
          if (uVar5 < 10) {
            lVar8 = uVar5 + 0x30;
          }
          else {
            lVar8 = uVar5 + 0x57;
          }
          func_0x00062624(0xf,*(undefined8 *)(in_x15 + -0x28),lVar8);
          uVar5 = *(ulong *)(in_x15 + -0x18) & 0xf;
          if (uVar5 < 10) {
            uVar7 = uVar5 + 0x30;
          }
          else {
            uVar5 = uVar5 + 0x57;
            uVar7 = uVar5;
          }
          func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),uVar7);
          plVar10 = extraout_x15_05;
        }
        else if ((long)uVar5 < 0xd) {
          func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),0x66);
          plVar10 = extraout_x15_03;
        }
        else {
          if ((uVar5 & 0x7fffffffffffffff) != 0xd) goto LAB_00079844;
          func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),0x72);
          plVar10 = extraout_x15_04;
        }
        lVar4 = *(long *)(in_x15 + -0x50);
      }
      else if ((uVar2 == 0x22) || (uVar2 == 0x5c)) {
        if (lVar4 < *(long *)(in_x15 + -0x20)) {
          *plVar10 = *(long *)(in_x15 + -0x20) << 1;
          uVar3 = func_0x00058960(uVar2,*(undefined8 *)(in_x15 + -0x48));
          func_0x0005f8a8(uVar3,*(undefined8 *)(in_x15 + -0x28),uVar3);
        }
        *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x20) + 1;
        uVar3 = func_0x00062624(*(long *)(in_x15 + -0x20),*(undefined8 *)(in_x15 + -0x28),0x5c);
        func_0x00062624(uVar3,*(undefined8 *)(in_x15 + -0x28),*(undefined8 *)(in_x15 + -0x18));
        lVar4 = *(long *)(in_x15 + -0x50);
        plVar10 = extraout_x15_06;
      }
    }
    else if (0xd7ff < uVar2) {
      if ((uVar2 & uVar7) == 0xd800) {
        lVar1 = lVar8 + 1;
        if (lVar1 < (long)uVar5) {
          if (lVar6 == 0xbc) {
            uVar9 = (ulong)*(byte *)(param_3 + lVar1 + 0xf);
          }
          else {
            uVar9 = (ulong)*(ushort *)(param_3 + lVar1 * 2 + 0xf);
          }
          if ((uVar9 & uVar7) == 0xdc00) goto LAB_000795ac;
        }
      }
      else {
LAB_000795ac:
        if ((uVar2 & uVar7) != 0xdc00) goto LAB_00079974;
        uVar2 = lVar8 - 1;
        if (-1 < (long)uVar2) {
          if (uVar5 <= uVar2) {
            func_0x0008e27c();
                    /* WARNING: Bad instruction - Truncating control flow here */
            halt_baddata();
          }
          if (lVar6 == 0xbc) {
            uVar5 = (ulong)*(byte *)(param_3 + uVar2 + 0xf);
          }
          else {
            uVar5 = (ulong)*(ushort *)(param_3 + uVar2 * 2 + 0xf);
          }
          if ((uVar5 & uVar7) == 0xd800) goto LAB_00079974;
        }
      }
      if (lVar4 < lVar8) {
        *plVar10 = lVar8 << 1;
        uVar3 = func_0x00058960(lVar8 << 1,param_3);
        func_0x0005f8a8(uVar3,*(undefined8 *)(in_x15 + -0x28),uVar3);
      }
      *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x20) + 1;
      uVar3 = func_0x00062624(*(long *)(in_x15 + -0x20),*(undefined8 *)(in_x15 + -0x28),0x5c);
      uVar3 = func_0x00062624(uVar3,*(undefined8 *)(in_x15 + -0x28),0x75);
      func_0x00062624(uVar3,*(undefined8 *)(in_x15 + -0x28),100);
      uVar5 = (*(ulong *)(in_x15 + -0x18) & 0xffffffff) >> 8 & 0xf;
      if (uVar5 < 10) {
        lVar8 = uVar5 + 0x30;
      }
      else {
        lVar8 = uVar5 + 0x57;
      }
      func_0x00062624(0xf,*(undefined8 *)(in_x15 + -0x28),lVar8);
      uVar5 = (*(ulong *)(in_x15 + -0x18) & 0xffffffff) >> 4 & 0xf;
      if (uVar5 < 10) {
        lVar8 = uVar5 + 0x30;
      }
      else {
        lVar8 = uVar5 + 0x57;
      }
      func_0x00062624(0xf,*(undefined8 *)(in_x15 + -0x28),lVar8);
      uVar5 = *(ulong *)(in_x15 + -0x18) & 0xf;
      if (uVar5 < 10) {
        uVar7 = uVar5 + 0x30;
      }
      else {
        uVar5 = uVar5 + 0x57;
        uVar7 = uVar5;
      }
      func_0x00062624(uVar5,*(undefined8 *)(in_x15 + -0x28),uVar7);
      lVar4 = *(long *)(in_x15 + -0x50);
      plVar10 = extraout_x15;
    }
LAB_00079974:
    lVar1 = *(long *)(in_x15 + -0x20);
    lVar8 = lVar1 + 1;
    param_3 = *(long *)(in_x15 + -0x48);
    lVar6 = *(long *)(in_x15 + -0x30);
    uVar5 = *(ulong *)(in_x15 + -0x38);
  } while( true );
}

