import 'dart:ffi';
class C implements Pointer {}
//    ^
// [diag.nonAbstractClassInheritsAbstractMemberOne] Missing concrete implementation of 'Pointer.cast'.
//                 ^^^^^^^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'Pointer' can't be implemented outside of its library because it's a final class.
