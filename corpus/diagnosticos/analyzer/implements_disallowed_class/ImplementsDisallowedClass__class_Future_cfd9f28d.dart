import 'dart:async';
class A<T> implements FutureOr<T> {}
//                    ^^^^^^^^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'FutureOr<T>'.
