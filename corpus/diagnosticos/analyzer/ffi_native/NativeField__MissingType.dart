import 'dart:ffi';

@Native()
external int invalid;
//           ^^^^^^^
// [diag.nativeFieldMissingType] The native type of this field could not be inferred and must be specified in the annotation.

@Native()
external Pointer<IntPtr> valid;
