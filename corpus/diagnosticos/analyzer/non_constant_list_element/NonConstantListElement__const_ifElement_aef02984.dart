final dynamic a = 0;
var v = const [if (1 > 0) 0 else a];
//                               ^
// [diag.nonConstantListElement] The values in a const list literal must be constants.
