class A {
  const A(int p);
}
@A()
// ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'A.new', but 0 found.
void f() {
}
