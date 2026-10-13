void f(Null n) {
  n as Never;
//^^^^^^^^^^
// [diag.castFromNullAlwaysFails] This cast always throws an exception because the expression always evaluates to 'null'.
}
