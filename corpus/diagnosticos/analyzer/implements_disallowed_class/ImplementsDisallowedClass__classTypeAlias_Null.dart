class A {}
class M {}
class C = A with M implements Null;
//                            ^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'Null'.
