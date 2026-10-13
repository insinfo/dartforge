void f((int,) i) {
  f((''));
//   ^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type '(int,)'.
}
