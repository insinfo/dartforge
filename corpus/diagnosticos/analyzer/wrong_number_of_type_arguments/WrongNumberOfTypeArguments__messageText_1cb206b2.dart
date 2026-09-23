class Foo<X> {
  Foo.bar();
}

main() {
  Foo.bar<int>();
//       ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'Foo.bar' doesn't have type parameters.
}
