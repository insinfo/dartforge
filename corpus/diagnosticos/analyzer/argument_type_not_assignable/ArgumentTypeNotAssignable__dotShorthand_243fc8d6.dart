class A {
  A.named(int x);
}
A f() => .named('');
//              ^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
