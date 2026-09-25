class S {
  static set s(int i) {}
}
class C extends S {}
f(p) {
  f(C.s = 1);
//    ^
// [diag.undefinedSetter] The setter 's' isn't defined for the type 'C'.
}
