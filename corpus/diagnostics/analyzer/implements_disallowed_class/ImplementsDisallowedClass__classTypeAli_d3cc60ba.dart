import 'dart:async';
class A {}
class M {}
class C = A with M implements FutureOr;
//                            ^^^^^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'FutureOr<dynamic>'.
