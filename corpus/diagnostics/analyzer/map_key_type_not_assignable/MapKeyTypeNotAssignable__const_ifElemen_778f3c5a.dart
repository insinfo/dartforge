const dynamic a = 'a';
var v = const <int, bool>{if (true) a: true};
//                                  ^
// [diag.mapKeyTypeNotAssignable] The element type 'String' can't be assigned to the map key type 'int'.
