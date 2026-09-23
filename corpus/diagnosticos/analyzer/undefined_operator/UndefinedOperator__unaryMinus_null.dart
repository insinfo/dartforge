m() {
  Null x;
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  -x;
//^
// [diag.invalidUseOfNullValue] An expression whose value is always 'null' can't be dereferenced.
}
