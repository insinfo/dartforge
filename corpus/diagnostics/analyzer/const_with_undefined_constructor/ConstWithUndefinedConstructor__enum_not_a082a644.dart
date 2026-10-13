void f() {
  const E.v();
//        ^
// [diag.constWithUndefinedConstructor] The class 'E' doesn't have a constant constructor 'v'.
}

enum E {
  v
}
