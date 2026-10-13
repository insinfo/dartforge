import 'dart:ffi';
@Native<IntPtr Function(IntPtr)>()
external int field;
//           ^^^^^
// [diag.nativeFieldInvalidType] 'IntPtr Function(IntPtr)' is an unsupported type for native fields. Native fields only support pointers, arrays or numeric and compound types.
