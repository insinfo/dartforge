class A {
  factory A() = C;
//              ^
// [diag.redirectToInvalidReturnType] The return type 'C' of the redirected constructor isn't a subtype of 'A'.
}
class B implements C {
//    ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: C, B.
  factory B() = C;
//              ^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
class C implements A, B {
//    ^
// [diag.recursiveInterfaceInheritance] 'C' can't be a superinterface of itself: C, B.
  factory C() = B;
//              ^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
