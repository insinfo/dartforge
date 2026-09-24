extension A on int {
  external static final int v = 0;
//                          ^
// [diag.externalFieldInitializer] External fields can't have initializers.
}
