void f<T extends Object>(Null n) {
  n as T;
//^^^^^^
// [diag.castFromNullAlwaysFails] This cast always throws an exception because the expression always evaluates to 'null'.
}
