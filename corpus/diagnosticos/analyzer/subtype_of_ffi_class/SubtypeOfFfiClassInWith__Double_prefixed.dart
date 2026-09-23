import 'dart:ffi' as ffi;
class C with ffi.Double {}
//           ^^^^^^^^^^
// [diag.classUsedAsMixin] The class 'Double' can't be used as a mixin because it's neither a mixin class nor a mixin.
