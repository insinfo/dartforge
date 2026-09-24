class A {
  A.named(int x);
}
A f() => .named(1, 2);
//                 ^
// [diag.extraPositionalArguments] Too many positional arguments: 1 expected, but 2 found.
