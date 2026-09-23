class A {}
abstract class X with Unresolved, M, CycleWithX {}
//             ^
// [diag.recursiveInterfaceInheritance] 'X' can't be a superinterface of itself: CycleWithX, X.
//                    ^^^^^^^^^^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
//                                ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'Object' because 'Object' doesn't implement 'A'.
mixin M on A {}
mixin CycleWithX on X {}
//    ^^^^^^^^^^
// [diag.recursiveInterfaceInheritance] 'CycleWithX' can't be a superinterface of itself: CycleWithX, X.
