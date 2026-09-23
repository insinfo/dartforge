const dynamic a = null;
var v = const <int, bool>{a : true};
//                        ^
// [diag.mapKeyTypeNotAssignableNullability] The element type 'Null' can't be assigned to the map key type 'int'.
