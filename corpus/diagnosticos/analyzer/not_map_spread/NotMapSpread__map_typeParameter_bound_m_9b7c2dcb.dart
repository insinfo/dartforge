void f<T extends Map<int, String>?>(T a) {
  var v = <int, String>{...?a};
  v;
}
