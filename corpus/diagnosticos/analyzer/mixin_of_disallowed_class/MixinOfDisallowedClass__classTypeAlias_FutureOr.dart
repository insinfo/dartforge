import 'dart:async';
class A {}
class C = A with FutureOr;
//               ^^^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'FutureOr<dynamic>'.
