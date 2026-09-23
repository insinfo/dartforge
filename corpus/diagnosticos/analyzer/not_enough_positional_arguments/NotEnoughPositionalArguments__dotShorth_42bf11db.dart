class A {
  static A make(int x) => throw 0;
}
A f() => .make();
//             ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'make', but 0 found.
