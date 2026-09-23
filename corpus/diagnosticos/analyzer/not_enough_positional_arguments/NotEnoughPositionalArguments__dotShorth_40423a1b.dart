class A {
  A.named(int x, int y);
}
A f() => .named(1);
//               ^
// [diag.notEnoughPositionalArgumentsNamePlural] 2 positional arguments expected by 'named', but 1 found.
