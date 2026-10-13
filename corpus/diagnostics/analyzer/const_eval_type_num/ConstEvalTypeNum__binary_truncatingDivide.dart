const num a = 0;
const b = a ~/ '';
//        ^^^^^^^
// [diag.constEvalTypeNum] In constant expressions, operands of this operator must be of type 'num'.
//             ^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'num'.
