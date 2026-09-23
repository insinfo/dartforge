class _A {
  _A([int? a]);
}

class _B extends _A {
  _B([super.a]);
}

var b = _B(1);
