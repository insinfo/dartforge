
/* WARNING: Unknown calling convention -- yet parameter storage is locked */

void convertFormatValue(void)

{
  uint uVar1;
  ulong uVar2;
  ulong extraout_x0;
  long lVar3;
  undefined8 uVar4;
  ulong in_x1;
  long lVar5;
  ulong uVar6;
  long in_x2;
  long in_x3;
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
  long extraout_x15_22;
  long unaff_x21;
  ulong unaff_x22;
  long unaff_x26;
  long unaff_x27;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  puVar7 = (undefined8 *)(in_x15 + -0x30);
  *(long *)(in_x15 + -0x18) = in_x3;
  *(ulong *)(in_x15 + -0x20) = in_x1;
  if (puVar7 <= *(undefined8 **)(unaff_x26 + 0x38)) {
    in_x2 = stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
    puVar7 = extraout_x15_21;
  }
  if (in_x2 == in_x3) {
    return;
  }
  lVar5 = *(long *)(in_x2 + 7);
  if (5 < lVar5) {
    if (8 < lVar5) {
      lVar5 = *(long *)(in_x3 + 7);
      if (5 < lVar5) {
        if (8 < lVar5) {
          return;
        }
        if (7 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = in_x1;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff2) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            uVar6 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar6 & 1) != 0) {
              uVar2 = *(ulong *)(uVar6 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar6,0xfffffffffffffffe,2)
            ;
            uVar6 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_17 = 0x100000000;
            extraout_x15_17[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
            puVar8 = extraout_x15_18;
          }
          else {
            uVar6 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar6 & 1) != 0) {
              uVar2 = *(ulong *)(uVar6 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar6,0xfffffffffffffffe,2)
            ;
            uVar6 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_19 = 0xfffffffe;
            extraout_x15_19[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
            puVar8 = extraout_x15_20;
          }
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *puVar8 = uVar2;
          (**(code **)(unaff_x21 + uVar6 * 8))();
          return;
        }
        if (6 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = in_x1;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff2) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            uVar6 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar6 & 1) != 0) {
              uVar2 = *(ulong *)(uVar6 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar6,0xfffffffffffffffe,2)
            ;
            uVar6 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_13 = 0x10000;
            extraout_x15_13[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
            puVar8 = extraout_x15_14;
          }
          else {
            uVar6 = *(ulong *)(in_x15 + -0x20);
            uVar2 = 0x3c;
            if ((uVar6 & 1) != 0) {
              uVar2 = *(ulong *)(uVar6 - 1) >> 0xc & 0xfffff;
            }
            uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar6,0xfffffffffffffffe,2)
            ;
            uVar6 = 0x3c;
            if ((uVar2 & 1) != 0) {
              uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
            }
            *extraout_x15_15 = 0xfffe;
            extraout_x15_15[1] = uVar2;
            uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
            puVar8 = extraout_x15_16;
          }
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *puVar8 = uVar2;
          (**(code **)(unaff_x21 + uVar6 * 8))();
          return;
        }
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff2) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          uVar6 = *(ulong *)(in_x15 + -0x20);
          uVar2 = 0x3c;
          if ((uVar6 & 1) != 0) {
            uVar2 = *(ulong *)(uVar6 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar6,0xfffffffffffffffe,2);
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_09 = 0x100;
          extraout_x15_09[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
          puVar8 = extraout_x15_10;
        }
        else {
          uVar6 = *(ulong *)(in_x15 + -0x20);
          uVar2 = 0x3c;
          if ((uVar6 & 1) != 0) {
            uVar2 = *(ulong *)(uVar6 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,uVar6,0xfffffffffffffffe,2);
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_11 = 0xfe;
          extraout_x15_11[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
          puVar8 = extraout_x15_12;
        }
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *puVar8 = uVar2;
        (**(code **)(unaff_x21 + uVar6 * 8))();
        return;
      }
      if (2 < lVar5) {
        if (4 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,in_x1,0,2);
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_07 = 0x1fffffffe;
          extraout_x15_07[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_08 = uVar2;
          (**(code **)(unaff_x21 + uVar6 * 8))();
          return;
        }
        if (3 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,in_x1,0,2);
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_05 = 0x1fffe;
          extraout_x15_05[1] = uVar2;
          uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
          uVar6 = 0x3c;
          if ((uVar2 & 1) != 0) {
            uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
          }
          *extraout_x15_06 = uVar2;
          (**(code **)(unaff_x21 + uVar6 * 8))();
          return;
        }
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,in_x1,0,2);
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_03 = 0x1fe;
        extraout_x15_03[1] = uVar2;
        uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_04 = uVar2;
        (**(code **)(unaff_x21 + uVar6 * 8))();
        return;
      }
      if (1 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,in_x1,0,2);
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_01 = 0x1e;
        extraout_x15_01[1] = uVar2;
        uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_02 = uVar2;
        (**(code **)(unaff_x21 + uVar6 * 8))();
        return;
      }
      if (0 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        uVar2 = (**(code **)(unaff_x21 + (uVar2 - 0xff8) * 8))(uVar2,in_x1,0,2);
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15 = 6;
        extraout_x15[1] = uVar2;
        uVar2 = (**(code **)(unaff_x21 + (uVar6 - 0xffb) * 8))();
        uVar6 = 0x3c;
        if ((uVar2 & 1) != 0) {
          uVar6 = *(ulong *)(uVar2 - 1) >> 0xc & 0xfffff;
        }
        *extraout_x15_00 = uVar2;
        (**(code **)(unaff_x21 + uVar6 * 8))();
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return;
    }
    if (7 < lVar5) {
      lVar5 = *(long *)(in_x3 + 7);
      if (5 < lVar5) {
        if (8 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0xfffffffe;
          puVar7[1] = in_x1;
          (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return;
        }
        if (7 < lVar5) {
          return;
        }
        if (6 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          return;
        }
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (2 < lVar5) {
        if (4 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = in_x1;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar5 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar5 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar5 = *(long *)(uVar2 + 7);
            }
            lVar5 = lVar5 << 1;
          }
          if (SCARRY8(lVar5,lVar5)) {
            lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
            *(long *)(lVar3 + 7) = lVar5;
          }
          return;
        }
        if (3 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = in_x1;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          return;
        }
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (1 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (0 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return;
    }
    if (6 < lVar5) {
      lVar5 = *(long *)(in_x3 + 7);
      if (5 < lVar5) {
        if (8 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0xfffe;
          puVar7[1] = in_x1;
          (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return;
        }
        if (7 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar5 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar5 = *(long *)(uVar2 + 7);
          }
          lVar3 = lVar5 * 0x10002;
          if (SCARRY8(lVar3,lVar3)) {
            lVar5 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 0x20004);
            *(long *)(lVar5 + 7) = lVar3;
          }
          return;
        }
        if (6 < lVar5) {
          return;
        }
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (2 < lVar5) {
        if (4 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = in_x1;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar5 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar5 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar5 = *(long *)(uVar2 + 7);
            }
            lVar5 = lVar5 * 0x20004;
          }
          if (SCARRY8(lVar5,lVar5)) {
            lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
            *(long *)(lVar3 + 7) = lVar5;
          }
          return;
        }
        if (3 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0;
          puVar7[1] = in_x1;
          uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
          if ((uVar1 >> 4 & 1) == 0) {
            lVar5 = 0;
          }
          else {
            if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
               (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
              assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
            }
            uVar2 = *(ulong *)(in_x15 + -0x20);
            lVar5 = (long)uVar2 >> 1;
            if ((uVar2 & 1) != 0) {
              lVar5 = *(long *)(uVar2 + 7);
            }
            lVar5 = lVar5 << 1;
          }
          if (SCARRY8(lVar5,lVar5)) {
            lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
            *(long *)(lVar3 + 7) = lVar5;
          }
          return;
        }
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (1 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (0 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return;
    }
    lVar5 = *(long *)(in_x3 + 7);
    if (5 < lVar5) {
      if (8 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0xfe;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
        return;
      }
      if (7 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x2040810;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      if (6 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x204;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      return;
    }
    if (2 < lVar5) {
      if (4 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar5 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar5 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar5 = *(long *)(uVar2 + 7);
          }
          lVar5 = lVar5 * 0x2040810;
        }
        if (SCARRY8(lVar5,lVar5)) {
          lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
          *(long *)(lVar3 + 7) = lVar5;
        }
        return;
      }
      if (3 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0;
        puVar7[1] = in_x1;
        uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
        if ((uVar1 >> 4 & 1) == 0) {
          lVar5 = 0;
        }
        else {
          if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
             (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar5 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar5 = *(long *)(uVar2 + 7);
          }
          lVar5 = lVar5 * 0x204;
        }
        if (SCARRY8(lVar5,lVar5)) {
          lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
          *(long *)(lVar3 + 7) = lVar5;
        }
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
      if ((uVar1 >> 4 & 1) == 0) {
        lVar5 = 0;
      }
      else {
        if (((*(ulong *)(in_x15 + -0x20) & 1) != 0) &&
           (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        uVar2 = *(ulong *)(in_x15 + -0x20);
        lVar5 = (long)uVar2 >> 1;
        if ((uVar2 & 1) != 0) {
          lVar5 = *(long *)(uVar2 + 7);
        }
        lVar5 = lVar5 << 1;
      }
      if (SCARRY8(lVar5,lVar5)) {
        lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
        *(long *)(lVar3 + 7) = lVar5;
      }
      return;
    }
    if (1 < lVar5) {
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
      if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
         (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
        assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      return;
    }
    if (0 < lVar5) {
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      uVar1 = (**(code **)(unaff_x21 + (uVar2 - 0xff5) * 8))();
      if ((((uVar1 >> 4 & 1) != 0) && ((*(ulong *)(in_x15 + -0x20) & 1) != 0)) &&
         (1 < (*(ulong *)(*(ulong *)(in_x15 + -0x20) - 1) >> 0xc & 0xfffff) - 0x3c)) {
        assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      return;
    }
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return;
  }
  if (2 < lVar5) {
    if (4 < lVar5) {
      lVar5 = *(long *)(in_x3 + 7);
      if (5 < lVar5) {
        if (8 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0x1fffffffe;
          puVar7[1] = in_x1;
          (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return;
        }
        if (7 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          return;
        }
        if (6 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          return;
        }
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (2 < lVar5) {
        if (4 < lVar5) {
          return;
        }
        if (3 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          return;
        }
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (1 < lVar5) {
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (0 < lVar5) {
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return;
    }
    if (3 < lVar5) {
      lVar5 = *(long *)(in_x3 + 7);
      if (5 < lVar5) {
        if (8 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0x1fffe;
          puVar7[1] = in_x1;
          (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
          return;
        }
        if (7 < lVar5) {
          uVar2 = 0x3c;
          if ((in_x1 & 1) != 0) {
            uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
          }
          *puVar7 = 0x100010;
          puVar7[1] = in_x1;
          (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
          return;
        }
        if (6 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          return;
        }
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (2 < lVar5) {
        if (4 < lVar5) {
          if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
            assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
          }
          uVar2 = *(ulong *)(in_x15 + -0x20);
          lVar5 = (long)uVar2 >> 1;
          if ((uVar2 & 1) != 0) {
            lVar5 = *(long *)(uVar2 + 7);
          }
          lVar3 = lVar5 * 0x100;
          if (SCARRY8(lVar3,lVar3)) {
            lVar5 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 0x200);
            *(long *)(lVar5 + 7) = lVar3;
          }
          return;
        }
        if (3 < lVar5) {
          return;
        }
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (1 < lVar5) {
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      if (0 < lVar5) {
        if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
          assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
        }
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
      return;
    }
    lVar5 = *(long *)(in_x3 + 7);
    if (5 < lVar5) {
      if (8 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x1fe;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
        return;
      }
      if (7 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x1010100;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      if (6 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x100;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      return;
    }
    if (2 < lVar5) {
      if (4 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x2020202;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      if (3 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x202;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      return;
    }
    if (1 < lVar5) {
      if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      return;
    }
    if (0 < lVar5) {
      if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      return;
    }
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return;
  }
  if (1 < lVar5) {
    lVar5 = *(long *)(in_x3 + 7);
    if (5 < lVar5) {
      if (8 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 6;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
        return;
      }
      if (7 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x11111110;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      if (6 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x1110;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x10;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return;
    }
    if (2 < lVar5) {
      if (4 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x22222222;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      if (3 < lVar5) {
        uVar2 = 0x3c;
        if ((in_x1 & 1) != 0) {
          uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
        }
        *puVar7 = 0x2222;
        puVar7[1] = in_x1;
        (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
        return;
      }
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x22;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return;
    }
    if (1 < lVar5) {
      return;
    }
    if (0 < lVar5) {
      if (((in_x1 & 1) != 0) && (1 < (*(ulong *)(in_x1 - 1) >> 0xc & 0xfffff) - 0x3c)) {
        assert_type_is_int(*(undefined8 *)(unaff_x27 + 0x7b8));
      }
      return;
    }
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return;
  }
  if (lVar5 < 1) {
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = in_x1;
    uVar1 = (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    if ((uVar1 >> 4 & 1) == 0) {
      lVar5 = 0;
    }
    else {
      _ImmutableLinkedHashMapMixin___();
      if (extraout_x0 == unaff_x22) {
        uVar4 = stub__iso_stub_NullCastErrorSharedWithoutFPURegsStub();
        *(undefined8 **)(extraout_x15_22 + -0x10) = (undefined8 *)(in_x15 + -0x10);
        *(code **)(extraout_x15_22 + -8) = Pixel_undefined;
        new_PixelUndefined(uVar4,*(undefined8 *)(unaff_x27 + 0x3f58));
        return;
      }
      lVar5 = (long)extraout_x0 >> 1;
      if ((extraout_x0 & 1) != 0) {
        lVar5 = *(long *)(extraout_x0 + 7);
      }
    }
    if (SCARRY8(lVar5,lVar5)) {
      lVar3 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub(lVar5 * 2);
      *(long *)(lVar3 + 7) = lVar5;
    }
    return;
  }
  lVar5 = *(long *)(in_x3 + 7);
  if (5 < lVar5) {
    if (8 < lVar5) {
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 6;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 - 0xfef) * 8))();
      return;
    }
    if (7 < lVar5) {
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x55555554;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return;
    }
    if (6 < lVar5) {
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 0x5554;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return;
    }
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0x54;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
    return;
  }
  if (lVar5 < 3) {
    if (1 < lVar5) {
      uVar2 = 0x3c;
      if ((in_x1 & 1) != 0) {
        uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
      }
      *puVar7 = 10;
      puVar7[1] = in_x1;
      (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
      return;
    }
    if (0 < lVar5) {
      return;
    }
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 + 0x31e8) * 8))();
    return;
  }
  if (4 < lVar5) {
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0xaaaaaaaa;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
    return;
  }
  if (3 < lVar5) {
    uVar2 = 0x3c;
    if ((in_x1 & 1) != 0) {
      uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
    }
    *puVar7 = 0xaaaa;
    puVar7[1] = in_x1;
    (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
    return;
  }
  uVar2 = 0x3c;
  if ((in_x1 & 1) != 0) {
    uVar2 = *(ulong *)(in_x1 - 1) >> 0xc & 0xfffff;
  }
  *puVar7 = 0x96;
  puVar7[1] = in_x1;
  (**(code **)(unaff_x21 + (uVar2 - 0xffb) * 8))();
  return;
}

