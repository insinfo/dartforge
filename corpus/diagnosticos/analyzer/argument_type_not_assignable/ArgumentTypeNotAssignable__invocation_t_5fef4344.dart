typedef A<T>(T p);
f(A<int> a) {
  a('1');
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}