class A {
  static set f(x) {}
}
f(A a) {
  a.f = 42;
//  ^
// [diag.instanceAccessToStaticMember] The static setter 'f' can't be accessed through an instance.
}
