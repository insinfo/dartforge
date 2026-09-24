class A {
  const A();
}
main() {
  const A(0);
//        ^
// [diag.extraPositionalArguments] Too many positional arguments: 0 expected, but 1 found.
}
