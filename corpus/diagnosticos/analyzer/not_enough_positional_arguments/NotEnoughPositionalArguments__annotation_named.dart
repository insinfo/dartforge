class A {
  const A.named(int p);
}
@A.named()
//       ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'named', but 0 found.
void f() {
}
