class A {
  static A make(int x, int y) => throw 0;
}
A f() => .make(1);
//              ^
// [diag.notEnoughPositionalArgumentsNamePlural] 2 positional arguments expected by 'make', but 1 found.
