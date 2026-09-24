class A {
  const A(int p);
  const A.named(int p) : this();
//                            ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'A.new', but 0 found.
}
