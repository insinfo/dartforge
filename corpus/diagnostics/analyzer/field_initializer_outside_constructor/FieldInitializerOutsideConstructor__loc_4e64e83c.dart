void f() {
  void foo({required this.x}) {}
//                   ^^^^
// [diag.fieldInitializerOutsideConstructor] Field formal parameters can only be used in a constructor.
  foo(x: 0);
}
