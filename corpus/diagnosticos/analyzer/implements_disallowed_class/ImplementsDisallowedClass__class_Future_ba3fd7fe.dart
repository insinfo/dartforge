import 'dart:async';
class A implements FutureOr<int> {}
//                 ^^^^^^^^^^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'FutureOr<int>'.
