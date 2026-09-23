import 'dart:ffi';
final class S extends Struct {}
//          ^
// [diag.emptyStruct] The class 'S' can't be empty because it's a subclass of 'Struct'.
final class C implements S {}
//                       ^
// [diag.baseClassImplementedOutsideOfLibrary] The class 'Struct' can't be implemented outside of its library because it's a base class.
// [diag.subtypeOfStructClassInImplements] The class 'C' can't implement 'S' because 'S' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
