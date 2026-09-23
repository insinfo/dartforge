import 'dart:ffi';
final class C extends Struct {
  external String str;
//         ^^^^^^
// [diag.invalidFieldTypeInStruct] Fields in struct classes can't have the type 'String'. They can only be declared as 'int', 'double', 'Array', 'Pointer', or subtype of 'Struct' or 'Union'.

  external Pointer notEmpty;
}
