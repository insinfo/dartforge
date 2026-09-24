import 'dart:ffi';

@Native()
@Array(10, 20)
// [diag.sizeAnnotationDimensions][column 1][length 14] 'Array's must have an 'Array' annotation that matches the dimensions.
external Array<IntPtr> field;
