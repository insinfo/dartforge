import 'dart:ffi';
class C implements Uint32 {}
//                 ^^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Uint32' can't be implemented outside of its library because it's a final class.
