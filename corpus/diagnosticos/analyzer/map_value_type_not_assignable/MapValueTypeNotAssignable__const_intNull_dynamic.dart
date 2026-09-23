const dynamic a = null;
var v = const <bool, int>{true: a};
//                              ^
// [diag.mapValueTypeNotAssignableNullability] The element type 'Null' can't be assigned to the map value type 'int'.
