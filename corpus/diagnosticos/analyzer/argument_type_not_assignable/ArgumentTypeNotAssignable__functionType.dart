m() {
  var a = new A();
  a.n(() => 0);
//    ^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'void Function()' can't be assigned to the parameter type 'void Function(int)'.
}
class A {
  n(void f(int i)) {}
}
