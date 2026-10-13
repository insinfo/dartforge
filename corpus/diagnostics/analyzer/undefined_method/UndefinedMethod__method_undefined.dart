class C {
  f() {
    abs();
//  ^^^
// [diag.undefinedMethod] The method 'abs' isn't defined for the type 'C'.
  }
}
