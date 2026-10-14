
/* WARNING: Unknown calling convention -- yet parameter storage is locked */

void _JsonStringifier_writeStringContent(void)

{
  long lVar1;
  ulong uVar2;
  long in_x1;
  long extraout_x1;
  long in_x2;
  long lVar3;
  ulong uVar4;
  long lVar5;
  ulong uVar6;
  ulong extraout_x8;
  long lVar7;
  long extraout_x9;
  ulong uVar8;
  long in_x15;
  long *plVar9;
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
  long extraout_x15_09;
  long *extraout_x15_10;
  long *extraout_x15_11;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  undefined1 auVar10 [16];
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  plVar9 = (long *)(in_x15 + -0x58);
  *(long *)(in_x15 + -0x40) = in_x1;
  *(long *)(in_x15 + -0x48) = in_x2;
  if (plVar9 <= *(long **)(unaff_x26 + 0x38)) {
    stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
    plVar9 = extraout_x15_07;
  }
  uVar4 = *(long *)(in_x2 + 7) >> 1;
  *(ulong *)(in_x15 + -0x38) = uVar4;
  lVar5 = (*(ulong *)(in_x2 + -1) >> 0xc & 0xfffff) << 1;
  *(long *)(in_x15 + -0x30) = lVar5;
  *(undefined8 *)(in_x15 + -0x28) = *(undefined8 *)(in_x1 + 0x17);
  lVar3 = 0;
  lVar7 = 0;
  do {
    uVar6 = 0xfc00;
    *(long *)(in_x15 + -0x20) = lVar7;
    if (plVar9 <= *(long **)(unaff_x26 + 0x38)) {
      stub__iso_stub_StackOverflowSharedWithoutFPURegsStub();
      uVar6 = extraout_x8;
      lVar7 = extraout_x9;
      plVar9 = extraout_x15_08;
    }
    if ((long)uVar4 <= lVar7) {
      if (lVar3 == 0) {
        _JsonStringStringifier_writeString();
      }
      else if (lVar3 < *(long *)(in_x15 + -0x38)) {
        _JsonStringStringifier_writeStringSlice();
      }
      return;
    }
    if (lVar5 == 0xbc) {
      uVar2 = (ulong)*(byte *)(in_x2 + lVar7 + 0xf);
    }
    else {
      uVar2 = (ulong)*(ushort *)(in_x2 + lVar7 * 2 + 0xf);
    }
    *(ulong *)(in_x15 + -0x18) = uVar2;
    if (uVar2 < 0x5d) {
      if (uVar2 < 0x20) {
        if (lVar3 < *(long *)(in_x15 + -0x20)) {
          *plVar9 = *(long *)(in_x15 + -0x20) << 1;
          _StringBase_substring();
          StringBuffer_write();
        }
        *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x20) + 1;
        StringBuffer_writeCharCode();
        uVar4 = *(ulong *)(in_x15 + -0x18);
        if ((long)uVar4 < 0xb) {
          if ((long)uVar4 < 10) {
            if ((long)uVar4 < 9) {
              if ((uVar4 & 0x7fffffffffffffff) != 8) goto code_r0x00179844;
              StringBuffer_writeCharCode();
              plVar9 = extraout_x15_00;
            }
            else {
              StringBuffer_writeCharCode();
              plVar9 = extraout_x15_01;
            }
          }
          else {
            StringBuffer_writeCharCode();
            plVar9 = extraout_x15_02;
          }
        }
        else if ((long)uVar4 < 0xc) {
code_r0x00179844:
          StringBuffer_writeCharCode();
          StringBuffer_writeCharCode();
          StringBuffer_writeCharCode();
          StringBuffer_writeCharCode();
          StringBuffer_writeCharCode();
          plVar9 = extraout_x15_05;
        }
        else if ((long)uVar4 < 0xd) {
          StringBuffer_writeCharCode();
          plVar9 = extraout_x15_03;
        }
        else {
          if ((uVar4 & 0x7fffffffffffffff) != 0xd) goto code_r0x00179844;
          StringBuffer_writeCharCode();
          plVar9 = extraout_x15_04;
        }
        lVar3 = *(long *)(in_x15 + -0x50);
      }
      else if ((uVar2 == 0x22) || (uVar2 == 0x5c)) {
        if (lVar3 < *(long *)(in_x15 + -0x20)) {
          *plVar9 = *(long *)(in_x15 + -0x20) << 1;
          _StringBase_substring();
          StringBuffer_write();
        }
        *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x20) + 1;
        StringBuffer_writeCharCode();
        StringBuffer_writeCharCode();
        lVar3 = *(long *)(in_x15 + -0x50);
        plVar9 = extraout_x15_06;
      }
    }
    else if (0xd7ff < uVar2) {
      if ((uVar2 & uVar6) == 0xd800) {
        lVar1 = lVar7 + 1;
        if (lVar1 < (long)uVar4) {
          if (lVar5 == 0xbc) {
            uVar8 = (ulong)*(byte *)(in_x2 + lVar1 + 0xf);
          }
          else {
            uVar8 = (ulong)*(ushort *)(in_x2 + lVar1 * 2 + 0xf);
          }
          if ((uVar8 & uVar6) == 0xdc00) goto code_r0x001795ac;
        }
      }
      else {
code_r0x001795ac:
        if ((uVar2 & uVar6) != 0xdc00) goto code_r0x00179974;
        uVar2 = lVar7 - 1;
        if (-1 < (long)uVar2) {
          if (uVar4 <= uVar2) {
            auVar10 = stub__iso_stub_RangeErrorSharedWithoutFPURegsStub();
            lVar7 = auVar10._8_8_;
            *(undefined8 **)(extraout_x15_09 + -0x10) = (undefined8 *)(in_x15 + -0x10);
            *(code **)(extraout_x15_09 + -8) = _JsonStringStringifier_writeStringSlice;
            plVar9 = (long *)(extraout_x15_09 + -0x20);
            if (plVar9 <= *(long **)(unaff_x26 + 0x38)) {
              stub__iso_stub_StackOverflowSharedWithoutFPURegsStub(auVar10._0_8_,lVar7,in_x2,lVar3);
              lVar7 = extraout_x1;
              plVar9 = extraout_x15_11;
            }
            *(undefined8 *)(extraout_x15_09 + -0x18) = *(undefined8 *)(lVar7 + 0x17);
            lVar7 = uVar4 * 2;
            if (SCARRY8(uVar4,uVar4)) {
              lVar7 = stub__iso_stub_AllocateMintSharedWithoutFPURegsStub();
              *(ulong *)(lVar7 + 7) = uVar4;
              plVar9 = extraout_x15_10;
            }
            *plVar9 = lVar7;
            _StringBase_substring();
            StringBuffer_write();
            return;
          }
          if (lVar5 == 0xbc) {
            uVar4 = (ulong)*(byte *)(in_x2 + uVar2 + 0xf);
          }
          else {
            uVar4 = (ulong)*(ushort *)(in_x2 + uVar2 * 2 + 0xf);
          }
          if ((uVar4 & uVar6) == 0xd800) goto code_r0x00179974;
        }
      }
      if (lVar3 < lVar7) {
        *plVar9 = lVar7 << 1;
        _StringBase_substring();
        StringBuffer_write();
      }
      *(long *)(in_x15 + -0x50) = *(long *)(in_x15 + -0x20) + 1;
      StringBuffer_writeCharCode();
      StringBuffer_writeCharCode();
      StringBuffer_writeCharCode();
      StringBuffer_writeCharCode();
      StringBuffer_writeCharCode();
      StringBuffer_writeCharCode();
      lVar3 = *(long *)(in_x15 + -0x50);
      plVar9 = extraout_x15;
    }
code_r0x00179974:
    lVar7 = *(long *)(in_x15 + -0x20) + 1;
    in_x2 = *(long *)(in_x15 + -0x48);
    lVar5 = *(long *)(in_x15 + -0x30);
    uVar4 = *(ulong *)(in_x15 + -0x38);
  } while( true );
}

