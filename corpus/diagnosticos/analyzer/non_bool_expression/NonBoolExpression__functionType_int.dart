int makeAssertion() => 1;
f() {
  assert(makeAssertion);
//       ^^^^^^^^^^^^^
// [diag.nonBoolExpression] The expression in an assert must be of type 'bool'.
}
