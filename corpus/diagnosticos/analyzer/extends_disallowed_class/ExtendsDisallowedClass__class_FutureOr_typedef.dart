import 'dart:async';
typedef F = FutureOr<void>;
class A extends F {}
//              ^
// [diag.extendsDisallowedClass] Classes can't extend 'F'.
