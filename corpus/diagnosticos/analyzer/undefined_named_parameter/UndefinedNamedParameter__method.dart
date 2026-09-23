class A {
  m() {}
}
main() {
  A().m(p: 0);
//      ^
// [diag.undefinedNamedParameter] The named parameter 'p' isn't defined.
}
