import 'dart:ffi';

@Native()
external Array<IntPtr> field;
//                     ^^^^^
// [diag.missingSizeAnnotationCarray] Fields of type 'Array' must have exactly one 'Array' annotation.
