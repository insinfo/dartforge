class A {
  const A(int p);
}
const a = A();
//          ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'A.new', but 0 found.
@a
void f() {
}
