import 'dart:ffi' as ffi;
class C implements ffi.Double {}
//                 ^^^^^^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Double' can't be implemented outside of its library because it's a final class.
