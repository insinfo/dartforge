void f(void x) {
  for (var v in x) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
//              ^
// [diag.uncheckedUseOfNullableValueAsIterator] A nullable expression can't be used as an iterator in a for-in loop.
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
