class A {
  static A make(int x) => throw 0;
}
A f() => .make(1, 2);
//                ^
// [diag.extraPositionalArguments] Too many positional arguments: 1 expected, but 2 found.
