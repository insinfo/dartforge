class A<T> implements B<T> {}
//    ^
// [diag.recursiveInterfaceInheritance] 'A' can't be a superinterface of itself: B, A.
class B<T> implements A<T> {}
//    ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: B, A.
