abstract class A implements B {}
//             ^
// [diag.recursiveInterfaceInheritance] 'A' can't be a superinterface of itself: B, A.
abstract class B implements A {}
//             ^
// [diag.recursiveInterfaceInheritance] 'B' can't be a superinterface of itself: B, A.
class C implements A {}
