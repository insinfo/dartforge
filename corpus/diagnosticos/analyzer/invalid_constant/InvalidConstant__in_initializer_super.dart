class A {
  const A(p);
}
class B extends A {
  static var C;
  const B() : super(C);
//                  ^
// [diag.invalidConstant] Invalid constant value.
}
