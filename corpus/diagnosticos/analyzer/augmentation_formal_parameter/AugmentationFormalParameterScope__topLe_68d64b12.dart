void f(int x);
augment void f(int _) {
  x;
//^
// [diag.undefinedIdentifier] Undefined name 'x'.
}
