import 'dart:ffi';
final class S extends Union {}
//          ^
// [diag.emptyStruct] The class 'S' can't be empty because it's a subclass of 'Union'.
final class C with S {}
//                 ^
// [diag.classUsedAsMixin] The class 'S' can't be used as a mixin because it's neither a mixin class nor a mixin.
// [diag.subtypeOfStructClassInWith] The class 'C' can't mix in 'S' because 'S' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
