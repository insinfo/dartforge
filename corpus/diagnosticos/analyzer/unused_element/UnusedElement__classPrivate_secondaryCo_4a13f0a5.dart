class _A {
  final int? f;
  _A({this.f});
  factory _A.named({int? f}) = _A;
}
f() => _A.named(f: 0);
