import 'dart:async';
class A extends Object with FutureOr<int> {}
//                          ^^^^^^^^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'FutureOr<int>'.
