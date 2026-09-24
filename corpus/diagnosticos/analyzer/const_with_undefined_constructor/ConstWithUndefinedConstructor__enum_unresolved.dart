void f() {
  const E.foo();
//        ^^^
// [diag.constWithUndefinedConstructor] The class 'E' doesn't have a constant constructor 'foo'.
}

enum E {
  v
}
