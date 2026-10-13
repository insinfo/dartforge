import 'dart:async';
class A extends FutureOr<int> {}
//              ^^^^^^^^^^^^^
// [diag.extendsDisallowedClass] Classes can't extend 'FutureOr<int>'.
