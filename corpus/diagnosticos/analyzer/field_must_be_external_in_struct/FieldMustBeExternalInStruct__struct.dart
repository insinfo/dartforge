import 'dart:ffi';

final class A extends Struct {
  @Int16()
  int a;
//    ^
// [diag.fieldMustBeExternalInStruct] Fields of 'Struct' and 'Union' subclasses must be marked external.
}
