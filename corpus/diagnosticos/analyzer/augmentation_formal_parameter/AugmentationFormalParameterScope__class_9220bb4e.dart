class A(int p1);
augment class A(int _) {
  final int value = p1;
//                  ^^
// [diag.undefinedIdentifier] Undefined name 'p1'.
}
