var nullVar = null;
const int? intConst = 0;
const String? stringConst = "";
const list = [?null, ?nullVar, intConst, stringConst];
//                    ^^^^^^^
// [diag.nonConstantListElement] The values in a const list literal must be constants.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
