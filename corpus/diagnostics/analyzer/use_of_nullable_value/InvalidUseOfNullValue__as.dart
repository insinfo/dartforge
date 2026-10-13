m() {
  Null x;
  x as int;
//^^^^^^^^
// [diag.castFromNullAlwaysFails] This cast always throws an exception because the expression always evaluates to 'null'.
}
