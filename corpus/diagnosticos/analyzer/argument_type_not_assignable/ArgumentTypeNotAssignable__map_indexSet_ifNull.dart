main() {
  Map<int, int> m = <int, int>{};
  m['x'] ??= 0;
//  ^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}
