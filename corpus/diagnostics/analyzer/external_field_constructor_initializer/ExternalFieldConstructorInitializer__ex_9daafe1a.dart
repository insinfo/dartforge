class A {
  external int x;
  A(this.x);
//       ^
// [diag.externalFieldConstructorInitializer] External fields can't have initializers.
}
