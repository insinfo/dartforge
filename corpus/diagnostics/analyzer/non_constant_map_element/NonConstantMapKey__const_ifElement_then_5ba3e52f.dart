final dynamic a = 0;
var v = const <int, int>{if (1 > 0) a: 0 else 0: 0};
//                                  ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
