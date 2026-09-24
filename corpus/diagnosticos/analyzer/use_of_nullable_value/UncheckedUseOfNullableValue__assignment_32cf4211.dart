m(int x, int? y) {
  x += 0;
  y += 0;
//  ^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '+' can't be unconditionally invoked because the receiver can be 'null'.
}
