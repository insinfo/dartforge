class A {
  A.a();
  const A.b() : this.a();
//                   ^
// [diag.redirectToNonConstConstructor] A constant redirecting constructor can't redirect to a non-constant constructor.
}
