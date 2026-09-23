class A {
  const A({int x = 0});
}
typedef B = A;
main() {
  const B(0);
//        ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 1 found.
}
