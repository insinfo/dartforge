class A {
  const A.fromInt(int p);
}
@A.fromInt('0')
//         ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
main() {}
