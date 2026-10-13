class A {}
class B extends A {
  m() {}
}
class C {
  f() {
    A a = new B();
    a.m();
//    ^
// [diag.undefinedMethod] The method 'm' isn't defined for the type 'A'.
  }
}
