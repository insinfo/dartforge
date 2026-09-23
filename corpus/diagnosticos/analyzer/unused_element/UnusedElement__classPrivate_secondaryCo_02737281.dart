class _A {
  _A([int? a]);
}

class _B extends _A {
  _B._named([super.a]);
//                 ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

var b = _B._named();
