class A {}
class M {}
class C = A with M implements int;
//                            ^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'int'.
