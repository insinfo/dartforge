const nullConst = null;
const int? intConst = 0;
String? stringVar = "";
const set = {nullConst, intConst, ?stringVar};
//                                 ^^^^^^^^^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
