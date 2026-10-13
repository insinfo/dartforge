class A {
  A.named(int x, int y, {int? n});
}

void f() {
  A.named(5, n: 1);
//         ^
// [diag.notEnoughPositionalArgumentsNamePlural] 2 positional arguments expected by 'named', but 1 found.
}
