class A {
  external final int x;
  A(this.x);
//       ^
// [diag.externalFieldConstructorInitializer] External fields can't have initializers.
}
