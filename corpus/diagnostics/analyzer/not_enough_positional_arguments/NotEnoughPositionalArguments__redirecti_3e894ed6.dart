class A {
  const A.named(int p);
  const A(int p) : this.named();
//                            ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'named', but 0 found.
}
