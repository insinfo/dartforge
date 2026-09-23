T f<T>() => throw '$T';
g({int? named}) {}
main() {
  g(f());
//  ^^^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 0 expected, but 1 found.
}
