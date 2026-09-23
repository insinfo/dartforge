const c = (true || 0);
//              ^^^^
// [diag.deadCode] Dead code.
//                 ^
// [diag.nonBoolOperand] The operands of the operator '||' must be assignable to 'bool'.
