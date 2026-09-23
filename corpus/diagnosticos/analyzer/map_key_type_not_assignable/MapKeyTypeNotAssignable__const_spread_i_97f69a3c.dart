const dynamic a = 'a';
var v = const <int, String>{...{a: 'a'}};
//                              ^
// [diag.mapKeyTypeNotAssignable] The element type 'String' can't be assigned to the map key type 'int'.
