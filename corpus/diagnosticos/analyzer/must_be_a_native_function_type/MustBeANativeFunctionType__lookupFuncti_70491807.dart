import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Int64x5Leaf =
  lib.lookupFunction<
    Int64 Function(Int64, VarArgs<(Int64, Int64, Int64)>, Int64),
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'Int64 Function(Int64, VarArgs<(Int64, Int64, Int64)>, Int64)' given to 'lookupFunction' must be a valid 'dart:ffi' native function type.
    int Function(int, int, int, int, int)
  >(
    "VariadicAt1Int64x5",
    isLeaf:true
  );
