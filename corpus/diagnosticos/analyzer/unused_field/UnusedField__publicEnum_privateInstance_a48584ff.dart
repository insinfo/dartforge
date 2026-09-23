enum E {
  v;
  final int _foo = 0;
//          ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
}
