abstract class A {
  void f(int p1);
  augment void f(int _) {
    p1;
//  ^^
// [diag.undefinedIdentifier] Undefined name 'p1'.
  }
}
