import 'dart:ffi';
class C with Uint32 {}
//           ^^^^^^
// [diag.classUsedAsMixin] The class 'Uint32' can't be used as a mixin because it's neither a mixin class nor a mixin.
