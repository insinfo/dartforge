extension type A(int it) {
  external static final int v = 0;
//                          ^
// [diag.externalFieldInitializer] External fields can't have initializers.
}
