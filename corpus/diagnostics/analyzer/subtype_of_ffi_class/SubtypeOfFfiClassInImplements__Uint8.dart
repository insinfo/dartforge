import 'dart:ffi';
class C implements Uint8 {}
//                 ^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Uint8' can't be implemented outside of its library because it's a final class.
