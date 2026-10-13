class A {
  A(int x);
}
A f() => .new('');
//            ^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
