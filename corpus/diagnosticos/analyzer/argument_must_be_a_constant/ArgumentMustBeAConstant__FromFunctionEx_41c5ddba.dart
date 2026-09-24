import 'dart:ffi';
typedef NativeDoubleUnOp = Double Function(Double);
double myTimesThree(double d) => d * 3;
void testFromFunctionFunctionExceptionValueMustBeConst() {
  final notAConst = 1.1;
  Pointer.fromFunction<NativeDoubleUnOp>(myTimesThree, notAConst);
//                                                     ^^^^^^^^^
// [diag.argumentMustBeAConstant] Argument 'exceptionalReturn' must be a constant.
}
