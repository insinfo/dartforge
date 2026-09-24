class A {}
class M {}
class C = A with M implements bool;
//                            ^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'bool'.
