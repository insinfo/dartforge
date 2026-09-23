import 'dart:ffi';

const a = Native();

@a
external int foo();
//           ^^^
// [diag.nativeFunctionMissingType] The native type of this function couldn't be inferred so it must be specified in the annotation.
