class A<T> implements B<T> {
//    ^
// [diag.recursiveInterfaceInheritance] 'A' can't be a superinterface of itself: C, B, A.
  factory A() = C;
//              ^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
class B<T> implements C<T> {
//    ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: C, B, A.
  factory B() = A;
//              ^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
class C<T> implements A<T> {
//    ^
// [diag.recursiveInterfaceInheritance] 'C' can't be a superinterface of itself: C, B, A.
  factory C() = B;
//              ^
// [diag.recursiveFactoryRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
