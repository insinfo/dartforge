import 'dart:ffi';
final class C with Union {}
//                 ^^^^^
// [diag.classUsedAsMixin] The class 'Union' can't be used as a mixin because it's neither a mixin class nor a mixin.
