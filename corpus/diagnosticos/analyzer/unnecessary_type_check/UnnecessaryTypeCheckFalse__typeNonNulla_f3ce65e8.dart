void f(int a) {
  a is! num;
//^^^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
}
