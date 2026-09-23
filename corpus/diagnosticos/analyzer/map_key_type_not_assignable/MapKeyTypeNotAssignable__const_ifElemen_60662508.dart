final a = 0;
var v = const <int, bool>{if (1 < 2) a: true};
//                                   ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
