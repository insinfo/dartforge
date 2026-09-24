void f<T>(T a) {
  a is! Object?;
//^^^^^^^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
}
