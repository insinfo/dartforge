class A implements B {
//    ^
// [diag.recursiveInterfaceInheritance] 'A' can't be a superinterface of itself: C, B, A.
  factory A.nameA() = C.nameC;
//                    ^^^^^^^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
class B implements C {
//    ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: C, B, A.
  factory B.nameB() = A.nameA;
//                    ^^^^^^^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
class C implements A {
//    ^
// [diag.recursiveInterfaceInheritance] 'C' can't be a superinterface of itself: C, B, A.
  factory C.nameC() = B.nameB;
//                    ^^^^^^^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
