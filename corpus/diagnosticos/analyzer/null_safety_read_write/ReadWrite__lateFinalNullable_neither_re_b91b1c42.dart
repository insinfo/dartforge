void f(bool b) {
  // ignore:unused_local_variable
  late final int? x;
  if (b) x = 0;
  x++;
// ^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '+' can't be unconditionally invoked because the receiver can be 'null'.
}
