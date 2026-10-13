import 'dart:ffi';
class C with Int32 {}
//           ^^^^^
// [diag.classUsedAsMixin] The class 'Int32' can't be used as a mixin because it's neither a mixin class nor a mixin.
