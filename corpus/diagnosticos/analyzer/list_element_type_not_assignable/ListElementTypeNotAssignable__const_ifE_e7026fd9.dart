const dynamic a = 'a';
var v = const <int>[if (true) a];
//                            ^
// [diag.listElementTypeNotAssignable] The element type 'String' can't be assigned to the list type 'int'.
