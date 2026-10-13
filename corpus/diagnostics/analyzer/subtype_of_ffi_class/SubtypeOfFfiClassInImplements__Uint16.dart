import 'dart:ffi';
class C implements Uint16 {}
//                 ^^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Uint16' can't be implemented outside of its library because it's a final class.
