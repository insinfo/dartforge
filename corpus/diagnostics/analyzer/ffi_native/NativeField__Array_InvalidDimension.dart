import 'dart:ffi';

@Native()
@Array(0)
//     ^
// [diag.nonPositiveArrayDimension] Array dimensions must be positive numbers.
external Array<IntPtr> field;
