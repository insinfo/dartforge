var a = 0;
var v = <int, int>{for (var i in []) ...a};
//                          ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
//                                      ^
// [diag.notMapSpread] Spread elements in map literals must implement 'Map'.
