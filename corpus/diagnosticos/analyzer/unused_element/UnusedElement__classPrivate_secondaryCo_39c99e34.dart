class _A {
  final int? f;
  _A._named([this.f]);
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A._named();
