m() {
  num y = 1;
  n(y);
//  ^
// [diag.argumentTypeNotAssignable] The argument type 'num' can't be assigned to the parameter type 'int'.
}
n(int x) {}
