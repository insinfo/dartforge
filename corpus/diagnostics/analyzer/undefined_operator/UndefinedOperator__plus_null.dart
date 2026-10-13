m() {
  Null x;
  x + 3;
//  ^
// [diag.invalidUseOfNullValue] An expression whose value is always 'null' can't be dereferenced.
}
