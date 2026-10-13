class C {}

extension E on C {}

f(C c) {
  c.a;
//  ^
// [diag.undefinedGetter] The getter 'a' isn't defined for the type 'C'.
}
