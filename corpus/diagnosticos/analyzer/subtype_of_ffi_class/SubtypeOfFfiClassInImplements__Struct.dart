import 'dart:ffi';
final class C implements Struct {}
//                       ^^^^^^
// [diag.baseClassImplementedOutsideOfLibrary] The class 'Struct' can't be implemented outside of its library because it's a base class.
