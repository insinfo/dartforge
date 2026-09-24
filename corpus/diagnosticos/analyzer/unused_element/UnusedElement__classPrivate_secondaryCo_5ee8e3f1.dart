class _A {
  _A._named({int? a});
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A._named();
