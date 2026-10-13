import 'dart:ffi';

@Native<NativeFunction<Void Function()>>()
external void Function() field;
//                       ^^^^^
// [diag.nativeFieldInvalidType] 'NativeFunction<Void Function()>' is an unsupported type for native fields. Native fields only support pointers, arrays or numeric and compound types.
