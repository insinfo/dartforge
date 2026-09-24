class A<T> {
  const A.fromInt(T p);
}
@A<int>.fromInt('0')
//              ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
main() {
}
