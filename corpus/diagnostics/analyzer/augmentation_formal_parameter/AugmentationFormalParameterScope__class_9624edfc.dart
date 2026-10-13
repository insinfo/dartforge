class A {
  A(int p1);
  augment A(int _) {
    p1;
//  ^^
// [diag.undefinedIdentifier] Undefined name 'p1'.
  }
}
