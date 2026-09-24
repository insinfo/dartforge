class C {
  void f() {
    foo();
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
  }
}

extension E on C {
  int foo() => 1;
}

set foo(int x) {}
