final dynamic a = 0;
var v = const <int>{if (1 < 0) 0 else a};
//                                    ^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
