class T {
  static int get foo => 42;
}
main() {
  T..foo;
//   ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'Type'.
}
