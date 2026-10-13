class A {
  bool operator==(covariant A other) => false;
}

void f(A a, A? aq) {
  a == 0;
//     ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'A?'.
  aq == 1;
//      ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'A?'.
  aq == aq;
  aq == null;
}
