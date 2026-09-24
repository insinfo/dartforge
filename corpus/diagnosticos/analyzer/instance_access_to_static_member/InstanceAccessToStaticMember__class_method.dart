class C {
  static void a() {}
}

f(C c) {
  c.a();
//  ^
// [diag.instanceAccessToStaticMember] The static method 'a' can't be accessed through an instance.
}
