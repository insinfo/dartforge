import 'dart:ffi';

const annotation = Native<Int32 Function(Int32)>();

@annotation
external int wrongFfiReturnType(int v);
