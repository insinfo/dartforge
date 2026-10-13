void f(Null n, num m) {
  (m as int) = n;
// ^^^^^^^^
// [diag.castFromNullAlwaysFails] This cast always throws an exception because the expression always evaluates to 'null'.
}
