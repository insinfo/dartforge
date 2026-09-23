class A {
  int Function()? x;
  m(int this.x()) {}
//      ^^^^
// [diag.fieldInitializerOutsideConstructor] Field formal parameters can only be used in a constructor.
}
