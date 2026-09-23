extension type A(int it) {}
mixin M on A {}
//         ^
// [diag.mixinSuperClassConstraintNonInterface] Only classes and mixins can be used as superclass constraints.
