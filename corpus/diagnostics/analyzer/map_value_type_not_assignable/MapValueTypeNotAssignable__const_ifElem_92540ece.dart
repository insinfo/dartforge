final a = 0;
var v = const <bool, int>{if (1 < 2) true: a};
//                                         ^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
