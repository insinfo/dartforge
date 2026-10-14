
/* WARNING: Control flow encountered bad instruction data */

ulong entry(undefined8 param_1,ulong param_2,long param_3,long param_4)

{
  uint uVar1;
  ulong uVar2;
  undefined8 uVar3;
  long lVar4;
  ulong uVar5;
  long lVar6;
  long in_x15;
  undefined8 *puVar7;
  undefined8 *extraout_x15;
  ulong *extraout_x15_00;
  undefined8 *extraout_x15_01;
  ulong *extraout_x15_02;
  undefined8 *extraout_x15_03;
  ulong *extraout_x15_04;
  undefined8 *extraout_x15_05;
  ulong *extraout_x15_06;
  undefined8 *extraout_x15_07;
  ulong *extraout_x15_08;
  undefined8 *extraout_x15_09;
  ulong *extraout_x15_10;
  undefined8 *extraout_x15_11;
  ulong *extraout_x15_12;
  ulong *puVar8;
  undefined8 *extraout_x15_13;
  ulong *extraout_x15_14;
  undefined8 *extraout_x15_15;
  ulong *extraout_x15_16;
  undefined8 *extraout_x15_17;
  ulong *extraout_x15_18;
  undefined8 *extraout_x15_19;
  ulong *extraout_x15_20;
  undefined8 *extraout_x15_21;
  long unaff_x21;
  ulong unaff_x22;
  long unaff_x26;
  long unaff_x27;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  puVar7 = (undefined8 *)(in_x15 + -0x30);
  *(long *)(in_x15 + -0x18) = param_4;
  *(ulong *)(in_x15 + -0x20) = param_2;
  if (puVar7 <= *(undefined8 **)(unaff_x26 + 0x38)) {
    param_3 = func_0x005133a4();
    puVar7 = extraout_x15_21;
  }
  if (param_3 == param_4) {
    return param_2;
  }
  lVar4 = *(long *)(param_3 + 7);
  if (5 < lVar4) {
    if (8 < lVar4) {
      lVar4 = *(long *)(param_4 + 7);
      if (5 < lVar4) {
        if (8 < lVar4) {
          return param_2;
        }
        if (7 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = param_2;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff2) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            uVar5 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar5 & 1) != 0) {
              uVar2 = *(ulong *)(uVar5 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar5,0xfffffffffffffffe,2)
            ;
            uVar5 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_17 = 0x100000000;
            extraout_x15_17[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
            puVar8 = extraout_x15_18;
          }
          else {
            uVar5 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar5 & 1) != 0) {
              uVar2 = *(ulong *)(uVar5 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar5,0xfffffffffffffffe,2)
            ;
            uVar5 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_19 = 0xfffffffe;
            extraout_x15_19[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
            puVar8 = extraout_x15_20;
          }
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *puVar8 = uVar2;
          uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
          return uVar2;
        }
        if (6 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = param_2;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff2) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            uVar5 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar5 & 1) != 0) {
              uVar2 = *(ulong *)(uVar5 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar5,0xfffffffffffffffe,2)
            ;
            uVar5 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_13 = 0x10000;
            extraout_x15_13[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
            puVar8 = extraout_x15_14;
          }
          else {
            uVar5 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar5 & 1) != 0) {
              uVar2 = *(ulong *)(uVar5 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar5,0xfffffffffffffffe,2)
            ;
            uVar5 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_15 = 0xfffe;
            extraout_x15_15[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
            puVar8 = extraout_x15_16;
          }
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *puVar8 = uVar2;
          uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
          return uVar2;
        }
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff2) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          uVar5 = *(ulong *)(in_x15 + -0x20);
          uVar2 = 0x3c;
          if ((uVar5 & 1) != 0) {
            uVar2 = *(ulong *)(uVar5 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar5,0xfffffffffffffffe,2);
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_09 = 0x100;
          extraout_x15_09[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
          puVar8 = extraout_x15_10;
        }
        else {
          uVar5 = *(ulong *)(in_x15 + -0x20);
          uVar2 = 0x3c;
          if ((uVar5 & 1) != 0) {
            uVar2 = *(ulong *)(uVar5 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar5,0xfffffffffffffffe,2);
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_11 = 0xfe;
          extraout_x15_11[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
          puVar8 = extraout_x15_12;
        }
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *puVar8 = uVar2;
        uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
        return uVar2;
      }
      if (2 < lVar4) {
        if (4 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,param_2,0,2);
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_07 = 0x1fffffffe;
          extraout_x15_07[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_08 = uVar2;
          uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
          return uVar2;
        }
        if (3 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,param_2,0,2);
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_05 = 0x1fffe;
          extraout_x15_05[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
          uVar5 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_06 = uVar2;
          uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
          return uVar2;
        }
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,param_2,0,2);
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_03 = 0x1fe;
        extraout_x15_03[1] = uVar2;
        uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_04 = uVar2;
        uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
        return uVar2;
      }
      if (1 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,param_2,0,2);
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_01 = 0x1e;
        extraout_x15_01[1] = uVar2;
        uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_02 = uVar2;
        uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
        return uVar2;
      }
      if (0 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,param_2,0,2);
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15 = 6;
        extraout_x15[1] = uVar2;
        uVar2 = (**(code **)(unaff_x21 + (uVar5 - 0xffb) * 8))();
        uVar5 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar5 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_00 = uVar2;
        uVar2 = (**(code **)(unaff_x21 + uVar5 * 8))();
        return uVar2;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return (ulong)((uVar2 & 0x10) != 0) << 1;
    }
    if (7 < lVar4) {
      lVar4 = *(long *)(param_4 + 7);
      if (5 < lVar4) {
        if (8 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0xfffffffe;
          puVar7[1] = param_2;
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return uVar2;
        }
        if (7 < lVar4) {
          return param_2;
        }
        if (6 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          return (lVar4 >> 0x10) << 1;
        }
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0x18) << 1;
      }
      if (2 < lVar4) {
        if (4 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = param_2;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar4 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar4 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar4 = *(long *)(uVar2 + 7);
            }
            lVar4 = lVar4 << 1;
          }
          uVar2 = lVar4 * 2;
          if (SCARRY8(lVar4,lVar4)) {
            uVar2 = func_0x00513524(uVar2);
            *(long *)(uVar2 + 7) = lVar4;
          }
          return uVar2;
        }
        if (3 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = param_2;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar4 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar4 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar4 = *(long *)(uVar2 + 7);
            }
            lVar4 = lVar4 >> 0x10;
          }
          return lVar4 << 1;
        }
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 >> 0x17;
        }
        return lVar4 << 1;
      }
      if (1 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 >> 0x1b;
        }
        return lVar4 << 1;
      }
      if (0 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 >> 0x1d;
        }
        return lVar4 << 1;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return (ulong)((uVar2 & 0x10) != 0) << 1;
    }
    if (6 < lVar4) {
      lVar4 = *(long *)(param_4 + 7);
      if (5 < lVar4) {
        if (8 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0xfffe;
          puVar7[1] = param_2;
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return uVar2;
        }
        if (7 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar6 = lVar4 * 0x10002;
          uVar2 = lVar4 * 0x20004;
          if (SCARRY8(lVar6,lVar6)) {
            uVar2 = func_0x00513524(uVar2);
            *(long *)(uVar2 + 7) = lVar6;
          }
          return uVar2;
        }
        if (6 < lVar4) {
          return param_2;
        }
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 8) << 1;
      }
      if (2 < lVar4) {
        if (4 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = param_2;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar4 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar4 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar4 = *(long *)(uVar2 + 7);
            }
            lVar4 = lVar4 * 0x20004;
          }
          uVar2 = lVar4 * 2;
          if (SCARRY8(lVar4,lVar4)) {
            uVar2 = func_0x00513524(uVar2);
            *(long *)(uVar2 + 7) = lVar4;
          }
          return uVar2;
        }
        if (3 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = param_2;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar4 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar4 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar4 = *(long *)(uVar2 + 7);
            }
            lVar4 = lVar4 << 1;
          }
          uVar2 = lVar4 * 2;
          if (SCARRY8(lVar4,lVar4)) {
            uVar2 = func_0x00513524(uVar2);
            *(long *)(uVar2 + 7) = lVar4;
          }
          return uVar2;
        }
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 >> 7;
        }
        return lVar4 << 1;
      }
      if (1 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 >> 0xb;
        }
        return lVar4 << 1;
      }
      if (0 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 >> 0xf;
        }
        return lVar4 << 1;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return (ulong)((uVar2 & 0x10) != 0) << 1;
    }
    lVar4 = *(long *)(param_4 + 7);
    if (5 < lVar4) {
      if (8 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0xfe;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
        return uVar2;
      }
      if (7 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x2040810;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      if (6 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x204;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      return param_2;
    }
    if (2 < lVar4) {
      if (4 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 * 0x2040810;
        }
        uVar2 = lVar4 * 2;
        if (SCARRY8(lVar4,lVar4)) {
          uVar2 = func_0x00513524(uVar2);
          *(long *)(uVar2 + 7) = lVar4;
        }
        return uVar2;
      }
      if (3 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = param_2;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar4 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar4 = lVar4 * 0x204;
        }
        uVar2 = lVar4 * 2;
        if (SCARRY8(lVar4,lVar4)) {
          uVar2 = func_0x00513524(uVar2);
          *(long *)(uVar2 + 7) = lVar4;
        }
        return uVar2;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
      if ((uVar1 >> 4 & 1) == 0) {
        lVar4 = 0;
      }
      else {
        if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        lVar4 = lVar4 << 1;
      }
      uVar2 = lVar4 * 2;
      if (SCARRY8(lVar4,lVar4)) {
        uVar2 = func_0x00513524(uVar2);
        *(long *)(uVar2 + 7) = lVar4;
      }
      return uVar2;
    }
    if (1 < lVar4) {
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
      if ((uVar1 >> 4 & 1) == 0) {
        lVar4 = 0;
      }
      else {
        if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        lVar4 = lVar4 >> 3;
      }
      return lVar4 << 1;
    }
    if (0 < lVar4) {
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
      if ((uVar1 >> 4 & 1) == 0) {
        lVar4 = 0;
      }
      else {
        if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        lVar4 = lVar4 >> 5;
      }
      return lVar4 << 1;
    }
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return (ulong)((uVar2 & 0x10) != 0) << 1;
  }
  if (2 < lVar4) {
    if (4 < lVar4) {
      lVar4 = *(long *)(param_4 + 7);
      if (5 < lVar4) {
        if (8 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0x1fffffffe;
          puVar7[1] = param_2;
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return uVar2;
        }
        if (7 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          return (lVar4 >> 1) << 1;
        }
        if (6 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          return (lVar4 >> 0x11) << 1;
        }
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0x19) << 1;
      }
      if (2 < lVar4) {
        if (4 < lVar4) {
          return param_2;
        }
        if (3 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          return (lVar4 >> 0x10) << 1;
        }
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0x18) << 1;
      }
      if (1 < lVar4) {
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0x1c) << 1;
      }
      if (0 < lVar4) {
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0x1e) << 1;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return (ulong)((uVar2 & 0x10) != 0) << 1;
    }
    if (3 < lVar4) {
      lVar4 = *(long *)(param_4 + 7);
      if (5 < lVar4) {
        if (8 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0x1fffe;
          puVar7[1] = param_2;
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return uVar2;
        }
        if (7 < lVar4) {
          uVar2 = 0x3c;
          if ((param_2 & 1) != 0) {
            uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0x100010;
          puVar7[1] = param_2;
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
          return uVar2;
        }
        if (6 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          return (lVar4 >> 1) << 1;
        }
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 9) << 1;
      }
      if (2 < lVar4) {
        if (4 < lVar4) {
          if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar4 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar4 = *(long *)(uVar2 + 7);
          }
          lVar6 = lVar4 * 0x100;
          uVar2 = lVar4 * 0x200;
          if (SCARRY8(lVar6,lVar6)) {
            uVar2 = func_0x00513524(uVar2);
            *(long *)(uVar2 + 7) = lVar6;
          }
          return uVar2;
        }
        if (3 < lVar4) {
          return param_2;
        }
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 8) << 1;
      }
      if (1 < lVar4) {
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0xc) << 1;
      }
      if (0 < lVar4) {
        if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar4 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar4 = *(long *)(uVar2 + 7);
        }
        return (lVar4 >> 0xe) << 1;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return (ulong)((uVar2 & 0x10) != 0) << 1;
    }
    lVar4 = *(long *)(param_4 + 7);
    if (5 < lVar4) {
      if (8 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x1fe;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
        return uVar2;
      }
      if (7 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x1010100;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      if (6 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x100;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      uVar2 = *(ulong *)(in_x15 + -0x20);
      lVar4 = (long)uVar2 >> 1;
      if ((uVar2 & 1) != 0) {
        lVar4 = *(long *)(uVar2 + 7);
      }
      return (lVar4 >> 1) << 1;
    }
    if (2 < lVar4) {
      if (4 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x2020202;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      if (3 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x202;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      return param_2;
    }
    if (1 < lVar4) {
      if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      uVar2 = *(ulong *)(in_x15 + -0x20);
      lVar4 = (long)uVar2 >> 1;
      if ((uVar2 & 1) != 0) {
        lVar4 = *(long *)(uVar2 + 7);
      }
      return (lVar4 >> 4) << 1;
    }
    if (0 < lVar4) {
      if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      uVar2 = *(ulong *)(in_x15 + -0x20);
      lVar4 = (long)uVar2 >> 1;
      if ((uVar2 & 1) != 0) {
        lVar4 = *(long *)(uVar2 + 7);
      }
      return (lVar4 >> 6) << 1;
    }
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return (ulong)((uVar2 & 0x10) != 0) << 1;
  }
  if (1 < lVar4) {
    lVar4 = *(long *)(param_4 + 7);
    if (5 < lVar4) {
      if (8 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 6;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
        return uVar2;
      }
      if (7 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x11111110;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      if (6 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x1110;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x10;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return uVar2;
    }
    if (2 < lVar4) {
      if (4 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x22222222;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      if (3 < lVar4) {
        uVar2 = 0x3c;
        if ((param_2 & 1) != 0) {
          uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x2222;
        puVar7[1] = param_2;
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return uVar2;
      }
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x22;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return uVar2;
    }
    if (1 < lVar4) {
      return param_2;
    }
    if (0 < lVar4) {
      if (((param_2 & 1) != 0) && (1 < (*(ulong *)(param_2 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        func_0x00524eac(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      uVar2 = *(ulong *)(in_x15 + -0x20);
      lVar4 = (long)uVar2 >> 1;
      if ((uVar2 & 1) != 0) {
        lVar4 = *(long *)(uVar2 + 7);
      }
      return (lVar4 >> 1) << 1;
    }
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return (ulong)((uVar2 & 0x10) != 0) << 1;
  }
  if (lVar4 < 1) {
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = param_2;
    uVar3 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    if (((uint)uVar3 >> 4 & 1) == 0) {
      lVar4 = 0;
    }
    else {
      uVar2 = func_0x0047828c(uVar3,*(undefined8 *)(unaff_x27 + 0x3d10),
                              *(undefined8 *)(in_x15 + -0x18));
      if (uVar2 == unaff_x22) {
        func_0x00513934();
                    /* WARNING: Bad instruction - Truncating control flow here */
        halt_baddata();
      }
      lVar4 = (long)uVar2 >> 1;
      if ((uVar2 & 1) != 0) {
        lVar4 = *(long *)(uVar2 + 7);
      }
    }
    uVar2 = lVar4 * 2;
    if (SCARRY8(lVar4,lVar4)) {
      uVar2 = func_0x00513524(uVar2);
      *(long *)(uVar2 + 7) = lVar4;
    }
    return uVar2;
  }
  lVar4 = *(long *)(param_4 + 7);
  if (5 < lVar4) {
    if (8 < lVar4) {
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 6;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
      return uVar2;
    }
    if (7 < lVar4) {
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x55555554;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return uVar2;
    }
    if (6 < lVar4) {
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x5554;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return uVar2;
    }
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0x54;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
    return uVar2;
  }
  if (lVar4 < 3) {
    if (1 < lVar4) {
      uVar2 = 0x3c;
      if ((param_2 & 1) != 0) {
        uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 10;
      puVar7[1] = param_2;
      uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return uVar2;
    }
    if (0 < lVar4) {
      return param_2;
    }
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return (ulong)((uVar2 & 0x10) != 0) << 1;
  }
  if (4 < lVar4) {
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0xaaaaaaaa;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
    return uVar2;
  }
  if (3 < lVar4) {
    uVar2 = 0x3c;
    if ((param_2 & 1) != 0) {
      uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0xaaaa;
    puVar7[1] = param_2;
    uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
    return uVar2;
  }
  uVar2 = 0x3c;
  if ((param_2 & 1) != 0) {
    uVar2 = *(ulong *)(param_2 - 1) >> 0xc & 0xfffff;
  }
  *puVar7 = 0x96;
  puVar7[1] = param_2;
  uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
  return uVar2;
}

