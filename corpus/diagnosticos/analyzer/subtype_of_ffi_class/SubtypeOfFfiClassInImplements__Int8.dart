import 'dart:ffi';
class C implements Int8 {}
//                 ^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Int8' can't be implemented outside of its library because it's a final class.
