enum _MyEnum {A, B}
//   ^^^^^^^
// [diag.unusedElement] The declaration '_MyEnum' isn't referenced.
void f(d) {
  d.A;
  d.B;
}
