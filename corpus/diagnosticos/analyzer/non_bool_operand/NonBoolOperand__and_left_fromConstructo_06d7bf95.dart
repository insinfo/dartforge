main() {
  new Object() && true;
//^^^^^^^^^^^^
// [diag.nonBoolOperand] The operands of the operator '&&' must be assignable to 'bool'.
}
