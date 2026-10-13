final dynamic a = 0;
const cond = true;
var v = const {if (cond) a : 0};
//                       ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
