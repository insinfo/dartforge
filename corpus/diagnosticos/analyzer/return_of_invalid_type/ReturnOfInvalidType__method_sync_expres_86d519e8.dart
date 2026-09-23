class A {
  int f() => '0';
//           ^^^
// [diag.returnOfInvalidTypeFromMethod] A value of type 'String' can't be returned from the method 'f' because it has a return type of 'int'.
}
