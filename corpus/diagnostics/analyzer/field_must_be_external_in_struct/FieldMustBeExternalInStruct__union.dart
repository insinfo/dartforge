import 'dart:ffi';

final class A extends Union {
  @Int16()
  int a;
//    ^
// [diag.fieldMustBeExternalInStruct] Fields of 'Struct' and 'Union' subclasses must be marked external.
}
