import 'dart:ffi';
class C with Uint8 {}
//           ^^^^^
// [diag.classUsedAsMixin] The class 'Uint8' can't be used as a mixin because it's neither a mixin class nor a mixin.
