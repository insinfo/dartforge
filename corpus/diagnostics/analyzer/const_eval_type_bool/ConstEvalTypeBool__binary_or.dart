const c = false || '';
//        ^^^^^^^^^^^
// [diag.constEvalTypeBool] In constant expressions, operands of this operator must be of type 'bool'.
//                 ^^
// [diag.nonBoolOperand] The operands of the operator '||' must be assignable to 'bool'.
