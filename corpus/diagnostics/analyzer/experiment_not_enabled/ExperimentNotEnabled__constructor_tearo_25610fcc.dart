// %before-language-feature: constructor-tearoffs
class Foo<X> {
  const Foo.bar();
  int get baz => 0;
}
main() {
  Foo<int>.bar.baz();
//   ^^^^^
// [diag.experimentNotEnabled] This requires the 'constructor-tearoffs' language feature to be enabled.
//             ^^^
// [diag.undefinedMethod] The method 'baz' isn't defined for the type 'Foo<int> Function()'.
}
