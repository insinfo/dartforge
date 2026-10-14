
/* WARNING: Control flow encountered bad instruction data */

void entry(undefined8 param_1,undefined8 param_2)

{
  undefined1 in_ZR;
  undefined1 uVar1;
  bool bVar2;
  undefined1 uVar3;
  uint uVar4;
  long lVar5;
  undefined8 uVar6;
  uint extraout_w8;
  int extraout_w8_00;
  long extraout_x8;
  long extraout_x8_00;
  long extraout_x8_01;
  long extraout_x9;
  char *unaff_x19;
  long unaff_x20;
  ulong uVar7;
  
  uVar7 = tpidrro_el0;
  func_0x00010165ed4c(uVar7 & 0xfffffffffffffff8);
  uVar1 = !(bool)in_ZR;
  bVar2 = (bool)in_ZR || unaff_x19 == (char *)0x0;
  uVar3 = bVar2;
  if (bVar2) {
    func_0x00010165dbb4();
  }
  func_0x00010165e144();
  if (((bool)uVar1) || (func_0x00010165fd48(), unaff_x20 == 0)) {
    unaff_x20 = func_0x00010165dd70();
  }
  if (bVar2) {
    func_0x00010165fd00();
  }
  func_0x00010165eb30();
  if (!(bool)uVar1) {
    func_0x0001000e527c();
    halt_baddata();
  }
  func_0x00010165e420();
  if (extraout_x8 == 0) {
    func_0x000101661108();
    if (*(char *)(unaff_x20 + 600) == '\0') {
      uVar7 = func_0x000101622e90();
    }
    else {
      uVar7 = *(ulong *)(unaff_x20 + 0x250);
    }
    uVar6 = func_0x00010165ffb8();
    if ((bool)uVar3) {
      uVar6 = func_0x00010165e048();
      lVar5 = extraout_x9;
      if (!(bool)uVar3) {
        lVar5 = extraout_x8_00;
      }
      bVar2 = *(long *)(unaff_x20 + 0x4e70) == lVar5;
      if ((!bVar2) || (uVar6 = func_0x000101661764(*(undefined8 *)(unaff_x20 + 0x4e78)), !bVar2))
      goto LAB_1016420e8;
      uVar4 = extraout_w8 & 1;
    }
    else {
LAB_1016420e8:
      uVar4 = func_0x00010165fe7c(0x4e70,uVar6,param_2,uVar7);
    }
    uVar3 = (uVar4 & 0xff) == 0;
    if ((bool)uVar3) {
      func_0x000101660470(0x1019fa000);
      func_0x00010165e3f0(extraout_x8_01 + 0x208);
      if (*unaff_x19 != '\0') goto LAB_101642194;
    }
    func_0x000101612c04();
    func_0x00010165f2d0();
    if (extraout_w8_00 == 0) {
      lVar5 = func_0x000101662c30();
      if (lVar5 == 0) {
        lVar5 = 1;
      }
      else if ((uVar7 & 1) == 0) {
        func_0x00010165ecd8();
        if ((bool)uVar3) {
          lVar5 = (long)*(int *)(uVar7 + 2);
        }
        else {
          lVar5 = func_0x00010165fcf8();
        }
      }
      else {
        lVar5 = 2;
      }
      func_0x00010165ef68(lVar5);
      if ((bool)uVar3) {
        uVar6 = *(undefined8 *)(unaff_x20 + 0x4e88);
      }
      else {
        uVar6 = func_0x00010165e660();
      }
      uVar6 = func_0x00010165e924(uVar6);
      if (*unaff_x19 == '\0') goto LAB_101642198;
    }
  }
  else {
    func_0x0001000dd13c();
  }
LAB_101642194:
  uVar6 = 0;
LAB_101642198:
  func_0x00010165fd74(uVar6);
                    /* WARNING: Bad instruction - Truncating control flow here */
  halt_baddata();
}


