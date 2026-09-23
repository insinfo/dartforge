class C {}

extension E on C {
  static int get a => 0;
}

C g(C c) => C();
f(C c) {
  g(c).a;
//     ^
// [diag.undefinedGetter] The getter 'a' isn't defined for the type 'C'.
}
