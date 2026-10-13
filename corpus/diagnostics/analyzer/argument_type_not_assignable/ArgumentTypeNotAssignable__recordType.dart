void f((int a, int b) r) {}

void g() {
  f((a: 1, b: 2));
//  ^^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type '({int a, int b})' can't be assigned to the parameter type '(int, int)'. Expected 2 positional arguments, but got 0 instead.
}
