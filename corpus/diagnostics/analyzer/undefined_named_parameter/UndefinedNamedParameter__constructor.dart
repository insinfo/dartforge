class A {
  A();
}
main() {
  A(p: 0);
//  ^
// [diag.undefinedNamedParameter] The named parameter 'p' isn't defined.
}
