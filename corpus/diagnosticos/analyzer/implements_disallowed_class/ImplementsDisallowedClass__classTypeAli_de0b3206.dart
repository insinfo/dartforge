class A {}
class M {}
class C = A with M implements String, num;
//                            ^^^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'String'.
//                                    ^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'num'.
