import 'dart:ffi';
class C with Int8 {}
//           ^^^^
// [diag.classUsedAsMixin] The class 'Int8' can't be used as a mixin because it's neither a mixin class nor a mixin.
