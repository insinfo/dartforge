class A {
  A();
  const A.named() : this();
//                  ^^^^
// [diag.redirectToNonConstConstructor] A constant redirecting constructor can't redirect to a non-constant constructor.
}
