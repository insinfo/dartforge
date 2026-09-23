void f<T>(T a) {
  a is dynamic;
//^^^^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
}
