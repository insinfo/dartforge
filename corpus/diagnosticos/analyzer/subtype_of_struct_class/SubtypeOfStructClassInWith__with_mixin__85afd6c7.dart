import 'dart:ffi';

base mixin HeaderFields on Struct {
  @Int32()
  external int fieldA;

  external Pointer<Void> fieldB;
}

final class ExampleStruct extends Struct with HeaderFields {
//                                            ^^^^^^^^^^^^
// [diag.subtypeOfStructClassInWith] The class 'ExampleStruct' can't mix in 'HeaderFields' because 'HeaderFields' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
  @Uint32()
  external int fieldC;
}
