class A {
  A(int x);
}
A f() => .new();
//            ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'new', but 0 found.
