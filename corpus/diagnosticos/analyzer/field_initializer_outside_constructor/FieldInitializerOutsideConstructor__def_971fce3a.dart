class A {
  int x = 0;
  m([this.x = 0]) {}
//   ^^^^
// [diag.fieldInitializerOutsideConstructor] Field formal parameters can only be used in a constructor.
}
