class A {
  int operator +({Object? other}) => 0;
//                ^^^^^^^^^^^^^
// [diag.optionalParameterInOperator] Optional parameters aren't allowed when defining an operator.
}
