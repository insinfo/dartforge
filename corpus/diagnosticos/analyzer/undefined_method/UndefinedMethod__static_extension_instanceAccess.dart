class C {}

extension E on C {
  static void a() {}
}

f(C c) {
  c.a();
//  ^
// [diag.undefinedMethod] The method 'a' isn't defined for the type 'C'.
}
