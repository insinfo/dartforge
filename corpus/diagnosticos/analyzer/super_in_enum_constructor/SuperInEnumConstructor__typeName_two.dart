enum E {
  v;
  const E() : super(), super();
//            ^^^^^
// [diag.superInEnumConstructor] The enum constructor can't have a 'super' initializer.
//                     ^^^^^
// [diag.superInEnumConstructor] The enum constructor can't have a 'super' initializer.
}
