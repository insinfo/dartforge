m() {
  Null x;
  for (var y in x) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'y' isn't used.
//              ^
// [diag.invalidUseOfNullValue] An expression whose value is always 'null' can't be dereferenced.
}
