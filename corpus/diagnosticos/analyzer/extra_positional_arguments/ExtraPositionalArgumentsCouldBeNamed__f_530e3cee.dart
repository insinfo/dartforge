main() {
  (int x, {int y = 0}) {} (0, 1);
//                            ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 1 expected, but 2 found.
}
