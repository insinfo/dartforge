const String? stringConst = "";
int? intVar = 0;
const map = {null: 1, 0: ?intVar, stringConst: 1};
//                        ^^^^^^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
