class A {}
class M {}
class C = A with M implements double;
//                            ^^^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'double'.
