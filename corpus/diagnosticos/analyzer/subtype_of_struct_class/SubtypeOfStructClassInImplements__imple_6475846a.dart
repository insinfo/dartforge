import 'dart:ffi';
@AbiSpecificIntegerMapping({
  Abi.androidArm: Uint32(),
})
final class AbiSpecificInteger1 extends AbiSpecificInteger {
  const AbiSpecificInteger1();
}
final class AbiSpecificInteger4 implements AbiSpecificInteger1 {
//                                         ^^^^^^^^^^^^^^^^^^^
// [diag.baseClassImplementedOutsideOfLibrary] The class 'AbiSpecificInteger' can't be implemented outside of its library because it's a base class.
// [diag.subtypeOfStructClassInImplements] The class 'AbiSpecificInteger4' can't implement 'AbiSpecificInteger1' because 'AbiSpecificInteger1' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
  const AbiSpecificInteger4();
}
