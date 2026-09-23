var a = 0;
var v = [for (var i in []) ...a];
//                ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
//                            ^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.
