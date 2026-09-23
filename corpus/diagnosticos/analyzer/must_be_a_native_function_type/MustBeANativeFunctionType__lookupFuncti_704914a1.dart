import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Int64x5Leaf =
  lib.lookupFunction<
    Int64 Function(Int64, VarArgs<(Int64, Int64, Int64, Int64)>),
    int Function(int, int, int, int, double)
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Int64 Function(Int64, VarArgs<(Int64, Int64, Int64, Int64)>)' must be a subtype of 'int Function(int, int, int, int, double)' for 'lookupFunction'.
  >(
    "VariadicAt1Int64x5",
    isLeaf:true
  );
