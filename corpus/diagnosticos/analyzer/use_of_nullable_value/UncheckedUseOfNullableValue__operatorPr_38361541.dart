m() {
  int? x;
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  --x;
//^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '-' can't be unconditionally invoked because the receiver can be 'null'.
}
