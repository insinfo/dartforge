class A {
  A(int x);
}
A f() => .new(1, 2);
//               ^
// [diag.extraPositionalArguments] Too many positional arguments: 1 expected, but 2 found.
