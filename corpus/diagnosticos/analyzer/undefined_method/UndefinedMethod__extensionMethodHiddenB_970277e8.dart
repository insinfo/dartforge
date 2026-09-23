class C {
  void f() {
    foo();
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
  }
  static set foo(int x) {}
}

extension E on C {
  int foo() => 1;
}

