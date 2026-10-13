class T {
  static void set foo(_) {}
}
main() {
  T..foo = 42;
//   ^^^
// [diag.undefinedSetter] The setter 'foo' isn't defined for the type 'Type'.
}
