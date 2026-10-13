class A {
  factory A() = A;
//              ^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
