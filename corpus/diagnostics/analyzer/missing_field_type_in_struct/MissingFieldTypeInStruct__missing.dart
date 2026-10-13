import 'dart:ffi';
final class C extends Struct {
  external var str;
//             ^^^
// [diag.missingFieldTypeInStruct] Fields in struct classes must have an explicitly declared type of 'int', 'double' or 'Pointer'.

  external Pointer notEmpty;
}
