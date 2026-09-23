typedef A = (
  int b,
  int c,
);

void f(A a){print(a);}

main() {
 f((3, 2, 1));
// ^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type '(int, int, int)' can't be assigned to the parameter type 'A'. Expected 2 positional arguments, but got 3 instead.
}
