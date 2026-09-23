enum A {
  e;
  external static int v = 0;
//                    ^
// [diag.externalFieldInitializer] External fields can't have initializers.
}
