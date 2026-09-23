import 'dart:ffi';
final class S extends Struct {
  external Pointer notEmpty;
}
final class C extends S {}
//                    ^
// [diag.subtypeOfStructClassInExtends] The class 'C' can't extend 'S' because 'S' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
