class A {}
mixin M {}
int B = 7;
class C = A with M implements B;
//                            ^
// [diag.implementsNonClass] Classes and mixins can only implement other classes and mixins.
