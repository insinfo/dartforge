var nullVar = null;
const int? intConst = 0;
const String? stringConst = "";
const set = {?nullVar, intConst, stringConst};
//            ^^^^^^^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
