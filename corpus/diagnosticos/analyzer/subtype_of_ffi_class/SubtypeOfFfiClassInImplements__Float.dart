import 'dart:ffi';
class C implements Float {}
//                 ^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Float' can't be implemented outside of its library because it's a final class.
