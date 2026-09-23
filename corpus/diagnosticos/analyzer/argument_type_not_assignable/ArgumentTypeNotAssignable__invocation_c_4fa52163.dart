class A {
  call(int p) {}
}
main() {
  A a = new A();
  a('0');
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}