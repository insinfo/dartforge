import 'dart:ffi';
final class C extends Struct {
  external Pointer? p;
//         ^^^^^^^^
// [diag.invalidFieldTypeInStruct] Fields in struct classes can't have the type 'Pointer?'. They can only be declared as 'int', 'double', 'Array', 'Pointer', or subtype of 'Struct' or 'Union'.
}
