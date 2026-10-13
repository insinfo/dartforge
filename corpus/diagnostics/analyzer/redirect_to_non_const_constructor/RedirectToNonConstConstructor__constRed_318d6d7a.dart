class A {
  const factory A.b() = A.a;
//                      ^^^
// [diag.redirectToMissingConstructor] The constructor 'A.a' couldn't be found in 'A'.
}
