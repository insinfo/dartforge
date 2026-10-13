void f<T extends num>(T a) {
  var v = [...a];
//            ^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.
  v;
}
