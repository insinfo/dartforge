class C = D with M;
//    ^
// [diag.recursiveInterfaceInheritance] 'C' can't be a superinterface of itself: D, C.
class D = C with M;
//    ^
// [diag.recursiveInterfaceInheritance] 'D' can't be a superinterface of itself: D, C.
mixin M {}
