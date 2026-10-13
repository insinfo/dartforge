class A {
  const A(int i) : assert(i < 0, 'isNegative = ${i.isNegative}');
//                                               ^^^^^^^^^^^^
// [diag.invalidConstant] Invalid constant value.
}
