enum E {
  v;
//^
// [diag.undefinedEnumConstructorUnnamed] The enum doesn't have an unnamed constructor.
  const E.named();
}
