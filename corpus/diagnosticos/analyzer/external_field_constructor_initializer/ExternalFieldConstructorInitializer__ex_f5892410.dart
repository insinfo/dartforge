class A {
  external int x;
  A() : x = 0;
//      ^
// [diag.externalFieldConstructorInitializer] External fields can't have initializers.
}
