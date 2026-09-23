class _A {
  final int? e;
  _A([this.e]);
}

class _B extends _A {
  _B(int e) : super(e);
}

var b = _B(1);
