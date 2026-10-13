class A {
  static var f;
}
f(A a) {
  a.f;
//  ^
// [diag.instanceAccessToStaticMember] The static getter 'f' can't be accessed through an instance.
}
