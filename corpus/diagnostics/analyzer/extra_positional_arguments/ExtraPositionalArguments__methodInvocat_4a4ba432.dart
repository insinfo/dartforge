f() {}
main() {
  f(0, 1, '2');
//  ^
// [diag.extraPositionalArguments] Too many positional arguments: 0 expected, but 3 found.
}
