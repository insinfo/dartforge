class A {
  int? x;
  m(this.x) {}
//  ^^^^
// [diag.fieldInitializerOutsideConstructor] Field formal parameters can only be used in a constructor.
}
