final dynamic a = 0;
const cond = true;
var v = const {if (cond) 'a' : a};
//                             ^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
