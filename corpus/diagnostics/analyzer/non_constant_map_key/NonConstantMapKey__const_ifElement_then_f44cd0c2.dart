final dynamic a = 0;
const cond = true;
var v = const {if (cond) 0: 1 else a : 0};
//                                 ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
