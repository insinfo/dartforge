class C {
  static void m() {}
}
extension on int {
  foo(C c) {
    c.m(); // ERROR
//    ^
// [diag.instanceAccessToStaticMember] The static method 'm' can't be accessed through an instance.
  }
}
test(int i) {
  i.foo(C());
}
