m() {
  var i = '';
  n(i);
//  ^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}
n(int i) {}
