class _A {
  final int? f;
  _A([this.f]);
  factory _A.named([int? a]) = _A;
}
f() => _A.named(0);
