class A {
  external final int x;
  A() : x = 0;
//      ^
// [diag.externalFieldConstructorInitializer] External fields can't have initializers.
}
