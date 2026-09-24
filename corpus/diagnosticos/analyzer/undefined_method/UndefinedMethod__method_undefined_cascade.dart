class C {}
f(C c) {
  c..abs();
//   ^^^
// [diag.undefinedMethod] The method 'abs' isn't defined for the type 'C'.
}
