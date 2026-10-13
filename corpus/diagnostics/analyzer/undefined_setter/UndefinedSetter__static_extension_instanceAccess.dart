class C {}

extension E on C {
  static set a(int v) {}
}

f(C c) {
  c.a = 2;
//  ^
// [diag.undefinedSetter] The setter 'a' isn't defined for the type 'C'.
}
