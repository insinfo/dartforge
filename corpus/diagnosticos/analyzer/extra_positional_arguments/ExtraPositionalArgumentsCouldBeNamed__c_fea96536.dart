class A {
  const A({int x = 0});
}
main() {
  const A(0);
//        ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 1 found.
}
