m() {
  int? y;
  n(y);
//  ^
// [diag.argumentTypeNotAssignable] The argument type 'int?' can't be assigned to the parameter type 'int'.
}
n(int x) {}
