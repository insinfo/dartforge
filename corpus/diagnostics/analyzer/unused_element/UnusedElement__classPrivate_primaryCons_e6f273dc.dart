class _A([this.f]) {
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A();
