typedef A(int p);
f(A a) {
  a('1');
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}
