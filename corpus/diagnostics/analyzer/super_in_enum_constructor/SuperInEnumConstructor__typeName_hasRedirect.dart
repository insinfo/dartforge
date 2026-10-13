enum E {
  v;
  const E.named();
  const E() : this.named(), super();
//                          ^^^^^
// [diag.superInEnumConstructor] The enum constructor can't have a 'super' initializer.
}
