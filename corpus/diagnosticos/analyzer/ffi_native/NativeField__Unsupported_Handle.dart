import 'dart:ffi';

@Native<Handle>()
external Object field;
//              ^^^^^
// [diag.nativeFieldInvalidType] 'Handle' is an unsupported type for native fields. Native fields only support pointers, arrays or numeric and compound types.
