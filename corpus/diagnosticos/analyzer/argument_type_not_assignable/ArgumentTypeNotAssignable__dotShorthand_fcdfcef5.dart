class A {
  static A make(int x) => throw 0;
}
A f() => .make('');
//             ^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
