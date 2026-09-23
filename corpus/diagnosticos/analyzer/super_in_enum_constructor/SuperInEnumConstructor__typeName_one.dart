enum E {
  v;
  const E() : super();
//            ^^^^^
// [diag.superInEnumConstructor] The enum constructor can't have a 'super' initializer.
}
