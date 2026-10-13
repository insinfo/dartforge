mixin class M1 = Object with M2;
//          ^^
// [diag.recursiveInterfaceInheritance] 'M1' can't be a superinterface of itself: M2, M1.
mixin class M2 = Object with M1;
//          ^^
// [diag.recursiveInterfaceInheritance] 'M2' can't be a superinterface of itself: M2, M1.
