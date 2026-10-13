class A extends B {}
//    ^
// [diag.recursiveInterfaceInheritance] 'A' can't be a superinterface of itself: B, A.
class B implements A {}
//    ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: B, A.
