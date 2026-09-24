class A {
  A.named(int x);
}
A f() => .named();
//              ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'named', but 0 found.
