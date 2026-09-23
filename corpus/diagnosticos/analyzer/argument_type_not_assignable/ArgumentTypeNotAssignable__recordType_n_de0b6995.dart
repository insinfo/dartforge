typedef A = ({
  int b,
  int c,
});

void f(A a){print(a);}

main() {
 f((bb:2, c:3));
// ^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type '({int bb, int c})' can't be assigned to the parameter type 'A'. Unexpected named argument `bb` with type `int`.
}
