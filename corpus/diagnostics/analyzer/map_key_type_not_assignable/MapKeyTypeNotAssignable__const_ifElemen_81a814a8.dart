const dynamic a = 0;
const dynamic b = 'b';
var v = const <int, bool>{if (1 < 0) a: true else b: false};
//                                                ^
// [diag.mapKeyTypeNotAssignable] The element type 'String' can't be assigned to the map key type 'int'.
