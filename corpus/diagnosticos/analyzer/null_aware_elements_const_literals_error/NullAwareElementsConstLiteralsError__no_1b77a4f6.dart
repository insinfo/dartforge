String? stringVar = "";
const map = {null: 1, 0: 1, ?stringVar: 1};
//                           ^^^^^^^^^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
