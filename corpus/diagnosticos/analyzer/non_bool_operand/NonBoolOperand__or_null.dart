m() {
  Null x;
  if(x || false) {}
//   ^
// [diag.nonBoolOperand] The operands of the operator '||' must be assignable to 'bool'.
}
