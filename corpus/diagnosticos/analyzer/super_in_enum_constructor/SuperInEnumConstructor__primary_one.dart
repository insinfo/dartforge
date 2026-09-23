enum E() {
  v;
  this : super();
//       ^^^^^
// [diag.superInEnumConstructor] The enum constructor can't have a 'super' initializer.
}
