void f<T extends num>(T a) {
  var v = <int, int>{...a};
//                      ^
// [diag.notMapSpread] Spread elements in map literals must implement 'Map'.
  v;
}
