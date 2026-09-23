class S {
  static int get g => 0;
}
class C extends S {}
f(p) {
  f(C.g);
//    ^
// [diag.undefinedGetter] The getter 'g' isn't defined for the type 'C'.
}