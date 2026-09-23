extension type A(int it) {}
class B implements A {}
//                 ^
// [diag.implementsNonClass] Classes and mixins can only implement other classes and mixins.
