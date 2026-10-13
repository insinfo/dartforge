class A {
  static int C = 0;
  final int a;
  const A() : a = C;
//                ^
// [diag.invalidConstant] Invalid constant value.
}
