import 'dart:ffi';
class C implements Int64 {}
//                 ^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Int64' can't be implemented outside of its library because it's a final class.
