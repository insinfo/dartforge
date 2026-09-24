class A {
  const A(int p);
}
main() {
  const A(p: 0);
//        ^
// [diag.undefinedNamedParameter] The named parameter 'p' isn't defined.
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'A.new', but 0 found.
}
