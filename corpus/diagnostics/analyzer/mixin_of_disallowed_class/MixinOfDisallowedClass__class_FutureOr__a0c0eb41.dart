import 'dart:async';
class A<T> extends Object with FutureOr<T> {}
//                             ^^^^^^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'FutureOr<T>'.
