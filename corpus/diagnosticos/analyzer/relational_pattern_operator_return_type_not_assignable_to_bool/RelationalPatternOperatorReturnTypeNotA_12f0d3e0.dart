class A {
  int operator >(_) => 42;
}

void f(A x) {
  if (x case > 0) {}
//           ^
// [diag.relationalPatternOperatorReturnTypeNotAssignableToBool] The return type of operators used in relational patterns must be assignable to 'bool'.
}
