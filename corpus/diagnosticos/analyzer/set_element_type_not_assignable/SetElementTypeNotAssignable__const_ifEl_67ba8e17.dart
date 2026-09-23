const dynamic a = 0;
const dynamic b = 'b';
var v = const <int>{if (1 < 0) a else b};
//                                    ^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
