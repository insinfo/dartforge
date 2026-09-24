f({int xx = 0, int yy = 0, int zz = 0}) {}

main() {
  f(xx: 1, yy: 2, z);
//                ^
// [diag.undefinedIdentifier] Undefined name 'z'.
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 1 found.
}
