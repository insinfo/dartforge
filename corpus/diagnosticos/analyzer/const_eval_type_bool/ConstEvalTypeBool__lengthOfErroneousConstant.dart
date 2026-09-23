const int i = (1 ? 'alpha' : 'beta').length;
//             ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
// [diag.constEvalTypeBool] In constant expressions, operands of this operator must be of type 'bool'.
