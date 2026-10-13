const String? stringConst = "";
int? intVar = 0;
const map = {null: 1, ?intVar: 1, stringConst: 1};
//                     ^^^^^^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
