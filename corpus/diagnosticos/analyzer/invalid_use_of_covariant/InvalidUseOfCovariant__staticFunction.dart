class C {
  static void m(covariant int x) {}
//              ^^^^^^^^^
// [diag.extraneousModifier] Can't have modifier 'covariant' here.
}
