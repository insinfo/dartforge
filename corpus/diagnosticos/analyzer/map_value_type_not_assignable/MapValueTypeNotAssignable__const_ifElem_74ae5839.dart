var v = const <bool, int>{if (1 < 0) true: 'a'};
//                                         ^^^
// [diag.mapValueTypeNotAssignable] The element type 'String' can't be assigned to the map value type 'int'.
