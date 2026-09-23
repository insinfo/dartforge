void f(int? a) {
  a is int?;
//^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
}
