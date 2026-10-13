void f<F extends Function>(List<F?> funcList) {
  funcList[0]();
//^^^^^^^^^^^
// [diag.uncheckedInvocationOfNullableValue] The function can't be unconditionally invoked because it can be 'null'.
}
