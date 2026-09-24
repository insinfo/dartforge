bool f(bool left, String right) {
  return left && right;
//               ^^^^^
// [diag.nonBoolOperand] The operands of the operator '&&' must be assignable to 'bool'.
}
