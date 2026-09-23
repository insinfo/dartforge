void f(void x, y) {
  for (y in x) {}
//          ^
// [diag.uncheckedUseOfNullableValueAsIterator] A nullable expression can't be used as an iterator in a for-in loop.
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
