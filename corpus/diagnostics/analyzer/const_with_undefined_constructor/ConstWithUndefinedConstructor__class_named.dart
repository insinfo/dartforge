class A {
  const A();
}
f() {
  return const A.noSuchConstructor();
//               ^^^^^^^^^^^^^^^^^
// [diag.constWithUndefinedConstructor] The class 'A' doesn't have a constant constructor 'noSuchConstructor'.
}
