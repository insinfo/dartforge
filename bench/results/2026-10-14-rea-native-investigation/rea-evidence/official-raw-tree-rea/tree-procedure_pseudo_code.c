
void entry(undefined8 param_1,long param_2)

{
  undefined8 uVar1;
  long lVar2;
  long extraout_x1;
  long in_x15;
  long unaff_x26;
  undefined8 unaff_x29;
  undefined8 unaff_x30;
  
  *(undefined8 *)(in_x15 + -0x10) = unaff_x29;
  *(undefined8 *)(in_x15 + -8) = unaff_x30;
  if (in_x15 - 0x28U <= *(ulong *)(unaff_x26 + 0x38)) {
    func_0x0008de34();
    param_2 = extraout_x1;
  }
  if (param_2 == 0) {
    func_0x0009d398();
  }
  else {
    param_2 = param_2 + -1;
    *(long *)(in_x15 + -0x18) = param_2;
    uVar1 = entry(param_2,param_2);
    *(undefined8 *)(in_x15 + -0x20) = uVar1;
    uVar1 = entry(uVar1,*(undefined8 *)(in_x15 + -0x18));
    *(undefined8 *)(in_x15 + -0x28) = uVar1;
    lVar2 = func_0x0009d398();
    *(undefined8 *)(lVar2 + 7) = *(undefined8 *)(in_x15 + -0x20);
    *(undefined8 *)(lVar2 + 0xf) = *(undefined8 *)(in_x15 + -0x28);
  }
  return;
}

