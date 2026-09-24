mixin M {
  num foo() => 0;
}
enum E with M {
//   ^
// [diag.invalidImplementationOverride] 'M.foo' ('num Function()') isn't a valid concrete implementation of 'E.foo' ('int Function()').
  v;
  int foo();
}
