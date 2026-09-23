class A {
  int operator [](int index) => 0;
  operator []=(String index, int value) {}
}
f(A a) {
  a['0'] += 0;
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
  ++a['0'];
//    ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
  a['0']++;
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}