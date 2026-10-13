import 'dart:ffi';
class C with Uint16 {}
//           ^^^^^^
// [diag.classUsedAsMixin] The class 'Uint16' can't be used as a mixin because it's neither a mixin class nor a mixin.
