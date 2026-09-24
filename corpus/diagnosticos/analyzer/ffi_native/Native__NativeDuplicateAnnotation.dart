import 'dart:ffi';
@Native<Int32 Function(Int32)>()
@Native<Int32 Function(Int32)>(isLeaf: true)
// [diag.ffiNativeInvalidMultipleAnnotations][column 2][length 6] Native functions and fields must have exactly one `@Native` annotation.
external int foo(int v);
