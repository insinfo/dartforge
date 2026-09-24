const int a = 0;
const b = a ^ '';
//        ^^^^^^
// [diag.constEvalTypeBoolInt] In constant expressions, operands of this operator must be of type 'bool' or 'int'.
//            ^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
