m<T extends Function>(List<T?> x) {
  x.first();
//^^^^^^^
// [diag.uncheckedInvocationOfNullableValue] The function can't be unconditionally invoked because it can be 'null'.
}
