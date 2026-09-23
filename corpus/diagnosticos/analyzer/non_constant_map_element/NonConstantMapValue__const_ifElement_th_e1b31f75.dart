final dynamic a = 0;
var v = const <int, int>{if (1 > 0) 0: a else 0: 0};
//                                     ^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
