const dynamic a = 'a';
var v = const <bool, int>{if (true) true: a};
//                                        ^
// [diag.mapValueTypeNotAssignable] The element type 'String' can't be assigned to the map value type 'int'.
