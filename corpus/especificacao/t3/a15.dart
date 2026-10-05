class C {
  final num? _f;
  C(this._f);

  void m(num? x) {
    var (a!) = x;
    x.isNegative;
    var (b!) = _f;
    _f.isNegative;
    num? y = x;
    if (y case var c?) {
      y.isNegative;
    }
    if (_f case int _) {
      _f.isEven;
    }
    for (var (d!) in [x]) {
      x.isNegative;
    }
    int? e;
    (e!) = x as int?;
  }
}
