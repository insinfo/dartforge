m() {
  Function? x;
  x();
//^
// [diag.uncheckedInvocationOfNullableValue] The function can't be unconditionally invoked because it can be 'null'.
}
