abstract class A implements B {}
//             ^
// [diag.recursiveInterfaceInheritance] 'A' can't be a superinterface of itself: C, B, A.
abstract class B implements C {}
//             ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: C, B, A.
abstract class C implements A {}
//             ^
// [diag.recursiveInterfaceInheritance] 'C' can't be a superinterface of itself: C, B, A.
class D implements A {}
