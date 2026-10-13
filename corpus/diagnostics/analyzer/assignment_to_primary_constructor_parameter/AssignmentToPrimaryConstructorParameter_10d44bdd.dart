class A(int x) {
  late int y = x = 0;
//             ^
// [diag.undefinedIdentifier] Undefined name 'x'.
}
