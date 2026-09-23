class C {}

extension E on C {
  void a() {}
}

f(C c) {
  c.c();
//  ^
// [diag.undefinedMethod] The method 'c' isn't defined for the type 'C'.
}
