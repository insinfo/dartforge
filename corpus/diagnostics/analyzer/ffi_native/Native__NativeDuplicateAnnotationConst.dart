import 'dart:ffi';

const duplicate = Native<Int32 Function(Int32)>(isLeaf: true);

@Native<Int32 Function(Int32)>()
@duplicate
// [diag.ffiNativeInvalidMultipleAnnotations][column 2][length 9] Native functions and fields must have exactly one `@Native` annotation.
external int foo(int v);
