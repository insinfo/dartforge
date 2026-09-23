import 'dart:ffi';

@Native<IntPtr>()
int field;
//  ^^^^^
// [diag.notInitializedNonNullableVariable] The non-nullable variable 'field' must be initialized.
// [diag.ffiNativeMustBeExternal] Native functions must be declared external.
