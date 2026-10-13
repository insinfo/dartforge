import 'dart:ffi';
final class C with Struct {}
//                 ^^^^^^
// [diag.classUsedAsMixin] The class 'Struct' can't be used as a mixin because it's neither a mixin class nor a mixin.
