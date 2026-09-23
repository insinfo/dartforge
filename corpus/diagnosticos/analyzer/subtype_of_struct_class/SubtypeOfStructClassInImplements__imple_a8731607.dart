import 'dart:ffi';

base mixin HeaderFields on Struct {
  @Int32()
  external int fieldA;
}

final class ExampleStruct implements HeaderFields {
//                                   ^^^^^^^^^^^^
// [diag.baseClassImplementedOutsideOfLibrary] The class 'Struct' can't be implemented outside of its library because it's a base class.
// [diag.subtypeOfStructClassInImplements] The class 'ExampleStruct' can't implement 'HeaderFields' because 'HeaderFields' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
  @override
  int fieldA = 0;
}
