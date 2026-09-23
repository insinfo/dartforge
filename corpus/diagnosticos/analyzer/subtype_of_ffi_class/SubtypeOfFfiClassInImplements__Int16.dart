import 'dart:ffi';
class C implements Int16 {}
//                 ^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Int16' can't be implemented outside of its library because it's a final class.
