import 'dart:async';
class M {}
class C = FutureOr with M;
//        ^^^^^^^^
// [diag.extendsDisallowedClass] Classes can't extend 'FutureOr<dynamic>'.
