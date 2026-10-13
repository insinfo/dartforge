var a = 0;
var v = [if (1 > 0) ...[] else ...a];
//                                ^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.
