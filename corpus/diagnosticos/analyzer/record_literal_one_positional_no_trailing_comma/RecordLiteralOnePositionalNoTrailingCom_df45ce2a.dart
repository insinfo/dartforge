void f((int,) i) {
  f(1);
//  ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type '(int,)'.
}
