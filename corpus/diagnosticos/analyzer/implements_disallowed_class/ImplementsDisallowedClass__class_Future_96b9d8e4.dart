import 'dart:async';
typedef F = FutureOr<void>;
class A implements F {}
//                 ^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'F'.
