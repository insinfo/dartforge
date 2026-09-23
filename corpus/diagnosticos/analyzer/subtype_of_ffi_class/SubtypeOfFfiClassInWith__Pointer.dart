import 'dart:ffi';
class C with Pointer {}
//           ^^^^^^^
// [diag.classUsedAsMixin] The class 'Pointer' can't be used as a mixin because it's neither a mixin class nor a mixin.
