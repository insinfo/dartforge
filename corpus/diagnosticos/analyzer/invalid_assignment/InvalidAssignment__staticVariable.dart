class A {
  static int x = 1;
}
f() {
  A.x = '0';
//      ^^^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
}
