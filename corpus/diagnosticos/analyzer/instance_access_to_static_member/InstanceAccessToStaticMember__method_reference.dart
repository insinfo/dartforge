class A {
  static m() {}
}
f(A a) {
  a.m;
//  ^
// [diag.instanceAccessToStaticMember] The static method 'm' can't be accessed through an instance.
}
