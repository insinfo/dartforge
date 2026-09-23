class A {
  static int? x;
  A([this.x = 0]) {}
//   ^^^^
// [diag.initializerForStaticField] 'x' is a static field in the enclosing class. Fields initialized in a constructor can't be static.
}
