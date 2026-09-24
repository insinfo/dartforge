const dynamic x = 'abc';
var v = const <int>{x};
//                  ^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
