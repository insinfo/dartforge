var nullVar = null;
const int? intConst = 0;
const String? stringConst = "";
const map = {?nullVar: 1, intConst: 1, stringConst: 1};
//            ^^^^^^^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
