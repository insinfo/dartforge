class A {
  A(int x, int y, {int? n});
}

void f() {
  A(5, n: 1);
//   ^
// [diag.notEnoughPositionalArgumentsNamePlural] 2 positional arguments expected by 'A.new', but 1 found.
}
