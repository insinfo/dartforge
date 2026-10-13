class A {
  final int value;
  A(int p1);
  augment A(int _) : value = p1;
//                           ^^
// [diag.undefinedIdentifier] Undefined name 'p1'.
}
