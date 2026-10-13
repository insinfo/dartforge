int A = 0;
f() {
  return const A();
//             ^
// [diag.constWithNonType] The name 'A' isn't a class.
}
