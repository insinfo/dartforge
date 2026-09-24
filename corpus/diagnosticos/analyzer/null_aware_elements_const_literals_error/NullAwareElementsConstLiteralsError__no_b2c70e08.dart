var nullVar = null;
const int? intConst = 0;
const String? stringConst = "";
const map = {null: ?nullVar, intConst: 1, stringConst: 1};
//                  ^^^^^^^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
