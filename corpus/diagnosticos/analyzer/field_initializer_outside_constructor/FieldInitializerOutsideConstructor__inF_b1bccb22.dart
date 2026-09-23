class A {
  int? x;
  A(int p(this.x));
//        ^^^^
// [diag.fieldInitializerOutsideConstructor] Field formal parameters can only be used in a constructor.
}
