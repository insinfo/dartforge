bool makeAssertion() => true;
f() {
  assert(makeAssertion);
//       ^^^^^^^^^^^^^
// [diag.nonBoolExpression] The expression in an assert must be of type 'bool'.
}
