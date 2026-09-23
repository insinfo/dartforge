class C {}

extension E1 on C {
  int foo(int x) => 1;
}

extension E2 on C {
  static set foo(int x) {}

  void f() {
    foo();
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
  }
}
