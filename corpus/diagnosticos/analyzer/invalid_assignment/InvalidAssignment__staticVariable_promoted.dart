class A {
  static int x = 7;
}
f(y) {
  if (y is String) {
    A.x = y;
//        ^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
  }
}
