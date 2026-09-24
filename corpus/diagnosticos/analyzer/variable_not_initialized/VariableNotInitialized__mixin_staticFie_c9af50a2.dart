mixin A {
  external static const int v = 0;
//                          ^
// [diag.externalFieldInitializer] External fields can't have initializers.
}
