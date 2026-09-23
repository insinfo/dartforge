import 'dart:ffi';
typedef Int8UnOp = Int8 Function(Int8);
typedef IntUnOp = int Function(int);
doThings(bool isLeaf) {
  DynamicLibrary l = DynamicLibrary.open("my_lib");
  l.lookupFunction<Int8UnOp, IntUnOp>("timesFour", isLeaf:isLeaf);
//                                                        ^^^^^^
// [diag.argumentMustBeAConstant] Argument 'isLeaf' must be a constant.
}
