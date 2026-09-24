f({x, y}) {}
main() {
  f(0, 1, '2');
//  ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 3 found.
}
