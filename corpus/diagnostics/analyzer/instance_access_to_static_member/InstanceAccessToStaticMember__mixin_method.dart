mixin A {
  static void a() {}
}

f(A a) {
  a.a();
//  ^
// [diag.instanceAccessToStaticMember] The static method 'a' can't be accessed through an instance.
}
