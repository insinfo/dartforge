enum _E {
  v;
  void _foo({int? a}) {}
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

void f() {
  _E.v._foo();
}
