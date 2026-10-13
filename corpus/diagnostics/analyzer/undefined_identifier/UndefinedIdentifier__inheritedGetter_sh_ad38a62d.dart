class A {
  int get foo => 0;
}

void set foo(int _) {}

class B extends A {
  void bar() {
    foo;
//  ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
  }
}
