typedef A = ({
  int b,
  int c,
});

void f(A a){print(a);}

main() {
 f((b:2));
// ^^^^^
// [diag.argumentTypeNotAssignable] The argument type '({int b})' can't be assigned to the parameter type 'A'. Expected 2 named arguments, but got 1 instead.
}
