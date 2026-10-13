import 'dart:ffi';
class C implements Uint64 {}
//                 ^^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Uint64' can't be implemented outside of its library because it's a final class.
