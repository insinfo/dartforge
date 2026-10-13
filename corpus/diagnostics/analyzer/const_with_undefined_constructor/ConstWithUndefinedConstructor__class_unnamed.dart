class A {
  const A.name();
}
f() {
  return const A();
//             ^
// [diag.constWithUndefinedConstructorDefault] The class 'A' doesn't have an unnamed constant constructor.
}
