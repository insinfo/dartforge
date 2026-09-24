enum E { v }
mixin M on E {}
//         ^
// [diag.mixinSuperClassConstraintNonInterface] Only classes and mixins can be used as superclass constraints.
