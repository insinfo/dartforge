var v = const <int, bool>{if (1 < 0) 'a': true};
//                                   ^^^
// [diag.mapKeyTypeNotAssignable] The element type 'String' can't be assigned to the map key type 'int'.
