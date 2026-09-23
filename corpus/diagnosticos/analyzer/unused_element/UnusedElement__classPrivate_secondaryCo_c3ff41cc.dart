class _A {
  final int? f;
  _A({this.f});
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  factory _A.named() = _A;
}
f() => _A.named();
