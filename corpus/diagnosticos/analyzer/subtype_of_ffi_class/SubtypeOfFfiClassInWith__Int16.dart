import 'dart:ffi';
class C with Int16 {}
//           ^^^^^
// [diag.classUsedAsMixin] The class 'Int16' can't be used as a mixin because it's neither a mixin class nor a mixin.
