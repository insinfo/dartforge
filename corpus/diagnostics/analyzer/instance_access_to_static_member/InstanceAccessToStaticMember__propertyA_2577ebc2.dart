class A {
  static get f => 42;
}
f(A a) {
  a.f;
//  ^
// [diag.instanceAccessToStaticMember] The static getter 'f' can't be accessed through an instance.
}
