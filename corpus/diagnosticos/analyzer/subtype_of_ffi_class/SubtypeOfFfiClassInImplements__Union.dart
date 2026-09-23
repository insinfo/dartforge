import 'dart:ffi';
final class C implements Union {}
//                       ^^^^^
// [diag.baseClassImplementedOutsideOfLibrary] The class 'Union' can't be implemented outside of its library because it's a base class.
