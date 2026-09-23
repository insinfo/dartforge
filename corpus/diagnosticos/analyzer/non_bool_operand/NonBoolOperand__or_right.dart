bool f(bool left, double right) {
  return left || right;
//               ^^^^^
// [diag.nonBoolOperand] The operands of the operator '||' must be assignable to 'bool'.
}
