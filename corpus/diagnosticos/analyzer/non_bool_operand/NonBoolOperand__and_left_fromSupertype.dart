bool f(Object left, bool right) {
  return left && right;
//       ^^^^
// [diag.nonBoolOperand] The operands of the operator '&&' must be assignable to 'bool'.
}
