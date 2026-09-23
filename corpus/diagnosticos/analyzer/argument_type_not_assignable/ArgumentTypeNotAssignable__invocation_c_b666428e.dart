class A {
  call(int p) {}
}
f(A a) {
  a('0');
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}