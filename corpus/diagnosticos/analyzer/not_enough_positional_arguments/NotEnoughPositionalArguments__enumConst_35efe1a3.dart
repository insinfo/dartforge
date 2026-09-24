enum E {
  v;
//^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'E', but 0 found.
  const E(int a);
}
