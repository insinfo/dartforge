class A {
  int m() {
    return '0';
//         ^^^
// [diag.returnOfInvalidTypeFromMethod] A value of type 'String' can't be returned from the method 'm' because it has a return type of 'int'.
  }
}
