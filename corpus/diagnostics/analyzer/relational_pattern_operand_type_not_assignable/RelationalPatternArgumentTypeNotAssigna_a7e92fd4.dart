class A {
  bool operator ==(covariant A other) => true;
}

void f(A x) {
  switch (x) {
    case == 0:
//          ^
// [diag.relationalPatternOperandTypeNotAssignable] The constant expression type 'int' is not assignable to the parameter type 'A' of the '==' operator.
      break;
  }
}
