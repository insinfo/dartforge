void f<T>(T a) {
  a is! dynamic;
//^^^^^^^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
}
