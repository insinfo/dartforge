class A {
  A();
}
class B {
  const B() : a = new A();
//                ^^^^^^^
// [context 1] The error is in the field initializer of 'B.new', and occurs here.
// [diag.invalidConstant] Invalid constant value.
  final a;
}
var b = const B();
//      ^^^^^^^^^
// [diag.invalidConstant][context 1] Invalid constant value.
