class A {
  const A();
}
main() {
  const A(p: 0);
//        ^
// [diag.undefinedNamedParameter] The named parameter 'p' isn't defined.
}
