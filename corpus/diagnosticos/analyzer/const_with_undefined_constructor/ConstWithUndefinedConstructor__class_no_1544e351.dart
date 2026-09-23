class A {
  const A.name();
}
typedef B = A;
f() {
  return const B();
//             ^
// [diag.constWithUndefinedConstructorDefault] The class 'B' doesn't have an unnamed constant constructor.
}
