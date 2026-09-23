class _A {
  _A.named({int? a});
//               ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A.named();
