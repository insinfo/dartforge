class A {}
class M {}
class C = A with M implements String;
//                            ^^^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'String'.
