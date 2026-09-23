import 'dart:async';
class A<T> extends FutureOr<T> {}
//                 ^^^^^^^^^^^
// [diag.extendsDisallowedClass] Classes can't extend 'FutureOr<T>'.
