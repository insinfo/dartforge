class A {
  const A(int i) : assert(i.isNegative);
//                        ^^^^^^^^^^^^
// [diag.invalidConstant] Invalid constant value.
}
