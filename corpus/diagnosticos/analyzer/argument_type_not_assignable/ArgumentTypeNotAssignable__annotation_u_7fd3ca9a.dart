class A {
  const A(int p);
}
@A('0')
// ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
main() {
}