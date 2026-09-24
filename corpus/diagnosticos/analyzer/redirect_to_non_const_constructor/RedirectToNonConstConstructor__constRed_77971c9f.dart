class A {
  A.a();
  const factory A.b() = A.a;
//                      ^^^
// [diag.redirectToNonConstConstructor] A constant redirecting constructor can't redirect to a non-constant constructor.
}
