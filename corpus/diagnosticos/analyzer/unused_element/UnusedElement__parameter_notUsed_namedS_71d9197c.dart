class _A {
  _A._named({int a = 0, int b = 0});
//               ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

class B extends _A {
  B() : super._named(b: 0);
}
