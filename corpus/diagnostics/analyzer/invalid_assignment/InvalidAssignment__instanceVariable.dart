class A {
  int x = 7;
}
f(y) {
  A a = A();
  if (y is String) {
    a.x = y;
//        ^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
  }
}
