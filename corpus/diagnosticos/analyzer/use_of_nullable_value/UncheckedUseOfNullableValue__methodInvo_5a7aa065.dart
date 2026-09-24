m(Function? x) {
  x.call();
//  ^^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'call' can't be unconditionally invoked because the receiver can be 'null'.
}
