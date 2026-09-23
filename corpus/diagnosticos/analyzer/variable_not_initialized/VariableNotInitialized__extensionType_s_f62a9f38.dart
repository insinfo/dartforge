extension type A(int it) {
  external static int v = 0;
//                    ^
// [diag.externalFieldInitializer] External fields can't have initializers.
}
