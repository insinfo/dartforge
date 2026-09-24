import 'dart:async';
class A extends Object with FutureOr {}
//                          ^^^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'FutureOr<dynamic>'.
