class A {}
class M {}
class C = A with M implements num;
//                            ^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'num'.
