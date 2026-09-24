class A {}
class B {}

void f(A x) {
  if (x case var z as B) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'z' isn't used.
}
