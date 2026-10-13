m() {
  Null x;
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  x -= 1;
//  ^^
// [diag.invalidUseOfNullValue] An expression whose value is always 'null' can't be dereferenced.
}
