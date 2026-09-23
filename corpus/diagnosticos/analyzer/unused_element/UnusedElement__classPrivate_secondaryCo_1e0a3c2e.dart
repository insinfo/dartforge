class _A {
  _A([int? a]);
}

class _B extends _A {
  _B._named([super.a]);
}

var b = _B._named(0);
