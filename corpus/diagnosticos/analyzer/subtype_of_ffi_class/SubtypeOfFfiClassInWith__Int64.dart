import 'dart:ffi';
class C with Int64 {}
//           ^^^^^
// [diag.classUsedAsMixin] The class 'Int64' can't be used as a mixin because it's neither a mixin class nor a mixin.
