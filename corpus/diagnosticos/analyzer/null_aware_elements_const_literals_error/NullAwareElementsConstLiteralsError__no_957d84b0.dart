const nullConst = null;
const String? stringConst = "";
int? intVar = 0;
const set = {nullConst, ?intVar, stringConst};
//                       ^^^^^^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
