import 'dart:ffi';
class C implements Int32 {}
//                 ^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Int32' can't be implemented outside of its library because it's a final class.
