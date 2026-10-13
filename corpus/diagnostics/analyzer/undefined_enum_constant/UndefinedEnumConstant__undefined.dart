enum E { ONE }
E e() {
  return E.TWO;
//         ^^^
// [diag.undefinedEnumConstant] There's no constant named 'TWO' in 'E'.
}
