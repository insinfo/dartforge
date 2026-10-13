m() {
  Null x;
  if(x && true) {}
//   ^
// [diag.nonBoolOperand] The operands of the operator '&&' must be assignable to 'bool'.
}
