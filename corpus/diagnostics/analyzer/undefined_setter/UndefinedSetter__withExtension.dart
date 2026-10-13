class C {}

extension E on C {}

f(C c) {
  c.a = 1;
//  ^
// [diag.undefinedSetter] The setter 'a' isn't defined for the type 'C'.
}
