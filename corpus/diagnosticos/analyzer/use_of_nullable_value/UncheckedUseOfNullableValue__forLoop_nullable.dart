m() {
  List? x;
  for (var y in x) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'y' isn't used.
//              ^
// [diag.uncheckedUseOfNullableValueAsIterator] A nullable expression can't be used as an iterator in a for-in loop.
}
