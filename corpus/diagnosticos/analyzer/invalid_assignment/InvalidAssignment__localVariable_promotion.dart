f(y) {
  if (y is String) {
    int x = y;
//          ^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
    print(x);
  }
}
