const dynamic a = 'a';
var v = const <int>{if (true) a};
//                            ^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
