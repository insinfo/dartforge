import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Doublex2 =
  lib.lookupFunction<
    Double Function(Double, VarArgs<(Double,)>),
    double Function(double, double)
  >(
    "VariadicAt1Doublex2"
  );
