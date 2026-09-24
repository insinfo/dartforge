var a = 0;
var v = <int, int>{if (1 > 0) ...a};
//                               ^
// [diag.notMapSpread] Spread elements in map literals must implement 'Map'.
