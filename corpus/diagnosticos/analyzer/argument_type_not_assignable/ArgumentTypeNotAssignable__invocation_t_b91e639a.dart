typedef A(int p);
A getA() => throw '';
main() {
  A a = getA();
  a('1');
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}