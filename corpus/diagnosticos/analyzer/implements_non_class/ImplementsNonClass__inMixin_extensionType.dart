extension type A(int it) {}
mixin M implements A {}
//                 ^
// [diag.implementsNonClass] Classes and mixins can only implement other classes and mixins.
