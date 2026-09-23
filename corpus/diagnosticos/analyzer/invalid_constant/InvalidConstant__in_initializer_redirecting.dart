class A {
  static var C;
  const A.named(p);
  const A() : this.named(C);
//                       ^
// [diag.invalidConstant] Invalid constant value.
}
