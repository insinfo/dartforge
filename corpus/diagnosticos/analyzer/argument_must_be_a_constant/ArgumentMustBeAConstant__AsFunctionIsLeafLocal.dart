import 'dart:ffi';
typedef Int8UnOp = Int8 Function(Int8);
typedef IntUnOp = int Function(int);
doThings() {
  bool isLeaf = false;
  Pointer<NativeFunction<Int8UnOp>> p = Pointer.fromAddress(1337);
  IntUnOp f = p.asFunction(isLeaf:isLeaf);
//                                ^^^^^^
// [diag.argumentMustBeAConstant] Argument 'isLeaf' must be a constant.
  f(8);
}
