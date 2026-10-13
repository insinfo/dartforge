import 'dart:ffi';
class C with Uint64 {}
//           ^^^^^^
// [diag.classUsedAsMixin] The class 'Uint64' can't be used as a mixin because it's neither a mixin class nor a mixin.
