final dynamic a = 0;
var v = const <int>{if (1 < 0) a else 0};
//                             ^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
