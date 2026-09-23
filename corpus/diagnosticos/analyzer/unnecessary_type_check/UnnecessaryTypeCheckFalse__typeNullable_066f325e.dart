void f(int? a) {
  a is! int?;
//^^^^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
}
