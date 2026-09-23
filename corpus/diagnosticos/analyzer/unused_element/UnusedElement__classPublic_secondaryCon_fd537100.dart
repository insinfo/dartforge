class A {
  final int? f;
  A._([this.f]);
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => A._();
